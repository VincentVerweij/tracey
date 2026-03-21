# Tasks: Window Activity Timetracking Tool

**Input**: Design documents from `/specs/001-window-activity-tracker/`
**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/ ✅ | quickstart.md ✅

**Stack**: Tauri 2.0 (Rust) · **Blazor WebAssembly** .NET 10 (C#) · BlazorBlueprint.Components · SQLite (local; Rust layer) · Postgres/Supabase (external sync) · Playwright (E2E) · xUnit (unit)

> **Rationale — Blazor WebAssembly chosen over Blazor Server**: Blazor WebAssembly compiles C# to WASM and runs entirely inside Tauri's WebView2 with no server-side SignalR dependency, matching the portable offline-capable single-binary requirement. All data access is handled by the Rust layer via Tauri IPC; no .NET server process is needed.

---

## Format: `[ID] [P?] [Story?] Description — file path`

- **[P]**: Can run in parallel (different files, no in-flight dependencies)
- **[Story]**: User story label (US1–US9)
- **Tests come FIRST** within each story: write and confirm they fail before implementing

---

## Phase 1: Setup

**Purpose**: Scaffold repository layout and initialize all projects. No functional code yet.

- [x] T001 Initialize Tauri 2.0 project: `cargo create-tauri-app`, configure `src-tauri/Cargo.toml` with all dependencies (tauri 2.0, tauri-plugin-system-idle, windows 0.58, image, serde, keyring, tokio) and `[features] test = []` flag — `src-tauri/Cargo.toml`
- [x] T002 [P] Initialize **Blazor WebAssembly** .NET 10 solution and application project (WASM hosting model — no server-side SignalR, no in-process .NET server); NuGet packages: BlazorBlueprint.Components, MailKit; SQLite and Postgres access are Rust-layer only and do not appear in this project — `src/Tracey.sln`, `src/Tracey.App/Tracey.App.csproj`
- [x] T003 [P] Initialize xUnit test project referencing Tracey.App — `src/Tracey.Tests/Tracey.Tests.csproj`
- [x] T004 [P] Initialize Playwright E2E test project with `playwright.config.ts` (app launch as subprocess, test fixture for IPC overrides, `--features test` build for screenshot tests) — `tests/e2e/`
- [x] T005 [P] Create UX tone-of-voice guide with tone principles and example copy — `docs/ux/tone.md`
 
  > Note: A user-facing string tone audit across all .razor files (T089) is deferred to the Final Phase when UI components exist.

**Checkpoint**: All projects build cleanly (`cargo build`, `dotnet build`, `npx playwright install`)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before any user story can begin.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T006 Implement Tauri app shell: `main.rs` entry point, `lib.rs` with plugin registration (tauri-plugin-system-idle, tauri-plugin-fs), window configuration in `tauri.conf.json` — `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`
- [x] T007 Configure Tauri capabilities with least-privilege grants (`fs:allow-write-file` singular, `system-idle:allow-get-idle-time`; CSP locking WebView2 to `tauri://localhost` only) — `src-tauri/capabilities/default.json`
- [x] T008 Implement SQLite DB initializer: open/create `tracey.db` in portable path (`{exe_dir}` or `{APPDATA}/tracey/` fallback), set `PRAGMA journal_mode = WAL`, `PRAGMA foreign_keys = ON` — `src-tauri/src/db/mod.rs`
- [x] T009 Implement sequential migration runner and write all DDL migrations (clients, projects, tasks, tags, time_entries, time_entry_tags, window_activity_records, user_preferences, sync_queue) matching `data-model.md` — `src-tauri/src/db/migrations/`
- [x] T010 [P] Create Rust model structs for all entities (Client, Project, Task, Tag, TimeEntry, TimeEntryTag, WindowActivityRecord, UserPreferences, SyncQueueEntry) with `serde::Serialize/Deserialize` — `src-tauri/src/models/mod.rs`
- [x] T011 [P] Implement structured JSON logger with fields `timestamp`, `level`, `component`, `event`, `trace_id`; apply deny-list redaction of sensitive process names before any log write — `src-tauri/src/`
- [x] T012 Implement portable first-launch initialization: create `{exe_dir}/screenshots/` directory, seed default `user_preferences` row on fresh DB — `src-tauri/src/`
- [x] T013 [P] Implement `preferences_get` and `preferences_update` Tauri IPC commands (used by Settings UI and test fixtures to override inactivity timeout) — `src-tauri/src/commands/`
- [x] T014 [P] Implement `health_get` Tauri IPC command returning running state, last-write timestamp, events/sec, memory MB, active errors, pending sync count — `src-tauri/src/commands/`
- [x] T015 Implement `TauriIpcService` in C# with `InvokeAsync<T>` wrapper around `IJSRuntime`, typed overloads for every command in `contracts/ipc-commands.md`, and Tauri event subscription helpers — `src/Tracey.App/Services/TauriIpcService.cs`
- [x] T016 [P] Register Blazor services in DI, configure Tauri event subscriptions (`tracey://timer-tick`, `tracey://idle-detected`, `tracey://sync-status-changed`, `tracey://error`) using JS interop in app startup — `src/Tracey.App/Program.cs`
- [x] T017 [P] Implement `App.razor` navigation shell: sidebar/nav links to Dashboard, Projects, Tags, Timeline, Settings pages — `src/Tracey.App/App.razor`
- [x] T017b [P] Define `PlatformHooks` trait in `src-tauri/src/platform/mod.rs` with methods `get_foreground_window_info() -> Option<WindowInfo>`, `get_idle_seconds() -> u64`, and `trigger_screenshot_capture() -> Result<()>`; Windows implementation in `src-tauri/src/platform/windows.rs` will implement this trait (referenced by T082) — `src-tauri/src/platform/mod.rs`
 

**Checkpoint**: `cargo tauri dev` launches the app, `health_get` returns a valid response, DB is created on first run.

---

## Phase 3: User Story 1 — Start Tracking Time on a Task (Priority: P1) 🎯 MVP

**Goal**: User can start a timer, see it counting up, stop it, and have the entry saved and visible in the list. Basic quick-entry with description only (full fuzzy matching added in US5).

**Independent Test**: Launch app → type a description in the quick-entry bar → press Enter → confirm timer counting up → stop timer → confirm entry appears in the list with correct start/end times.

### Tests for User Story 1

> Write and confirm they FAIL before implementing

 - [x] T018 [P] [US1] Write Playwright E2E tests covering all US1 acceptance scenarios (timer start/stop, auto-stop on new timer, continue button, list grouped by date, UTC storage + local display, **and overlap-warning modal on manual entry creation**: verify warning shown when new entry overlaps an existing entry, and confirm `force: true` override proceeds to save) — `tests/e2e/specs/timer.spec.ts`
 - [x] T019 [P] [US1] Write xUnit tests for `TimerStateService` (start, stop, continue, elapsed tick, single-timer invariant) — `src/Tracey.Tests/TimerStateServiceTests.cs`

### Implementation for User Story 1

 - [x] T020 [P] [US1] Implement `timer_start` Tauri command: stop any running timer, insert new TimeEntry with `ended_at = NULL`, enqueue sync — `src-tauri/src/commands/timer.rs`
 - [x] T021 [P] [US1] Implement `timer_stop` and `timer_get_active` Tauri commands — `src-tauri/src/commands/timer.rs`
 - [x] T022 [US1] Implement `time_entry_list` Tauri command: paginated query grouped by date descending, joins project/task/tag names — `src-tauri/src/commands/timer.rs`
 - [x] T023 [US1] Implement `time_entry_create_manual` Tauri command with overlap detection warning and `force: true` override — `src-tauri/src/commands/timer.rs`
 - [x] T024 [US1] Implement `time_entry_continue` Tauri command (copies description/project/task/tags from source entry, creates new running timer) — `src-tauri/src/commands/timer.rs`
 - [x] T025 [US1] Implement `time_entry_autocomplete` Tauri command: query distinct descriptions from history, fuzzy-ranked in C#; for each result, verify that the linked `project_id` and `task_id` still exist in the DB — set `is_orphaned: true` in the result payload when either is missing (project deleted or task deleted) — `src-tauri/src/commands/timer.rs`
 - [x] T025a [P] [US1] Write Playwright E2E test for orphaned autocomplete: stop a timer with a linked project/task; hard-delete that project via the Tauri IPC from the test fixture; type the description in the quick-entry bar; verify the autocomplete suggestion appears with a visual orphan-warning indicator; select it and confirm the entry is saved with the orphaned fields flagged — `tests/e2e/specs/timer.spec.ts`
 - [x] T026 [US1] Implement `tracey://timer-tick` event emitter: background tokio task that emits elapsed seconds every second while a timer is running — `src-tauri/src/services/`
 - [x] T027 [US1] Implement `TimerStateService` in C#: subscribes to `tracey://timer-tick`, exposes reactive `ElapsedSeconds` and `ActiveTimer` state — `src/Tracey.App/Services/TimerStateService.cs`
 - [x] T028 [P] [US1] Build `QuickEntryBar.razor` component: description input, Enter to call `timer_start`, description autocomplete dropdown (historical suggestions from `time_entry_autocomplete`); display an inline warning indicator on any autocomplete suggestion where `is_orphaned: true`; selecting an orphaned suggestion shows a tooltip or inline banner stating that the linked project/task no longer exists — `src/Tracey.App/Components/QuickEntryBar.razor`
 - [x] T029 [US1] Build `TimeEntryList.razor` component: paginated list grouped by date, live running timer row with elapsed counter, Continue button per past entry, page-size from preferences; preserve scroll position across Blazor component re-renders and after page navigation (store position in a `sessionStorage` JS key; restore on mount via JS interop) — `src/Tracey.App/Components/TimeEntryList.razor`
 - [x] T029a [P] [US1] Write Playwright E2E test for scroll-position preservation: scroll partway down the time entry list, navigate to another page (e.g., Projects), return to Dashboard, and verify the scroll position is restored to the same offset — `tests/e2e/specs/timer.spec.ts`
 - [x] T030 [US1] Build `Dashboard.razor` page: assembles `QuickEntryBar` + running timer display + `TimeEntryList`; all datetimes displayed in user's configured local timezone — `src/Tracey.App/Pages/Dashboard.razor`
 - [x] T030a [US1] Implement `time_entry_update` Tauri command: accept `id`, `description`, `project_id`, `task_id`, `tag_ids`, `start_at`, `ended_at`; validate overlap (reuse overlap logic from T023); enqueue sync; return updated entry — `src-tauri/src/commands/timer.rs`
 - [x] T030b [US1] Add inline edit mode to `TimeEntryList.razor`: clicking a completed entry's row opens its fields in-place (description, project/task picker, tag picker, start/end datetime); auto-saves via `time_entry_update` on blur from any field (satisfying FR-030); shows an explicit close/cancel control to discard edits — `src/Tracey.App/Components/TimeEntryList.razor`
 - [x] T030c [P] [US1] Write Playwright E2E test for in-place editing and auto-save on blur: click a past entry to open inline edit; modify the description; press Tab to blur the field; verify the update is persisted without a manual save button; also verify blurring from the time fields triggers an auto-save — `tests/e2e/specs/timer.spec.ts`
 

**Checkpoint**: US1 acceptance tests pass. User can start, stop, and continue timers and see them in the list. MVP deliverable.

---

## Phase 4: User Story 2 — Idle Detection and On-Return Prompt (Priority: P1)

**Goal**: System detects inactivity after configurable timeout; on user return, shows a modal (Break / Meeting / Specify / Keep) only if a timer was running. Modal outcome creates or adjusts entries accordingly.

**Independent Test**: Set inactivity timeout to 5 seconds via `preferences_update` IPC. Wait without input. Move mouse. Verify modal appears with all four options and each option produces the correct outcome.

### Tests for User Story 2

 - [x] T031 [P] [US2] Write Playwright E2E tests covering all US2 acceptance scenarios (idle detection, modal appears with all options, each option outcome, no modal when no active timer) — `tests/e2e/specs/idle-detection.spec.ts`

### Implementation for User Story 2

 - [x] T032 [US2] Implement `IdleService`: background tokio loop polling `tauri-plugin-system-idle` every second; track idle start time; emit `tracey://idle-detected` event (with `idle_since` and `had_active_timer` payload) only when threshold crossed and active timer was running — `src-tauri/src/services/idle_service.rs`
 - [x] T033 [P] [US2] Implement `idle_get_status` Tauri command — `src-tauri/src/commands/idle.rs`
 - [x] T034 [US2] Implement `idle_resolve` Tauri command: handle Break (insert break TimeEntry, resume work timer), Meeting (insert pre-filled TimeEntry), Specify (insert with provided details), Keep (no-op) — `src-tauri/src/commands/idle.rs`
 - [x] T035 [US2] Build `IdleReturnModal.razor` component: overlays Dashboard on `tracey://idle-detected` event; presents Break / Meeting / Specify / Keep options; calls `idle_resolve` on selection — `src/Tracey.App/Components/IdleReturnModal.razor`
 - [x] T036 [US2] Wire idle event subscription and modal state into `Dashboard.razor` — `src/Tracey.App/Pages/Dashboard.razor`
 

**Checkpoint**: US2 acceptance tests pass. Idle prompt appears reliably within 3 seconds of user activity resuming (SC-002).

---

## Phase 5: User Story 3 — Manage Clients, Projects, and Tasks (Priority: P2)

**Goal**: User can create clients (with name, color, optional logo), create projects under clients, create tasks under projects; archive/unarchive; delete client with confirmation and cascade.

**Independent Test**: Create a client with color, add two projects, add tasks under each, archive one project, verify it disappears from the picker while the other remains.

### Tests for User Story 3

 - [x] T037 [P] [US3] Write Playwright E2E tests covering all US3 acceptance scenarios (create client/project/task, archive/unarchive, delete cascade, archived items absent from picker) — `tests/e2e/specs/projects.spec.ts`

### Implementation for User Story 3

 - [x] T038 [P] [US3] Implement client Tauri commands (`client_list`, `client_create`, `client_update`, `client_archive`, `client_unarchive`, `client_delete`) with input validation (hex color, non-empty name, logo path canonicalization) — `src-tauri/src/commands/hierarchy.rs`
 - [x] T039 [P] [US3] Implement project Tauri commands (`project_list`, `project_create`, `project_update`, `project_archive`, `project_unarchive`, `project_delete`) — `src-tauri/src/commands/hierarchy.rs`
 - [x] T040 [P] [US3] Implement task Tauri commands (`task_list`, `task_create`, `task_update`, `task_delete`) — `src-tauri/src/commands/hierarchy.rs`
 - [x] T041 [US3] Build `Projects.razor` page: client list with color swatch, create/update/archive/delete actions; collapsible project list per client; task list per project; delete-confirmation modal using BlazorBlueprint modal component — `src/Tracey.App/Pages/Projects.razor`
 

**Checkpoint**: US3 acceptance tests pass. Full client/project/task hierarchy is manageable via the UI.

---

## Phase 6: User Story 4 — Screenshot Timeline Review (Priority: P2)

**Goal**: App captures screenshots at configurable interval and on window change (debounced 2 s). User opens timeline and scrolls through chronological screenshots. Files older than retention window are automatically deleted.

**Independent Test**: Launch app, wait one interval, switch windows to trigger a capture, open Timeline page, confirm screenshots appear with correct timestamps and are scrollable.

### Tests for User Story 4

 - [x] T042 [P] [US4] Write Playwright E2E tests covering all US4 acceptance scenarios (interval capture, window-change capture, timeline scroll, nearest screenshot displayed, expired deletion; use `--features test` build with GDI test double) — `tests/e2e/specs/screenshot-timeline.spec.ts`

### Implementation for User Story 4

 - [x] T043 [US4] Add `#[cfg(feature = "test")]` GDI test double: when `test` feature is active, write a pre-canned 100×100 JPEG instead of calling Win32 APIs; used by Playwright E2E fixture — `src-tauri/src/services/screenshot_service.rs`
 - [x] T044 [US4] Implement GDI screenshot capture pipeline in `spawn_blocking`: `MonitorFromWindow` → `GetMonitorInfo` (active monitor rect) → `GetDesktopWindow` (from `Win32_UI_WindowsAndMessaging`) → `GetWindowDC` → `BitBlt` → `GetDIBits` → Triangle resize to 50% → **JPEG encode** (implementation decision: JPEG chosen for storage efficiency; the `image` crate's PNG encoder is available as a future extension without pipeline changes) → write to storage path — `src-tauri/src/services/screenshot_service.rs`
 - [x] T045 [US4] Implement storage path canonicalization and validation (prevent path traversal; default to `{exe_dir}/screenshots/`) — `src-tauri/src/services/screenshot_service.rs`
 - [x] T046 [US4] Implement interval screenshot timer (default 60 s) and window-change-triggered screenshot with 2-second debounce; emit `tracey://screenshot-captured` event after each successful save; on any IO error (disk full, folder inaccessible, permission denied), log a structured error entry (`component: "screenshot_service"`, `event: "screenshot_write_failed"`, `path`, `error`) and emit a `tracey://error` event so the in-app notification handler (see T049) can surface the failure to the user without crashing — `src-tauri/src/services/screenshot_service.rs`
 - [x] T047 [P] [US4] Implement `screenshot_list` Tauri command (queries local `screenshots` SQLite table by time range) and `screenshot_delete_expired` command (removes expired file + row pairs); both backed by the `screenshots` table defined in the migration from T009 — `src-tauri/src/commands/screenshot.rs`
 - [x] T048 [US4] Implement screenshot retention cleanup background job (delete files + records older than `screenshot_retention_days`; log failures without crashing) — `src-tauri/src/services/screenshot_service.rs`
 
- [ ] T049 [US4] Build `ScreenshotTimeline.razor` page: scrollable chronological timeline, screenshot viewer, nearest-time query via `screenshot_list`, in-app error notification when storage fails; subscribe to the `tracey://error` event emitted by T046 and surface a dismissible in-app banner (using the BlazorBlueprint alert component) whenever a screenshot write failure occurs — `src/Tracey.App/Pages/Timeline.razor`

**Checkpoint**: US4 acceptance tests pass. Screenshot timeline displays captured images at the correct times.

---

## Phase 7: User Story 5 — Keyboard-First Quick Entry with Fuzzy Matching (Priority: P2)

**Goal**: Power user creates entries entirely from keyboard using `project/task/description` or `project/description` slash notation with VS Code-style live fuzzy dropdown, arrow-key navigation, Tab/Enter confirm, and client disambiguation only when needed.

**Independent Test**: With two projects and tasks configured (from US3), type partial project name with typos → fuzzy matches surface → navigate with arrows → Tab/Enter → second segment populates → Enter starts timer. No mouse required.

**Dependency**: Requires US3 (clients/projects/tasks exist in DB) to be implemented first to have test data.

### Tests for User Story 5

 - [x] T050 [P] [US5] Write xUnit tests for `FuzzyMatchService` covering prefix match, character-spread match, case-insensitivity, typo tolerance, score ordering — `src/Tracey.Tests/FuzzyMatchServiceTests.cs`
 - [x] T051 [P] [US5] Write Playwright E2E tests covering all US5 acceptance scenarios (live dropdown, slash delimiting, two-segment vs three-segment parsing, trailing slash, arrow-key navigation, Tab/Enter confirm, client disambiguation) — `tests/e2e/specs/quick-entry.spec.ts`

### Implementation for User Story 5

 - [x] T052 [P] [US5] Implement `FuzzyMatchService` in C#: weighted scorer combining exact-prefix rank, consecutive-match rank, character-spread rank (VS Code Ctrl+P style); case-insensitive throughout — `src/Tracey.App/Services/FuzzyMatchService.cs`
 - [x] T053 [P] [US5] Implement `fuzzy_match_projects` and `fuzzy_match_tasks` Tauri commands (query from SQLite, return candidates for C#-side scoring) — `src-tauri/src/commands/timer.rs`
 - [x] T054 [US5] Extend `QuickEntryBar.razor` with slash-delimited segment parser: one slash → (project, description); two slashes → (project, task, description); trailing slash → empty description; parser does NOT infer missing segments; after fuzzy-selecting a project, query how many clients own that project name — if exactly one, silently resolve the client and skip the disambiguation dropdown entirely; only show the disambiguation dropdown (T056) when two or more clients own a project with the same name — `src/Tracey.App/Components/QuickEntryBar.razor`
 - [x] T054a [P] [US5] Write Playwright E2E test for single-client silent inference: configure two projects each with a unique name under different clients; type a project name that belongs to only one client; verify no disambiguation dropdown appears and the client is silently resolved; then configure a second project with the same name under a different client and verify the disambiguation dropdown does appear — `tests/e2e/specs/quick-entry.spec.ts`
 - [x] T055 [US5] Add live fuzzy-match dropdown to `QuickEntryBar.razor`: appears as user types each segment, sorted by score, narrows char-by-char, navigable with arrow keys, confirmed with Tab or Enter — `src/Tracey.App/Components/QuickEntryBar.razor`
 - [x] T056 [US5] Add client disambiguation inline dropdown to `QuickEntryBar.razor`: shown only when selected project name matches more than one client; arrow-key navigable; only interruption in the one-pass flow — `src/Tracey.App/Components/QuickEntryBar.razor`

**Checkpoint**: US5 acceptance tests pass. Full keyboard-driven entry flow works end-to-end with fuzzy matching in under 15 seconds (SC-001).

---

## Phase 8: User Story 6 — Tag Management and Assignment (Priority: P3)

**Goal**: User pre-creates tags in a dedicated management area and assigns them to time entries. Tags cannot be created on the fly. Deleting a tag shows a warning and only removes the tag link from entries, not the entries themselves.

**Independent Test**: Create tags in Settings → assign one to a time entry → delete the tag with confirmation → verify entry still exists without the deleted tag.

### Tests for User Story 6

 - [x] T057 [P] [US6] Write Playwright E2E tests covering all US6 acceptance scenarios (create tags, assign during entry creation/edit, delete with warning, entry survives tag deletion without the tag, autocomplete with tags) — `tests/e2e/specs/tags.spec.ts`

### Implementation for User Story 6

 - [x] T058 [P] [US6] Implement `tag_list`, `tag_create`, `tag_delete` Tauri commands (delete returns `affected_entries` count; UI shows delete-warning modal before calling) — `src-tauri/src/commands/activity.rs`
 - [x] T059 [P] [US6] Build `Tags.razor` page: tag list with create form, delete button with BlazorBlueprint confirmation modal showing affected entry count — `src/Tracey.App/Pages/Tags.razor`
 - [x] T060 [US6] Build `TagPicker.razor` reusable component: multi-select from predefined tag list; used in QuickEntryBar and manual entry creation form — `src/Tracey.App/Components/TagPicker.razor`
 - [x] T061 [US6] Wire `TagPicker.razor` into `QuickEntryBar.razor` and manual time entry form; ensure tags are passed through `timer_start`, `time_entry_create_manual`, and `time_entry_continue` commands — `src/Tracey.App/Components/`

**Checkpoint**: US6 acceptance tests pass. Tags are fully manageable and assignable without disrupting entries on deletion.

---

## Phase 9: User Story 7 — Long-Running Timer Notification (Priority: P3)

**Goal**: System notifies user when a timer exceeds configurable threshold (default 8 h) via all configured channels (Email and Telegram built-in). Adding a new channel requires only implementing one interface.

**Independent Test**: Set threshold to 5 minutes → start timer → wait → verify notification received on a configured channel (Telegram message or email received).

### Tests for User Story 7

 - [x] T062 [P] [US7] Write Playwright E2E tests covering US7 acceptance scenarios (threshold trigger, all configured channels notified; use mock channel in tests) — `tests/e2e/specs/notifications.spec.ts`
 - [x] T063 [P] [US7] Write xUnit tests for `EmailNotificationChannel` and `TelegramNotificationChannel` using mock `HttpClient` / `SmtpClient` — `src/Tracey.Tests/NotificationChannelTests.cs`

### Implementation for User Story 7

 - [x] T064 [P] [US7] Define `INotificationChannel` interface with `ChannelId`, `SendAsync(NotificationMessage, CancellationToken)`, and `DefaultSettings`; define `NotificationMessage` and `NotificationChannelSettings` types — `src/Tracey.App/Services/Notifications/INotificationChannel.cs`
 - [x] T065 [P] [US7] Implement `EmailNotificationChannel` using MailKit SMTP — `src/Tracey.App/Services/Notifications/EmailNotificationChannel.cs`
 - [x] T066 [P] [US7] Implement `TelegramNotificationChannel` using `HttpClient` (Telegram Bot API) — `src/Tracey.App/Services/Notifications/TelegramNotificationChannel.cs`
 - [x] T067 [US7] Implement `NotificationOrchestrationService`: background timer checks every minute if active timer has exceeded `timer_notification_threshold_hours`; sends via all enabled channels; emit `tracey://notification-sent` event — `src/Tracey.App/Services/NotificationOrchestrationService.cs`
 - [x] T068 [US7] Add notification channel settings sections to `Settings.razor` (email SMTP config, Telegram bot token/chat config, enable/disable per channel) — `src/Tracey.App/Pages/Settings.razor`

**Checkpoint**: US7 acceptance tests pass. Notifications fired on all configured channels when threshold crossed (SC-010 verified: adding a third channel requires no changes to Email or Telegram).

---

## Phase 10: User Story 8 — Cloud Sync and Cross-Device Visibility (Priority: P3)

**Goal**: User pastes a Postgres/Supabase connection URI in Settings. Time entries, clients, projects, tasks, tags, window activity records, and user preferences sync across devices sharing the same URI. Running timer from device A is visible on device B. Offline writes sync on reconnect. Last-write-wins conflict resolution.

**Independent Test**: Two app instances, same connection URI. Start timer on instance A. Refresh instance B. Verify timer is visible on B. Stop from B. Verify change reflected on A within 10 seconds (SC-008).

### Tests for User Story 8

- [x] T069 [P] [US8] Write Playwright E2E tests covering all US8 acceptance scenarios (cross-instance timer visibility, offline queue sync-on-reconnect, conflict resolution, window activity synced, screenshots not synced) — `tests/e2e/specs/cloud-sync.spec.ts`

### Implementation for User Story 8

- [x] T070 [US8] Implement `sync_configure` Tauri command: validate connection URI, store securely in OS keychain via `keyring` crate (never in SQLite), set `external_db_enabled = true` in preferences — `src-tauri/src/commands/sync.rs`
- [x] T071 [US8] Implement external DB schema migration runner: connect to Postgres, apply all DDL migrations from `contracts/sync-api.md` (versioned via `schema_migrations` table) — `src-tauri/src/services/sync_service.rs`
- [x] T072 [US8] Implement `SyncService` background loop: process `sync_queue` in upsert batches (50 entries / 500 window records per cycle), 30-second interval, immediate trigger on local write; use `ON CONFLICT ... DO UPDATE` with `modified_at` GREATEST for last-write-wins — `src-tauri/src/services/sync_service.rs`
- [x] T073 [US8] Implement offline resilience: queue all writes to `sync_queue` when external DB unreachable; auto-replay on reconnect; preserve queue across app restarts (persisted in SQLite) — `src-tauri/src/services/sync_service.rs`
- [x] T074 [P] [US8] Implement `sync_get_status` and `sync_trigger` Tauri commands; emit `tracey://sync-status-changed` event on state changes — `src-tauri/src/commands/sync.rs`
- [x] T075 [US8] Add sync connection settings section to `Settings.razor` (URI input masked, connect button, sync status indicator, last sync time) — `src/Tracey.App/Pages/Settings.razor`

**Checkpoint**: US8 acceptance tests pass. Timer visible cross-device within 10 s (SC-008). Offline entries sync within 60 s of reconnect (SC-004).

---

## Phase 11: User Story 9 — Run as Portable Application Without Admin Rights (Priority: P3)

**Goal**: Single `.exe` file, placed anywhere, run by a standard user without installation, registry access, or elevation. All configuration and data files created in user-writable locations on first launch.

**Independent Test**: Copy executable to a temp folder on a standard user account. Run it. Complete a timer start/stop. Verify no installation prompt, no registry writes, no admin UAC prompt.

### Tests for User Story 9

- [x] T076 [P] [US9] Write Playwright E2E tests verifying portable execution: app starts from arbitrary path, completes full timer cycle without elevation, data files created in exe directory — `tests/e2e/specs/portable.spec.ts`

### Implementation for User Story 9

- [x] T077 [US9] Configure `tauri.conf.json` bundle settings to produce a single portable `.exe` with no installer (no NSIS/MSI), no registry writes, all data paths relative to `{exe_dir}` — `src-tauri/tauri.conf.json`
- [x] T078 [US9] Implement and unit-test portable path resolution logic: write Rust unit tests in `tests/portable_path.rs` covering (a) `{exe_dir}` primary path is used when that directory is writable, (b) fallback to `{APPDATA}/tracey/` when `{exe_dir}` is read-only (simulate with a tempdir set read-only), (c) first-launch directory creation (`tracey.db` parent and `screenshots/`) succeeds without elevation; all tests runnable with `cargo test` using temporary directory fixtures — `src-tauri/src/`, `src-tauri/tests/portable_path.rs`
- [x] T079 [P] [US9] Add CI job: `cargo tauri build` (release), verify output is a single `.exe`, run the executable as a restricted user in a GitHub Actions Windows runner — `.github/workflows/ci.yml`

**Checkpoint**: US9 acceptance tests pass. Portable executable confirmed working on a standard user account (SC-005).

---

## Final Phase: Polish & Cross-Cutting Concerns

**Purpose**: Hardening, settings completion, performance, CI pipeline.

- [ ] T080 [P] Build core `Settings.razor` page: timezone picker (IANA list), inactivity timeout, screenshot interval, screenshot retention days, screenshot storage folder (path picker), page size — `src/Tracey.App/Pages/Settings.razor`
- [ ] T081 [P] Implement delete-all-data operation in Settings: wipe all local SQLite tables, delete all screenshot files, complete within 60 seconds, show confirmation modal and post-deletion acknowledgement — `src/Tracey.App/Pages/Settings.razor`, `src-tauri/src/commands/`
- [ ] T082 [P] Implement `PlatformHooks` trait (defined in T017b) for Windows: `GetForegroundWindow` → `GetWindowThreadProcessId` → `GetModuleFileNameExW` polling loop (1-second interval), HWND null-check via `std::ptr::null_mut()` not `== 0`, deny-list redaction of window titles before storage, call `ScreenshotService::trigger_on_window_change()` directly on window change (note: `tracey://screenshot-captured` is emitted by `ScreenshotService` after a successful save, not by the activity tracker). **Terminology note**: “active window” throughout the spec is synonymous with “foreground window” in Win32 terminology — `GetForegroundWindow` is the correct Win32 API for this concept and the two terms are used interchangeably in this codebase. — `src-tauri/src/platform/windows.rs`, `src-tauri/src/services/activity_tracker.rs`
- [ ] T083 [P] Implement window activity sync: batch-write `WindowActivityRecord` rows to SQLite; flush unsynced records every **30 s** (satisfying SC-007) to external DB via `SyncService`; enforce process deny-list at collection boundary (Constitution V) — `src-tauri/src/services/activity_tracker.rs`
- [ ] T084 [P] Add performance benchmark tests: timer start/stop < 50 ms, time_entry_list at 1 M rows < 500 ms p95, window activity write throughput; fail CI if > 10% regression — `tests/`
- [ ] T085 [P] Configure CI pipeline: lint (cargo clippy -D warnings, dotnet format verify), unit tests (cargo test, dotnet test), E2E tests (playwright test), cargo audit (CVE scan), performance benchmarks — `.github/workflows/ci.yml`
- [ ] T086 Run quickstart.md validation: follow document from scratch on a clean machine, confirm all steps produce a working app, update any incorrect instructions — `specs/001-window-activity-tracker/quickstart.md`
- [ ] T087 [P] Build deny-list editor section in `Settings.razor`: text input for a per-process name pattern (glob or exact string), add/remove rows in a dynamic list, persist the full list to `user_preferences.process_deny_list_json` via `preferences_update` IPC; the saved list is applied at the collection boundary in T082 before any window title or process name is written to storage — `src/Tracey.App/Pages/Settings.razor`
- [ ] T088 [P] Automated accessibility audit: integrate `@axe-core/playwright`; write Playwright tests that assert zero WCAG 2.1 AA violations on all pages (Dashboard, Projects, Tags, Timeline, Settings); additionally validate that keyboard-only navigation (Tab, Shift+Tab, Enter, Escape, arrow keys) can reach and activate every interactive element on each page — `tests/e2e/specs/accessibility.spec.ts`
- [ ] T089 [P] Tone audit pass: after all .razor pages are complete (post-T030 through T081), review every user-facing literal string in `.razor` files against the tone guide from T005; correct any deviations (avoid jargon, keep copy concise and action-oriented); run concurrently with T086 — `docs/ux/tone.md`, `src/Tracey.App/`

**Checkpoint**: All tests green in CI. Performance budgets met. Portable build confirmed. quickstart.md validated.

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup                   → No dependencies. Start immediately.
Phase 2: Foundational            → Depends on Phase 1. BLOCKS all user stories.
Phase 3: US1 (P1)               → Depends on Phase 2.
Phase 4: US2 (P1)               → Depends on Phase 2. Can run parallel with US1.
Phase 5: US3 (P2)               → Depends on Phase 2. Can run parallel with US1/US2.
Phase 6: US4 (P2)               → Depends on Phase 2. Can run parallel with US1/US2/US3.
Phase 7: US5 (P2)               → Depends on Phase 2 AND US3 (needs projects/tasks in DB for test data).
Phase 8: US6 (P3)               → Depends on Phase 2. Integrates with US1 UI (timer commands already accept tag_ids).
Phase 9: US7 (P3)               → Depends on Phase 2. Fully independent.
Phase 10: US8 (P3)              → Depends on Phase 2. Fully independent.
Phase 11: US9 (P3)              → Can run alongside any phase; mostly configuration.
Final Phase: Polish             → Depends on all user stories being complete.
```

### User Story Dependencies

| Story | Depends On | Reason |
|-------|-----------|--------|
| US1 | Phase 2 | Core timer and DB infrastructure |
| US2 | Phase 2 | Idle service is independent of timer UI |
| US3 | Phase 2 | Client/project/task commands only need DB layer |
| US4 | Phase 2 | Screenshot service is independent |
| US5 | Phase 2 + US3 | Fuzzy match needs projects/tasks to exist for E2E test data |
| US6 | Phase 2 | Tag commands only need DB layer; tag picker integrates with US1 entry form |
| US7 | Phase 2 | Notification channel interface is fully independent |
| US8 | Phase 2 | Sync service is independent; adds settings section alongside US7 |
| US9 | Phase 1 + 2 | Portable config is set at scaffold time; verified as final step |

---

## Parallel Example: Phase 2 (Foundational)

```
# These can run in parallel across developers (7 tasks — Phase 2 total: 13):
Task T010:  Create Rust model structs         (src-tauri/src/models/mod.rs)
Task T011:  Implement structured logger       (src-tauri/src/)
Task T013:  preferences_get/update commands   (src-tauri/src/commands/)
Task T014:  health_get command                (src-tauri/src/commands/)
Task T016:  Register Blazor DI + events       (src/Tracey.App/Program.cs)
Task T017:  App.razor navigation shell        (src/Tracey.App/App.razor)
Task T017b: PlatformHooks trait definition    (src-tauri/src/platform/mod.rs)

# These must run sequentially:
T006 (app shell) → T007 (capabilities) → T008 (DB open) → T009 (migrations) → T012 (first-launch init) → T015 (TauriIpcService)
```

## Parallel Example: User Story 1

```
# Tests first (must FAIL before implementation proceeds):
Task T018: Playwright E2E timer tests        (tests/e2e/specs/timer.spec.ts)
Task T019: xUnit TimerStateService tests     (src/Tracey.Tests/)

# These can run in parallel once tests are written:
Task T020: timer_start command               (src-tauri/src/commands/timer.rs)
Task T021: timer_stop + timer_get_active     (src-tauri/src/commands/timer.rs)

# Then sequentially:
T022 (time_entry_list) → T023 (create_manual) → T024 (continue) → T025 (autocomplete)
T026 (timer-tick emitter) → T027 (TimerStateService)
T028 (QuickEntryBar) → T029 (TimeEntryList) → T030 (Dashboard)
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (**CRITICAL — blocks everything**)
3. Complete Phase 3: US1 — Start Tracking Time
4. **STOP and VALIDATE**: all US1 Playwright tests pass, user can start/stop timers
5. Complete Phase 4: US2 — Idle Detection
6. **STOP and VALIDATE**: idle-return modal works, prompt appears within 3 s (SC-002)
7. Deploy/demo the P1 MVP

### Incremental Delivery

| Sprint | Delivers | SC Verified |
|--------|---------|-------------|
| 1 | Setup + Foundational | — |
| 2 | US1 (Timer + Quick Entry) | SC-001, SC-006 |
| 3 | US2 (Idle Detection) | SC-002 |
| 4 | US3 (Client/Project/Task) + US4 (Screenshots) | SC-003, SC-009 |
| 5 | US5 (Fuzzy Quick Entry) | SC-001 hardened |
| 6 | US6 (Tags) + US7 (Notifications) | SC-010 |
| 7 | US8 (Cloud Sync) | SC-004, SC-007, SC-008 |
| 8 | US9 (Portability) + Polish | SC-005 |

---

## Summary

| Metric | Count |
|--------|-------|
| Total tasks | 86 |
| Setup (Phase 1) | 5 |
| Foundational (Phase 2) | 12 |
| US1 — Timer (P1) | 13 |
| US2 — Idle Detection (P1) | 6 |
| US3 — Client/Project/Task (P2) | 5 |
| US4 — Screenshot Timeline (P2) | 8 |
| US5 — Fuzzy Quick Entry (P2) | 7 |
| US6 — Tags (P3) | 5 |
| US7 — Notifications (P3) | 7 |
| US8 — Cloud Sync (P3) | 6 |
| US9 — Portability (P3) | 4 |
| Polish | 7 |
| Parallelizable tasks [P] | 42 |
| Test tasks (Playwright E2E) | 9 spec files |
| Test tasks (xUnit) | 3 test files |
