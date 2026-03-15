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
