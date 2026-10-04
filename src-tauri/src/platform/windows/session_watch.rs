//! Session watch: the sole owner of the `Locked` suspension reason (ADR-0002).
//!
//! A dedicated thread owns a message-only window registered for WTS session
//! notifications for this session, and runs its own message pump.
//! `WTS_SESSION_LOCK` raises `Locked` and `WTS_SESSION_UNLOCK` clears it. At
//! startup the reason is seeded from the session-state query, failing closed.
//! Not started under the `test` feature: tests drive `Suspension` directly.
//!
//! Before a loop records an observation, `reconcile` re-runs the same query
//! and a definite answer wins over the events (ADR-0002 decision 5).
//!
//! A failed registration, typically `RPC_S_INVALID_BINDING` at logon before
//! Remote Desktop Services is ready, is retried on `TermSrvReadyEvent` with
//! bounded backoff until it succeeds (decision 7). Meanwhile `Locked` is kept
//! correct by the seed and the reconcile alone; the only cost is lock latency.

#![cfg_attr(feature = "test", allow(dead_code))]

use std::cell::OnceCell;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_CLASS_ALREADY_EXISTS, E_UNEXPECTED, HWND, LPARAM, LRESULT, WAIT_OBJECT_0,
    WPARAM,
};
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSRegisterSessionNotification,
    WTSSessionInfoEx, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION, WTSINFOEXW,
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK,
    WTS_SESSIONSTATE_UNLOCK,
};
use windows::Win32::System::Threading::{OpenEventW, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    RegisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_WTSSESSION_CHANGE,
    WNDCLASSW, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use crate::services::suspension::Suspension;

thread_local! {
    /// The suspension the window procedure pushes into. Set once, on the
    /// watch thread, before the window exists.
    static SUSPENSION: OnceCell<Arc<Suspension>> = const { OnceCell::new() };
}

/// Start the session watch on its own thread. Returns once `Locked` has been
/// seeded, so no loop observes before the reason reflects the session.
pub fn start(suspension: Arc<Suspension>) {
    let (seeded_tx, seeded_rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("session-watch".into())
        .spawn(move || run(suspension, seeded_tx));
    match spawned {
        Ok(_) => {
            if seeded_rx.recv().is_err() {
                log::error!("session watch: thread ended before seeding Locked");
            }
        }
        Err(e) => log::error!("session watch: failed to spawn thread: {e}"),
    }
}

/// The watch thread: window, registration, seed, then the message pump.
/// Registering before seeding means a lock that lands after the seed query is
/// queued as an event rather than lost.
fn run(suspension: Arc<Suspension>, seeded: mpsc::Sender<()>) {
    SUSPENSION.with(|cell| {
        let _ = cell.set(Arc::clone(&suspension)); // fresh thread: the cell is empty
    });

    let hwnd = match create_message_window() {
        Ok(hwnd) => Some(hwnd),
        Err(e) => {
            log::error!("session watch: failed to create message-only window: {e}");
            None
        }
    };
    let first_attempt = hwnd.map(register);

    seed(&suspension, query_session_flags());
    let _ = seeded.send(()); // start() may have stopped waiting; nothing to do then

    // Retry after seeding, so a slow Remote Desktop Services never delays the
    // tracking loops; they stay guarded by the reconcile in the meantime.
    let registered = match (hwnd, first_attempt) {
        (Some(hwnd), Some(first_attempt)) => {
            let failures = retry_registration(
                first_attempt,
                || register(hwnd),
                wait_for_termsrv_ready,
                std::thread::sleep,
            );
            if failures > 0 {
                log::info!("session watch: registered after {failures} failed attempts");
                // No events arrived while unregistered: catch up on any missed.
                reconcile(&suspension);
            }
            true
        }
        _ => false,
    };

    if registered {
        let mut msg = MSG::default();
        // GetMessageW returns 0 on WM_QUIT and -1 on error; both end the pump.
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
            unsafe { DispatchMessageW(&msg) };
        }
        log::warn!("session watch: message pump ended; lock events no longer arrive");
    }

    if let Some(hwnd) = hwnd {
        // Learn: unregister before the window is destroyed. Only reached if the
        // pump ends; at process exit the OS tears both down.
        if registered {
            if let Err(e) = unsafe { WTSUnRegisterSessionNotification(hwnd) } {
                log::warn!("session watch: WTSUnRegisterSessionNotification failed: {e}");
            }
        }
        if let Err(e) = unsafe { DestroyWindow(hwnd) } {
            log::warn!("session watch: DestroyWindow failed: {e}");
        }
    }
}

fn register(hwnd: HWND) -> windows::core::Result<()> {
    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }
}

/// Delay before the first retry. It doubles after each failure, up to
/// `MAX_RETRY_DELAY`.
const FIRST_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// Retry a failed registration until it succeeds (ADR-0002 decision 7),
/// logging each failure. Until Terminal Services has signalled ready, each
/// wait is on its ready event, bounded by the backoff delay, so a retry
/// follows the event at once; after that the backoff is a plain sleep.
/// Returns the number of failed attempts, the first one included.
fn retry_registration(
    first_attempt: windows::core::Result<()>,
    mut register: impl FnMut() -> windows::core::Result<()>,
    mut wait_for_termsrv: impl FnMut(Duration) -> bool,
    mut sleep: impl FnMut(Duration),
) -> u32 {
    let mut attempt = first_attempt;
    let mut failures = 0;
    let mut delay = FIRST_RETRY_DELAY;
    let mut termsrv_ready = false;
    while let Err(e) = attempt {
        failures += 1;
        log::warn!(
            "session watch: WTSRegisterSessionNotification failed (attempt {failures}): {e}"
        );
        if termsrv_ready {
            sleep(delay);
        } else {
            termsrv_ready = wait_for_termsrv(delay);
        }
        delay = (delay * 2).min(MAX_RETRY_DELAY);
        attempt = register();
    }
    failures
}

/// Wait up to `timeout` for `Global\TermSrvReadyEvent`. Returns whether it is
/// signalled. The event is opened on each call: it may not exist yet this
/// early in logon, and then the wait is a plain sleep.
fn wait_for_termsrv_ready(timeout: Duration) -> bool {
    let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    match unsafe {
        OpenEventW(
            SYNCHRONIZATION_SYNCHRONIZE,
            false,
            w!("Global\\TermSrvReadyEvent"),
        )
    } {
        Ok(event) => unsafe {
            let ready = WaitForSingleObject(event, millis) == WAIT_OBJECT_0;
            let _ = CloseHandle(event); // a handle we just opened; nothing to do if it fails
            ready
        },
        Err(e) => {
            log::debug!("session watch: TermSrvReadyEvent not available ({e}); sleeping instead");
            std::thread::sleep(timeout);
            false
        }
    }
}

fn create_message_window() -> windows::core::Result<HWND> {
    let class_name = w!("TraceySessionWatch");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe {
        if RegisterClassW(&class) == 0 {
            let e = windows::core::Error::from_win32();
            if e.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
                return Err(e);
            }
        }
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            None,
            None,
        )
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        // try_with: a panic cannot unwind out of an extern "system" fn.
        let _ = SUSPENSION.try_with(|cell| {
            if let Some(suspension) = cell.get() {
                on_session_change(suspension, wparam.0 as u32);
            }
        });
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// `WTSINFOEX_LEVEL1.SessionFlags` for this session (ADR-0002 decision 4).
fn query_session_flags() -> windows::core::Result<u32> {
    unsafe {
        let mut buffer = PWSTR::null();
        let mut bytes = 0u32;
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )?;
        // Level 1 is the only level WTSINFOEXW defines; anything else is a
        // failed query, so the caller fails closed.
        let flags = if bytes as usize >= std::mem::size_of::<WTSINFOEXW>()
            && (*(buffer.0 as *const WTSINFOEXW)).Level == 1
        {
            let info = &*(buffer.0 as *const WTSINFOEXW);
            Ok(info.Data.WTSInfoExLevel1.SessionFlags as u32)
        } else {
            Err(windows::core::Error::from(E_UNEXPECTED))
        };
        WTSFreeMemory(buffer.0.cast());
        flags
    }
}

/// Apply one `WM_WTSSESSION_CHANGE` code. Lock and unlock are the only codes
/// that affect `Locked` (ADR-0003); every other code makes no claim.
fn on_session_change(suspension: &Suspension, code: u32) {
    match code {
        WTS_SESSION_LOCK => {
            suspension.raise_locked();
            log::info!("Locked raised (source: event)");
        }
        WTS_SESSION_UNLOCK => {
            suspension.clear_locked();
            log::info!("Locked cleared (source: event)");
        }
        other => log::debug!("session watch: session change {other:#x} ignored"),
    }
}

/// Seed `Locked` from the startup `SessionFlags` query (ADR-0002 decision 4).
/// Anything but a definite answer fails closed.
fn seed(suspension: &Suspension, session_flags: windows::core::Result<u32>) {
    match session_flags {
        Ok(WTS_SESSIONSTATE_LOCK) => {
            suspension.raise_locked();
            log::info!("Locked raised (source: seed)");
        }
        Ok(WTS_SESSIONSTATE_UNLOCK) => {
            log::info!("session unlocked at startup; Locked stays clear (source: seed)");
        }
        Ok(flags) => {
            suspension.raise_locked();
            log::warn!(
                "Locked raised (source: seed): session state unknown ({flags:#x}), failing closed"
            );
        }
        Err(e) => {
            suspension.raise_locked();
            log::warn!(
                "Locked raised (source: seed): session state query failed ({e}), failing closed"
            );
        }
    }
}

/// Reconcile `Locked` from the session-state query (ADR-0002 decision 5).
/// Called by the loops, through `suspension::reconcile_locked`, immediately
/// before they record an observation. Runs on the caller's thread.
pub fn reconcile(suspension: &Suspension) {
    reconcile_from(suspension, query_session_flags);
}

/// The event `Locked` was out of step by, found by the reconcile.
#[derive(Debug, PartialEq, Eq)]
enum MissedEvent {
    Lock,
    Unlock,
}

impl std::fmt::Display for MissedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MissedEvent::Lock => f.write_str("missed lock event"),
            MissedEvent::Unlock => f.write_str("missed unlock event"),
        }
    }
}

/// Apply one `SessionFlags` answer to `Locked`. A definite answer that
/// disagrees wins and names the missed event; one that agrees, `UNKNOWN` or a
/// failed query changes nothing.
///
/// `Locked` is read before the query: an event landing in between then moves
/// the reason towards the answer, never away from it, so the reconcile cannot
/// undo a lock event it raced with.
fn reconcile_from(
    suspension: &Suspension,
    query: impl FnOnce() -> windows::core::Result<u32>,
) -> Option<MissedEvent> {
    let held = suspension.is_locked();
    let missed = match query() {
        Ok(WTS_SESSIONSTATE_LOCK) if !held => {
            suspension.raise_locked();
            MissedEvent::Lock
        }
        Ok(WTS_SESSIONSTATE_UNLOCK) if held => {
            suspension.clear_locked();
            MissedEvent::Unlock
        }
        _ => return None,
    };
    let change = if held { "cleared" } else { "raised" };
    log::warn!("Locked {change} (source: query): {missed}");
    Some(missed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::E_FAIL;
    use windows::Win32::System::RemoteDesktop::WTS_SESSIONSTATE_UNKNOWN;

    #[test]
    fn lock_event_raises_locked() {
        let suspension = Suspension::new();
        on_session_change(&suspension, WTS_SESSION_LOCK);
        assert!(suspension.is_locked());
    }

    #[test]
    fn unlock_event_clears_locked() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        on_session_change(&suspension, WTS_SESSION_UNLOCK);
        assert!(!suspension.is_locked());
    }

    #[test]
    fn other_session_codes_change_nothing() {
        // 1–6 and 9–11 are documented, 0 and 0xC–0xE are not (#67).
        let other_codes = (0..=0xE).filter(|c| *c != WTS_SESSION_LOCK && *c != WTS_SESSION_UNLOCK);
        for code in other_codes {
            let suspension = Suspension::new();
            on_session_change(&suspension, code);
            assert!(!suspension.is_locked(), "code {code:#x} raised Locked");

            suspension.raise_locked();
            on_session_change(&suspension, code);
            assert!(suspension.is_locked(), "code {code:#x} cleared Locked");
        }
    }

    #[test]
    fn a_definite_lock_answer_raises_a_wrongly_clear_locked() {
        let suspension = Suspension::new();
        let missed = reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_LOCK));
        assert!(suspension.is_locked());
        assert_eq!(missed, Some(MissedEvent::Lock));
        assert_eq!(MissedEvent::Lock.to_string(), "missed lock event");
    }

    #[test]
    fn a_definite_unlock_answer_clears_a_wrongly_raised_locked() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        let missed = reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_UNLOCK));
        assert!(!suspension.is_locked());
        assert_eq!(missed, Some(MissedEvent::Unlock));
        assert_eq!(MissedEvent::Unlock.to_string(), "missed unlock event");
    }

    #[test]
    fn an_agreeing_answer_changes_nothing() {
        let suspension = Suspension::new();
        assert_eq!(
            reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_UNLOCK)),
            None
        );
        assert!(!suspension.is_locked());

        suspension.raise_locked();
        assert_eq!(
            reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_LOCK)),
            None
        );
        assert!(suspension.is_locked());
    }

    #[test]
    fn an_unknown_or_failed_query_leaves_locked_alone() {
        for held in [false, true] {
            let suspension = Suspension::new();
            if held {
                suspension.raise_locked();
            }
            assert_eq!(
                reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_UNKNOWN)),
                None
            );
            assert_eq!(suspension.is_locked(), held);
            let failed = || Err(windows::core::Error::from(E_FAIL));
            assert_eq!(reconcile_from(&suspension, failed), None);
            assert_eq!(suspension.is_locked(), held);
        }
    }

    #[test]
    fn the_session_state_wins_over_a_contradicting_stale_event() {
        // After sleep/wake the queued LOCK is delivered once the user is
        // already back (#73); the next query reports the present.
        let suspension = Suspension::new();
        on_session_change(&suspension, WTS_SESSION_LOCK);
        reconcile_from(&suspension, || Ok(WTS_SESSIONSTATE_UNLOCK));
        assert!(!suspension.is_locked());
    }

    #[test]
    fn a_lock_event_racing_the_query_is_not_undone() {
        // The query reads UNLOCK, then the lock event lands before the answer
        // is applied: the reconcile must leave the event's raise in place.
        let suspension = Suspension::new();
        reconcile_from(&suspension, || {
            on_session_change(&suspension, WTS_SESSION_LOCK);
            Ok(WTS_SESSIONSTATE_UNLOCK)
        });
        assert!(suspension.is_locked());
    }

    fn failing(times: u32) -> impl FnMut() -> windows::core::Result<()> {
        let mut left = times;
        move || {
            if left == 0 {
                return Ok(());
            }
            left -= 1;
            registration_failed()
        }
    }

    fn registration_failed() -> windows::core::Result<()> {
        Err(windows::core::Error::from(E_FAIL))
    }

    #[test]
    fn a_successful_registration_is_not_retried() {
        let mut attempts = 0;
        let failures = retry_registration(
            Ok(()),
            || {
                attempts += 1;
                Ok(())
            },
            |_| panic!("waited for Terminal Services"),
            |_| panic!("slept"),
        );
        assert_eq!(failures, 0);
        assert_eq!(attempts, 0);
    }

    #[test]
    fn a_failed_registration_is_retried_until_it_succeeds() {
        let mut register = failing(3);
        let mut attempts = 0;
        let failures = retry_registration(
            registration_failed(),
            || {
                attempts += 1;
                register()
            },
            |_| false,
            |_| {},
        );
        assert_eq!(
            attempts, 4,
            "three failed retries, then the one that succeeds"
        );
        assert_eq!(failures, 4, "the first attempt and three retries failed");
    }

    #[test]
    fn retries_wait_on_terminal_services_with_bounded_backoff() {
        let mut waits = Vec::new();
        retry_registration(
            registration_failed(),
            failing(9),
            |timeout| {
                waits.push(timeout.as_secs());
                false
            },
            |_| panic!("slept before Terminal Services was ready"),
        );
        assert_eq!(waits, [1, 2, 4, 8, 16, 32, 60, 60, 60, 60]);
    }

    #[test]
    fn once_terminal_services_is_ready_retries_back_off_by_sleeping() {
        let mut waits = Vec::new();
        let mut sleeps = Vec::new();
        retry_registration(
            registration_failed(),
            failing(4),
            |timeout| {
                waits.push(timeout.as_secs());
                waits.len() == 2 // the ready event fires during the second wait
            },
            |delay| sleeps.push(delay.as_secs()),
        );
        assert_eq!(waits, [1, 2], "a signalled event is not waited on again");
        assert_eq!(sleeps, [4, 8, 16]);
    }

    #[test]
    fn seed_from_locked_session_raises_locked() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_LOCK));
        assert!(suspension.is_locked());
    }

    #[test]
    fn seed_from_unlocked_session_leaves_locked_clear() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_UNLOCK));
        assert!(!suspension.is_locked());
    }

    #[test]
    fn unknown_seed_fails_closed() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_UNKNOWN));
        assert!(suspension.is_locked());
    }

    #[test]
    fn failed_seed_fails_closed() {
        let suspension = Suspension::new();
        seed(&suspension, Err(windows::core::Error::from(E_FAIL)));
        assert!(suspension.is_locked());
    }
}
