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
- Tauri 2.0 IPC params: top-level Rust params must be **camelCase** in C# anonymous objects
- Rust timestamps: use `DateTimeOffset.TryParse` (not `DateTime`); Rust `+00:00` suffix causes `Kind=Local` errors
- `BackgroundService` in WASM is singleton-scoped; injecting scoped services is safe (single DI root scope)
- `@@` in Razor renders as literal `@` (e.g. `@@BotFather`)

### Files Implemented (Phases 1–9)
- `Services/TauriIpcService.cs`: All IPC DTOs + command wrappers (all phases); `NotificationChannelsJson` added Phase 9
- `Services/TauriEventService.cs`: `DotNetObjectReference` bridge; `[JSInvokable] RouteEvent`; `RaiseNotificationSent()` added Phase 9
- `Services/TimerStateService.cs`: `ITimerStateService` + local `PeriodicTimer` fallback ticker; `CurrentTagIds`/names
- `Services/FuzzyMatchService.cs`: subsequence scorer, `MatchMask`, `RankMatches<T>()`
- `Services/NotificationOrchestrationService.cs`: `BackgroundService`, 60s loop, single-fire guard
- `Services/Notifications/INotificationChannel.cs`: interface + `NotificationMessage` + `NotificationChannelSettings`
- `Services/Notifications/EmailNotificationChannel.cs`: WASM stub (NotSupportedException)
- `Services/Notifications/TelegramNotificationChannel.cs`: Telegram Bot API via IHttpClientFactory
- `wwwroot/js/tauri-bridge.js`: IIFE bridge; `initializeTauriBridge`; `disposeTauriBridge`; `convertFileSrc`
- `Components/QuickEntryBar.razor`: slash-notation state machine, fuzzy/disambiguation dropdowns, breadcrumb
- `Components/TimeEntryList.razor`: grouped list, inline edit with explicit Save/Cancel, delete confirmation
- `Components/TagPicker.razor`: always-visible; live tag persistence on running entries
- `Components/IdleReturnModal.razor`: idle return `<dialog>`; Break/Meeting/Specify/Keep
- `Pages/Dashboard.razor`: QuickEntryBar + TimeEntryList; subscribes events
- `Pages/Projects.razor`: full client/project/task CRUD; lazy expand; archive toggle
- `Pages/Timeline.razor`: 24h bar, screenshot dots, hover preview, scroll-zoom
- `Pages/Settings.razor`: Notifications section (threshold, email stub, Telegram)
- `_Imports.razor`: `@using Tracey.App.Components` added

### Timer Architecture
- `TimerStateService`: local `PeriodicTimer` 1s ticker for smooth UI; `HandleTimerTick(long)` snaps to Rust's authoritative value
- `StartLocalTicker()` called from `StartAsync()` and `InitializeAsync()` (if restoring running timer); `StopLocalTicker()` from `StopAsync()`
- `Dashboard.OnInitializedAsync` must call `await Events.InitializeAsync()` before subscribing tick events
- `CurrentTagIds`, `CurrentProjectName/ClientId/ClientName/TaskName` all stored in service; restored on cold boot from enriched `timer_get_active` response

### Timeline (Feature 7)
- Horizontal 24h bar: `.timeline-day-bar` > `.timeline-bar-inner` > dots + markers
- `TimeToPercent()`: instance method; maps `(localHours - _viewStartHours) / _zoomHours * 100`
- Scroll-zoom: `_zoomHours` (0.5–24) + `_viewStartHours`; `HandleBarWheel` anchors on mouse position; double-click resets
- Hover: `HandleBarMouseMove` + `.timeline-hover-indicator` + `.hover-time-label`
- `GetImgSrcAsync` calls `traceyBridge.convertFileSrc` for correctly-encoded asset URLs

### Selector Contracts (Shaw-driven)
- `role="timer" aria-live="off" aria-atomic="true"` — elapsed counter
- `role="listbox"` / `role="option"` — autocomplete dropdown
- `.autocomplete-dropdown`, `.suggestion-item.is-orphaned`, `.orphan-warning[title]`
- `.time-entry-list`, `.entry-description-btn`, `.entry-edit-form`
- `.entry-input`, `.fuzzy-dropdown`, `.fuzzy-item-selected`, `.entry-segment-project`, `.entry-segment-task`
- `.disambiguation-dropdown`, `.match-char`, `.running-elapsed`
- Timeline: `[data-testid="screenshot-item"]`, `[data-testid="screenshot-timestamp"]`, `[data-testid="trigger-badge"]`

---

## Learnings

### 2026-03-18: Running-timer tag fixes — always-visible TagPicker, CurrentTagIds restore, tag-only partial update

**TagPicker always-visible:** Removed `@if (!Timer.IsRunning)` guard from `QuickEntryBar.razor`.

**Tag restore:** `string[] CurrentTagIds` added to `ITimerStateService`. Restore in `RestoreFromTimer()` guarded by `_selectedTagIds.Count == 0` — prevents clobbering user mid-session edits.

**Partial tag update:** `TimeEntryUpdateTagsAsync` sends only `{ id, tag_ids }`. `ended_at = NULL` preserved by Rust — running timer unaffected. Failures silently swallowed (non-fatal).

### 2026-03-18: Phase 9 — Notification channels + orchestration service + Settings.razor

**New notification files:**
- `INotificationChannel.cs`: `SendAsync(NotificationMessage, NotificationChannelSettings, CancellationToken)`. Settings at call time; channels stateless. `NotificationChannelSettings.Get(config, channelId)` + `Disabled` singleton.
- `EmailNotificationChannel.cs`: WASM stub, `NotSupportedException`. Placeholder for future `notifications_send_email` Tauri command.
- `TelegramNotificationChannel.cs`: Full Telegram Bot API `sendMessage`, `IHttpClientFactory`, MarkdownV2 escaping.
- `NotificationOrchestrationService.cs`: `BackgroundService`, 60s `PeriodicTimer`, `_notifiedForEntryId` single-fire guard, raises `tracery://notification-sent` via `TauriEventService.RaiseNotificationSent()`.

**Modified:**
- `TauriIpcService.cs`: `NotificationChannelsJson` with `[JsonPropertyName("notification_channels_json")]` added to `UserPreferences` and `PreferencesUpdateRequest` (IPC contract gap since Phase 2).
- `TauriEventService.cs`: `public void RaiseNotificationSent(NotificationSentPayload)` — raises typed C# event without JS bridge.
- `Settings.razor`: Full Notifications section — threshold (hours), email fields (WASM warning), Telegram fields.
- `Program.cs`: `AddHttpClient()`, two `AddSingleton<INotificationChannel>`, `AddHostedService<NotificationOrchestrationService>()`.

---

## Archived Sessions (condensed)

### 2026-03-16: T027–T030 — TimerStateService + Dashboard + QuickEntryBar + TimeEntryList
Implemented `ITimerStateService`/`TimerStateService` (startup restore via `timer_get_active`), `QuickEntryBar` (Ctrl+Space, autocomplete, 200ms debounce, orphan indicator), `TimeEntryList` (grouped by date, `role="timer"` running row, inline edit stubs), `Dashboard`. `_Imports.razor` + `Program.cs` updated.

### 2026-03-16: T035/T036 — IdleReturnModal + Dashboard idle wiring
`IdleReturnModal.razor` with `<dialog>`, four buttons, inline Specify input. Dashboard subscribes `OnIdleDetected`, guards `HadActiveTimer`, calls `InvokeAsync`. `idle_ended_at` captured at `Show()` time.

### 2026-03-16: T041 — Projects.razor CRUD
Full client/project/task lazy-expand CRUD. Inline add forms, inline delete confirmation. `@bind:after` archive toggle. `ExpandLabel` helper.

### 2026-03-17: T049 — Timeline.razor C# Plumbing
`ScreenshotListAsync` corrected to `new { request = new { from, to } }`. `ErrorPayload` → `{ Component, Event, Error }`. `GetImgSrc` corrected to `convertFileSrc`.

### 2026-03-17–18: IPC camelCase + DateTimeOffset + Timeline zoom + FuzzyMatchService + QuickEntryBar slash-notation
Tauri 2.0 bridge: top-level params must be camelCase. `DateTimeOffset.TryParse` for all Rust timestamps. Timeline zoom state (`_zoomHours`, `_viewStartHours`). `FuzzyMatchService`: subsequence scorer + `MatchMask` + `RankMatches`. QuickEntryBar slash-notation state machine, fuzzy/disambiguation dropdowns, breadcrumb prefix replacing chips, backward-navigation via Backspace.