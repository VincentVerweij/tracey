# ADR-0002 — WTS session notifications raise and clear `Locked`

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** Vincent Verweij
- **Amended by:** [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md) (decision 1: only lock/unlock count; decision 5: the reconcile also runs before each activity write)
- **Resolves:** [#69](https://github.com/VincentVerweij/tracey/issues/69), under map [#65](https://github.com/VincentVerweij/tracey/issues/65)
- **Builds on:** [ADR-0001](0001-suspension-is-one-concept-with-reasons.md), which defines the `Locked` reason and its owner-only operations
- **Vocabulary:** [`CONTEXT.md` § Suspension](../../CONTEXT.md#suspension)

## Context

ADR-0001 says the `Locked` reason is raised and cleared by "the Windows session-lock signal" but leaves the signal unchosen. Two candidates were investigated:

- **Foreground-process detection.** Treat `LockApp.exe` in the foreground as locked. This reuses `PlatformHooks::get_foreground_window_info()` and adds no API surface. [#66](https://github.com/VincentVerweij/tracey/issues/66) showed that it holds for a whole default Win+L lock, but only if it latches, because the foreground is `NULL` at the lock instant and throughout the credential UI. [#72](https://github.com/VincentVerweij/tracey/issues/72) then showed that it fails in both directions outside that default. Learn documents configurations that have no LockApp phase at all: `NoLockScreen`, Ctrl+Alt+Del required by default on domain-joined machines, and AVD with Entra SSO, which disconnects the session instead of locking it. A community report has LockApp staying in the foreground ~46 minutes *after* unlock. Microsoft promises nothing about the process name, and Learn's `LockWorkStation` page points lock tracking to `WTSRegisterSessionNotification`.
- **WTS session notifications.** `WM_WTSSESSION_CHANGE` with `WTS_SESSION_LOCK` / `WTS_SESSION_UNLOCK` is documented and exact. [#67](https://github.com/VincentVerweij/tracey/issues/67) found it attachable. It also found three costs: it needs an HWND and a message pump, `windows` 0.58 and 0.61 both sit in the tree, and there is **no runtime way to detect that a successful registration has stopped delivering**.

#66 also changed what the reactive net is worth. `BitBlt` *succeeds* for most of a lock and returns a real lock-screen image. `ERR_SESSION_LOCKED` therefore fires only at the lock instant and during the credential UI. A missed proactive signal means lock-screen screenshots get persisted, not a silent no-op.

## Decision

### 1. WTS session notifications are the mechanism; the foreground process is not consulted

`WTS_SESSION_LOCK` raises `Locked`. `WTS_SESSION_UNLOCK` clears it. `LockApp.exe` plays no part in lock detection, either as a primary signal or as a cross-check. As a cross-check it would bring #72's false-lock failure back in.

Which *other* WTS codes (console or remote connect and disconnect, logon and logoff) also affect `Locked` is [#70](https://github.com/VincentVerweij/tracey/issues/70)'s decision. Until #70 settles it, the producer acts on lock and unlock only. It logs any other code at debug level, and it treats a `wParam` it doesn't recognise as unknown, never as "no change" (#67: 0xC–0xE are undefined).

> **Settled by [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md):** lock and unlock are the only codes that affect `Locked`. Every other code, recognised or not, is logged at debug level and makes no claim either way.

### 2. Notifications land in a message-only window on a dedicated thread

A dedicated `std::thread` creates its own `HWND_MESSAGE` window using the repo's `windows` 0.58 crate. It registers that window with `WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)` and runs its own `GetMessage` pump. Before the window is destroyed it unregisters, as Learn requires.

We rejected subclassing Tauri's main window. Its delivery would depend on that window's lifetime, and [`lib.rs`](../../src-tauri/src/lib.rs) only hides the window when `minimize_to_tray` is on. Otherwise a close destroys it. Subclassing would also need the 0.61→0.58 `HWND` cast and installation from the event-loop thread (#67: subclassing helpers cannot cross threads). The message-only window avoids all three, at the cost of one OS thread and the window-class and pump boilerplate. Message-only windows do not receive broadcasts such as `WM_POWERBROADCAST`. That doesn't matter here: `WM_WTSSESSION_CHANGE` is sent directly to registered windows, and sleep/resume is out of scope for map #65.

### 3. The producer lives outside `PlatformHooks`; the two-method decision stands

The session watch is a module under `platform::windows` and is started from `setup()`. It *pushes* into the suspension module's owner operations for `Locked` (ADR-0001 decision 2 and decision 4). It is the **sole owner** of the `Locked` reason, and the suspension module is the single source of truth for it.

`PlatformHooks` keeps exactly two methods. The trait is a *pull* abstraction: synchronous queries the loops call. Lock is a *push* event with a thread and a lifecycle. Adding `is_session_locked()` would make `Locked` exist twice, once in a latched atomic inside the platform implementation and once in the suspension module, with copying between them. Adding `start_session_watch(sink)` would put a lifecycle method into a query trait. Neither earns the reopening.

The 2026-03-15 *"exactly TWO methods"* decision is therefore **not reopened**, and no separate ADR is needed for it. When this is implemented, the doc comment in [`platform/mod.rs`](../../src-tauri/src/platform/mod.rs) gains one line pointing here, so the next reader doesn't take lock detection for a missing third method. (This map is plan-only, so the line isn't added now.)

### 4. Startup seed: `WTSSessionInfoEx`, failing closed

WTS events report only transitions. At startup the producer seeds `Locked` from `WTSQuerySessionInformation(…, WTSSessionInfoEx, …)` → `WTSINFOEX_LEVEL1.SessionFlags`:

| `SessionFlags` | Seed |
|---|---|
| `WTS_SESSIONSTATE_LOCK` | raise `Locked` |
| `WTS_SESSIONSTATE_UNLOCK` | leave `Locked` clear |
| `WTS_SESSIONSTATE_UNKNOWN`, or the call fails | **raise `Locked`**, log a warning |

Failing closed is how the standing decision *"a missed signal must degrade, not silently reopen"* applies to the seed. It replaces the map's earlier candidate, `WTSConnectState`. Learn defines that only as correlated with the lock screen ("such as when the user has chosen to exit to the lock screen"), never as a lock query. `SessionFlags` *is* documented as lock state. Learn notes the `LOCK`/`UNLOCK` values are reversed on Windows 7 / Server 2008 R2. Tracey doesn't target either, but the implementation should not paper over it if it does.

~~**Conditional on evidence.**~~ **Confirmed by [#73](https://github.com/VincentVerweij/tracey/issues/73).** On Windows 11 the flag matched the lock state on every observation and never read `UNKNOWN`. That covers a seed taken while already locked, Win+L, `LockWorkStation`, Ctrl+Alt+Del required, and sleep/wake. Decisions 4 and 5 stand as written.

### 5. Before each capture, the query reconciles the reason

Failing closed alone could leave capture shut indefinitely: an `UNKNOWN` seed while the user is actually unlocked stays raised until the next real lock/unlock cycle. A dead registration (#67) would go unnoticed in the same way. So the producer re-runs the same `SessionFlags` query **immediately before each capture attempt**:

- A definite answer that **agrees** with the reason: no change.
- A definite answer that **disagrees**: the query wins. The reason is raised or cleared, and a warning is logged naming the missed event ("missed lock event" or "missed unlock event"). A missed event of either kind is recovered at the next capture opportunity.
- `UNKNOWN` or failure: the reason keeps its current value.

The query wins disagreements because it reports *state*, while events report *transitions*, which can be missed. With the query in place, events serve as the low-latency path that raises `Locked` the moment the lock happens, rather than at the next capture tick, and they also supply the transition log. Both inputs belong to one producer, which remains the reason's only owner.

> **Amended by [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md):** the same reconcile also runs before each activity write. The rule is: the query reconciles `Locked` before any loop records an observation under it.

### 6. The reactive net only skips; it never touches `Locked`

The existing `ERR_SESSION_LOCKED` skip in [`screenshot_service.rs`](../../src-tauri/src/services/screenshot_service.rs) stays as defence-in-depth. It skips the one failing capture and does **not** raise or clear `Locked`. This settles the ambiguity over which signal is authoritative: the WTS producer is, and the net is a last-ditch filter on a single frame. #66 showed it covers only the lock instant and the credential UI, so it couldn't be authoritative anyway.

### 7. Registration failure: wait for Terminal Services, then retry

If `WTSRegisterSessionNotification` fails, typically with `RPC_S_INVALID_BINDING` when the app starts at logon before Remote Desktop Services is ready, the producer waits on the documented `Global\TermSrvReadyEvent` and retries with bounded backoff, logging each failure. The registration result is never discarded (`let _ =` on it is the shape to ban). Until registration succeeds, decisions 4 and 5 keep `Locked` correct at every capture, so capture is never unguarded. The only cost is lock latency.

### 8. Observability: logs only

This settles the logging and event branch of map #65's observability question, which ADR-0001 decision 7 deliberately left here. The UI branch (`Locked` is never shown) was settled there.

- **info** on every raise or clear of `Locked`, naming its source: `event`, `query` or `seed`.
- **warn** on an `UNKNOWN` or failed seed, an event/query disagreement, and each registration failure.
- **No `tracey://` event.** Nothing in the frontend would consume one, and adding it later is cheap.

## Consequences

- `src-tauri/Cargo.toml` adds the `Win32_System_RemoteDesktop` feature (#67). Creating a message-only window needs only features that are already enabled.
- One extra OS thread for the life of the process.
- The capture loop's pre-capture step becomes "reconcile `Locked`, then consult suspension". The query costs one `WTSQuerySessionInformation` call per capture attempt, and capture attempts are already rate-limited by the interval and the 2s debounce.
- Lock detection is no longer tied to which application fronts the lock screen. Locks with no LockApp phase (`NoLockScreen`, Ctrl+Alt+Del required, Secure Lock) are caught by the same event.
- **Testing.** The `test` cargo feature already stubs GDI capture. Under it, the session watch is not started, and tests drive the suspension module's `Locked` operations directly. That's possible because the producer is a separate push source rather than a trait method. A real lock still can't be automated, so verifying the producer end to end is an **accepted manual gap** ([#74](https://github.com/VincentVerweij/tracey/issues/74)). The producer is thin, and #73 already verified the `SessionFlags` behaviour it relies on. The [manual checklist](../manual-checks/session-watch.md) is Win+L and unlock, startup while locked, and sleep/wake, with the #73 probe log as the baseline. No fake session source is introduced and no scripted lock check is kept.
- `Locked` remains capture-only ([#70](https://github.com/VincentVerweij/tracey/issues/70) owns widening), and nothing here changes ADR-0001.

## Open question this creates

**Is `WTSINFOEX_LEVEL1.SessionFlags` accurate on Windows 10/11, through the same phases #66 observed** (lock instant, steady LockApp, credential UI, unlock), and ideally also under Ctrl+Alt+Del required and inside RDP? Decisions 4 and 5 depend on it. It is an empirical question, like #66, and is ticketed under map #65.

> **Resolved by [#73](https://github.com/VincentVerweij/tracey/issues/73):** yes. It held through every phase and under Ctrl+Alt+Del required, and the query's transitions lined up with the events. RDP was not run, and remote session states are out of scope under #70. One timing fact matters for decision 5: after sleep, the queued `LOCK` and `UNLOCK` events arrived together, 20 ms apart, and only *after* the unlock. The query already read `UNLOCK` inside the `LOCK` handler. Events report history, the query reports the present, and that is why the query wins.

## Alternatives considered

- **Foreground-process detection on `LockApp.exe`, latched.** Cheapest, with no new API surface. Rejected on #72: it relies on an undocumented process, documented configurations bypass it, and it can produce false locks.
- **WTS events with `LockApp.exe` as a cross-check.** Rejected because it adds the false-lock mode back in. The `SessionFlags` query does the cross-check job without it.
- **Query only, no notifications.** No window, thread or registration, and no dead-registration problem. Rejected because it rests entirely on the not-yet-verified query with nothing to fall back on, and because lock latency would grow to the next capture tick.
- **Subclassing Tauri's main window.** No second pump. Rejected under decision 2: lifetime coupling, the HWND type cast, and the main-thread installation constraint.
- **A third `PlatformHooks` method.** Rejected under decision 3.
- **`WTSConnectState` as the seed.** Rejected under decision 4: it is correlated with lock, not defined as lock.
- **Fail-open seed.** Rejected: #66 showed the reactive net misses most of a lock, so failing open means persisting lock-screen screenshots.
