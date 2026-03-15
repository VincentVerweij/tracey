# Finch — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Tauri 2.0 (Rust, Windows 11) + Blazor WASM .NET 10 (C#, WebView2) + BlazorBlueprint.Components + SQLite (WAL) + optional Postgres/Supabase
- **Team:** Finch (Lead), Reese (Rust), Root (Blazor), Shaw (QA), Fusco (DevOps), Control (Security), Leon (Data)
- **Spec files:** `specs/001-window-activity-tracker/` → spec.md (9 user stories), plan.md, tasks.md, data-model.md, research.md
- **Contracts:** `contracts/ipc-commands.md`, `contracts/sync-api.md`
- **Constraints:** Portable exe, no installer, no admin, no registry. Screenshots local only. Blazor WASM (no server process).
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup
- Team assembled. 7 domain agents + Scribe + Ralph.
- All architecture decisions seeded in `.squad/decisions.md`.
- Constitution check in plan.md: all 7 principles pass (pre- and post-design).
- Tasks in 7+ phases (tasks.md): Phase 1 (setup) → Phase 2 (foundational) → Phases 3-9 (user stories US1-US9).
- Key reviewer gates: Shaw (test coverage), Control (security), Finch (architecture/IPC).
- IPC contract in `contracts/ipc-commands.md` is the source of truth for all Tauri commands.
