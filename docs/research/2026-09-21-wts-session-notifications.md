# How WTS session notifications attach to a Tauri v2 app

Research for [#67](https://github.com/VincentVerweij/tracey/issues/67), part of map [#65](https://github.com/VincentVerweij/tracey/issues/65).
Date: 2026-09-21. Plan-only — no production code was touched.

**Question.** What does it take to receive `WM_WTSSESSION_CHANGE` in this app, and does it keep
arriving in the states this app actually runs in?

**Source policy.** Microsoft Learn (Win32 reference), the vendored source of the exact crate
versions this repo resolves (`windows 0.58.0`, `tauri 2.10.3`, `tao 0.34.6` — all primary, all
verbatim from `~/.cargo/registry`). Where a claim could not be traced to a primary source it is
marked **GAP** and left unanswered rather than inferred.

---

## 1. Registration mechanics

### `WTSRegisterSessionNotification`

```cpp
BOOL WTSRegisterSessionNotification(
  [in] HWND  hWnd,
  [in] DWORD dwFlags
);
```

- `dwFlags` is exactly one of:
  - **`NOTIFY_FOR_THIS_SESSION`** — "Only session notifications involving the session attached to
    by the window identified by the *hWnd* parameter value are to be received."
  - **`NOTIFY_FOR_ALL_SESSIONS`** — "All session notifications are to be received."
- Returns `TRUE` on success, `FALSE` otherwise; `GetLastError` for detail.
- "Session change notifications are sent in the form of a `WM_WTSSESSION_CHANGE` message. These
  notifications are sent **only** to the windows that have registered for them using this function."
- "When a window no longer requires these notifications, it **must** call
  `WTSUnRegisterSessionNotification` before being destroyed. For every call to this function, there
  must be a corresponding call to `WTSUnRegisterSessionNotification`."
- "If the window handle passed in this function is already registered, the value of the *dwFlags*
  parameter is **ignored**." (So you cannot widen/narrow the flags by re-registering.)
- Header `wtsapi32.h`, lib `Wtsapi32.lib`, DLL `Wtsapi32.dll`. Minimum client Windows Vista.

Source: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification>

`WTSUnRegisterSessionNotification(HWND)` — same return convention; "must be called once for every
call to `WTSRegisterSessionNotification`".
Source: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsunregistersessionnotification>

For this app, `NOTIFY_FOR_THIS_SESSION` is the correct flag: the capture gate cares only about the
session tracey itself runs in. Note its numeric value is **0**, which means a wrong-typed `0` and a
deliberate "this session" are indistinguishable at the call site.

### The message

One message only: **`WM_WTSSESSION_CHANGE` = 689 (0x2B1)**, delivered to the window's `WindowProc`.

- `wParam` = status code (table below)
- `lParam` = **the identifier of the session** the event concerns
- "The return value is ignored."
- "This message is sent only to applications that have registered to receive this message by
  calling `WTSRegisterSessionNotification`." — i.e. it is a *directed* message, not a broadcast.

Source: <https://learn.microsoft.com/en-us/windows/win32/termserv/wm-wtssession-change>

### Full `wParam` code set

| Code | Value | Microsoft's meaning (verbatim) |
| --- | --- | --- |
| `WTS_CONSOLE_CONNECT` | 0x1 | "The session identified by *lParam* was connected to the console terminal or RemoteFX session." |
| `WTS_CONSOLE_DISCONNECT` | 0x2 | "The session identified by *lParam* was disconnected from the console terminal or RemoteFX session." |
| `WTS_REMOTE_CONNECT` | 0x3 | "The session identified by *lParam* was connected to the remote terminal." |
| `WTS_REMOTE_DISCONNECT` | 0x4 | "The session identified by *lParam* was disconnected from the remote terminal." |
| `WTS_SESSION_LOGON` | 0x5 | "A user has logged on to the session identified by *lParam*." |
| `WTS_SESSION_LOGOFF` | 0x6 | "A user has logged off the session identified by *lParam*." |
| `WTS_SESSION_LOCK` | 0x7 | "The session identified by *lParam* has been locked." |
| `WTS_SESSION_UNLOCK` | 0x8 | "The session identified by *lParam* has been unlocked." |
| `WTS_SESSION_REMOTE_CONTROL` | 0x9 | "The session identified by *lParam* has changed its remote controlled status. To determine the status, call `GetSystemMetrics` and check the `SM_REMOTECONTROL` metric." |
| `WTS_SESSION_CREATE` | 0xA | "Reserved for future use." |
| `WTS_SESSION_TERMINATE` | 0xB | "Reserved for future use." |
| `WTS_SESSION_DESKTOP_READY` | 0xF | "The session identified by *lParam* has switched to the user's desktop." |

Two things to carry forward:

- 0xC–0xE are undefined in the documentation. Treat unknown `wParam` values as "unknown", not as
  "no change".
- `WTS_SESSION_DESKTOP_READY` (0xF) is documented on Learn but **is not exposed as a constant by
  `windows 0.58`** (see §6). If the design wants it, the value must be written out literally.

---

## 2. Where the HWND comes from

### Tauri does expose an HWND — verified in the resolved source

`tauri 2.10.3`, `src/window/mod.rs:1579`:

```rust
/// Returns the native handle that is used by this window.
#[cfg(windows)]
pub fn hwnd(&self) -> crate::Result<HWND> {
  self.window.dispatcher.window_handle()
    .map_err(Into::into)
    .and_then(|handle| {
      if let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
        Ok(HWND(h.hwnd.get() as _))
      } else {
        Err(crate::Error::InvalidWindowHandle)
      }
    })
}
```

`WebviewWindow::hwnd()` (`src/webview/webview_window.rs:1776`) forwards to it. Both return
`crate::Result<HWND>`, so the failure mode is a `Result`, not a panic.

Source (API doc): <https://docs.rs/tauri/2/tauri/window/struct.Window.html#method.hwnd>

**A version trap that must be planned around.** `tauri 2.10.3` and `tauri-runtime-wry 2.10.1`
depend on `windows` **0.61** (`Cargo.toml` `[target."cfg(windows)".dependencies.windows] version = "0.61"`),
while `src-tauri/Cargo.toml` pins `windows` **0.58**. `Cargo.lock` in this repo confirms both
`windows 0.58.0` and `windows 0.61.3` are present. So `Window::hwnd()` returns a
`windows_0_61::Foundation::HWND`, which is a *different Rust type* from the `windows_0_58` `HWND`
used everywhere else in `src-tauri`. Both are `#[repr(transparent)] pub struct HWND(pub *mut core::ffi::c_void)`
(0.58 `Foundation/mod.rs:10866`; 0.61.3 `Foundation/mod.rs:5670`), so the bridge is a one-line
re-wrap of the inner pointer — but it is a deliberate, documented cast, not an implicit conversion,
and it silently depends on both crates keeping that representation.

### Is a message pump already running?

Yes, on the main thread — but it is not free to hook. `tao 0.34.6` installs **its own window
procedure via `SetWindowSubclass`**, not via a window class `WNDPROC`:

- `src/platform_impl/windows/event_loop.rs:750` — `SetWindowSubclass(window, Some(public_window_callback::<T>), WINDOW_SUBCLASS_ID /* 0 */, input_ptr)`
- `:703` — a second subclass on the thread-message target window, `THREAD_EVENT_TARGET_SUBCLASS_ID = 1`
- unhandled messages fall through to `DefSubclassProc` (`:2442`) or `DefWindowProcW` (`:2264`)

Also verified: `grep -rn "WTSRegisterSessionNotification\|WM_WTSSESSION_CHANGE" tao-0.34.6/src`
returns **nothing**, and `HWND_MESSAGE` appears nowhere in tao. So there is no existing WTS
registration to piggyback on and nothing in Tauri/tao that would already see the message.

### Option A — subclass the Tauri window

Because tao itself uses the ComCtl32 v6 subclass helpers, adding a second subclass with a
*different* `uIdSubclass` is the mechanism the API is designed for, and it composes rather than
clobbers:

- "Each subclass is uniquely identified by the address of the *pfnSubclass* and its *uIdSubclass*."
- "The `DefSubclassProc` function calls the next handler in the subclass chain."
- Removal is `RemoveWindowSubclass(hwnd, proc, id)`.

Sources: <https://learn.microsoft.com/en-us/windows/win32/controls/subclassing-overview>,
<https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass>

Rules that fall out of the primary docs:

1. Handle `WM_WTSSESSION_CHANGE` and **always** return `DefSubclassProc(...)` for every other
   message. Never call `DefWindowProcW` directly from the added subclass — that would skip tao's
   handler and break Tauri's own window behaviour.
2. **Do not use `SetWindowLongPtr`/`GWLP_WNDPROC`.** Learn's own list of disadvantages of the
   pre-ComCtl32-v6 approach: "The window procedure can only be replaced once", "It is difficult to
   remove a subclass after it is created", and the next proc must be invoked via `CallWindowProc`.
   Mixing that with tao's subclass chain is the way to break Tauri's handling.
3. **Hard constraint, verbatim from Learn:** "**You cannot use the subclassing helper functions to
   subclass a window across threads.**" So `SetWindowSubclass` must be called *on the thread that
   owns the HWND* — the Tauri main/event-loop thread. A `tokio` task cannot install it. In Tauri
   that means doing it from `setup`/main-thread context (e.g. `AppHandle::run_on_main_thread`),
   not from the screenshot loop.

### Option B — message-only window on a dedicated thread

Learn, "Message-Only Windows": "A *message-only window* enables you to send and receive messages.
It is not visible, has no z-order, cannot be enumerated, and **does not receive broadcast
messages**. The window simply dispatches messages." Created by passing `HWND_MESSAGE` as
`hWndParent` to `CreateWindowEx` (or `SetParent`).

Source: <https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features> (§ Message-Only Windows)

The "no broadcast messages" caveat does **not** disqualify it for WTS: `WM_WTSSESSION_CHANGE` is
documented as sent only to registered windows, i.e. directed. It *would* matter for
`WM_POWERBROADCAST` (§5), which is a broadcast — **GAP: Learn does not state whether
`WM_POWERBROADCAST` reaches a message-only window; do not assume it does.**

Trade-offs, stated from the docs rather than taste:

| | A: subclass Tauri window | B: `HWND_MESSAGE` + own thread |
| --- | --- | --- |
| Needs its own message pump | No (tao pumps) | Yes (`GetMessage`/`DispatchMessage` loop) |
| Thread constraint | Must install on Tauri's main thread | Window must be created *and* pumped on the owning thread; registration from that thread |
| Lifetime coupled to Tauri window | Yes — dies if the window is destroyed | No — independent of window show/hide/close |
| Risk to Tauri's own handling | Real; mitigated by `DefSubclassProc` discipline | None |
| Extra unsafe surface | Subclass proc + removal ordering | `RegisterClassW` + `CreateWindowEx` + pump + `DestroyWindow` |

**Which is idiomatic?** I could not find a primary source that settles this — **GAP.** Neither the
Tauri v2 docs nor Learn prescribes one. What the primary sources *do* establish is that (B) is the
only option whose delivery guarantee is independent of the Tauri window's lifetime, and that (A) is
the only option that needs no second message pump. Given the map's standing decision that "a missed
signal must degrade, not silently reopen the gate", the lifetime-independence of (B) is the
decision-relevant property; the ergonomic cost of (A) is lower. That is the trade to decide in #69,
not something the docs resolve.

---

## 3. Does delivery survive `window.hide()`?

**Yes, for a hidden window. No, for a destroyed one.**

Learn, "Window Visibility" (verbatim):

> A window can be either visible or hidden. The system displays a *visible window* on the screen.
> It hides a *hidden window* by not drawing it. […] If a window is hidden, it is effectively
> disabled. **A hidden window can process messages from the system or from other windows**, but it
> cannot process input from the user or display output.

Source: <https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features> (§ Window Visibility)

Also from the same page: `ShowWindow`/`SetWindowPos`/`SetWindowPlacement`/`SetWindowLong` "show or
hide a window by setting or removing the `WS_VISIBLE` style" — hiding is a style change, not a
destruction. The HWND remains valid, remains registered, and remains in the same message queue.

Applied to this repo: `src-tauri/src/lib.rs:116-134` intercepts `WindowEvent::CloseRequested`,
calls `api.prevent_close()` then `window.hide()` when `minimize_to_tray` is set. That path only
clears `WS_VISIBLE`. Registration made against that HWND keeps delivering `WM_WTSSESSION_CHANGE`.
This holds for both option A and option B (a message-only window is never visible in the first
place — "It is not visible").

**If the window is destroyed** the guarantee is gone, and worse than gone:

- The registration is per-HWND, and `WM_WTSSESSION_CHANGE` goes only to registered windows — a
  destroyed HWND has no `WindowProc` to receive it. Notifications simply stop.
- Learn *requires* `WTSUnRegisterSessionNotification` "before being destroyed". Destroying without
  unregistering violates the documented contract. **GAP: Learn does not state what actually happens
  if you skip it** (leak in the WTS service, stale-HWND delivery attempt, or benign) — do not guess;
  just honour the contract.
- Practical consequence for #69: if the mechanism registers against the Tauri window, anything that
  destroys and recreates that window (not observed in `lib.rs` today, but nothing prevents it later)
  silently drops the signal — exactly the "silently reopen the gate" failure the map forbids. A
  message-only window owned by the app's own lifetime has no such coupling.

---

## 4. Can registration fail silently, and how is failure detectable?

It can fail, and there is one specific, documented, *timing-dependent* failure that a naive
startup-time registration will hit.

- Return value is `BOOL`; failure detail via `GetLastError`. So a caller that ignores the return
  value fails **silently by construction**.
- The documented named failure (verbatim): "If this function is called before the dependent
  services of Remote Desktop Services have started, an **`RPC_S_INVALID_BINDING`** error code may
  be returned. When the `Global\TermSrvReadyEvent` global event is set, all dependent services have
  started and this function can be successfully called."

Source: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification>

This matters here: tracey is a desktop app that a user may have configured to launch at logon, i.e.
exactly the window in which Terminal Services dependents may not be up yet. The documented remedy
is to wait on `Global\TermSrvReadyEvent` (or retry), not to assume success.

In Rust with `windows 0.58` the `BOOL` is already converted for you — the generated binding calls
`.ok()` and returns `windows_core::Result<()>` (see §6). So `let _ = WTSRegisterSessionNotification(..)`
is the silent-failure shape to ban; a checked `Result` gives both the `HRESULT` and the underlying
Win32 code.

**What is detectable at runtime:**

| Failure | Detectable? | How |
| --- | --- | --- |
| Registration rejected (incl. `RPC_S_INVALID_BINDING`) | Yes, immediately | the `Result`/`BOOL` at the call site |
| Registration never attempted (thread/HWND unavailable) | Yes | `Window::hwnd()` returns `Err(InvalidWindowHandle)`; the main-thread dispatch can fail |
| Registered but message never arrives | **No — GAP.** There is no documented query for "is this HWND registered", no `GetWindowSubclass`-style introspection for WTS, and no heartbeat. | — |

Because the last row is undetectable, the standing decision ("a missed signal must degrade") needs
something outside the WTS mechanism to lean on. Two primary-source-backed candidates for a
*pollable* cross-check, neither of which is a lock signal by itself:

- `WTSQuerySessionInformation` with `WTSConnectState` → `WTS_CONNECTSTATE_CLASS`. Note the verbatim
  definition of `WTSDisconnected`: "The WinStation is active but the client is disconnected. This
  state occurs when a user is signed in but not actively connected to the device, **such as when
  the user has chosen to exit to the lock screen**." So connect-state is *correlated* with lock on
  the console, but Learn does not define it as a lock indicator. **GAP: this is not a documented
  lock query.**
  Source: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ne-wtsapi32-wts_connectstate_class>
- The existing reactive `ERR_SESSION_LOCKED` skip in `src-tauri/src/services/screenshot_service.rs:85-94`
  (`BitBlt` → `0x80070006` `ERROR_INVALID_HANDLE`), which the map already keeps as defence in depth.
  **GAP: I found no Microsoft documentation stating that `BitBlt` against the desktop DC fails with
  `ERROR_INVALID_HANDLE` on a locked session.** It is an empirical observation of this codebase, not
  a documented contract. The closest documented rationale is §5's window-station/desktop model.

---

## 5. What `WTS_SESSION_LOCK` does not cover

### Screensaver — not a WTS event at all

Confirmed by exclusion: the `wParam` table in §1 is the complete documented set and contains no
screensaver code. Two documented mechanisms exist instead, and neither is equivalent to a lock:

- **`WM_SYSCOMMAND` / `SC_SCREENSAVE` (0xF140)** — "Executes the screen saver application specified
  in the [boot] section of the System.ini file." When the command code is `SC_SCREENSAVE`, the low
  four bits carry `SCF_ISSECURE` (0x0001) = "The screen saver is secure." Caveats from the same
  page: `WM_SYSCOMMAND` is a *window menu / system command* message, "Any `WM_SYSCOMMAND` messages
  not handled by the application must be passed to `DefWindowProc`", and "If password protection is
  enabled by policy, the screen saver is started regardless of what an application does with the
  `SC_SCREENSAVE` notification even if it fails to pass it to `DefWindowProc`." This is a
  *pre-start* notification, and **GAP: Learn does not state that it is delivered to non-foreground
  or hidden windows**, so a tray-hidden tracey cannot be assumed to see it.
  Source: <https://learn.microsoft.com/en-us/windows/win32/menurc/wm-syscommand>
- **`SystemParametersInfo(SPI_GETSCREENSAVERRUNNING, …)` (0x0072)** — "Determines whether a screen
  saver is currently running on the window station of the calling process. […] Note that only the
  interactive window station, WinSta0, can have a screen saver running." This is a **poll**, not an
  event. Related: `SPI_GETSCREENSAVESECURE` (0x0076) reports whether the screensaver requires a
  password; `SPI_GETSCREENSAVEACTIVE` (0x0010) reports whether screen saving is *enabled* — and
  carries its own warning ("Windows 7, Windows Server 2008 R2 and Windows 2000: The function
  returns TRUE even when screen saving is not enabled").
  Source: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfoa>

So: a screensaver gate would need polling, which is a different architecture from an event-driven
lock gate. Consistent with #65 holding this out of scope.

Worth recording, because it explains *why* lock and screensaver are different animals — Learn,
Remote Desktop Sessions: "There are **three standard desktops** for each window station: the
Winlogon desktop, the **screen saver desktop**, and the interactive desktop."
Source: <https://learn.microsoft.com/en-us/windows/win32/termserv/terminal-services-sessions>

### Sleep / resume — `WM_POWERBROADCAST`, a separate mechanism

`WM_POWERBROADCAST` = 536 (0x218). `wParam` values:

| Event | Value | Meaning (verbatim) |
| --- | --- | --- |
| `PBT_APMSUSPEND` | 0x4 | "System is suspending operation." |
| `PBT_APMRESUMESUSPEND` | 0x7 | "Operation is resuming from a low-power state. This message is sent after `PBT_APMRESUMEAUTOMATIC` if the resume is triggered by user input, such as pressing a key." |
| `PBT_APMPOWERSTATUSCHANGE` | 0xA | "Power status has changed." |
| `PBT_APMRESUMEAUTOMATIC` | 0x12 | "Operation is resuming automatically from a low-power state. This message is sent every time the system resumes." |
| `PBT_POWERSETTINGCHANGE` | 0x8013 | "A power setting change event has been received." (`lParam` → `POWERBROADCAST_SETTING`) |

Remarks worth keeping: "The system **always** sends a `PBT_APMRESUMEAUTOMATIC` message whenever the
system resumes"; "`WM_POWERBROADCAST` messages do not distinguish between different low-power
states"; an app "should return `TRUE` if it processes this message".
Source: <https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast>

No registration call is needed for this one — but see §2's message-only-window caveat: a
`HWND_MESSAGE` window "does not receive broadcast messages", and **GAP: whether that excludes
`WM_POWERBROADCAST` is not stated by Learn.** If sleep/resume ever comes into scope (#65 currently
excludes it), this is the question that decides the HWND choice, so §2's answer should not be
over-fitted to WTS alone.

### What connect/disconnect actually mean for a session's desktop

From the primary model (Remote Desktop Sessions, linked above):

- Every session gets its own interactive window station, always named `WinSta0`, with three
  desktops (Winlogon / screen saver / interactive).
- `WTSGetActiveConsoleSessionId` identifies the session currently attached to the console.
- Log**off** destroys the session's window stations and desktops. Log-off is therefore categorically
  different from disconnect.

And from `WTSDisconnectSession`: it "Disconnects the logged-on user from the specified Remote
Desktop Services session **without closing the session**. If the user subsequently logs on to the
same RD Session Host server, the user is reconnected to the same session."
Source: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsdisconnectsession>

Reading the four codes against that model:

- **`WTS_CONSOLE_DISCONNECT` (0x2)** — the session lost the *console terminal* (physical
  monitor/keyboard). The canonical trigger is fast user switching: another user takes the console,
  tracey's session keeps running, its processes keep running, but it is no longer the session
  attached to the console.
- **`WTS_CONSOLE_CONNECT` (0x1)** — the session (re)acquired the console.
- **`WTS_REMOTE_DISCONNECT` (0x4)** — the session lost its *remote* terminal (RDP client closed /
  network dropped), session still alive per `WTSDisconnectSession`.
- **`WTS_REMOTE_CONNECT` (0x3)** — an RDP client attached to the session. A session can move
  between console and remote terminals, which is why both pairs exist.

The decision-relevant point: in all four cases the session and its desktops still exist — this is
not log-off. But in the two *disconnect* cases no terminal is presenting the desktop, so there is no
user looking at anything, which is arguably the same thing the lock gate is for.

### The disconnected-capture question (a downstream ticket depends on this)

**GAP — I could not establish this from a primary source, and I am not going to guess.**

Specifically: whether `BitBlt` against the desktop DC on a *disconnected* session fails (as it
empirically does on lock, with `ERROR_INVALID_HANDLE`) or succeeds and returns a black/stale frame.
What I searched and what I found:

- No statement in the `BitBlt`, `GetDC`, or Remote Desktop Services reference pages about GDI
  behaviour on a disconnected session.
- A Learn search scoped to `learn.microsoft.com` for disconnected-session/GDI/black-frame turned up
  only Q&A and troubleshooting threads about RDP black screens — community content and
  troubleshooting guides, not API contract, so out of policy for this note.
- The closest *primary* adjacent statement is for a different API: `AcquireNextFrame` returns
  `DXGI_ERROR_ACCESS_LOST` when "the desktop duplication interface is invalid. The desktop
  duplication interface typically becomes invalid when a different type of image is displayed on
  the desktop. Examples of this situation are: Desktop switch; Mode change; Switch from DWM on, DWM
  off, or other full-screen application."
  (<https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutputduplication-acquirenextframe>)
  "Desktop switch" is consistent with the window-station/three-desktops model explaining why capture
  breaks at lock — but Desktop Duplication is not GDI `BitBlt`, and this says nothing about
  *disconnect*.

Recommendation: #65's open item "Whether disconnected sessions produce garbage the reactive net
misses" cannot be closed from documentation. It needs an empirical answer — a throwaway prototype
that disconnects (fast user switch, and separately an RDP disconnect) while logging `BitBlt`'s
return and a checksum of the resulting bitmap. Until then, treat a black frame on a disconnected
session as **possible but unconfirmed**.

---

## 6. `windows` crate 0.58 — exposure and feature flags

All verified against the vendored source at
`~/.cargo/registry/src/index.crates.io-*/windows-0.58.0/`.

### Functions — module `windows::Win32::System::RemoteDesktop`

`src/Windows/Win32/System/RemoteDesktop/mod.rs:314`:

```rust
pub unsafe fn WTSRegisterSessionNotification<P0>(hwnd: P0, dwflags: u32) -> windows_core::Result<()>
where P0: windows_core::Param<super::super::Foundation::HWND>
```

`:439`:

```rust
pub unsafe fn WTSUnRegisterSessionNotification<P0>(hwnd: P0) -> windows_core::Result<()>
where P0: windows_core::Param<super::super::Foundation::HWND>
```

Both link `wtsapi32.dll` via `windows_targets::link!` and call `.ok()` on the returned `BOOL`, so
**the BOOL is already surfaced as a `Result`** — failure is a `Result::Err`, not a sentinel. The
`…Ex` variants (`WTSRegisterSessionNotificationEx` / `WTSUnRegisterSessionNotificationEx`, taking
an `hserver: HANDLE`) are also present at `:322` and `:447`.

`unsafe fn` — these are unsafe in the crate, so a wrapper is needed to keep the unsafe surface
contained.

### Constants

`windows::Win32::System::RemoteDesktop` (`:4797-4798`):

```rust
pub const NOTIFY_FOR_ALL_SESSIONS: u32 = 1u32;
pub const NOTIFY_FOR_THIS_SESSION: u32 = 0u32;
```

`windows::Win32::UI::WindowsAndMessaging` (`:5492`, `:5555-5565`):

```rust
pub const WM_WTSSESSION_CHANGE: u32 = 689u32;
pub const WTS_CONSOLE_CONNECT: u32 = 1u32;
pub const WTS_CONSOLE_DISCONNECT: u32 = 2u32;
pub const WTS_REMOTE_CONNECT: u32 = 3u32;
pub const WTS_REMOTE_DISCONNECT: u32 = 4u32;
pub const WTS_SESSION_LOGON: u32 = 5u32;
pub const WTS_SESSION_LOGOFF: u32 = 6u32;
pub const WTS_SESSION_LOCK: u32 = 7u32;
pub const WTS_SESSION_UNLOCK: u32 = 8u32;
pub const WTS_SESSION_REMOTE_CONTROL: u32 = 9u32;
pub const WTS_SESSION_CREATE: u32 = 10u32;
pub const WTS_SESSION_TERMINATE: u32 = 11u32;
```

**`WTS_SESSION_DESKTOP_READY` (0xF) is absent** — the grep for `^pub const WTS_` in that module
returns exactly the twelve lines above. Documented on Learn, not bound by `windows 0.58`.

### Feature flags

The repo's current list (`src-tauri/Cargo.toml`) already includes `Win32_Foundation`,
`Win32_UI_WindowsAndMessaging` and `Win32_Graphics_Gdi`. So:

| Needed for | Feature | Already enabled? |
| --- | --- | --- |
| `WM_WTSSESSION_CHANGE`, all `WTS_*` `wParam` constants | `Win32_UI_WindowsAndMessaging` | **Yes** |
| `HWND`, `WPARAM`, `LPARAM`, `BOOL` | `Win32_Foundation` | **Yes** |
| `WTSRegisterSessionNotification` / `WTSUnRegisterSessionNotification` / `NOTIFY_FOR_*` | **`Win32_System_RemoteDesktop`** | **No — must be added** |

`windows-0.58.0/Cargo.toml:658` confirms `Win32_System_RemoteDesktop = ["Win32_System"]`, so
enabling it pulls `Win32_System` transitively; no separate entry needed.

If the chosen mechanism goes beyond bare registration, the same file gives:

- `SetWindowSubclass` / `DefSubclassProc` / `RemoveWindowSubclass` → `Win32_UI_Controls` (not currently enabled)
- `CreateWindowExW`, `RegisterClassW`, `GetMessageW`, `DispatchMessageW`, `HWND_MESSAGE`, `DefWindowProcW`, `ShowWindow`, `SystemParametersInfoW` → `Win32_UI_WindowsAndMessaging` (already enabled)
- `WM_POWERBROADCAST` / `PBT_*` → `Win32_System_Power` (`Cargo.toml:652`, not currently enabled)
- `WTSQuerySessionInformationW` / `WTS_CONNECTSTATE_CLASS` → `Win32_System_RemoteDesktop`

Sources: vendored `windows 0.58.0` crate source (`src/Windows/...`, `Cargo.toml`); API docs
<https://docs.rs/windows/0.58.0/windows/Win32/System/RemoteDesktop/index.html>

---

## Summary for the #69 decision

**Established from primary sources:**

1. Registration is `WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)`, one message
   (`WM_WTSSESSION_CHANGE` = 689) with twelve documented `wParam` codes, `lParam` = session id, and
   a mandatory matching unregister before the window is destroyed. Re-registering an HWND ignores
   the new flags.
2. Tauri v2 really does hand out the HWND (`Window::hwnd()` / `WebviewWindow::hwnd()`), but as a
   `windows 0.61` `HWND` — a different type from this repo's `windows 0.58`, needing a deliberate
   pointer re-wrap.
3. tao already owns the main-thread pump *via `SetWindowSubclass`*, registers nothing WTS-related,
   and uses no message-only window. A second subclass with a distinct `uIdSubclass` chained through
   `DefSubclassProc` is the API-sanctioned way to intercept; `SetWindowLongPtr` is not. Subclassing
   must happen on the HWND-owning thread — never from a `tokio` task.
4. **Hiding the window does not stop delivery** — "A hidden window can process messages from the
   system or from other windows". `lib.rs`'s `prevent_close` + `hide()` only clears `WS_VISIBLE`.
   Destroying the window does stop delivery, and skipping the unregister breaks a documented
   contract.
5. Registration can fail, notably `RPC_S_INVALID_BINDING` before RDS dependents start
   (`Global\TermSrvReadyEvent` is the documented gate) — relevant for launch-at-logon. In
   `windows 0.58` the failure arrives as a `Result`, so silent failure requires actively discarding it.
6. Screensaver is not a WTS event (poll `SPI_GETSCREENSAVERRUNNING`, or the foreground-only
   `SC_SCREENSAVE` pre-notification). Sleep/resume is `WM_POWERBROADCAST` with its own five codes.
   All four connect/disconnect codes leave the session and its desktops alive — unlike log-off —
   but with no terminal presenting the desktop in the disconnect cases.
7. Only `Win32_System_RemoteDesktop` needs adding to the existing feature list for bare
   registration; `WTS_SESSION_DESKTOP_READY` is not bound by `windows 0.58` at all.

**Could not establish (do not let these get rounded up into facts):**

- Whether subclassing a Tauri window or a dedicated `HWND_MESSAGE` window is *idiomatic* — no
  primary source prescribes either. The docs give the trade-offs, not the verdict.
- Whether `BitBlt` on a disconnected session fails or yields a black frame. No Microsoft
  documentation found. Needs a prototype; #65's open item cannot be closed from docs.
- Whether a message-only window receives `WM_POWERBROADCAST`, given that such windows "do not
  receive broadcast messages".
- What actually happens if an HWND is destroyed without `WTSUnRegisterSessionNotification`.
- Any way to detect at runtime that a *successful* registration has stopped delivering. There is no
  documented "am I still registered" query and no heartbeat — which is precisely why the map's
  "degrade, not silently reopen" decision needs an independent cross-check to lean on.
- That `BitBlt` → `ERROR_INVALID_HANDLE` on a locked session is a documented contract. It is this
  repo's empirical observation; Microsoft does not document it.
