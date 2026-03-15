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

### 2026-03-15: Phase 1 + 2 Kickoff Review

**IPC Contract Gap Fixed**: `time_entry_autocomplete` response was missing `is_orphaned: boolean`. Field added via inbox decision. Reese (T025) and Root (T028) must both use this field. Contract is now authoritative.

**T082 Ordering Bug**: Win32 polling loop (foreground window detection) placed in Final Phase, but Phase 6 (US4) requires it for window-change-triggered screenshots (T046). Decision filed in inbox for Vincent: split T082 into T082a (polling loop → Phase 2) and T082b (ActivityRecord writes → Final Phase). If Vincent approves, Reese carries T082a as early Phase 2 work and it unlocks US4 fully.

**PlatformHooks Trait Simplified**: T017b originally defined three methods including `trigger_screenshot_capture()`. This created an ownership ambiguity (T082 says ActivityTracker calls ScreenshotService directly, bypassing PlatformHooks). Decision: trait reduced to two methods — `get_foreground_window_info()` and `get_idle_seconds()`. Capture triggering stays in ActivityTracker/ScreenshotService layer.

**T058 File Routing**: Tag commands incorrectly routed to `activity.rs` in T058. Corrected to `tags.rs`. Leon and Reese to note.

**research.md Section 5 Inconsistency**: research.md still says "Blazor Server" as the selected approach. It's a historical artifact from the options phase. Blazor WASM is the final decision (decisions.md, tasks.md). Not a blocker.

**Task Ownership Confirmed**:
- Phase 1: T001(Reese), T002(Root), T003(Shaw), T004(Shaw), T005(Root), T017b(Reese)
- Phase 2: T006-T014(Reese, with Leon on T008/T009 schema), T015-T017(Root), T007 gates on Control review
- TDD gate: Shaw writes T018+T019 and they MUST FAIL before any Phase 3 implementation begins

**Control gate**: T007 (Tauri capabilities) requires Control review before merge. Confirmed permission slug: `fs:allow-write-file` (singular). Wrong slug fails silently.

**Phase 2 strict ordering**: T006 → T007 → T008 → T009 → T012. Everything else in Phase 2 is parallelizable once scaffolding exists.
