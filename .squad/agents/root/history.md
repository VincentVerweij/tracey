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
