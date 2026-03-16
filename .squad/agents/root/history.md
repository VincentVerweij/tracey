# Root — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Blazor WebAssembly .NET 10 (no server process), BlazorBlueprint.Components, IJSRuntime for Tauri IPC, WebView2 inside Tauri
- **My files:** `src/Tracey.App/` (Pages/, Components/, Services/, wwwroot/)
- **IPC contract:** `specs/001-window-activity-tracker/contracts/ipc-commands.md` — all commands I call come from here
- **Data model:** `specs/001-window-activity-tracker/data-model.md` — entity shapes I render
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: T002 — Blazor WASM Scaffold (completed)
- **.NET version**: net10.0 (SDK 10.0.104 installed — .NET 10 was available, no fallback needed)
- **Solution format**: .NET 10 `dotnet new sln` creates `.slnx` (new XML-based format), not `.sln` — use `Tracey.slnx` not `Tracey.sln`
- **BlazorBlueprint.Components**: FOUND on NuGet — version **3.5.2** installed successfully (with BlazorBlueprint.Primitives 3.5.0 and BlazorBlueprint.Icons.Lucide 2.0.0 as transitive deps)
- **MailKit**: installed version **4.15.1** successfully
- **dotnet build outcome**: `Build succeeded. 0 Warning(s). 0 Error(s)` on Tracey.slnx after scaffold
- **Stub pages created**: Dashboard.razor (`/dashboard`, also `/`), Projects.razor, Tags.razor, Timeline.razor, Settings.razor — all in `Tracey.App/Pages/`
- **Service stub created**: `Tracey.App/Services/TauriIpcService.cs` — empty class, typed overloads deferred to T015
- **Template pages kept**: Home.razor, Counter.razor, Weather.razor, NotFound.razor — can be removed in a later cleanup task
- **T005**: Tone-of-voice guide written to `docs/ux/tone.md`

### 2026-03-15: T015/T016/T017 — IPC Service, DI, Nav Shell (completed)

**Build result**: `Build succeeded in 8.5s — 0 Warning(s), 0 Error(s)` on `Tracey.slnx`

**TauriIpcService structure** (`src/Tracey.App/Services/TauriIpcService.cs`):
- Private `Invoke<T>(command, args?)` helper calls `window.__TAURI_INTERNALS__.invoke` (Tauri 2.0)
- Typed methods for all 34 IPC commands in the contract: health, preferences, timer (start/stop/get_active), time_entry (list/create_manual/continue/autocomplete), client CRUD+archive, project CRUD+archive, task CRUD, tag CRUD, fuzzy_match (projects/tasks), screenshot_list/delete_expired, idle_get_status/resolve, sync_get/configure/trigger
- All request/response types as C# `record` with `[property: JsonPropertyName("snake_case")]` — serialization matches Rust field names
- `AutocompleteSuggestion.IsOrphaned` included per architectural decision 2026-03-15
- `ScreenshotItem[]` returned directly (raw array) per contract's "Array of..." wording — see decisions inbox

**TauriEventService structure** (`src/Tracey.App/Services/TauriEventService.cs`):
- All 7 contract events registered: `timer-tick`, `idle-detected`, `idle-resolved`, `screenshot-captured`, `sync-status-changed`, `notification-sent`, `error`
- Payload types match contract (not task-prompt example — see decisions inbox for deviations)
- `Listen<T>` is a stub; JS shim for `DotNetObjectReference` callback bridge is deferred to Final Phase

**DI registrations** (`Program.cs`): `AddScoped<TauriIpcService>`, `AddScoped<TauriEventService>`

**App.razor**: `TauriEventService.InitializeAsync()` called from `OnAfterRenderAsync(firstRender)` — event subscriptions fire once at app root. Standard `<NotFound>` render fragment replaces erroneous `NotFoundPage` attribute.

**MainLayout.razor**: Replaced default Bootstrap shell with Tracey sidebar nav (Timer/Projects/Tags/Timeline/Settings). Health check logged to browser console on first render. Uses `HealthResponse.Running`/`EventsPerSec`/`MemoryMb` (contract fields, not the task-example's non-existent `Status`/`Version` fields).

**`_Imports.razor`**: Added `@using Tracey.App.Services`.

---

### 2026-03-15: Team Setup & Key Design Notes
- Blazor WASM (not Hybrid, not Server) — compiles C# to WASM, runs in WebView2 with no .NET server process
- All data writes go through Reese's IPC commands; I use `Microsoft.Data.Sqlite` for local read-only queries only
- Fuzzy match quickentry algorithm: pure C#, no JS, fully unit-testable, slash-delimited notation
- Time entry autocomplete shows `is_orphaned: true` indicator when referenced project/task was deleted
- All modals use the same BlazorBlueprint modal component (UX consistency, verified by Finch)
- Timezone: display in user's configured local timezone; all storage in UTC via Reese's commands
- Quick-entry bar is the highest-priority UI surface (drives daily workflow — US1)
- Tauri events to subscribe: `tracey://timer-tick`, `tracey://idle-detected`, `tracey://sync-status-changed`, `tracey://error`
- `TauriIpcService` is the single IPC abstraction — all Tauri calls go through it

---

### 2026-03-15: Phase 2 Session Completion Note (Scribe)

T015/T016/T017 complete. dotnet build 0 errors, 0 warnings. **Known open item for Final Phase:** JS event shim (`wwwroot/tauri-events.js`) is not yet implemented. `TauriEventService.Listen<T>` is a stub — all event subscriptions are wired as C# events but payloads are never delivered to components until the `DotNetObjectReference` callback bridge is built. Any component that depends on real-time Tauri events (TimerStateService T027, idle prompts, etc.) must be aware of this gap and must not assume events arrive before the Final Phase shim is in place.

---

### 2026-03-16: T030b — Full Inline Edit Mode in TimeEntryList (completed)

**Build result**: `Build succeeded in 4.4s — 0 Warning(s), 0 Error(s)` on `Tracey.slnx`

**Files changed**:
- `src/Tracey.App/Components/TimeEntryList.razor` — full inline edit form + AutoSave
- `src/Tracey.App/Services/TauriIpcService.cs` — `TimeEntryUpdateAsync` + `TimeEntryUpdateRequest`
- `specs/001-window-activity-tracker/contracts/ipc-commands.md` — `time_entry_update` contract added

---

### 2026-03-16: T041 — Projects.razor + TauriIpcService wrappers (completed)

**Build result**: `Build succeeded in 23.0s — 0 Warning(s), 0 Error(s)` on `Tracey.slnx`

**TauriIpcService.cs**: Already had all client/project/task DTOs and IPC methods from a prior session — no changes needed. Existing types used: `ClientItem`, `ProjectItem`, `TaskItem`, `ClientListResponse`, `ProjectListResponse`, `TaskListResponse`, `ClientCreateRequest`, `ProjectCreateRequest`, `TaskCreateRequest`, `ClientDeleteResponse`, `ModifiedAtResponse`, etc.

**Projects.razor** (`src/Tracey.App/Pages/Projects.razor`): Full implementation replacing stub.
- Lazy-load pattern: clients on `OnInitializedAsync`, projects on client expand, tasks on project expand
- Inline add forms (toggle `bool` flag per level) — no modal needed
- Delete confirmation: `_pendingDeleteClient` / `_pendingDeleteProject` inline `role="dialog"` inline-confirm divs
- Archive/Unarchive buttons with `aria-label` on all actions
- "Show archived" checkbox with `@bind:after="LoadClients"` — reload on toggle
- Error display: `<div role="alert" class="alert alert-danger">` with dismiss button
- All lambdas using string interpolation in attributes use `@($"...")` to avoid Razor quote-conflict
- `ExpandLabel(bool, string)` helper method avoids nested quotes in `aria-label` attributes
- `@bind` on dictionary indexers (`_newTaskName[project.Id]`) works when key pre-initialized in `ShowAddTask`/`ShowAddProject`

**App.razor**: No changes — `/projects` nav link already present in `MainLayout.razor`

**Learnings**:
- T041 done — Projects.razor + TauriIpcService wrappers, dotnet build 0 errors

---

### 2026-03-16: T035/T036 — IdleReturnModal + Dashboard idle wiring (completed)

**Build result**: `Build succeeded in 6.1s — 0 Warning(s), 0 Error(s)` on `Tracey.slnx`

**Task A — IPC verification**: `IdleGetStatusAsync()` and `IdleResolveAsync(IdleResolveRequest)` already existed with correct signatures. `IdleResolveRequest`, `IdleEntryDetails`, `IdleResolveResponse`, `IdleStatusResponse` all present. No changes to `TauriIpcService.cs` needed.

**Task B (T035) — `IdleReturnModal.razor`** (`src/Tracey.App/Components/`):
- `<dialog>` element with `role="dialog"` and `aria-label="You're back"` — matches Shaw's Playwright locator `role="dialog" name=/idle|away|back/i`
- Displays human-readable idle duration computed from `idle_since` UTC timestamp
- Four buttons in order: Break / Meeting / Specify / Keep
- "Specify" reveals inline description input (`aria-label="What were you doing?"`) — matches Shaw's locator `role="textbox" name=/description|what were you doing/i`
- All `@onclick` lambdas with string literals use single-quote outer attribute to avoid Razor conflicting-quote parse error
- Calls `IdleResolveAsync` on selection; raises `OnResolved` EventCallback for Dashboard refresh
- `Dispose()` is a no-op (no subscriptions owned by this component)

**Task C (T036) — `Dashboard.razor`**:
- Added `@inject TauriEventService Events`
- Added `<IdleReturnModal @ref="_idleModal" OnResolved="HandleIdleResolved" />`
- `OnInitializedAsync` subscribes `Events.OnIdleDetected += HandleIdleDetected`
- `HandleIdleDetected` guards on `payload.HadActiveTimer`; calls `_idleModal?.Show(payload.IdleSince)` inside `InvokeAsync` for thread-safe component update
- `Dispose()` unsubscribes `Events.OnIdleDetected -= HandleIdleDetected`

**Idle modal pattern established**:
- Modal is a child component with a `Show(string idleSince)` method called by the parent page
- Parent subscribes/unsubscribes the Tauri event in `OnInitializedAsync`/`Dispose`
- Resolution always flows: user picks option → `IdleResolveAsync` → `OnResolved.InvokeAsync()` → Dashboard `RefreshList()`

**Inline edit behaviour**:
- Click completed entry row → `StartInlineEdit(TimeEntryItem entry)` captures full entry (description + both UTC timestamps converted to local `DateTime`)
- Edit form: description `<input type="text">`, start and end `<input type="datetime-local">` — all in-place, no modal
- Blur from any field → `AutoSave(entry)` — converts local `DateTime` back to UTC ISO string (`.ToString("o")`), calls `time_entry_update` IPC, clears `_editingId`, reloads list
- `_isSaving` guard prevents concurrent saves on rapid tab-through
- Overlap / invalid-time errors shown inline via `<p class="edit-error" role="alert">`; `_editingId` is NOT cleared on error — user can correct and blur again
- Cancel button discards edits; no save call

**Type corrections**:
- `StartInlineEdit` signature changed from `(string entryId)` to `(TimeEntryItem entry)` — caller updated to pass full object
- `SaveInlineEdit` removed; replaced by `AutoSave(TimeEntryItem entry)`

**IPC additions**:
- `time_entry_update` contract written into `ipc-commands.md` (input: id, description, project_id, task_id, tag_ids, started_at, ended_at, force; output: `{ "modified_at": ... }`; errors: not_found, invalid_time_range, overlap_detected)
- `TimeEntryUpdateRequest` record + `TimeEntryUpdateAsync` method added to `TauriIpcService.cs` (returns `ModifiedAtResponse`, consistent with other update commands)

---

### 2026-03-16: T027/T028/T029/T030 — TimerStateService + Dashboard Components (completed)

**Build result**: `Build succeeded in 9.4s — 0 Warning(s), 0 Error(s)` on `Tracey.slnx`

**T027 — TimerStateService** (`src/Tracey.App/Services/TimerStateService.cs`):
- Implements full `ITimerStateService` as specified by Shaw (T019), including `CurrentProjectId` and `CurrentTaskId` (present in Shaw's test file, not in prompt stub)
- `HandleTimerTick(long elapsedSeconds)` — called from `TauriEventService.OnTimerTick` via wiring in `App.razor`
- `InitializeAsync()` — calls `timer_get_active` IPC on startup to restore state across restarts; calculates elapsed from `started_at` UTC diff
- `StopAsync()` — no-op guard when `!_isRunning`; swallows `no_active_timer` errors
- Registered in `Program.cs` as `AddScoped<ITimerStateService, TimerStateService>()`
- Timer tick wired in `App.razor` `OnAfterRenderAsync`: `Events.OnTimerTick += p => ts.HandleTimerTick(p.ElapsedSeconds)` (cast pattern — same as Dashboard)

**T028 — QuickEntryBar** (`src/Tracey.App/Components/QuickEntryBar.razor`):
- `aria-label="What are you working on?"` — matches Shaw's Playwright locator
- Enter → `Timer.StartAsync(description.Trim())`
- Ctrl+Space → toggles running/stopped, focuses input when idle
- Stop button with `aria-label="Stop timer"` — matches Shaw's `role="button" name=/stop/i`
- Autocomplete: 200ms debounce, ≥2 char threshold, `⚠` orphan indicator
- Type corrections applied: `TimeEntryAutocompleteRequest` (not `AutocompleteRequest`), `result.Suggestions.ToList()`

**T029 — TimeEntryList** (`src/Tracey.App/Components/TimeEntryList.razor`):
- Running timer row with `role="timer"` — matches Shaw's Playwright locator
- Grouped by date (descending), `DateOnly.Parse(e.StartedAt[..10])` for grouping
- `LoadPage` is `public` — called from `Dashboard.razor` via `@ref`
- Type corrections: `TimeEntryItem` (not `TimeEntryListItem`), `TimeEntryContinueAsync(entryId)` string not request object
- `FormatTime` handles nullable `string?` (`ended_at` can be null on running entry hypothetically)

**T030 — Dashboard** (`src/Tracey.App/Pages/Dashboard.razor`):
- Replaced stub; composes `<QuickEntryBar>` + `<TimeEntryList>`; calls `ts.InitializeAsync()` on mount via cast

**Other file updates**:
- `_Imports.razor`: added `@using Tracey.App.Components`
- `App.razor`: added `@inject ITimerStateService TimerService` + timer tick wiring in `OnAfterRenderAsync`
- `Program.cs`: `AddScoped<ITimerStateService, TimerStateService>()` registered

**Components directory**: `src/Tracey.App/Components/` created (new)

---

### 2026-03-15: Bug Fix — JsonPropertyName Mismatches + beforeDevCommand (Root)

**Bug fix (Finch blocking bug on T015):**
Fixed 4 incorrect `[JsonPropertyName]` attributes in `TauriIpcService.cs`:
- `UserPreferences.Timezone`: `"timezone"` → `"local_timezone"`
- `UserPreferences.EntriesPerPage`: `"entries_per_page"` → `"page_size"`
- `PreferencesUpdateRequest.Timezone`: `"timezone"` → `"local_timezone"`
- `PreferencesUpdateRequest.EntriesPerPage`: `"entries_per_page"` → `"page_size"`

All other fields in both records were confirmed correct against the IPC contract.

**Dev server decision (`beforeDevCommand`):**
`Tracey.App` uses `Microsoft.NET.Sdk.BlazorWebAssembly` (pure WASM, no ASP.NET host process), but includes `Microsoft.AspNetCore.Components.WebAssembly.DevServer` which provides a lightweight static-file dev server invokable via `dotnet run`. Set `beforeDevCommand` in `tauri.conf.json` to:
```
dotnet watch run --project src/Tracey.App --urls http://localhost:5000
```
`devUrl` remains `http://localhost:5000`. This gives hot-reload in dev without a separate tool.

**Build result:** `dotnet build Tracey.slnx` — Build succeeded, 0 errors, 0 warnings.
