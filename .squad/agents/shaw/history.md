# Shaw — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Playwright E2E (against full Tauri app) + xUnit (.NET 10) + cargo test (Rust)
- **My files:** `tests/e2e/` (Playwright), `src/Tracey.Tests/` (xUnit)
- **Spec:** `specs/001-window-activity-tracker/spec.md` — 9 user stories, all acceptance scenarios map to my tests
- **TDD rule:** Failing tests MUST be committed before implementation begins. No exceptions.
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup & Test Strategy
- 9 user stories: US1 (timer), US2 (idle), US3 (client/project/task mgmt), US4 (screenshot timeline), US5 (fuzzy match), US6 (tags), US7 (reports), US8 (settings), US9 (multi-device sync)
- GDI screenshot capture stubbed via `#[cfg(feature="test")]` — Playwright runs use `--features test` build (Fusco configures CI)
- Playwright fixture provides IPC overrides for idle detection and screenshot triggers (no real OS events needed in E2E)
- Orphaned autocomplete has its own E2E test (T025a): create entry → delete project → type description → verify orphan indicator
- Overlap warning modal is tested: create overlapping manual entry, verify warning shown, verify `force: true` saves
- Reviewer gate: I hold test coverage gate. Shaw rejects → different agent revises (never original author)
- Rust coverage target: ≥ 80% branch coverage for business logic
- Tasks needing my initial tests (first): T018, T019 (US1), T026, T027 (US2), then US3–US9 in order
