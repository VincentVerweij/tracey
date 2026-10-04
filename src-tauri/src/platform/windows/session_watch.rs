//! Session watch: the sole owner of the `Locked` suspension reason (ADR-0002).
//!
//! A dedicated thread owns a message-only window registered for WTS session
//! notifications for this session, and runs its own message pump.
//! `WTS_SESSION_LOCK` raises `Locked` and `WTS_SESSION_UNLOCK` clears it. At
//! startup the reason is seeded from the session-state query, failing closed.
//! Not started under the `test` feature: tests drive `Suspension` directly.
//!
//! Still to come: the pre-observation reconcile from the same query (ADR-0002
//! decision 5, #81) and the registration retry on `TermSrvReadyEvent`
//! (decision 7, #82). Until then a failed registration is logged and the
//! watch keeps only its seed.

#![cfg_attr(feature = "test", allow(dead_code))]

use std::cell::OnceCell;
use std::sync::{mpsc, Arc};

use windows::core::{w, PWSTR};
use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, E_UNEXPECTED, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSRegisterSessionNotification,
    WTSSessionInfoEx, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION, WTSINFOEXW,
    WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK,
    WTS_SESSIONSTATE_UNLOCK,
};
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
    let registered = match hwnd {
        Some(hwnd) => {
            match unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } {
                Ok(()) => true,
                Err(e) => {
                    log::warn!("session watch: WTSRegisterSessionNotification failed: {e}");
                    false
                }
            }
        }
        None => false,
    };

    seed(&suspension, query_session_flags());
    let _ = seeded.send(()); // start() may have stopped waiting; nothing to do then

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::suspension::TrackingLoop;
    use windows::Win32::Foundation::E_FAIL;
    use windows::Win32::System::RemoteDesktop::WTS_SESSIONSTATE_UNKNOWN;

    fn is_locked(suspension: &Suspension) -> bool {
        suspension.is_suspended(TrackingLoop::ScreenshotCapture)
    }

    #[test]
    fn lock_event_raises_locked() {
        let suspension = Suspension::new();
        on_session_change(&suspension, WTS_SESSION_LOCK);
        assert!(is_locked(&suspension));
    }

    #[test]
    fn unlock_event_clears_locked() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        on_session_change(&suspension, WTS_SESSION_UNLOCK);
        assert!(!is_locked(&suspension));
    }

    #[test]
    fn other_session_codes_change_nothing() {
        // 1–6 and 9–11 are documented, 0 and 0xC–0xE are not (#67).
        let other_codes =
            (0..=0xE).filter(|c| *c != WTS_SESSION_LOCK && *c != WTS_SESSION_UNLOCK);
        for code in other_codes {
            let suspension = Suspension::new();
            on_session_change(&suspension, code);
            assert!(!is_locked(&suspension), "code {code:#x} raised Locked");

            suspension.raise_locked();
            on_session_change(&suspension, code);
            assert!(is_locked(&suspension), "code {code:#x} cleared Locked");
        }
    }

    #[test]
    fn seed_from_locked_session_raises_locked() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_LOCK));
        assert!(is_locked(&suspension));
    }

    #[test]
    fn seed_from_unlocked_session_leaves_locked_clear() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_UNLOCK));
        assert!(!is_locked(&suspension));
    }

    #[test]
    fn unknown_seed_fails_closed() {
        let suspension = Suspension::new();
        seed(&suspension, Ok(WTS_SESSIONSTATE_UNKNOWN));
        assert!(is_locked(&suspension));
    }

    #[test]
    fn failed_seed_fails_closed() {
        let suspension = Suspension::new();
        seed(&suspension, Err(windows::core::Error::from(E_FAIL)));
        assert!(is_locked(&suspension));
    }
}
