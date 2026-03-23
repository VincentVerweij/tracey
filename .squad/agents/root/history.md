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

### Files Implemented (Final Phase — T080, T081, T087)
- `Services/TauriIpcService.cs`: Added `DataDeleteAllAsync()` + `DataDeleteAllResponse` record
- `Pages/Settings.razor`: Extended with General (timezone, page size), Screenshots (interval, retention, storage path), Process Deny List (dynamic add/remove), Danger Zone (delete-all with confirmation)
- `Pages/Settings.razor.css`: Added `.settings-select`, `.settings-input-path`, deny-list block (`.settings-deny-*`), danger zone block (`.settings-card-danger`, `.settings-delete-confirm-*`)
- `Pages/Projects.razor`: Fixed pre-existing `&quot;` HTML-entity bug in `AriaLabel` attributes (lines 129, 198) — reverted to `@($"...")`  Razor pattern


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

### 2026-03-21: Final Phase — T080 / T081 / T087 — Settings completions

**T080 — General + Screenshots sections:**
- `UserPreferences` C# field names differ from IPC contract names: `Timezone` maps to `local_timezone`, `EntriesPerPage` maps to `page_size` — the task brief used wrong names; always verify with TauriIpcService.cs.
- `@bind:after` on `<select>` is valid in Blazor .NET 10 for auto-save on change.
- IANA timezone list added as `private static readonly string[] _ianaTimezones = [...]` — collection expression, .NET 10 / C# 12 compatible.
- Inner `try/catch` inside outer `try` block in `OnInitializedAsync` is fine for resilient JSON deserialization of deny list without aborting full prefs load.

**T087 — Process Deny List:**
- `_denyListInput` as a staging field; `AddDenyEntryAsync` trims, deduplicates with `OrdinalIgnoreCase`, appends to `_denyList`, and immediately calls `SaveDenyListAsync`.
- `@onkeydown="HandleDenyKeyDownAsync"` (named method) avoids Razor quote-conflict with Enter key check.
- `@onclick='() => RemoveDenyEntryAsync(entry)'` with single outer quote used for lambda — established pattern.
- `foreach` closure capture in .NET 10 is safe (each iteration gets its own binding).

**T081 — Danger Zone:**
- `DataDeleteAllAsync` + `DataDeleteAllResponse` added to `TauriIpcService.cs` under a new `// ── Data ──` section.
- `_deleteSuccess` (string?) shown with 5s auto-clear; `_deleteError` scoped to the danger zone section.
- `_deleting` bool drives `disabled="@_deleting"` on both confirm buttons to prevent double-submit.

**Projects.razor bugfix (incidental):**
- Pre-existing modification had `@($&quot;Confirm delete {name}&quot;)` — HTML entity `&quot;` inside a Razor C# expression causes CS8802/CS0116 cascading compile errors. Fixed to `@($"Confirm delete {name}")` — the standard `"@($"...")"` Blazor double-quote-in-attribute pattern.


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

### 2026-03-22: dotnet format — whitespace fixes + local dev integration

- `dotnet format src/Tracey.App/Tracey.App.csproj` is the auto-fix command; `--verify-no-changes` is the CI gate
- `.vscode/settings.json` enables `editor.formatOnSave` for C# to prevent future whitespace drift
- These are WHITESPACE-only errors — dotnet format handles them automatically, no manual editing needed

### 2026-03-23: E2E fixes — Projects WCAG contrast + Settings graceful degradation

**Projects.razor — WCAG color-contrast fix:**
- `BbButton` without `Variant=` renders with default (low-contrast) styling — axe-core flags [serious] color-contrast.
- All primary-action buttons (Add client, Save client, Save project, Save task) must have `Variant="ButtonVariant.Primary"` explicitly. Default is NOT Primary.
- Four buttons were missing explicit Variant: "Add client" (toolbar), "Save" in add-client form, "Save" in add-project form, "Save" in add-task form.

**Settings.razor — split OnInitializedAsync try/catch:**
- A single try/catch that wraps both `PreferencesGetAsync()` AND `SyncGetStatusAsync()` hides the entire Settings form if either fails.
- Pattern: PreferencesGetAsync failure → fatal (`_error` set, show error banner). SyncGetStatusAsync failure → non-fatal (`_syncStatus` stays null, template guards handle it with `@if (_syncStatus != null)`).
- Split into two separate try/catch blocks; `_loading = false` goes AFTER both blocks (not in finally of first).
- When the Tauri IPC bridge is absent (CI devserver, Phase 10 not yet implemented), SyncGetStatusAsync throws — must be isolated so prefs still load.

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

### 2026-03-18: Phase 9 build fix — missing packages + Razor backslash-escape bug

**Root cause (packages):** `Microsoft.NET.Sdk.BlazorWebAssembly` in .NET 10 does not transitively expose `Microsoft.Extensions.Hosting.Abstractions` (needed by `BackgroundService`) or `Microsoft.Extensions.Http` (needed by `IHttpClientFactory`). Fixed by adding both as explicit `PackageReference` at version `10.0.4` in `Tracey.App.csproj`.

**Root cause (Razor):** `Settings.razor` dictionary bindings used `@bind="_dict[\"key\"]"` — backslash escapes are illegal inside double-quoted Razor HTML attributes and cause CS1056/CS1003 syntax errors. Fixed by switching to single-quoted outer attributes: `@bind='_dict["key"]'`. This is consistent with the existing pattern already used for `@onclick` lambdas.

### 2026-03-18: Phase 4 idle detection — double-init fix + inactivity timeout in Settings

**Bug 1 — Dashboard.razor double-init:**
- `App.razor` owns `Events.InitializeAsync()` (runs once on first render). `Dashboard.razor` must NOT call it. The former comment "Dashboard.OnInitializedAsync must call await Events.InitializeAsync()" in history is now superseded.
- Dashboard's `HandleTimerTick` and its subscribe/unsubscribe also removed — `App.razor` already wires `OnTimerTick → ts.HandleTimerTick`. Duplicate wiring caused double state updates.
- Pattern: `App.razor` = event bridge owner; components = consumers for their own events only.

**Bug 2 — TauriIpcService idle types:** All already present (`IdleStatusResponse`, `IdleResolveRequest`, `IdleEntryDetails`, `IdleResolveResponse`, `IdleDetectedPayload`). No changes needed; verify before adding to avoid duplicates.

**Task 3 — Settings inactivity timeout:**
- `SaveInactivityAsync` is isolated (does not reuse `SaveAsync`/`SaveChannelConfigsAsync`) because it only updates one field. Avoids accidentally overwriting notification channels JSON with stale state.
- Load pattern: `prefs.InactivityTimeoutSeconds / 60.0` with fallback to `5.0` if zero.

**Task 4 — IdleReturnModal.razor.css:** File pre-existed with design-token-based styles. Always check before overwriting CSS scoped files.

### 2026-03-19: Phase 4 Final — IdleReturnModal Plain Overlay + Parse Error Fix

**BbDialog portal failure (net10):** `BbDialog` cannot be used in this project. `BbPortalHost` silently fails to register with `PortalService` on net10.0. RZ10012 is NOT harmless. The fix: replace with `@if (_isVisible)` conditional `<div class="idle-modal-backdrop">` overlay at `position: fixed; inset: 0; z-index: 9999`. This pattern is now standard for all modals in this project.

**Escaped quotes in Razor lambdas cause parse errors:** Never use `\"` inside a `@code` lambda or Razor inline expression. Razor's parser fails with RZ1027/CS1039/CS1073 on escaped quotes inside `@($"...")` string interpolations within lambdas. Hoist the expression to a local variable first. Example: `var label = $"Was: \"{value}\"";` then reference `label` inside the lambda.

**IdleReturnModal final state:**
- Plain HTML overlay (`@if (_isVisible)` + `<div class="idle-modal-backdrop">`)
- `@ref` + `FocusAsync()` for programmatic focus (autofocus does not work for dynamically-shown elements in WebView2)
- `_specifyError` field + `<p role="alert">` for empty-field validation
- `@onkeydown` handler submits Specify form on Enter
- Modal title shows duration string (e.g. "You were away for 7 minutes"), not static "You're back"