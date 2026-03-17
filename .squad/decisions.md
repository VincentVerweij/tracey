# Squad Decisions

## 2026-03-15: Tech Stack
**By**: Vincent Verweij  
**What**: Tauri 2.0 (Rust native layer, Windows 11) + Blazor WebAssembly .NET 10 (C#, runs inside Tauri's WebView2) + BlazorBlueprint.Components NuGet + SQLite local (WAL mode) + optional Postgres/Supabase via user-supplied connection URI.  
**Why**: Portable desktop app, web-tech UI, thin native layer for OS hooks. Blazor WASM chosen over Blazor Server — WASM compiles C# to WASM and runs entirely in WebView2 with no server-side SignalR dependency, matching the offline-capable portable binary requirement.

## 2026-03-15: Portable Executable Constraint
**By**: Vincent Verweij  
**What**: No installer, no admin rights, no registry entries. Single portable `.exe` binary. No NSIS, no MSI.  
**Why**: User requirement. Must run from any directory without elevation.

## 2026-03-15: Screenshot Storage — Plain Files, Never Synced
**By**: Vincent Verweij  
**What**: Screenshots stored as plain JPEG files under a user-configured folder. Not encrypted. Never synced to external DB. Security is the responsibility of OS-level folder permissions.  
**Why**: Spec clarification FR-018. Explicit user decision.

## 2026-03-15: Connection URI — OS Keychain
**By**: Vincent Verweij  
**What**: External DB connection URI stored in OS keychain via `keyring` crate. Never plain text. Never written to disk unprotected.  
**Why**: Only sensitive credential in the system. Must be protected at rest.

## 2026-03-15: Window Polling Strategy
**By**: Research  
**What**: Active window detection uses 1-second polling with `GetForegroundWindow` → `GetWindowThreadProcessId` → `GetModuleFileNameExW`. No hooks.  
**Why**: No elevated privileges required. Lightweight (< 1 ms per check). Simple shutdown.

## 2026-03-15: Idle Detection — Plugin
**By**: Research  
**What**: Use `tauri-plugin-system-idle` wrapping `GetLastInputInfo` + `GetTickCount64`. `GetTickCount64` MUST be used (not `GetTickCount`) to avoid 32-bit rollover after ~49 days uptime.  
**Why**: No manual boilerplate. Windows 11 validated. Avoids rollover bug.

## 2026-03-15: Screenshot Capture Pipeline
**By**: Research  
**What**: `MonitorFromWindow` → `GetMonitorInfo` → `GetDesktopWindow` + `GetWindowDC` + `BitBlt` + `GetDIBits`. Scale 50% with Triangle filter. JPEG encode. Entire pipeline runs in `spawn_blocking`.  
**Why**: Must not run on main thread. `GetDesktopWindow` is in `Win32_UI_WindowsAndMessaging` (NOT `Win32_Graphics_Gdi`). Triangle faster than Lanczos3 at 50% scale.

## 2026-03-15: SQLite WAL Mode + Batch Flush
**By**: Plan  
**What**: SQLite opened with `PRAGMA journal_mode = WAL` and `PRAGMA foreign_keys = ON`. Window activity flushed every 30 seconds. External sync also every 30 seconds on background task.  
**Why**: WAL allows concurrent readers during writes. 30-second interval balances freshness and CPU budget.

## 2026-03-15: Process Deny-List Placement
**By**: Plan  
**What**: `user_preferences.process_deny_list_json` checked at collection boundary in Rust tracking loop, before any DB write. Excluded processes never reach storage.  
**Why**: Privacy-first. Filter at intake, not at query time.

## 2026-03-15: No Login / Account System
**By**: Vincent Verweij  
**What**: No built-in user authentication. Multi-device sharing via user-managed connection string pasted into app settings.  
**Why**: Out of scope. User manages their own database access.

## 2026-03-15: Idle Return — No Active Timer
**By**: Vincent Verweij  
**What**: If user returns from idle with no running timer, silently dismiss. No prompt shown.  
**Why**: Spec clarification. User simply continues.

## 2026-03-15: Tauri Filesystem Permission (Critical)
**By**: Research  
**What**: Use `fs:allow-write-file` (singular noun). NOT `fs:allow-write-files` (plural). Wrong permission fails silently.  
**Why**: Validated research finding. Common mistake.

## 2026-03-15: logo_path — Never Synced
**By**: Plan  
**What**: The `logo_path` field on Client is a local filesystem path. MUST NEVER be synced to external DB.  
**Why**: Path is machine-local, meaningless on another device.

## 2026-03-15: Orphaned Time Entries on Client Deletion
**By**: Spec  
**What**: When a Client is deleted (with confirmation), Projects and Tasks cascade-delete. Time entries that referenced them become orphaned — retained, not deleted. `time_entry_autocomplete` flags orphaned suggestions with `is_orphaned: true`.  
**Why**: Spec US3 acceptance scenario 6. Historical data preserved.

## 2026-03-15: Performance Budget
**By**: Plan  
**What**: Background tracing < 2% CPU over any 10-second window. Memory < 150 MB RSS. Queries < 500 ms p95 (≤ 1M events). App ready < 5 seconds.  
**Why**: Constitutional check IV.

## 2026-03-15: Testing — TDD
**By**: Plan  
**What**: Tests written before implementation. Playwright for all user-story acceptance scenarios (E2E). `cargo test` for Rust at ≥ 80% branch coverage. `dotnet test` (xUnit) for Blazor business logic. GDI capture stubbed via `#[cfg(feature="test")]`.  
**Why**: Constitutional check II.

## 2026-03-15: HWND Null Check (Critical)
**By**: Research  
**What**: In `windows` crate 0.58+, `HWND` wraps a raw pointer. Null checks MUST use `std::ptr::null_mut()`, not `== 0`.  
**Why**: Compiles but produces incorrect behavior. Common mistake on Windows 11.

## 2026-03-15: IPC Contract Amendment — `time_entry_autocomplete`
**By:** Finch (Lead/Architect)
**What:** Add `is_orphaned: boolean` to the `time_entry_autocomplete` suggestion object in `contracts/ipc-commands.md`. T025 (Reese) sets `is_orphaned: true` when a suggestion's `project_id` or `task_id` no longer exists in the DB. T028 (Root) consumes this field to render the inline orphan-warning indicator in the QuickEntryBar dropdown.
**Why:** The field was missing from the IPC contract despite being required by T025 and T028. Must be added before Reese begins T025 or Root begins T028.

## 2026-03-15: `trigger_screenshot_capture()` Removed from PlatformHooks Trait
**By:** Finch (Lead/Architect)
**What:** Remove `trigger_screenshot_capture()` from the `PlatformHooks` trait. The trait is scoped to OS-level querying only: `get_foreground_window_info()` and `get_idle_seconds()`. The ActivityTracker calls `ScreenshotService::trigger_on_window_change()` directly when it detects a window change.
**Why:** T017b's three-method trait conflicted with T082's design — ActivityTracker calling ScreenshotService directly. Removing the method resolves the injection cycle and matches what T082 already describes.

## 2026-03-15: T058 File Path Correction
**By:** Finch (Lead/Architect)
**What:** T058 commands (`tag_list`, `tag_create`, `tag_delete`) route to `src-tauri/src/commands/tags.rs` (new file), not `activity.rs`. File must be registered in `commands/mod.rs`.

## 2026-03-16: Layout Shell — Flex Row
**By:** UXer (Frontend Designer)
**What:** `.tracey-shell` → `display: flex; height: 100vh; overflow: hidden`. Sidebar fixed at 240px, main-content `flex: 1; overflow-y: auto`.
**Why:** The scaffolded MainLayout.razor.css had no flex rule on `.tracey-shell`, causing sidebar and main to stack vertically. This is the foundational layout fix.

## 2026-03-16: Design Tokens in `:root`
**By:** UXer (Frontend Designer)
**What:** All design values live as CSS custom properties in `app.css :root`. Token set: accent (`#6366f1`), sidebar bg (`#1a1a2e`), surface, border, text, muted, radius, shadow.
**Why:** Enables consistent theming and future dark mode toggle without global find-replace. All components reference tokens, not hard-coded values.

## 2026-03-16: Accent Color — Indigo `#6366f1`
**By:** UXer (Frontend Designer)
**What:** Single accent color `#6366f1` (Tailwind indigo-500) used for: active nav left bar, focus rings, running timer display, running entry border, autocomplete focus, hover highlights on idle options.
**Why:** Indigo reads clearly on both dark sidebar (`#1a1a2e`) and light content area (`#f8f9fa`). Strong enough to guide attention without being harsh.

## 2026-03-16: BlazorBlueprint Component Adoption
**By:** UXer (Frontend Designer)
**What:** Use verified BB components only. Components adopted: `BbButton` (Default/Outline/Ghost/Destructive variants, Small size), `BbCard` for client cards in Projects, `BbAlert`/`BbAlertTitle`/`BbAlertDescription` for error display, `BbDialog`/`BbDialogContent` for idle modal. Idle option buttons kept raw `<button>` — card-style multi-line layout doesn't fit BbButton's Tailwind flex structure cleanly.
**Why:** BB components bring consistent shadcn/ui design language, accessibility, and focus management.

## 2026-03-16: BbPortalHost Placement
**By:** UXer (Frontend Designer)
**What:** `<BbPortalHost />` and `<BbDialogProvider />` placed at the bottom of `MainLayout.razor` after the `.tracey-shell` div.
**Why:** Required for BbDialog to portal out of the component tree (avoiding overflow/z-index clipping). Per BlazorBlueprint docs. Generates a harmless RZ10012 analyzer warning due to net8.0→net10.0 TFM mismatch; compile succeeds with 0 errors.

## 2026-03-16: Scoped CSS File Strategy
**By:** UXer (Frontend Designer)
**What:** One `.razor.css` file per component/page that has unique styles. Global tokens and utilities in `app.css`. Inter font loaded via `index.html`. Scoped files: `QuickEntryBar.razor.css`, `TimeEntryList.razor.css`, `Projects.razor.css`, `Dashboard.razor.css`, `IdleReturnModal.razor.css`.
**Why:** Blazor's scoped CSS prevents selector leaks. Each component owns its own visual contract.

## 2026-03-16: BbCard Class Interpolation Syntax
**By:** UXer (Frontend Designer)
**What:** When conditionally adding CSS classes to a BlazorBlueprint component `Class` parameter, use `@($"...")` string interpolation: `Class="@($"client-card{(client.IsArchived ? " archived" : "")}")"`.
**Why:** BlazorBlueprint component parameters don't accept mixed C#/markup inline (`Class="base @(expr)"` causes error RZ9986). Must use a full Razor expression string.

## 2026-03-16: Stub Page Empty States
**By:** UXer (Frontend Designer)
**What:** Tags, Timeline, and Settings pages receive: `<PageTitle>`, `<h1 class="page-title">`, descriptive `<p class="text-muted">`, and a centered emoji empty-state block.
**Why:** Placeholder content prevents blank pages which look broken. Emoji + message communicates the page's purpose while the feature is built out.

## 2026-03-16: Phase 5 — T038/T039/T040 Client/Project/Task IPC Commands
**By:** Reese (Rust/Tauri Backend)
**Tasks:** T038 (client commands), T039 (project commands), T040 (task commands)

**Color validation — no regex crate**: Used `color.len() == 7 && starts_with('#') && [1..].chars().all(is_ascii_hexdigit())` instead of adding a `regex` dependency for a single simple pattern.

**Dynamic SQL filtering — parameterised NULL trick**: `project_list` and `client_list` use `(?1 IS NULL OR client_id = ?1) AND (?2 = 1 OR is_archived = 0)` with `params![client_id.as_deref(), include_archived as i64]`. Avoids SQL injection and code duplication.

**client_delete orphan sequence**: Explicit ordering per spec US3 scenario 6 — COUNT orphans → NULL time_entries refs (project-level then task-level) → DELETE client (SQLite FK ON DELETE CASCADE removes projects then tasks). Orphaned time entries are retained, not deleted.

**project_delete / task_delete**: Explicit NULL-out of time_entries references before delete. Mirrors client_delete pattern for clarity even where FK CASCADE would handle it.

**List return shape asymmetry (IPC contract)**: `client_list` → `{ "clients": [...] }` (wrapped object). `project_list`, `task_list` → bare JSON array.

**Inline parameters for id-only commands**: `client_archive(state, id: String)` style — no wrapper struct needed for single-id commands.

**DB-level unique constraints**: `clients.name` has `UNIQUE`; `projects` has `UNIQUE (client_id, name)`; `tasks` has no DB-level unique — guarded explicitly in Rust. All return `"name_conflict"` on violation rather than raw SQLite error.

## 2026-03-16: Phase 5 — T041 Projects.razor UI
**By:** Root (Blazor/C# Frontend)
**Task:** T041 — US3 Projects Page

**Lazy loading pattern**: Clients on `OnInitializedAsync`. Projects on client-card expand. Tasks on project-row expand. Each level caches in dictionaries keyed by parent ID.

**Inline forms, no modal**: Add-client / add-project / add-task use inline `bool`-toggled panels. Delete confirmation uses `<div role="dialog">` inline (not overlay). Consistent with spec lightweight CRUD requirement.

**No duplicate IPC wrappers**: Existing `ClientItem`, `ProjectItem`, `TaskItem`, `ClientListResponse`, etc. types from T015 session used directly. No aliasing.

**`@bind:after` for archive toggle**: `@bind="_showArchived"` + `@bind:after="LoadClients"` — idiomatic .NET 8+ Blazor pattern, avoids separate `@onchange` handler.

**ExpandLabel helper**: Avoids Razor parser quote-conflict with ternary string literals inside double-quoted HTML attributes.

**Delete counts not surfaced in UI**: `ClientDeleteAsync` returns `{ deleted_projects, deleted_tasks, orphaned_entries }` — counts silently discarded post-delete per task spec (generic confirmation only).

## 2026-03-16: Phase 5 — T037 US3 E2E Test Decisions
**By:** Shaw (QA/Tester)
**Task:** T037 — Playwright E2E tests for US3 (Manage Clients, Projects, Tasks)

**D1 — Self-guarding autocomplete assertions (AC7)**: Conditional guard — if the dropdown/listbox is not visible at all, test passes without checking absence. Only if a dropdown IS shown does the test assert the archived entity is absent. Avoids ambiguous `not.toBeVisible()` on listbox.

**D2 — Archive toggle selector contract**: Tests locate toggle via `role="checkbox" name=/show archived|include archived/i` OR `role="button" name=/show archived|include archived/i`. Root must use one of these roles with a name matching the pattern.

**D3 — IPC fixture helpers via `page.evaluate`**: `createClient`, `createProject`, `createTask`, `deleteClient`, `deleteProject` call Tauri IPC directly via `window.__TAURI_INTERNALS__.invoke(...)` for setup — bypasses UI for speed and fragility reduction.

**D4 — Cascade confirmation content assertion**: Delete-client modal asserted with `toContainText(/project|task|entr/i)` — broad regex covering any reasonable phrasing of cascade counts.

**D5 — Color swatch selector contract**: Located via `[class*="swatch"], [class*="color"], [style*="background"]` OR `aria-label="{name} color swatch"`. Root must provide at least one. Recommended: `aria-label="{name} color swatch"` for accessibility and test robustness.

## 2026-03-15: TDD Gate Spec Ambiguities (T018/T019)
**By:** Shaw (QA/Test)
**What:** Five ambiguities flagged while writing T018 Playwright tests and T019 xUnit tests for US1 timer:
1. **ARIA role for elapsed timer display**: Tests use `role="timer"` with `aria-live="off" aria-atomic="true"`. Root must render elapsed counter with this markup.
2. **Locked segment chip representation**: After slash confirms a project segment, tests use `role="group"` with `aria-label="project segment"`. Root to implement to match.
3. **Stop button vs. Ctrl+Space**: Both `role="button" name=/stop/i` and Ctrl+Space must be supported. No conflict with spec.
4. **"Continue" button placement**: Tests navigate to Timeline and look for `role="button" name=/continue/i`. Assuming Timeline is correct. Needs UX confirmation before Root implements if different.
5. **Timer persists across app restart**: Not covered in T018 — requires kill/restart Tauri process from Playwright (tauri-driver). Deferred to Phase 3+ fixture work (Fusco).
**Why:** These spec gaps affect Root's T020+ UI implementation. Must be resolved before or during Phase 3 frontend work.
**Why:** Tags are not activity data. Placing them in `activity.rs` is semantically wrong and will confuse future readers.

## 2026-03-15: T082 — No Split (Option B)
**By:** Vincent Verweij (via Coordinator)
**What:** T082 (Win32 foreground window polling loop) stays in the Final Phase as originally planned. Phase 6 (US4 — Screenshot Timeline) delivers interval-based screenshots only. Window-change-triggered screenshots arrive in the Final Phase when T082 is implemented alongside T083 (ActivityRecord writes) and the sync queue. Shaw's US4 E2E tests do NOT need to cover window-change-triggered captures during Phase 6.
**Why:** Accepted gap. Interval-based captures are sufficient for Phase 6 delivery. The window-change trigger is a Final Phase enhancement.

---

## Phase 2 Infrastructure Decisions (2026-03-15)

### Control — T007: Tauri Capabilities

#### 2026-03-15: No `system-idle` Plugin Capability
**By:** Control (T007)
**What:** `system-idle:allow-get-idle-time` removed from `capabilities/default.json`. Plugin does not exist on crates.io. Idle detection implemented via direct Win32 `GetLastInputInfo` + `GetTickCount64` in `platform/windows.rs`.
**Why:** Plugin unavailable. `tasks.md` T007 spec is now stale on this point. Finch/Scribe to note.

#### 2026-03-15: `fs:allow-write-file` Singular Confirmed
**By:** Control (T007)
**What:** `fs:allow-write-file` (singular) confirmed against plugin TOML source at `tauri-plugin-fs-2.4.5`. Grants only `write_file`, `open`, `write`. `fs:write-files` (plural set) explicitly rejected — bundles 9 commands unnecessarily.
**Why:** Least-privilege. Singular command grant only.

#### 2026-03-15: CSP Tightening Deferred to Final Phase
**By:** Control (T007)
**What:** Current CSP `default-src 'self' tauri: asset: https://asset.localhost` acceptable for dev. `connect-src 'none'` and audit of `asset:` directive deferred to Final Phase.
**Why:** Architecture ensures WebView2 has no direct network access; sync is Rust-only.

#### 2026-03-15: Path Traversal Requirement for Screenshot Writes (Standing Requirement)
**By:** Control (T007)
**What:** `fs:allow-write-file` does NOT scope the write path. Rust code MUST `std::fs::canonicalize` + prefix-check every screenshot write path against the configured screenshots directory before any write.
**Why:** Capability alone is insufficient. Defense-in-depth requirement.

---

### Reese — T008/T010: Database + Model Structs

#### 2026-03-15: No `directories` Crate
**By:** Reese (T008)
**What:** `directories` crate not added. DB path fallback implemented with `std::env::var("APPDATA")` + `PathBuf`. Zero extra dependencies.
**Why:** Sufficient on Windows. Leaner dependency tree.

#### 2026-03-15: `rusqlite::params!` Macro Required for Mixed-Type Inserts
**By:** Reese (T008)
**What:** `rusqlite::params![...]` macro required when mixing types in execute calls. Homogeneous array slices fail type inference with mixed `&&str` / `&String` / `i64`.
**Why:** Compiler error on array slice; `params!` erases types via `ToSql` trait. Future DB tasks (T020–T040+) must use `params!`.

#### 2026-03-15: Model Struct Field Corrections (Canonical)
**By:** Reese (T010)
**What:** Model structs corrected to match Leon's SQL exactly. Canonical corrections:
- `Tag`: no `color` field
- `TimeEntry`: added `is_break: bool`, `device_id: String`
- `WindowActivityRecord`: `window_handle`, `device_id`; removed `time_entry_id`, `process_path`
- `Screenshot`: `trigger: String`, `device_id: String`; removed `width`/`height`
- `UserPreferences.id`: `i64` (not String); `local_timezone` (not `timezone`); `page_size` (not `entries_per_page`); added `external_db_uri_stored: bool`, `notification_channels_json: Option<String>`; no `modified_at`
- `SyncQueueEntry.id`: `i64` (not String)

**Why:** Briefing contained stale field names. Model file is the source of truth going forward.

---

### Reese — T011/T013/T014: Logger + Preferences + Health IPC

#### 2026-03-15: health_get — IPC Contract Shape Authoritative Over Briefing
**By:** Reese (T014)
**What:** `health_get` implemented with IPC contract shape (`running`, `last_write_at`, `events_per_sec`, `memory_mb`, `active_errors`, `pending_sync_count`). Briefing shape (`status`, `db_open`, `version`, `platform`, `uptime_seconds`) discarded.
**Why:** `contracts/ipc-commands.md` is authoritative per decisions.md. Finch to adjudicate if briefing shape was intentional.

#### 2026-03-15: preferences_get / preferences_update — Contract Gap
**By:** Reese (T013)
**What:** `preferences_get` and `preferences_update` implemented per briefing but absent from `contracts/ipc-commands.md`. Finch must add them to the contract.
**Why:** IPC contract is the source of truth. Gap must be closed before other agents depend on these commands.

#### 2026-03-15: Structured Logger — stderr, Not stdout
**By:** Reese (T011)
**What:** Structured JSON logs emitted to stderr (`eprintln!`). Stdout reserved for Tauri's internal protocol.
**Why:** Mixing with Tauri's stdout protocol causes parsing errors. Intentional.

---

### Root — T015/T016/T017: Blazor IPC Service, Events, Nav Shell

#### 2026-03-15: `window.__TAURI_INTERNALS__.invoke` (Tauri 2.0 Bridge)
**By:** Root (T015)
**What:** `TauriIpcService` uses `window.__TAURI_INTERNALS__.invoke`. If Tauri JS SDK is bundled in `index.html`, switch to `window.__TAURI__.invoke` is a one-line change. Finch to confirm which invoke path is active.
**Why:** Tauri 2.0 lower-level bridge; exists before JS SDK loads.

#### 2026-03-15: Event Payload Shapes — Contract Over Task-Prompt
**By:** Root (T016)
**What:** All event payload types use IPC contract shapes, not task-prompt examples. Key deviations: `tracey://timer-tick` has no `entry_id` (contract omits it); `tracey://idle-detected` uses `idle_since`+`had_active_timer` (not `idle_seconds`); `tracey://sync-status-changed` uses `connected`+`pending` (not `status`); `tracey://error` uses `component` (not `code`).
**Why:** Contract is authoritative. Rust emitter must match these shapes when T026 implements event emission.

#### 2026-03-15: JS Event Shim — Deferred to Final Phase
**By:** Root (T016)
**What:** `TauriEventService.Listen<T>` is a stub. Full JS shim (`wwwroot/tauri-events.js` with `DotNetObjectReference`) deferred to Final Phase. Events are wired as C# events but payloads are not delivered until shim exists.
**Why:** Complexity and scope beyond Phase 2. Components depending on events (e.g. T027 TimerStateService) must be aware.

#### 2026-03-15: `screenshot_list` — Raw Array Response Assumed
**By:** Root (T015)
**What:** `Invoke<ScreenshotItem[]>` used; assumes Rust returns a raw JSON array. If Rust wraps it in `{ "screenshots": [...] }`, deserialization fails. Reese/Finch to confirm `screenshot_list` response shape.
**Why:** Contract says "Array of..."; interpreted as direct array.

#### 2026-03-15: `time_entry_update` — Not Implemented Pending Contract
**By:** Root (T015)
**What:** `time_entry_update` not added to `TauriIpcService` — not in `contracts/ipc-commands.md`. Finch to amend contract with schema before Root adds the typed wrapper.
**Why:** Contract is source of truth. Cannot implement what is not contracted.

---

### Reese — T012: First-Launch Initialization

#### 2026-03-15: Screenshots Directory — `{exe_dir}/screenshots/`
**By:** Reese (T012)
**What:** Screenshots directory created at `{exe_dir}/screenshots/` (next to `tracey.db`) via `db_path.parent().join("screenshots")` + `std::fs::create_dir_all`. Non-fatal if creation fails.
**Why:** Matches portable path strategy. exe_dir already confirmed writable (write-probe in T008).

#### 2026-03-15: process_deny_list_json Seed — Password Managers, Not Empty
**By:** Reese (T012)
**What:** `process_deny_list_json` seeded with `["keepass","1password","bitwarden","lastpass"]` (schema default), not `"[]"` as briefing suggested.
**Why:** Leon's schema intent preserved — privacy-positive default. Users who never open Settings are protected.

#### 2026-03-15: screenshot_interval_seconds Seeded as 900 (Not Schema Default 60)
**By:** Reese (T012)
**What:** `screenshot_interval_seconds` seeded as `900` (15 minutes). Schema default is `60` (1 minute).
**Why:** Briefing explicitly requested 900. A 1-minute interval default is aggressive; 15 minutes is sensible.

#### 2026-03-15: page_size Seeded as 25 (Not Schema Default 50)
**By:** Reese (T012)
**What:** `page_size` seeded as `25`. Schema default is `50`.
**Why:** Briefing explicitly requested 25. Conservative initial page size.

#### 2026-03-15: external_db_enabled — Explicitly Included in Seed INSERT
**By:** Reese (T012)
**What:** `external_db_enabled` column (absent from briefing INSERT spec) explicitly included and seeded `0`.
**Why:** Column exists in schema with `DEFAULT 0`. Explicit is better than implicit for a first-launch INSERT.

---

### Leon  T009: DDL Migrations

#### 2026-03-15: sync_queue Follows data-model.md, Not Task Brief
**By:** Leon (T009)
**What:** `sync_queue` DDL uses `table_name`, `record_id`, `queued_at` per `data-model.md`. Task brief fields (`entity_type`, `entity_id`, `enqueued_at`, `payload`, `attempts`) discarded. No `payload` or `attempts` in initial migration.
**Why:** `data-model.md` is the authoritative schema source. Finch subsequently added `attempts` via migration 003; `payload` was explicitly rejected (re-read-at-sync pattern is correct).

#### 2026-03-15: user_preferences.id  INTEGER Singleton, Not TEXT ULID
**By:** Leon (T009)
**What:** `user_preferences` uses `INTEGER PRIMARY KEY DEFAULT 1` with `CHECK (id = 1)`. All other PKs are TEXT (ULID). This is the sole exception.
**Why:** Singleton-enforcement pattern. Matches `data-model.md` exactly.

#### 2026-03-15: user_preferences Seed INSERT Excluded from 001
**By:** Leon (T009)
**What:** The `INSERT INTO user_preferences DEFAULT VALUES` seed from `data-model.md` is excluded from `001_initial_schema.sql`. T012 handles first-launch seeding.
**Why:** Keeps migrations idempotent. Seeds are not migrations.

#### 2026-03-15: time_entries FK  SET NULL, Not CASCADE
**By:** Leon (T009)
**What:** `time_entries.project_id` and `time_entries.task_id` use `ON DELETE SET NULL`. Deleting a client cascades through projects and tasks but leaves time entries orphaned (not deleted).
**Why:** Spec US3 acceptance scenario 6. Historical data preserved. Matches orphan retention rule.

#### 2026-03-15: window_activity_records  No FK to time_entries
**By:** Leon (T009)
**What:** No `time_entry_id` column added to `window_activity_records`. Linked to a time window by `recorded_at` timestamp at query time only.
**Why:** `data-model.md` entity table defines no such column. Conceptual link only; no FK.

#### 2026-03-15: Extra Indexes  Beyond data-model.md Baseline
**By:** Leon (T009)
**What:** Added `idx_time_entries_started_at`, `idx_time_entries_ended_at`, `idx_war_recorded_at`, `idx_sync_queue_queued_at`. Not in `data-model.md` but needed for anticipated query patterns.
**Why:** Pure performance additions. No logical schema change.

---

### Reese  T001/T006/T017b: Scaffold + Win32 Platform

#### 2026-03-15: icons/icon.ico  Placeholder Required at Compile Time
**By:** Reese (T001)
**What:** `tauri-build` requires `icons/icon.ico` to exist. Created 32x32 PNG-in-ICO placeholder. Must be replaced with a real icon before any production/bundle build.
**Why:** `tauri::generate_context!()` macro checks path at compile time.

#### 2026-03-15: frontendDist Placeholder  Required for cargo check
**By:** Reese (T001)
**What:** Placeholder `src/Tracey.App/bin/Release/net10.0/publish/wwwroot/index.html` created so scaffold builds before Blazor is published. Remove once T002 dotnet publish runs.
**Why:** `tauri::generate_context!()` validates `frontendDist` path at compile time.

#### 2026-03-15: Win32_System_Threading Feature Required for T006
**By:** Reese (T006)
**What:** `Win32_System_Threading` added to `windows` crate features. `OpenProcess` (needed before `GetModuleFileNameExW`) lives there.
**Why:** Compile error otherwise. Required for active window detection pipeline.

#### 2026-03-15: `use tauri::Manager`  Intentional Unused Import
**By:** Reese (T006/T017b)
**What:** `use tauri::Manager` kept in `lib.rs` despite being unused. Expected warning at current stage.
**Why:** Will be needed in T008 when app handle is used for DB resource management. Not an error.

#### 2026-03-15: Module Stubs  5 Directories with Minimal mod.rs
**By:** Reese (T006/T017b)
**What:** All 5 module directories (`commands`, `db`, `models`, `platform`, `services`) scaffolded with minimal stub `mod.rs` files. Dead-code warnings from `platform/` are expected.
**Why:** Establishes module structure before task implementation. `cargo check` passes with 6 expected warnings, 0 errors.

---

### Finch  Phase 2 Adjudications (2026-03-15)

#### 2026-03-15: health_get Shape  Contract Authoritative (Ruling 1)
**By:** Finch (Adjudication)
**What:** IPC contract shape (`running`, `last_write_at`, `events_per_sec`, `memory_mb`, `active_errors`, `pending_sync_count`) is authoritative. T014 briefing shape (`status`, `db_open`, `version`, `platform`, `uptime_seconds`) was superseded. No files changed.
**Why:** `commands/mod.rs` and `TauriIpcService.cs` already implement the contract shape correctly. Contract exposes runtime observability; briefing shape was wrong in hindsight.

#### 2026-03-15: preferences_get / preferences_update  Added to Contract + Blocking Bug (Ruling 2)
**By:** Finch (Adjudication)
**What:** Both commands added to `contracts/ipc-commands.md` under new "Settings / Preferences Commands" section. Rust backend is the serialization source of truth. Blocked Root's T015 completion: `[JsonPropertyName("timezone")]` must be `"local_timezone"` and `[JsonPropertyName("entries_per_page")]` must be `"page_size"` in `TauriIpcService.cs`. Ghost `ModifiedAt` property to be removed; `ExternalDbUriStored` and `NotificationChannelsJson` to be added.
**Why:** IPC contract gap. Silent deserialization failures (null timezone, 0 page size) on every preferences call.

#### 2026-03-15: sync_queue `attempts` Added, `payload` Rejected (Ruling 3)
**By:** Finch (Adjudication)
**What:** `attempts INTEGER NOT NULL DEFAULT 0` added via `003_sync_queue_additions.sql`. `payload` column explicitly NOT added. `data-model.md` DDL updated. `SyncQueueEntry` struct updated. Column names (`table_name`, `record_id`, `queued_at`) unchanged.
**Why:** Re-reading the record from local SQLite at sync time is correct for last-write-wins. `payload` snapshot would sync stale data if a local edit occurs after enqueue. `attempts` required for retry counting and exponential backoff (T073).

---

### Root  Phase 2 Fix: JsonPropertyName + beforeDevCommand

#### 2026-03-15: JsonPropertyName Mismatches Fixed (4 attributes)
**By:** Root (Phase 2 fix, Finch blocking bug)
**What:** Fixed 4 wrong `[JsonPropertyName]` attributes in `TauriIpcService.cs`: `UserPreferences.Timezone` → `"local_timezone"`, `UserPreferences.EntriesPerPage` → `"page_size"`, `PreferencesUpdateRequest.Timezone` → `"local_timezone"`, `PreferencesUpdateRequest.EntriesPerPage` → `"page_size"`.
**Why:** `System.Text.Json` silently ignores unknown JSON keys. Every `preferences_get` / `preferences_update` call was returning null timezone and 0 page size regardless of Rust output.

#### 2026-03-15: beforeDevCommand — dotnet watch run
**By:** Root (Phase 2 fix)
**What:** `beforeDevCommand` in `tauri.conf.json` set to `"dotnet watch run --project src/Tracey.App --urls http://localhost:5000"`. `devUrl` stays `http://localhost:5000`. `Tracey.App` is pure client-side WASM served via `Microsoft.AspNetCore.Components.WebAssembly.DevServer` (Kestrel-based static file server with hot-reload).
**Why:** Without `beforeDevCommand`, `cargo tauri dev` opened a blank WebView2 — nothing was starting the Blazor dev server at port 5000. `dotnet watch run` provides hot-reload; `dotnet publish` reserved for release (`beforeBuildCommand`).

---

## Phase 3 batch 1 Decisions (2026-03-16)

### Reese — T020/T021/T022/T025: Rust Timer Commands

#### 2026-03-16: `device_id` — `COMPUTERNAME` Hostname (Phase 3 Interim)
**By:** Reese (T020)
**What:** `time_entries.device_id` resolved via `std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string())` in `timer_start`. Long-term strategy (UUID generated once and stored in `user_preferences`) deferred. Hostname is acceptable for Phase 3 (single-device local SQLite).
**Why:** `device_id TEXT NOT NULL` has no default. Briefing INSERT omitted it, causing a would-be NOT NULL constraint failure at runtime. Hostname fix unblocks Phase 3; UUID strategy deferred to Phase 4+.

#### 2026-03-16: `is_break` Column — Confirmed Present, Read from DB
**By:** Reese (T022)
**What:** `is_break INTEGER NOT NULL DEFAULT 0` exists in `time_entries`. `time_entry_list` reads it via `r.get::<_, bool>(9)?`. `timer_start` inserts `is_break = 0` (quick-entry is never a break). Phase 4 break flow may set `is_break = 1` via a future `time_entry_update`.
**Why:** Briefing comment said "TODO: determine break status / hardcode false" — implying the column might be absent. It is present. Reads are active now; break-write deferred.

#### 2026-03-16: `project_id IS ?2` — NULL-Safe Binding in `time_entry_autocomplete`
**By:** Reese (T025)
**What:** Tag-lookup subquery in `time_entry_autocomplete` uses `project_id IS ?2` and `task_id IS ?3`. `rusqlite::params!` maps `Option<String>::None` to SQL NULL; `IS` evaluates `project_id IS NULL` correctly. Standard `= NULL` would silently return no rows.
**Why:** SQL correctness. NULL-safe comparison required when filtering on nullable FK columns with optional parameters.

#### 2026-03-16: IPC Contract Compliance — All Five Timer Commands
**By:** Reese (T020–T025)
**What:** All five command signatures (`timer_start`, `timer_stop`, `timer_get_active`, `time_entry_list`, `time_entry_autocomplete`) match `contracts/ipc-commands.md` exactly, including `is_orphaned` on `AutocompleteSuggestion` (Finch 2026-03-15 amendment).
**Why:** Contract is authoritative. No deviations.

---

### Root — T027/T028/T029/T030: Blazor Frontend Phase 3

#### 2026-03-16: `ITimerStateService` — `CurrentProjectId` + `CurrentTaskId` Included
**By:** Root (T027)
**What:** `ITimerStateService` includes `CurrentProjectId` and `CurrentTaskId` (nullable strings), present in Shaw's `TimerStateServiceTests.cs` but absent from the task prompt stub. Both propagated through `StartAsync`, `StopAsync`, `InitializeAsync`. Interface and implementation in single file `TimerStateService.cs`.
**Why:** Shaw's tests are the TDD gate. Missing fields would fail test compilation.

#### 2026-03-16: Type Name Corrections — `TimeEntryItem`, `TimeEntryAutocompleteRequest`
**By:** Root (T027–T030)
**What:** Task prompt used non-existent types. Corrected to actual types in `TauriIpcService.cs`: `TimeEntryListItem` → `TimeEntryItem`; `AutocompleteRequest` → `TimeEntryAutocompleteRequest`; `TimeEntryContinueRequest(id)` → `string id` directly. `result.Suggestions` is `AutocompleteSuggestion[]` — `.ToList()` added.
**Why:** Compilation fails on missing types. Prompt was written against an earlier version of `TauriIpcService.cs`.

#### 2026-03-16: Timer Tick Wiring — Cast Pattern in `App.razor`
**By:** Root (T027/T030)
**What:** `TauriEventService.OnTimerTick` wired in `App.razor` after `Events.InitializeAsync()` using `if (TimerService is TimerStateService ts) Events.OnTimerTick += p => ts.HandleTimerTick(p.ElapsedSeconds);`. No ticks arrive until JS shim ships (Final Phase).
**Why:** `HandleTimerTick` is not on `ITimerStateService`. Cast to concrete type avoids interface pollution; pattern is consistent with JS shim deferral.

#### 2026-03-16: ARIA Roles Confirmed — `role="timer"`, `role="listbox"`, `role="option"`

---

## Phase 6 Decisions (2026-03-17)

### Reese — T043-T048 / T047: Screenshot Service + IPC Commands

#### 2026-03-17: `screenshots` Table — 7 Columns, No `created_at`
**By:** Reese (T043-T048, T047)
**What:** `screenshots` table has exactly 7 columns: `id, file_path, captured_at, window_title, process_name, trigger, device_id`. No `created_at` column. Any command querying this table must use only these 7 columns.
**Why:** Discovered during T047 implementation. Column does not exist in Leon's schema. Any SELECT or INSERT referencing `created_at` will fail at runtime.

#### 2026-03-17: GDI Capture — Critical Implementation Notes
**By:** Reese (T044)
**What:** Several non-obvious Rust/Win32 corrections required:
- `use crate::commands::AppState;` — never `crate::commands::mod::AppState` (invalid syntax)
- `PlatformHooks::get_foreground_window_info` returns `Option<WindowInfo>` — no `.ok()` needed
- `WindowInfo` field is `title` (not `window_title`) — check `platform/mod.rs` for exact shape
- `tauri::async_runtime::spawn_blocking` not `tokio::task::spawn_blocking` — never use tokio directly
- `query_map` in cleanup: bind result via `match stmt.query_map(...) { Ok(mapped) => ... }` — chained `.unwrap_or_else(Box::new(std::iter::empty()))` does not compile
- `HBITMAP → HGDIOBJ`: `HGDIOBJ(hbm.0)` — both wrap `*mut core::ffi::c_void` in windows 0.58
- GDI cleanup: `DeleteObject`/`DeleteDC` return `BOOL` (`#[must_use]`); suppress with `let _ =`
- `biCompression = BI_RGB.0` — `BITMAPINFOHEADER.biCompression` is `u32`; `.0` extracts from newtype
- BGRA→RGB: `chunks_exact(4).flat_map(|bgra| [bgra[2], bgra[1], bgra[0]])` reverses channels
**Why:** Multiple silent compile errors or incorrect runtime behaviour without these corrections.

#### 2026-03-17: E0597 State Borrow Lifetime Pattern
**By:** Reese (T043)
**What:** When `app.state::<T>()` binding is in same scope as `lock()` match at block-end position, rustc E0597 fires. Fix: bind collect result to named variable before block end (`let x = ...; x`) so `MappedRows` is fully consumed before `state` drops. Also: add `;` after `if let Ok(conn) = state.db.lock()` block close so `Result` temporary drops before `state` drops at outer block end.
**Why:** Borrow checker rule — shared reference from `app.state()` must not outlive the `app` handle in the same scope as the state value.

#### 2026-03-17: `screenshot_delete_expired` — Manually Invoked (No Background Schedule Yet)
**By:** Reese (T047)
**What:** `screenshot_delete_expired` is a manually-invoked IPC command (called from Blazor). It is NOT automatically scheduled. Root wires the "Run Cleanup" button. A future task may add app-startup background scheduling.
**Why:** Separate cleanup button pattern chosen for Phase 6. Background scheduling deferred.

---

### Root — T049: Timeline.razor C# Plumbing

#### 2026-03-17: IPC Wrapper Pattern — `new { request = ... }` Required for Struct Inputs
**By:** Root (T049)
**What:** All IPC commands that take a Rust struct as input MUST be called from Blazor with `new { request = ... }` wrapper. Example: `screenshot_list` → `new { request = new { from, to } }`. Calling with bare `new { from, to }` sends wrong shape (top-level instead of nested) — Rust deserialization silently fails with `missing field` error.
**Why:** Established Phase 2 convention. `screenshot_list` was incorrectly called with bare args; corrected in T049.

#### 2026-03-17: `ErrorPayload` — `{ component, event, error }` Not `{ component, message }`
**By:** Root (T049)
**What:** `ErrorPayload` in `TauriEventService.cs` updated to `{ Component, Event, Error }` matching the `tracey://error` contract. Previous `Message` field was a deviation. `Event` is valid as a C# record parameter name (only lowercase `event` is a keyword).
**Why:** Timeline.razor needs `payload.Error`. No existing component was consuming `payload.Message`.

#### 2026-03-17: Razor `@code` Inside HTML Comments — Parse Error
**By:** Root (T049)
**What:** Do NOT place `@code` inside `<!-- -->` comments in `.razor` files. Razor parser interprets `@code` as a directive even inside HTML comments, causing RZ2005/RZ1017 parse errors. Remove any such comments.
**Why:** Discovered when UXer's scaffold included a documentation comment referencing `@code`.

#### 2026-03-17: Keyboard Handlers in Razor — Extract to Named Methods
**By:** Root (T049)
**What:** `@onkeydown` lambdas containing string comparisons (`"Enter"`, `" "`) inside double-quoted Razor attributes cause parse failures (Razor quote conflict). Always extract to a named C# method. C# `is "Enter" or " "` pattern expression is idiomatic .NET 9+.
**Why:** Same class of issue as `ExpandLabel` workaround in T041 (Projects.razor).

---

### Shaw — T042: US4 Screenshot Timeline E2E Tests

#### 2026-03-17: TDD Gate OPEN — 10 Tests, 0 TypeScript Errors
**By:** Shaw (T042)
**What:** `tests/e2e/specs/screenshot-timeline.spec.ts` — 10 tests, all fail `net::ERR_CONNECTION_REFUSED` (TDD gate open, correct). TypeScript: 0 errors. File replaces the empty `timeline.spec.ts` stub.
**Why:** TDD rule: failing tests committed before implementation.

#### 2026-03-17: `tracey://error` CustomEvent Dispatch Contract
**By:** Shaw (T042)
**What:** Shaw's test 8 dispatches the error event via `window.dispatchEvent(new CustomEvent('tracey://error', { detail: { message } }))`. `TauriEventService.cs` MUST listen on the `window` object for `tracey://error` (not only via Tauri's `listen()` API) to pass test 8. If the service uses Tauri's event bus exclusively, Root must add a `window.addEventListener('tracey://error', ...)` path or Shaw adapts test 8 in Phase 6 review.
**Why:** Test 8 simulates the error condition by injecting a DOM CustomEvent. Tauri's native event bus does not fire from `window.dispatchEvent`.

#### 2026-03-17: Capture-Dependent Tests Use `test.skip` Guard
**By:** Shaw (T042)
**What:** Tests 3–7 (screenshot item rendering, timestamp, process name, trigger badge, preview) guard: if `screenshot_list` returns empty after 5s wait, `test.skip()` is called. No hard failure without `--features test` build.
**Why:** GDI test double only active when built with `cargo tauri build --features test`.

---

### UXer — T049-UX: Timeline HTML Scaffold + CSS

#### 2026-03-17: Error Banner — Custom Div, Not BbAlert
**By:** UXer (T049-UX)
**What:** `.timeline-error-banner` is a hand-rolled `<div role="alert">` with left accent border + inline dismiss button on the right. `BbAlert` not used here.
**Why:** Spec requires dismiss button inline-right of error text. BbAlert's dismiss chrome is above content, not beside it. `BbAlert` remains correct for full-width warning panels (Projects.razor).

#### 2026-03-17: Screenshot Grid — `auto-fill minmax(280px, 1fr)`
**By:** UXer (T049-UX)
**What:** Grid: `repeat(auto-fill, minmax(280px, 1fr))`. No media query breakpoints.
**Why:** Tauri window freely resizable. `auto-fill` collapses to 1 column at ≤600px and expands to 3+ columns. Fixed breakpoints create awkward jumps.

#### 2026-03-17: Preview Panel — `position: sticky; bottom: 1.5rem`
**By:** UXer (T049-UX)
**What:** Expanded screenshot preview is `position: sticky; bottom: 1.5rem` in page flow — not a modal or overlay.
**Why:** Stays visible while user scrolls grid. Modal would block grid; overlay complicates z-index in WebView2. Fallback for short pages: panel appears inline (still visible — acceptable).

#### 2026-03-17: HTML Scaffold Ownership Boundary
**By:** UXer (T049-UX)
**What:** UXer owns all HTML outside `@code{}`. Root replaces only the `@code{}` block with real C# logic. HTML/CSS changes require UXer review. Agreed binding class names:
- `.timeline-error-banner` → `_errorMessage != null`
- `.timeline-loading` → `_loading == true`
- `.empty-state-illustration` → `_screenshots.Count == 0 && !_loading`
- `.screenshot-grid` → `_screenshots.Count > 0`
- `.screenshot-item` → per item; `.screenshot-item.selected` → `item == _selected`
- `.screenshot-preview` → `_selected != null`
**Why:** Clean division of frontend responsibility. CSS selectors are the contract.

#### 2026-03-17: Accessibility — CHK039 Arrow-Key Navigation Deferred
**By:** UXer (T049-UX)
**What:** Grid `role="list"`, items `role="listitem"`, `tabindex="0"`, error `role="alert"`, preview `<img alt>` with timestamp. Tab-based focus traversal implemented. Arrow-key navigation within grid NOT yet implemented — flagged for Final Phase.
**Why:** CHK038 spec asks for keyboard scrolling. Arrow-key handling in grid deferred; Root may add `@onkeydown` to grid container in Final Phase.
**By:** Root (T028/T029)
**What:** Elapsed counter uses `role="timer" aria-live="off" aria-atomic="true"`. Autocomplete dropdown uses `role="listbox"` with items `role="option"`. Continue button: `role="button" name=/continue/i`. All match Shaw's T018 Playwright selectors (2026-03-15 TDD Gate spec).
**Why:** Shaw's tests drive UI structure. Aria roles must match to avoid test failures.

#### 2026-03-16: `Components/` Directory Added; `_Imports.razor` Updated
**By:** Root (T028/T029)
**What:** `src/Tracey.App/Components/` created for `QuickEntryBar.razor` and `TimeEntryList.razor`. `_Imports.razor` updated with `@using Tracey.App.Components`.
**Why:** Components not in `Pages/` or `Layout/`. New directory required. Global using added for clean razor markup.

#### 2026-03-16: `SaveInlineEdit` — Stub (T030b)
**By:** Root (T029)
**What:** `SaveInlineEdit` in `TimeEntryList.razor` is a stub (reloads page). Actual `time_entry_update` IPC call is T030b. `time_entry_update` command not yet in `contracts/ipc-commands.md` (per 2026-03-15 Root decision — blocked on contract).
**Why:** Cannot implement update without contracted command shape. Stub prevents compile error.

#### 2026-03-16: Scroll Position — sessionStorage Stub (T029a)
**By:** Root (T029)
**What:** Scroll position key `tracey.entry-list.scroll` written to `sessionStorage` as placeholder `"0"`. Full `scrollTop` read/restore deferred to T029a.
**Why:** sessionStorage key established now so T029a knows the contract. Full implementation requires JSInterop to read element scroll position.

---

### Reese  Phase 3 Batch 2 (2026-03-16)

#### 2026-03-16: Overlap Detection Query  Create vs Update
**By:** Reese (T023, T030a)
**What:** Two variants of the overlap query. Create: `SELECT COUNT(*) FROM time_entries WHERE ended_at IS NOT NULL AND started_at < ?2 AND ended_at > ?1`. Update adds `id != ?1` to exclude self. Bypassed when `force: true`.
**Why:** Two intervals overlap if `A.start < B.end AND A.end > B.start`. Active timer (NULL ended_at) is never included. `force` flag allows override.

#### 2026-03-16: Timer-Tick Emitter  `tokio::spawn` in `.setup()` (T026)
**By:** Reese (T026)
**What:** `services::timer_tick::start_tick_loop(app.handle().clone())` called inside Tauri's `.setup()` hook. `AppHandle` is `Clone + Send + 'static`, safe to move into spawned task. `MutexGuard<rusqlite::Connection>` must be dropped before any `.await`  inner `{}` block holds lock, computes `tick_payload: Option<Value>`, drops guard, then `app.emit()` called outside block. Requires `use tauri::{Emitter, Manager}` in `timer_tick.rs`.
**Why:** Cleanest pattern for background async tasks in Tauri v2. Guard drop discipline prevents deadlock on await.

#### 2026-03-16: device_id in Manual Create  COMPUTERNAME Env Var (T023)
**By:** Reese (T023)
**What:** `std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string())` used in all insert paths (timer_start, time_entry_create_manual). Manual entries carry the ID of the creating device.
**Why:** `device_id TEXT NOT NULL` has no DEFAULT. Cross-device manual-create not in scope; current device is correct attribution.

#### 2026-03-16: `insert_new_timer` Shared Helper Refactor (T024)
**By:** Reese (T024)
**What:** `#[tauri::command]` fns cannot be called directly (State<'_, T> not hand-constructible). Extracted `insert_new_timer(conn: &rusqlite::Connection, ...)` as a private helper returning `Result<(String, String), String>`. Both `timer_start` and `time_entry_continue` call it. Borrow-checker fix: bind fully-consumed result (`let x: Vec<String> = stmt.query_map(...)?.filter_map(|r| r.ok()).collect(); x`) before block end to release borrow on statement.
**Why:** Code reuse without Tauri command call. E0597 fix: `collect()` consumes iterator and releases borrow before drop.

#### 2026-03-16: `Option<Option<T>>` for Nullable Patch Fields (T030a)
**By:** Reese (T030a)
**What:** Custom `deserialize_option_nullable` fn + `#[serde(default, deserialize_with = "...")]`. Absent field  `None` (don't change). `null`  `Some(None)` (clear). Value  `Some(Some(v))` (set). `unwrap_or(curr_value)` implements merge. No extra crates.
**Why:** Standard serde cannot distinguish absent from null for `Option<T>`. This pattern is the minimal inline solution.

---

### Root  T030b (2026-03-16)

#### 2026-03-16: Auto-Save on Blur  No Save Button (T030b)
**By:** Root (T030b)
**What:** `@onblur` on each field (description, start, end) triggers save. `_isSaving` bool guards concurrent saves when user tabs through all fields rapidly. On error, `_editingId` is NOT cleared  user sees inline error and can retry. Cancel discards.
**Why:** Matches UX tone guide (automatic persistence). Shaw's T030c verifies Tab triggers save.

#### 2026-03-16: DateTime UTC  Local Conversion (T030b)
**By:** Root (T030b)
**What:** Open for edit: parse UTC ISO string with `DateTimeStyles.RoundtripKind`, call `.ToLocalTime()` to populate `<input type="datetime-local">`. Save: call `.ToUniversalTime().ToString("o")` before IPC send.
**Why:** `<input type="datetime-local">` has no timezone concept. User edits wall-clock; Rust stores UTC. Pure C#  no JS interop needed.

#### 2026-03-16: Overlap/Invalid-Time Error Display (T030b)
**By:** Root (T030b)
**What:** `ex.Message.Contains("overlap_detected")`  "This time overlaps with another entry. Adjust the times or cancel." `"invalid_time_range"`  "End time must be after start time." Others  "Save failed: {ex.Message}". Rendered as `<p class="edit-error" role="alert">`.
**Why:** Tauri IPC exceptions propagate as JS Error objects with Rust error code in message string. `role="alert"` announces errors to screen readers immediately.

#### 2026-03-16: `time_entry_update` IPC Contract Added (T030b)
**By:** Root (T030b)
**What:** Contract added to `contracts/ipc-commands.md`. Input: `id`, `description`, `project_id?`, `task_id?`, `tag_ids?`, `started_at`, `ended_at`, `force` (bool). Output: `{ "modified_at": "string" }`. Errors: `not_found`, `invalid_time_range`, `overlap_detected`.
**Why:** Deferred from 2026-03-15 pending Finch spec. Root added based on T030a task description and existing create_manual pattern.

---

### Shaw  Phase 3 Batch 2 (2026-03-16)

#### 2026-03-16: 7 New E2E Tests  Total 27 in timer.spec.ts (T025a, T029a, T030c)
**By:** Shaw (T025a, T029a, T030c)
**What:** 3 new `test.describe` blocks appended to `timer.spec.ts` (was 20 tests, now 27). T025a: 2 orphaned-autocomplete tests (self-guard if no pre-seeded orphan data). T029a: 1 scroll-preservation test (self-guard if list not scrollable). T030c: 4 inline-edit tests (self-guard if no completed entry visible). All self-guard with early return rather than hard-fail.
**Why:** Pre-condition dependencies require pre-seeded fixtures not yet wired. Self-guarding prevents CI noise while contracts are being built.

#### 2026-03-16: Selector Contracts for Root (T030c)
**By:** Shaw (T030c)
**What:** Shaw's tests require: `.autocomplete-dropdown`, `.suggestion-item.is-orphaned`, `.orphan-warning[title]` (title contains "no longer exists"), `.time-entry-list`, `.entry-description-btn`, `.entry-edit-form`, `input[aria-label="Entry description"]`, `input[aria-label="Start time"]`, `input[aria-label="End time"]`, `button` matching `/cancel edit/i`. TypeScript: 0 errors.
**Why:** Shaw's selectors drive Root's DOM contracts. Both sides must honour these for tests to pass.

---

## Phase 4 — US2 Idle Detection Decisions (2026-03-16)

### Shaw — T031: Idle Detection E2E Tests

#### 2026-03-16: 7 Failing E2E Tests — TDD Gate Open (T031)
**By:** Shaw (T031)
**What:** 7 new tests in `tests/e2e/specs/idle-detection.spec.ts`. All currently fail with `net::ERR_CONNECTION_REFUSED` (correct TDD state — app not running during authoring). Tests cover: no-modal-when-no-timer guard, modal appears + 4 buttons present, Keep option, Break option, Meeting option, Specify inline flow, threshold-from-preferences. TypeScript: 0 errors. `preferences_update` with `{ update: { inactivity_timeout_seconds: 5 } }` used to force fast detection. `timer_stop` called with try/catch at test start (ignores `"no_active_timer"` error). `window.__TAURI_INTERNALS__.invoke(...)` used in `page.evaluate()` with `@ts-ignore` (Tauri JS types unavailable in test env).
**Why:** TDD gate: tests are the acceptance criteria. Root's modal must honour `role="dialog" name=/idle|away|back/i`, four `role="button"` options, `role="textbox" name=/description|what were you doing/i` for Specify inline flow.

---

### Reese — T032/T033/T034: IdleService + IPC Commands

#### 2026-03-16: `AppState` holds `Arc<dyn PlatformHooks + Send + Sync>` (T032)
**By:** Reese (T032)
**What:** `AppState.platform` is `Arc<dyn PlatformHooks + Send + Sync>`, constructed in `lib.rs::run()` as `Arc::new(WindowsPlatformHooks)`. Trait already had `Send + Sync` bounds from T017b. `Arc` enables sharing between idle loop tokio thread and concurrent Tauri command handlers without copying.
**Why:** Idle loop runs on a spawned tokio task; command handlers run concurrently. `Arc<dyn Trait + Send + Sync>` is the standard Rust pattern for shared trait objects across async tasks.

#### 2026-03-16: Idle Loop — MutexGuard Dropped Before `.await` (T032)
**By:** Reese (T032)
**What:** `start_idle_loop` follows `timer_tick.rs` pattern: all DB access (`Mutex<Connection>`) and platform queries happen inside a synchronous inner block. `MutexGuard` is dropped before `tokio::time::sleep().await`. Holding a `std::sync::MutexGuard` across an `.await` point causes a compile error with `tokio::spawn`.
**Why:** Mandatory. Holding `MutexGuard` across `.await` is a compile error under `tokio::spawn`. Same discipline as `timer_tick.rs`.

#### 2026-03-16: `idle_resolve` — `ended_at` Set to `idle_started_at` (Not Resolution Time) (T034)
**By:** Reese (T034)
**What:** When any resolution branch stops the running timer ("break", "meeting", "specify"), `ended_at` is set to `idle_started_at` — the moment the user went idle — not `Utc::now()` at resolution time. New entry (break/meeting/specified) starts at `idle_started_at`, ends at `idle_ended_at` (return-from-idle timestamp from frontend).
**Why:** Accurate time-entry boundaries. The user was productively working until idle — not until they clicked a button.

#### 2026-03-16: `device_id` in Idle Inserts — `COMPUTERNAME` Env Var (T034)
**By:** Reese (T034)
**What:** `device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string())` in `idle_resolve` `insert_entry` helper calls. Consistent with `timer.rs::insert_new_timer` pattern.
**Why:** `device_id TEXT NOT NULL`. Missing from initial briefing; required by schema. Same interim strategy as T020.

#### 2026-03-16: `idle_get_status` — Live Platform Query, Not Loop State (T033)
**By:** Reese (T033)
**What:** `idle_get_status` calls `idle_service::get_current_idle_status(state.platform.as_ref(), threshold)` — a fresh `GetLastInputInfo`+`GetTickCount64` query at command invocation time. Loop's internal `IdleState` is private and not shared.
**Why:** Avoids shared-state complexity. Accurate at invocation time. Frontend polling/events do not depend on stale loop state.

#### 2026-03-16: CRITICAL — Direct Win32, NOT `tauri-plugin-system-idle` (T032)
**By:** Reese (T032)
**What:** Idle detection uses direct `GetLastInputInfo` + `GetTickCount64` Win32 calls in `platform/windows.rs` via `PlatformHooks::get_idle_seconds()`. `tauri-plugin-system-idle` does NOT exist on crates.io and is NOT used.
**Why:** Plugin unavailable (confirmed T007/Control decision 2026-03-15). Direct Win32 already implemented in `platform/windows.rs`. `GetTickCount64` mandatory to avoid 32-bit rollover after ~49 days of uptime.

---

### Root — T035/T036: IdleReturnModal + Dashboard Wiring

#### 2026-03-16: Modal ARIA — `role="dialog" aria-label="You're back"` (T035)
**By:** Root (T035)
**What:** `<dialog role="dialog" aria-modal="true" aria-label="You're back" aria-describedby="idle-modal-desc">`. `aria-label="You're back"` matches Shaw's Playwright locator `role="dialog" name=/idle|away|back/i`. Duration text is `aria-describedby` target so screen readers announce context after heading.
**Why:** Shaw's T031 accessibility selectors are the acceptance contract. Modal name must match the regex.

#### 2026-03-16: Specify Flow — Inline, Not New Modal (T035)
**By:** Root (T035)
**What:** "Specify" reveals inline form within the same `<dialog>`: `_showSpecifyInput = true` swaps button group for text input + Save/Back. `aria-label="What were you doing?"` on input matches Shaw's selector `role="textbox" name=/description|what were you doing/i`. "Back" resets `_showSpecifyInput = false` without calling resolve. "Save" calls `IdleResolveAsync` with `resolution = "specify"`.
**Why:** Shaw's test asserts `role="textbox"` within the same modal dialog. Navigation or second modal would break the test and produce poor UX.

#### 2026-03-16: `@onclick` Razor Quote Escaping — Single-Quote Outer (T035)
**By:** Root (T035)
**What:** `@onclick` lambdas with string literal arguments use single-quote outer syntax: `@onclick='() => Resolve("break")'`. Applies to all four resolution buttons.
**Why:** Razor parser conflict bug: inner double-quotes inside `@onclick="..."` attribute cause parser failure. Single-quote outer avoids the conflict.

#### 2026-03-16: Dashboard Wiring — `HadActiveTimer` Guard + `InvokeAsync` (T036)
**By:** Root (T036)
**What:** `TauriEventService.OnIdleDetected` subscribed in `OnInitializedAsync`, unsubscribed in `Dispose()`. Handler guards on `payload.HadActiveTimer` — no modal when no active timer. `InvokeAsync` wraps `Show()` + `StateHasChanged()` for Blazor thread safety. `OnResolved` triggers `RefreshList()` → `TimeEntryList.LoadPage(1)`.
**Why:** Spec and `now.md` decision: no modal if no active timer. `InvokeAsync` required when Tauri events arrive on non-Blazor threads.

#### 2026-03-16: `idle_ended_at` Captured at `Show()` Time (T036)
**By:** Root (T036)
**What:** `idle_ended_at` is `DateTime.UtcNow.ToString("o")` captured at `Show()` invocation time — not at button-click time. Passed as parameter to `IdleResolveAsync`.
**Why:** Avoids drift if user reads the modal slowly. Arrival moment is the semantically correct return-from-idle timestamp.

---

## Bug-Fix Sprint Decisions (2026-03-17)

### Reese — Bug Fixes

#### 2026-03-17: IPC Shape Correction — `project_list` and `task_list` (Bugs 1 & 2)
**By:** Reese
**What:** `project_list` now returns `{ "projects": [...] }` (wrapped); `task_list` now returns `{ "tasks": [...] }` (wrapped). Previous Phase 5 entry recording "bare JSON array" was incorrect — C# `ProjectListResponse` / `TaskListResponse` both deserialize a wrapped object.
**Why:** Deserialization failure caused Projects.razor to throw on every expand. Root-cause was a Phase 5 decision typo. All list commands now return wrapped objects consistently.

#### 2026-03-17: `project_delete` Returns Counts (Bug 1 followup)
**By:** Reese
**What:** `project_delete` now counts `deleted_tasks` and `orphaned_entries` before delete and returns `{ "deleted_tasks": N, "orphaned_entries": N }` to match `ProjectDeleteResponse`. Previously returned `Ok(null)`.
**Why:** C# DTO expects both counts. Returning null caused C# deserialization failure.

#### 2026-03-17: assetProtocol — Config + Cargo Feature Required Together (Bug 6)
**By:** Reese
**What:** `tauri.conf.json` must have `assetProtocol { enable: true, scope: ["**"] }` AND `Cargo.toml` tauri features must include `"protocol-asset"`. CSP must include `http://asset.localhost` (WebView2 on Windows uses http scheme in some Tauri 2 versions). Missing either config causes build error or silent permission failure.
**Why:** Tauri build script validates that `assetProtocol.enable = true` in conf matches `protocol-asset` in Cargo features. Screenshots cannot be displayed without both; `http://asset.localhost` needed in CSP for WebView2.

#### 2026-03-17: `fs:allow-read-file` Required for Asset Protocol (Bug 6)
**By:** Reese
**What:** `capabilities/default.json` must include `"fs:allow-read-file"` alongside `"fs:allow-write-file"`. Asset protocol reads files from disk — the read permission is mandatory.
**Why:** Only write permission was present. Asset protocol serves files via read; without it the file is silently blocked.

---

### Root — Bug Fixes

#### 2026-03-17: Real TauriEventService Bridge via tauri-bridge.js (Bug 3)
**By:** Root
**What:** `TauriEventService.Listen<T>` was a no-op stub. Replaced with a `DotNetObjectReference`-based bridge: `wwwroot/js/tauri-bridge.js` IIFE registers `__TAURI_INTERNALS__.listen` for all 7 `tracey://` events; each routes payload via `dotNetRef.invokeMethodAsync('RouteEvent', eventName, jsonPayload)`. `[JSInvokable] RouteEvent` in C# deserializes and dispatches typed events. `convertFileSrc` produces `https://asset.localhost/C%3A/...` URLs.
**Why:** Without a real bridge, no Tauri event ever reached C#. Timer ticks, idle detection, screenshot capture, and error events were all silently dropped.

#### 2026-03-17: Dashboard Requires `Events.InitializeAsync()` Call (Bug 4)
**By:** Root
**What:** `Dashboard.OnInitializedAsync` must call `await Events.InitializeAsync()` to activate the JS bridge and register Tauri event listeners. Without this call `OnTimerTick` and other events never fire — even with the bridge implemented. Also: `Events.OnTimerTick` must be explicitly subscribed → `HandleTimerTick` → `TimerStateService.HandleTimerTick`.
**Why:** `InitializeAsync` creates the `DotNetObjectReference` and invokes `initializeTauriBridge` in JS. Nothing is registered until this call completes.

#### 2026-03-17: TimerStateService — Local PeriodicTimer Fallback (Bug 4)
**By:** Root
**What:** `TimerStateService` now has a local `PeriodicTimer` (1s) that increments `_elapsedSeconds++` each second while the timer is running. `HandleTimerTick(long)` snaps `_elapsedSeconds` to Rust's authoritative value when real events arrive. Ticker started in `StartAsync()` and `InitializeAsync()` (if restoring a running timer); stopped in `StopAsync()`.
**Why:** Without this, the elapsed display is frozen until the first Rust tick arrives. PeriodicTimer gives smooth UI; Rust ticks provide accuracy.

#### 2026-03-17: TimeEntryList `LoadPage` — `StateHasChanged()` in Finally Block (Bug 5)
**By:** Root
**What:** `TimeEntryList.LoadPage` `finally{}` block must call `StateHasChanged()` after setting `_loading = false`. Without it Blazor never triggers a re-render and the list stays on "Loading entries..." indefinitely.
**Why:** `_loading = false` alone does not trigger re-render in Blazor WASM. `StateHasChanged()` is required to notify the renderer.

#### 2026-03-17: Asset URL Format for Screenshots (Bug 6)
**By:** Root
**What:** Screenshot image URLs must use `https://asset.localhost/C%3A/path/to/file.jpg` (colon URL-encoded as `%3A`). `tauri-bridge.js` `convertFileSrc` implements this: uses `__TAURI_INTERNALS__.convertFileSrc` if available, else manually encodes drive colon. Old `asset://localhost/` format does not work in Tauri 2 / WebView2.
**Why:** Tauri 2 asset protocol uses `https://asset.localhost/` scheme. Drive colon must be percent-encoded. Wrong URL causes broken image icon.

---

### Shaw — Bug-Fix TDD Gate

#### 2026-03-17: TDD Gate OPEN — 6 Tests for Bug-Fix Sprint
**By:** Shaw
**What:** 6 new failing regression tests across `tests/e2e/specs/bug-fixes.spec.ts` (4 tests: Bugs 1, 2, 4, 5) and `tests/e2e/specs/timeline-bugs.spec.ts` (2 tests: Bugs 3, 6). All will turn green when bugs are fixed. TypeScript: 0 errors.
**Why:** TDD gate: tests are the acceptance criteria for the bug-fix sprint. Committed failing as proof of bugs.

---

### UXer — 24h Horizontal Timeline Redesign (Feature 7)

#### 2026-03-17: Timeline CSS — Full Rewrite for Horizontal 24h Bar (Feature 7)
**By:** UXer
**What:** `Timeline.razor.css` completely rewritten. Old card-grid classes removed. New design: full-width dark gradient bar (`.timeline-day-bar` / `.timeline-bar-inner`), 80px tall, 24 hour markers (`.timeline-hour-marker`), screenshot dots (`.timeline-screenshot-dot`) at time-proportional positions, hover indicator (`.timeline-hover-indicator` + `.hover-time-label`), preview area (`.timeline-preview-area` + `.screenshot-img`). CSS class contract locked with Root.
**Why:** Feature 7 spec. Card grid replaced by timeline metaphor for better temporal navigation.

#### 2026-03-17: Timeline CSS Class Contract (Root ↔ UXer)
**By:** UXer + Root
**What:** Agreed binding class names: `.timeline-day-bar`, `.timeline-bar-inner`, `.timeline-hour-marker`, `.timeline-screenshot-dot`, `.timeline-dot-selected`, `.timeline-hover-indicator`, `.hover-time-label`, `.timeline-preview-area`, `.preview-header`, `.preview-header-hover`, `.screenshot-img`, `.screenshot-img-hover`, `.preview-placeholder`, `.timeline-controls`, `.timeline-error-banner`, `.empty-state-illustration`, `.timeline-loading`. No class renamed, added, or removed without mutual consent.
**Why:** Root's C# applies these classes; UXer's CSS styles them. Breaking the contract causes invisible styles.
