# Tracey — Squad Team

**Project**: Tracey — Window Activity Timetracking Tool  
**Branch**: `001-window-activity-tracker`  
**Created**: 2026-03-15  
**Requested by**: Vincent Verweij  

## Coordinator

| Name | Role | Notes |
|------|------|-------|
| Squad | Coordinator | Routes work, enforces handoffs and reviewer gates. |

## Members

| Name | Role | Domains | Badge |
|------|------|---------|-------|
| Finch | Lead/Architect | IPC contract design, code review, architecture gates, constitution check | 🏗️ Lead |
| Reese | Rust/Tauri Dev | Win32 APIs, window tracking, screenshots, SQLite, sync engine, keychain | 🔧 Backend |
| Root | Blazor/C# Dev | Blazor WASM UI, BlazorBlueprint, quick-entry bar, fuzzy matching, idle-return prompt | ⚛️ Frontend |
| Shaw | QA/Tester | Playwright E2E, xUnit, cargo test, test-first development, edge cases | 🧪 QA |
| Fusco | DevOps/CI | GitHub Actions, Tauri build pipeline, portable exe packaging, versioning | ⚙️ DevOps |
| Control | Security | Threat model, Tauri capabilities, keychain audit, IPC validation, path traversal | 🔒 Security |
| Leon | Data Engineer | SQLite schema, migration runner, Postgres sync strategy, WAL mode | 📊 Data |
| Scribe | Session Logger | Memory, decisions, orchestration logs | 📋 Scribe |
| Ralph | Work Monitor | Work queue, backlog tracking, issue triage | 🔄 Monitor |
| UXer | Frontend Designer | HTML, CSS, Blazor Component UI | 🎨 Design |

## Project Context

- **Project:** Tracey — Window Activity Timetracking Tool
- **Created:** 2026-03-15
- **Stack:** Tauri 2.0 (Rust, Windows 11) + Blazor WebAssembly .NET 10 (C#, WebView2) + BlazorBlueprint.Components + SQLite local (WAL mode) + optional Postgres/Supabase sync
- **Key constraints:** Portable exe — no installer, no admin rights, no registry. Screenshots stored locally only, never synced. Blazor WASM runs entirely in WebView2 (no .NET server process).
- **Spec files:** `specs/001-window-activity-tracker/` (spec.md, plan.md, tasks.md, data-model.md, research.md)
- **Contracts:** `specs/001-window-activity-tracker/contracts/ipc-commands.md` · `contracts/sync-api.md`

## Task Marking Policy

- **Who:** All implementation owners (the agent who authored the change) and the reviewer (Lead or assigned reviewer) share responsibility for marking tasks as complete in `specs/001-window-activity-tracker/tasks.md`.
- **When:** After an implementation is merged (or the work is landed in the branch), update the corresponding task entry from `- [ ]` to `- [x]` in `specs/001-window-activity-tracker/tasks.md` and include a short note (commit or PR body) pointing to the implementing files/PR.
- **Verification:** The reviewer (typically Finch or the assigned reviewer) must verify the implementation matches the task acceptance criteria before the task is checked off.
- **Scribe:** Scribe consolidates and archives task-marking metadata into `.squad/orchestration-log/` and `.squad/decisions.md` when appropriate.
