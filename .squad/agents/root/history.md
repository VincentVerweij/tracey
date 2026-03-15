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
