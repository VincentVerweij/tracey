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
- Tests written: timer.spec.ts (27), idle-detection.spec.ts (10), projects.spec.ts (14), screenshot-timeline.spec.ts (10), bug-fixes.spec.ts (4), timeline-bugs.spec.ts (2), issue-regressions.spec.ts (6), quick-entry.spec.ts (9), notifications.spec.ts (Phase 9), FuzzyMatchTests.cs (20 xUnit), NotificationChannelTests.cs (xUnit Phase 9)

### Test Strategy
- GDI screenshot capture stubbed via `#[cfg(feature="test")]` — Playwright runs use `--features test` build
- Playwright fixture provides IPC overrides for idle detection and screenshot triggers
- IPC fixture helpers use `window.__TAURI_INTERNALS__.invoke(...)` via `page.evaluate()`
- Shaw holds the test coverage gate — Shaw rejects → different agent revises (never original author)
- Rust coverage target: ≥ 80% branch coverage for business logic

### IPC Bridge Casing Contract (Tauri 2.0)
- Top-level Rust command params → **camelCase** in `window.__TAURI_INTERNALS__.invoke(...)`
- Fields inside Rust struct wrappers (`request: { ... }`) → **snake_case**
- Example: `invoke('project_create', { request: { client_id, name } })` vs `invoke('client_list', { includeArchived: true })`

### Key Selector Contracts (accumulated)
- `role="dialog" name=/idle|away|back/i` — idle modal
- `role="button" name=/break|meeting|specify|keep/i` — idle option buttons
- `role="textbox" name=/description|what were you doing/i` — Specify inline input
- `role="timer" aria-live="off" aria-atomic="true"` — elapsed counter
- `role="listbox"` / `role="option"` — autocomplete dropdown
- `.autocomplete-dropdown`, `.suggestion-item.is-orphaned`, `.orphan-warning[title]`
- `.time-entry-list`, `.entry-description-btn`, `.entry-edit-form`
- `.entry-input`, `.fuzzy-dropdown`, `.fuzzy-item-selected`, `.entry-segment-project`, `.entry-segment-task`
- `.disambiguation-dropdown`, `.match-char`
- `.client-card`, `.client-header`, `.project-row`, `.task-list`, `.chevron-btn`
- `.timeline-bar-inner`, `.timeline-zoom-indicator`
- `[data-testid="screenshot-item"]`, `[data-testid="screenshot-timestamp"]`, `[data-testid="trigger-badge"]`
- Phase 9: `[data-testid="notification-threshold"]`, `[data-testid="telegram-token"]`, `[data-testid="telegram-chat-id"]`

---

## Learnings

### 2026-03-19: Phase 4 Idle Detection — 3 Specify Sub-flow Tests Added

Added 3 tests to `idle-detection.spec.ts` (7 → 10 total). All three require genuine idle wait (Option A) — DOM dispatchEvent is not viable per prior notes.

- **Specify + Enter key**: fills input, presses Enter, asserts modal closes and entry appears in `time_entry_list`.
- **Specify + empty validation**: clicks Save without typing, asserts `role="alert"` contains "Please describe what you were doing.", asserts modal remains open.
- **Specify + Back button**: clicks Specify, then Back, asserts input is hidden and all four option buttons reappear.

**Infrastructure gaps found:**
- No `data-testid` attributes on any idle modal elements — all selectors rely on ARIA roles and accessible names. Resilient but any text copy change to option button labels would break `/break|meeting|specify|keep/i` regexes.
- Modal title is dynamic (`_durationText`) — `role="dialog" name=/away/i` matches "You were away for..." which is fine, but "You were away for a while" is the fallback when `idle_since` doesn't parse. Both match `/away/i`.
- `BbButton` components must render as native `<button>` for `getByRole('button', { name: /save|back/i })` to work. If BlazorBlueprint renders a `<div>` or `<a>`, these selectors will fail.
- `<p role="alert">` is hidden (not rendered) when `_specifyError` is empty string — Playwright alert selector only finds it after validation fires. This is correct behaviour.


**What changed from Phase 3 draft:**
- Replaced `page.goto('/')` with `page.goto(APP_URL)` (`http://localhost:5000`) — no baseURL in playwright.config.ts so naked `/` fails.
- Replaced `window.__TAURI_INTERNALS__` + `@ts-ignore` with `(window as any).__TAURI_INTERNALS__` (consistent with all other spec files).
- Replaced UI quick-entry form interactions with direct `timer_start` IPC calls (`{ request: { description, project_id: null, task_id: null, tag_ids: [] } }`) — faster and deterministic.
- Break/Meeting/Keep resolution outcomes now verified via `timer_get_active` IPC + `time_entry_list` IPC (`{ request: { page: 1, page_size: 20 } }`), not by navigating to Timeline and relying on text search.
- Added `test.afterEach` to restore 300 s timeout and stop any running timer between tests.
- Collapsed `IDLE_THRESHOLD_SECONDS = 5` / `WAIT_FOR_IDLE_MS = 10_000` into named constants so threshold can be adjusted in one place.
- Confirmed Option B (DOM `dispatchEvent`) is NOT viable: the JS bridge registers listeners via `plugin:event|listen` (Tauri native event system), not `window.addEventListener`.
- 7 tests total: no-timer guard, modal+4-buttons (AS1), Keep (AS2), Break (AS3), Meeting (AS4), Specify inline (AS5), threshold-not-exceeded (AS6).

**IPC shape reminders confirmed:**
- `timer_start`: `{ request: { description, project_id, task_id, tag_ids } }` — request-wrapped, snake_case.
- `time_entry_list`: `{ request: { page, page_size } }` — request-wrapped, snake_case.
- `timer_get_active`: no params, returns null when idle.
- `preferences_update`: `{ update: { inactivity_timeout_seconds } }` — update-wrapped, snake_case.

### 2026-03-18: T050 + T051 + T054a — Phase 7 US5 Fuzzy Quick-Entry Tests

**FuzzyMatchTests.cs** (xUnit, 20 tests): `Score` basics (empty query → 1.0, exact match, case-insensitive, empty candidate → 0.0, non-subsequence → 0.0), ordering (prefix > spread, consecutive > disjoint), `Theory` rows, `MatchMask`, `RankMatches`. Build: 0 errors. Fix: `Assert.DoesNotContain` lambda form (no `Comparer` named param in xUnit).

**quick-entry.spec.ts** (Playwright, 9 tests): AS1–AS5 (fuzzy dropdown open, ArrowDown selects, Tab confirms project chip, two `/` → project+task chips, Enter starts timer), T054a ×2 (unique → no disambiguation; shared name → disambiguation with both client names), highlight (`.match-char`). Test data via `beforeAll`/`afterAll` IPC helpers.

### 2026-03-18: T062 + T063 — Phase 9 US7 Notification Tests

**notifications.spec.ts** (Playwright E2E, T062): Settings UI structure (threshold input, email fields, Telegram fields), `preferences_get` IPC contract includes notification fields, `tracey://notification-sent` event routing no-crash, DI smoke test (no JS errors). Threshold-trigger integration test deferred to Fusco/`tauri-driver`.

**NotificationChannelTests.cs** (xUnit, T063): `NotificationChannelSettings.Get` fallback, `Disabled` singleton, `NotificationChannelConfigEntry` JSON round-trip, message body assertions, `NotificationOrchestrationService` StartAsync/StopAsync smoke. `FakeTauriIpcService` uses `new` (not `override`) — tech debt. `RecordingHttpMessageHandler` captures outgoing requests without any mock library.

TDD gate held: all tests written before implementation files existed.

### 2026-03-23: Tauri availability guards — shared helper + three spec fixes

Created `tests/e2e/specs/tauri-helpers.ts` with `hasTauriAvailable(page: Page): Promise<boolean>` — uses `page.evaluate(() => typeof (window as any).__TAURI_INTERNALS__ !== 'undefined')`.

**Pattern applied to three spec files:**
- **cloud-sync.spec.ts**: Added `test.beforeEach` guard inside AC2, AC3, AC4, AC5, AC6 describe blocks. Did NOT touch AC1 (UI), AC7 (UI), or the `sync_trigger` test that already passes via try/catch.
- **idle-detection.spec.ts**: Added `test.beforeAll` (browser fixture) guard at top of the US2 describe — checks Tauri availability once per suite, closes context, calls `test.skip()` if unavailable. Kept `mode: 'serial'` and all tests unchanged.
- **bug-fixes.spec.ts**: Added `test.beforeEach` guard to Bug 1+2, Bug 4, Bug 5 inner describes. For Bug 6, added both `beforeEach` guard AND an early-return guard in the existing `beforeAll` (prevents TypeError when `createClient` would call IPC without Tauri bridge).

**Key lesson:** `beforeEach` alone is insufficient when a describe also has `beforeAll` that calls IPC — `beforeAll` runs before `beforeEach` can skip the test. Fix: guard both hooks. Use early `return` in `beforeAll` after the availability check; use `test.skip()` in `beforeEach` to mark tests as skipped.

**Standard established (decision recorded):** Any new E2E test calling `window.__TAURI_INTERNALS__` MUST be inside a describe block with a `hasTauriAvailable` guard.

### 2026-03-21: T076 — Phase 11 US9 Portable Execution E2E Tests

**portable.spec.ts** (Playwright, 4 tests): Created `tests/e2e/specs/portable.spec.ts` covering US9 behavioral guarantees.

- **Test 1 — first launch preferences seeded**: calls `preferences_get`, asserts `inactivity_timeout_seconds=300`, `screenshot_interval_seconds=300`, `screenshot_retention_days > 0`, `local_timezone` non-empty. Verifies T012 first-launch init ran (DB created and seeded in exe directory).
- **Test 2 — full timer cycle no permission errors**: wraps `timer_start` + `timer_get_active` + `timer_stop` + `time_entry_list` in `.resolves.not.toThrow()`. A SQLite "readonly database" or "permission denied" error would surface as a rejected IPC promise.
- **Test 3 — data survives page navigation**: starts timer, navigates to `/tags`, returns to `/`, re-calls `timer_get_active` — must still return the running entry (stored in SQLite on disk, not in-memory).
- **Test 4 — health check clean**: `health_get` returns `running: true`, `active_errors.length === 0`.

**Scope note**: Binary-in-tempdir portability (copy `.exe` to temp folder, run as restricted user) is a CI-level test (T079/Fusco). These tests cover in-process behavioral guarantees only. Explicit comment in spec header explains the boundary.

### 2026-03-21: T084 + T088 — Performance Benchmarks & Accessibility Audit

**db_benchmarks.rs** (Criterion, 4 benchmarks): Created `src-tauri/benches/db_benchmarks.rs` with criterion harness. Added `criterion = { version = "0.5", features = ["html_reports"] }` to `[dev-dependencies]` and `[[bench]] name = "db_benchmarks" harness = false` in `Cargo.toml`.

- `bench_time_entry_list_100_rows_page1`: 100 pre-inserted rows, paged SELECT LIMIT 25 — baseline.
- `bench_time_entry_list_1000_rows_page1`: 1 000 rows — primary budget check (< 500 ms p95 per decisions.md).
- `bench_window_activity_insert`: single-row INSERT throughput — must be negligible vs 1-second polling interval.
- `bench_window_activity_unsynced_select_500`: SELECT of 500 un-synced rows (flush-to-external-DB query pattern).

**Key decisions on benchmark design:**
- In-memory SQLite (`Connection::open_in_memory()`) — isolates query cost from disk I/O; production WAL-mode latency will be higher but still within budget.
- `chrono` is already in `[dependencies]` (not dev-only) — available in benches without extra declaration.
- Timestamps use fixed ISO-8601 strings to keep chrono overhead out of the hot loop.
- `criterion::black_box` wraps collected rows to prevent compiler from eliding the query.

**accessibility.spec.ts** (Playwright + axe-core, 7 tests): Created `tests/e2e/specs/accessibility.spec.ts`. Added `@axe-core/playwright@^4.10.0` to `tests/e2e/package.json` devDependencies.

- 5 per-page axe audits (Dashboard, Projects, Tags, Timeline, Settings) — WCAG 2.1 AA tags: `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`.
- Dashboard keyboard nav: 50 Tab presses, asserts > 3 focusable elements receive focus.
- Settings keyboard nav: 80 Tab presses, asserts > 8 native form elements (`INPUT|SELECT|TEXTAREA|BUTTON`) receive focus.

**Key design choices:**
- No `baseURL` in `playwright.config.ts` — tests use `APP_URL = 'http://localhost:5000'` constant (consistent with all other specs).
- Violations are printed to stdout on failure for CI triage without re-running.
- `waitForTimeout(500)` after `networkidle` gives Blazor WASM component lifecycle time to complete (same pattern as portable.spec.ts).
- `AxeBuilder` imported from `@axe-core/playwright` — requires `npm install` before first run (`npm install --save-dev @axe-core/playwright@^4.10.0`).

**Pattern used**: Same `APP_URL`, `waitForApp`, `test.afterEach` stopTimer pattern as `timer.spec.ts` and `projects.spec.ts`.

---

## Archived Sessions (condensed)

### 2026-03-15: T018/T019 — US1 Timer TDD Gate
20 Playwright tests (timer.spec.ts) + xUnit tests (TimerStateServiceTests.cs). ARIA role contracts: `role="timer"` for elapsed, `role="listbox"` for autocomplete, `role="button" name=/continue/i` for continue. Tests written as failing gate before Phase 3 implementation.

### 2026-03-16: T031 — US2 Idle Detection E2E (idle-detection.spec.ts, 7 tests)
All acceptance scenarios: no-timer silent dismiss, modal appears + 4 option buttons, Keep/Break/Meeting/Specify flows, threshold-from-preferences. `preferences_update { inactivity_timeout_seconds: 5 }` for fast test runs.

### 2026-03-16: T037 — US3 Projects E2E (projects.spec.ts, 14 tests)
Full US3: navigate, create client/project/task, archive/unarchive, show-archived toggle, archived absent from autocomplete, cascade-delete with confirmation. IPC helpers via `window.__TAURI_INTERNALS__.invoke`.

### 2026-03-16: T025a/T029a/T030c — Phase 3 Batch 2 (7 tests added to timer.spec.ts)
T025a: orphan autocomplete warning + click-through (self-guard). T029a: scroll-position preservation (self-guard). T030c: inline edit opens, autosave on Tab, cancel discards, overlap error (self-guard). All self-guard while fixtures not yet wired.

### 2026-03-17: Bug-Fix TDD Gate (bug-fixes.spec.ts + timeline-bugs.spec.ts, 6 tests)
4+2 regression tests for Bugs 1–6. All FAIL before fixes — TDD gate open. Covers: project list deserialization, timer display frozen, entry list refresh, timeline reactive update, asset URL screenshot load.

### 2026-03-17: Issue Regression Tests (issue-regressions.spec.ts, 6 tests)
6 tests for 5 issues: elapsed time drift (DateTimeOffset fix), task_list camelCase, project_list clientId filter, archived-client hidden by default, archive name re-use.

### 2026-03-17: T042 — US4 Screenshot Timeline E2E (screenshot-timeline.spec.ts, 10 tests)
10 tests: navigate, empty-state, screenshot items appear, timestamp, process name + window title, trigger badge, click → preview, `tracey://error` → alert + dismiss, `screenshot_delete_expired` shape, time-range filter. Capture-dependent tests guard with `test.skip` if no screenshots. Key: `TauriEventService` must listen on `window` object for `tracey://error` CustomEvent.