# What Tracey's team is focused on now

**Phase:** Phase 2 — Core Infrastructure (near complete)
**Current focus:** Phase 2 checkpoint validation

## Completed this session
- T001-T006: All Phase 1 scaffolding + app shell
- T007: Capabilities locked
- T008: DB initializer + migration runner
- T009: DDL migrations (Leon)
- T010: Rust model structs
- T011: Structured JSON logger
- T012: First-launch init (screenshots dir + preferences seed)
- T013: preferences IPC
- T014: health_get IPC
- T015: TauriIpcService (34 typed methods)
- T016: TauriEventService + DI
- T017: App.razor nav shell
- T017b: PlatformHooks trait + Windows implementation

## Open — Phase 2 checkpoint
- Run `cargo tauri dev` to verify the app launches
- Verify DB is created and health_get responds

## Open flags requiring decisions
- preferences_get/update missing from ipc-commands.md contract
- health_get shape (contract vs briefing) — Finch to adjudicate
- sync_queue field conflict (data-model.md vs T072)
- JS event shim deferred to Final Phase

## Next phase: TDD gate
- T018: Shaw writes failing Playwright US1 tests
- T019: Shaw writes failing xUnit TimerStateService tests
- Tests committed and failing BEFORE any Phase 3 implementation
