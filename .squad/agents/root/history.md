# Root — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Blazor WebAssembly .NET 10, BlazorBlueprint.Components 3.5.2, IJSRuntime for Tauri IPC, WebView2 inside Tauri
- **My files:** `src/Tracey.App/` (Pages/, Components/, Services/, wwwroot/)
- **IPC contract:** `specs/001-window-activity-tracker/contracts/ipc-commands.md`
- **Created:** 2026-03-15
- Solution: `Tracey.slnx` (.NET 10 XML format); builds 0 errors, 1 pre-existing RZ10012 warning (BbPortalHost TFM mismatch — harmless)
- `window.__TAURI_INTERNALS__.invoke` — Tauri 2.0 IPC bridge
- `TauriEventService.cs`: real `DotNetObjectReference` bridge (`wwwroot/js/tauri-bridge.js`); `[JSInvokable] RouteEvent` dispatches typed C# events

### Critical Patterns
- BB class interpolation: `Class="@($"base{(cond ? " extra" : "")}")"` — `Class="base @(expr)"` causes RZ9986
- `@bind:after` for post-change callbacks (e.g., archive toggle + reload)
- `@onclick` lambdas with string literals: use single-quote outer attribute `@onclick='() => Resolve("break")'`
- `@onkeydown` with string comparisons: extract to named method (Razor quote conflict)
- `@code` inside HTML comments causes `RZ2005`/`RZ1017` parse errors — never do this
- IPC wrapper: `new { request = new { ... } }` for Rust struct params; single-field commands pass arg name directly
- `JsonPropertyName`: `local_timezone` (not `timezone`), `page_size` (not `entries_per_page`)
- `ErrorPayload`: `{ Component, Event, Error }` — not `{ message }`
- `tracey://error` must be wired via both Tauri `listen()` AND `window.addEventListener` path (Shaw test 8 requirement)
- `convertFileSrc`: `https://asset.localhost/C%3A/...` — colon URL-encoded, provided by `tauri-bridge.js`

### Files Implemented (Phases 1–6)
- `Services/TauriIpcService.cs`: All IPC DTOs + command wrappers (all phases)
- `Services/TauriEventService.cs`: `DotNetObjectReference` bridge; `[JSInvokable] RouteEvent`; `IDisposable`
- `Services/TimerStateService.cs`: `ITimerStateService` + local `PeriodicTimer` fallback ticker
- `wwwroot/js/tauri-bridge.js`: IIFE bridge; `initializeTauriBridge`; `disposeTauriBridge`; `convertFileSrc`
- `Components/QuickEntryBar.razor`: Entry input + autocomplete + Ctrl+Space toggle; `⚠` orphan indicator
- `Components/TimeEntryList.razor`: Grouped list, running row (`role="timer"`), inline edit + autosave on blur
- `Components/IdleReturnModal.razor`: Idle return `<dialog>`; Break/Meeting/Specify/Keep; inline Specify input
- `Pages/Dashboard.razor`: QuickEntryBar + TimeEntryList; subscribes `OnTimerTick` + `OnIdleDetected`
- `Pages/Projects.razor`: Full client/project/task CRUD; lazy expand; inline forms; archive toggle
- `Pages/Timeline.razor`: 24h horizontal timeline bar, screenshot dots, hover preview, auto-refresh
- `_Imports.razor`: `@using Tracey.App.Components` added

### Timer Architecture
- `TimerStateService`: local `PeriodicTimer` 1s ticker for smooth UI; `HandleTimerTick(long)` snaps to Rust's authoritative value
- `StartLocalTicker()` called from `StartAsync()` and `InitializeAsync()` (if restoring running timer); `StopLocalTicker()` from `StopAsync()`
- `Dashboard.OnInitializedAsync` must call `await Events.InitializeAsync()` before subscribing tick events

### Timeline (Feature 7)
- Horizontal 24h bar: `.timeline-day-bar` > `.timeline-bar-inner` > dots + markers
- `TimeToPercent()`: UTC→local→seconds/86400×100%
- Hover: `HandleBarMouseMove` + `.timeline-hover-indicator` + `.hover-time-label`
- `GetImgSrcAsync` calls `traceyBridge.convertFileSrc` for correctly-encoded asset URLs
- CSS class contract locked with UXer (see decisions.md 2026-03-17 Bug-Fix Sprint section)

### Projects.razor Key Patterns
- Lazy load: clients on `OnInitializedAsync`, projects on expand, tasks on project expand
- Inline forms with `bool` toggle flags per level — no modal
- `ExpandLabel(bool, string)` helper avoids nested quotes in `aria-label` attributes
- `@bind` on dictionary indexers works when key is pre-initialized in show handlers
- Delete counts silently discarded (per spec — generic confirmation only)

### IdleReturnModal Pattern
- `<dialog role="dialog" aria-label="You're back">` — matches Shaw's `/idle|away|back/i`
- Four buttons: Break / Meeting / Specify / Keep
- Specify → inline input `aria-label="What were you doing?"` (not a second modal)
- `idle_ended_at` captured at `Show()` time (not button-click time)
- `InvokeAsync` wraps `Show()` + `StateHasChanged()` for Blazor thread safety

### Selector Contracts (Shaw-driven)
- `role="timer" aria-live="off" aria-atomic="true"` — elapsed counter
- `role="listbox"` / `role="option"` — autocomplete dropdown
- `.autocomplete-dropdown`, `.suggestion-item.is-orphaned`, `.orphan-warning[title]`
- `.time-entry-list`, `.entry-description-btn`, `.entry-edit-form`
- `input[aria-label="Entry description"]`, `input[aria-label="Start time"]`, `input[aria-label="End time"]`
- Timeline: `[data-testid="screenshot-item"]`, `[data-testid="screenshot-timestamp"]`, `[data-testid="trigger-badge"]`

---

## Learnings

### 2026-03-17: Bug Fixes — Tauri bridge, timer tick, Timeline redesign (dotnet build PASS)

**Build result:** 0 errors, 0 warnings on `Tracey.App.csproj`

**tauri-bridge.js** (`wwwroot/js/tauri-bridge.js`): IIFE bridge replacing no-op stub. `initializeTauriBridge(dotNetRef)` registers `__TAURI_INTERNALS__.listen` for all 7 `tracey://` events; each routes payload via `dotNetRef.invokeMethodAsync('RouteEvent', eventName, JSON.stringify(payload))`. `convertFileSrc` handles Windows drive-letter `%3A` encoding. Added `<script>` to `index.html` after `blazor.webassembly.js`.

**TauriEventService.cs**: `DotNetObjectReference`-based; `[JSInvokable] RouteEvent` deserializes JSON and dispatches typed events; graceful `JSException` fallback when outside Tauri host; `IDisposable`.

**Dashboard.razor**: `OnInitializedAsync` subscribes `OnTimerTick` + `OnIdleDetected`, then calls `await Events.InitializeAsync()`, then `ts.InitializeAsync()`. Added `HandleTimerTick(TimerTickPayload)` → `ts.HandleTimerTick(payload.ElapsedSeconds)`. `Dispose()` unsubscribes both.

**TimerStateService.cs**: Added `_localTicker` (`PeriodicTimer`) + `_tickerCts`. `StartLocalTicker()` / `StopLocalTicker()` manage 1s increment loop. `HandleTimerTick(long)` snaps to Rust value. Ticker wired in `StartAsync`, `StopAsync`, and `InitializeAsync` (if `_isRunning`).

**TimeEntryList.razor**: `StateHasChanged()` added to `LoadPage` finally block.

**Timeline.razor**: 24h horizontal bar replacing card grid (Feature 7). Hour markers, screenshot dots at `TimeToPercent()` positions, hover indicator + nearest preview, selected dot with close. Auto-refresh on `OnScreenshotCaptured`. `GetImgSrcAsync` calls `traceyBridge.convertFileSrc`.

---

## Archived Sessions (condensed)

### 2026-03-16: T027–T030 — TimerStateService + Dashboard + QuickEntryBar + TimeEntryList (build PASS)
Implemented `ITimerStateService` / `TimerStateService` (startup restore via `timer_get_active`), `QuickEntryBar` (Ctrl+Space, autocomplete, 200ms debounce, orphan indicator), `TimeEntryList` (grouped by date, `role="timer"` running row, inline edit stubs), `Dashboard` (composes both). `_Imports.razor` + `Program.cs` updated.

### 2026-03-16: T035/T036 — IdleReturnModal + Dashboard idle wiring (build PASS)
`IdleReturnModal.razor` with `<dialog>`, four buttons, inline Specify input. Dashboard subscribes `OnIdleDetected`, guards `HadActiveTimer`, calls `InvokeAsync`. `idle_ended_at` captured at `Show()` time.

### 2026-03-16: T041 — Projects.razor CRUD (build PASS)
Full client/project/task lazy-expand CRUD. Inline add forms, inline delete confirmation `role="dialog"`. `@bind:after` archive toggle. `ExpandLabel` helper. All existing TauriIpcService DTOs reused — no new types needed.

### 2026-03-17: T049 — Timeline.razor C# Plumbing (1 pre-existing warning)
Initial C# plumbing for screenshot timeline. `ScreenshotListAsync` corrected to `new { request = new { from, to } }`. `ErrorPayload` updated to `{ Component, Event, Error }`. `GetImgSrc` used `asset://localhost/` (old format — corrected to `convertFileSrc` in bug-fix session above). `SelectScreenshot` is `async Task`.
