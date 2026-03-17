# Reese — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Tauri 2.0 (Rust, Windows 11) + Blazor WASM .NET 10 + SQLite (WAL, `rusqlite`) + optional Postgres/Supabase
- **My files:** `src-tauri/src/` (commands/, models/, db/, platform/)
- **Contracts:** `specs/001-window-activity-tracker/contracts/ipc-commands.md` — source of truth for all IPC commands I implement
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup & Critical Rules
- `GetTickCount64` only — never `GetTickCount` (32-bit rollover after ~49 days)
- `HWND` null check: `std::ptr::null_mut()`, not `== 0`
- `GetDesktopWindow` is in `Win32_UI_WindowsAndMessaging`, NOT `Win32_Graphics_Gdi`
- Screenshot pipeline: `spawn_blocking` always — main thread causes "Not Responding"
- `fs:allow-write-file` (singular) — `fs:allow-write-files` (plural) fails silently
- Screenshot paths: `std::fs::canonicalize` before write (path traversal mitigation)
- `logo_path` on Client entity: NEVER synced to external DB
- Process deny-list: applied at Rust collection boundary BEFORE any DB write
- Schema designed by Leon — I implement what Leon specifies, not vice versa
- `PlatformHooks` trait defined in `src-tauri/src/platform/mod.rs` (T017b); Windows impl in `platform/windows.rs`

### 2026-03-15: T001 — Tauri 2.0 Scaffold (cargo check PASS)
- `tauri-plugin-system-idle` **does not exist on crates.io** — idle detection must be implemented via Win32 manually in `platform/windows.rs` (T017b). Removed from Cargo.toml; documented in decisions inbox.
- `tauri::generate_context!()` macro validates `frontendDist` path at **compile time** — created placeholder `src/Tracey.App/bin/Release/net10.0/publish/wwwroot/index.html` so scaffold builds without running dotnet publish.
- `tauri-build` on Windows requires `icons/icon.ico` — created 32x32 PNG-in-ICO placeholder via System.Drawing; real icon needed before production build.
- `cargo check` result: **PASS** — exit code 0, zero warnings, zero errors.
- Directory structure created:
  - `src-tauri/Cargo.toml` — all deps (minus non-existent idle plugin)
  - `src-tauri/tauri.conf.json`
  - `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
  - `src-tauri/build.rs`
  - `src-tauri/capabilities/default.json` — stub for Control (T007)
  - `src-tauri/icons/icon.ico` — 32x32 placeholder
  - `.cargo/config.toml` — `target = "x86_64-pc-windows-msvc"`

### 2026-03-15: T006 — Tauri App Shell (cargo check PASS)
- `lib.rs` replaced T001 stub with full plugin registration, module declarations, logging init, and commented-out future command stubs.
- Module directories created: `commands/`, `db/`, `models/`, `platform/`, `services/` — each with a stub `mod.rs`.
- `tauri.conf.json` confirmed correct from T001: title "Tracey", 1200×800, resizable, correct frontendDist, CSP set.
- `env_logger = "0.11"` added to `Cargo.toml`.
- `Win32_System_Threading` added to windows crate features (required by `OpenProcess` in `GetModuleFileNameExW` pipeline).
- `cargo check` result: **PASS** — 6 warnings (all expected dead_code/unused_import from stubs not yet wired up), zero errors.

### 2026-03-15: T008 + T010 — DB Initializer, Migration Runner, Model Structs (cargo check PASS)

**DB path resolution strategy:**
- Primary: `{exe_dir}/tracey.db` — tested with write-probe (`is_writable` creates + deletes `.tracey_write_test`)
- Fallback: `%APPDATA%\tracey\tracey.db` — created via `std::fs::create_dir_all`
- No `directories` crate needed; `std::env::var("APPDATA")` is sufficient on Windows

**PRAGMA order:** `journal_mode = WAL` and `foreign_keys = ON` set via `execute_batch` immediately after `Connection::open`, before migrations run.

**Migration runner approach:**
- SQL files embedded at compile time with `include_str!` — zero runtime filesystem reads
- `schema_migrations` table bootstrapped with `CREATE TABLE IF NOT EXISTS` before any version query
- Each migration runs in `unchecked_transaction` (requires `&Connection`, not `&mut`); `tx.commit()` on success, auto-rollback on drop if an error propagates
- Homogeneous array type issue: `[version, &String]` rejected by rustc (`&&str` vs `&String`); fixed by switching to `rusqlite::params![version, chrono::Utc::now().to_rfc3339()]`
- `cargo check` result: **PASS** — 21 dead_code warnings (all expected stubs), zero errors

**Model field corrections vs briefing — matched Leon's SQL exactly:**
- `Tag`: no `color` field (not in SQL or data-model.md)
- `TimeEntry`: added `is_break: bool` and `device_id: String`
- `WindowActivityRecord`: uses `window_handle: String` + `device_id: String`; removed `time_entry_id` and `process_path` (not in SQL)
- `Screenshot`: uses `trigger: String` + `device_id: String`; removed `width`/`height` (not in SQL)
- `UserPreferences`: `id: i64` (INTEGER singleton), `local_timezone` (not `timezone`), `page_size` (not `entries_per_page`), added `external_db_uri_stored: bool` + `notification_channels_json: Option<String>`, no `modified_at` column
- `SyncQueueEntry`: `id: i64` (INTEGER AUTOINCREMENT, not TEXT); `queued_at` confirmed correct

### 2026-03-15: T017b — PlatformHooks Trait (TWO methods, cargo check PASS)
- `platform/mod.rs`: trait with exactly 2 methods (`get_foreground_window_info`, `get_idle_seconds`). Architecture decision by Finch: `trigger_screenshot_capture()` removed; capture triggering belongs in ActivityTracker/ScreenshotService.
- `platform/windows.rs`: `WindowsPlatformHooks` struct implementing the trait.
  - `get_foreground_window_info`: `GetForegroundWindow` → `GetWindowThreadProcessId` → `OpenProcess` → `GetModuleFileNameExW`.
  - HWND null check: `hwnd == HWND(std::ptr::null_mut())` — confirmed compiling on windows crate 0.58.
  - `get_idle_seconds`: `GetLastInputInfo` + `GetTickCount64` (64-bit; 32-bit `GetTickCount` NOT used — rollover avoidance).
- `cargo check` result: **PASS**

### 2026-03-15: T011 — Structured JSON Logger (cargo check PASS)

**File:** `src-tauri/src/services/logger.rs`

- JSON lines on stderr (`eprintln!`) — avoids mixing with Tauri's stdout protocol.
- `LogEntry<'a>` struct: `ts` (RFC 3339), `level`, `component`, `event`, optional `trace_id`, optional `detail`.
- `DENY_LIST: OnceLock<Vec<String>>` — populated at startup via `init_deny_list(json_list)`.
- `redact_value` / `redact_str` — recursively redacts strings in JSON detail values if they contain a deny-list pattern (case-insensitive substring match).
- Three macros exported: `log_info!`, `log_warn!`, `log_err!` (crate-level via `#[macro_export]`).
- `services/mod.rs` updated: `pub mod logger;`
- `cargo check` result: **PASS** (dead_code warnings for logger functions — expected, not yet called)

### 2026-03-15: T013 — preferences_get + preferences_update IPC Commands (cargo check PASS)

**File:** `src-tauri/src/commands/mod.rs`

- `AppState { db: Mutex<rusqlite::Connection> }` — shared state registered with Tauri `.manage()`.
- `preferences_get`: SELECT all 12 columns from `user_preferences LIMIT 1`, returns `UserPreferences`.
- `preferences_update`: read-modify-write pattern (read full row, apply partial delta from `PreferencesUpdateRequest`, write all non-keychain fields back).
- **Field name corrections vs briefing** (briefing had stale names):
  - `timezone` → `local_timezone`
  - `entries_per_page` → `page_size`
  - `modified_at` removed (column doesn't exist in schema)
  - Added `external_db_uri_stored` (read-only in this command — managed by `sync_configure`)
  - Added `notification_channels_json` (updatable)
- `external_db_uri_stored` intentionally omitted from `PreferencesUpdateRequest` — that flag belongs to OS keychain / `sync_configure` flow.
- `cargo check` result: **PASS**

### 2026-03-15: T014 — health_get IPC Command (cargo check PASS)

**File:** `src-tauri/src/commands/mod.rs` (same file as T013)

- **IPC contract shape used** (not briefing shape) — this is the authoritative source.
- `HealthResponse`: `{ running: bool, last_write_at: Option<String>, events_per_sec: f64, memory_mb: f64, active_errors: Vec<String>, pending_sync_count: i64 }`
- Phase-1 stubs: `running = true`, `last_write_at = None`, `events_per_sec = 0.0`, `memory_mb = 0.0`, `active_errors = []`.
- `pending_sync_count` is a live `SELECT COUNT(*) FROM sync_queue` query.
- `init_health()` is a no-op placeholder (future tasks will start metric collection).
- **Deviation documented** in `.squad/decisions/inbox/reese-t011-t013-t014.md`.
- `cargo check` result: **PASS**

### 2026-03-15: lib.rs Wiring (T011/T013/T014)

- `use commands::AppState;` added.
- `env_logger::init()` moved before builder (so DB open log messages are captured).
- `commands::init_health()` called before builder.
- `db::open()` called before builder; `AppState` registered via `.manage()`.
- Three commands registered: `commands::preferences_get`, `commands::preferences_update`, `commands::health_get`.
- Removed stale `use tauri::Manager` (was unused after removing `app` param from setup).
- Final `cargo check` result: **PASS** — 20 dead_code/unused warnings (all expected stubs), zero errors.

### 2026-03-15: T012 — First-Launch Initialization (cargo check PASS)

**File:** `src-tauri/src/db/mod.rs`

**What was added:**
- `seed_first_launch(&conn, &path)?;` called in `open()` immediately after `migrations::run(&conn)?;`
- `seed_first_launch(conn: &Connection, db_path: &PathBuf) -> SqlResult<()>` function at bottom of file

**Logic:**
1. `SELECT COUNT(*) FROM user_preferences` — if ≥ 1, return early (idempotent)
2. `std::fs::create_dir_all(db_path.parent().join("screenshots"))` — non-fatal if fails
3. `INSERT INTO user_preferences (...)` with all 12 column values explicit

**Exact column names used (verified against 001_initial_schema.sql):**
- `id` = 1 (singleton, CHECK(id = 1))
- `inactivity_timeout_seconds` = 300
- `screenshot_interval_seconds` = 900 (15 min; schema default is 60)
- `screenshot_retention_days` = 30
- `screenshot_storage_path` = NULL
- `local_timezone` = "UTC"
- `page_size` = 25 (schema default is 50)
- `process_deny_list_json` = `["keepass","1password","bitwarden","lastpass"]` (schema default, NOT `"[]"` as briefing suggested)
- `external_db_uri_stored` = 0
- `external_db_enabled` = 0 (column was missing from briefing — added explicitly)
- `notification_channels_json` = NULL (column was missing from briefing — added explicitly)
- `timer_notification_threshold_hours` = 8.0
- **NO `modified_at`** — column does not exist in schema (confirmed)

**Screenshots dir:** `{exe_dir}/screenshots/` (next to `tracey.db`), created via `db_path.parent().join("screenshots")`. Non-fatal on failure.

**cargo check result: PASS** — 19 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-16: T020/T021/T022/T025 — Timer & Time Entry IPC Commands (cargo check PASS)

**File:** `src-tauri/src/commands/timer.rs` (new file)

**What was added:**
- `timer_start` (T020), `timer_stop` (T021), `timer_get_active` (T021), `time_entry_list` (T022), `time_entry_autocomplete` (T025)
- `pub mod timer;` added to `commands/mod.rs`
- All 5 commands registered in `lib.rs` `generate_handler![]`
- `ulid = "1"` added to `Cargo.toml`

**ULID generation:** `use ulid::Ulid; fn new_id() -> String { Ulid::new().to_string() }`

**`stop_running_timer` helper pattern:**
- Private fn taking `&rusqlite::Connection` and `ended_at: &str`
- Shared by `timer_start` (auto-stop) and `timer_stop` (manual stop)
- Matches `rusqlite::Error::QueryReturnedNoRows` to distinguish "no running timer" from a real error

**Schema findings (from 001_initial_schema.sql):**
- `is_break INTEGER NOT NULL DEFAULT 0` EXISTS in `time_entries` — read from DB in `time_entry_list`, NOT hardcoded false
- `device_id TEXT NOT NULL` EXISTS in `time_entries` — briefing's INSERT was missing it; fixed to use `std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string())`
- `projects.client_id` confirmed present → two-level JOIN `projects p → clients c` is correct in `time_entry_list`
- `is_break` column confirmed, so `time_entry_list` maps `r.get::<_, bool>(9)?` directly

**NULL-safe SQL pattern:** `project_id IS ?2` with rusqlite `params![]` — when `?2 = None`, SQLite evaluates `project_id IS NULL` (correct null-safe comparison). Used in `time_entry_autocomplete` tag-lookup subquery.

**cargo check result: PASS** — 19 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-16: T023/T024/T026/T030a — Manual Create, Continue, Tick Emitter, Update (cargo check PASS)

**Files changed:**
- `src-tauri/src/commands/timer.rs` — T023, T024, T030a added; `timer_start` refactored
- `src-tauri/src/services/timer_tick.rs` — new file (T026)
- `src-tauri/src/services/mod.rs` — `pub mod timer_tick;` added
- `src-tauri/src/lib.rs` — `.setup()` wires tick loop; 3 new commands registered

**T023 — `time_entry_create_manual`**
- Validates `started_at < ended_at`
- Overlap check (skipped if `force: true`): `started_at < ?2 AND ended_at > ?1`
- INSERT uses exact schema columns including `device_id` (COMPUTERNAME env var, fallback "local") and `is_break = 0`
- 9 bound params: id, description, project_id, task_id, started_at, ended_at, device_id, created_at, modified_at

**T024 — `time_entry_continue`**
- Refactored `timer_start` to call private `insert_new_timer(conn, description, project_id, task_id, tag_ids, now) -> (id, started_at)`
- Both `timer_start` and `time_entry_continue` call `insert_new_timer`; `stopped_entry` assembled by the caller
- Borrow-checker gotcha: `tag_stmt.query_map(...)?` chain inside a block fails with E0597 because the `?` operator creates a `ControlFlow` temporary that extends `tag_stmt`'s borrow. Fix: bind collect result to named `x` before block end (`let x = ...; x`), so MappedRows is fully consumed before tag_stmt drops.

**T026 — `tracey://timer-tick` emitter**
- `services/timer_tick.rs`: `tokio::spawn` in `.setup()` hook; polls DB every 1 second
- Lock released before each `await` — MutexGuard dropped in inner block, not held across sleep
- `tauri::Emitter` + `tauri::Manager` traits both required for `app.emit()` and `app.state::<T>()`
- Emits only when a running timer exists (`ended_at IS NULL`); silent when idle

**T030a — `time_entry_update`**
- `Option<Option<String>>` for nullable field updates (absent = don't touch, `null` = clear, value = set)
- Uses custom `deserialize_option_nullable` fn with `#[serde(default, deserialize_with = "...")]` — no extra crates needed
- Overlap check excludes self: `WHERE id != ?1`
- Tag update uses delete-then-insert pattern

**cargo check result: PASS** — 19 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-15: Phase 2 Session Completion Note (Scribe)

T012 confirmed complete: first-launch init runs inside `db::open()` immediately after migrations. Creates `{exe_dir}/screenshots/` directory (non-fatal on failure) and seeds `user_preferences` with 12 explicit column values. Idempotent — guarded by `COUNT(*)` check. T011, T013, T014, T017b also complete this session. cargo check 0 errors across all tasks.

---

### 2026-03-16: T032/T033/T034 — IdleService, idle_get_status, idle_resolve (cargo check PASS)

**Files changed:**
- `src-tauri/src/commands/mod.rs` — `platform: Arc<dyn PlatformHooks + Send + Sync>` added to `AppState`; `pub mod idle;` added
- `src-tauri/src/lib.rs` — constructs `WindowsPlatformHooks` wrapped in `Arc`, passes to `AppState`; wires `idle_service::start_idle_loop` in `.setup()`; registers `idle_get_status` + `idle_resolve`
- `src-tauri/src/services/idle_service.rs` — new file (T032)
- `src-tauri/src/services/mod.rs` — `pub mod idle_service;` added
- `src-tauri/src/commands/idle.rs` — new file (T033 + T034)

**Platform hooks thread safety (Task A):**
- `platform/mod.rs` already had `Send + Sync` bounds on `PlatformHooks` trait from T017b — no change required.
- `WindowsPlatformHooks` is a zero-field struct; bounds satisfied implicitly.

**AppState platform injection (Task B):**
- `AppState` now holds `pub platform: Arc<dyn PlatformHooks + Send + Sync>`
- Constructed in `lib.rs::run()` before builder; `Arc::new(WindowsPlatformHooks)` cast to trait object
- `use platform::windows::WindowsPlatformHooks` is `#[cfg(target_os = "windows")]` — safe for current Windows-only target

**T032 — IdleService (`services/idle_service.rs`):**
- Background `tokio::spawn` loop; polls every 1 second
- Never holds `Mutex<Connection>` across `await` — conn locked and dropped in inner sync block, same pattern as `timer_tick.rs`
- Reads `inactivity_timeout_seconds` from `user_preferences` each tick (supports runtime changes)
- Idle transition (not_idle → idle): emits `tracey://idle-detected` **only** when `had_active_timer = true`
- Active return (idle → not_idle): resets state silently (decision: no prompt if no running timer on return)
- `get_current_idle_status()` public fn queries platform directly — used by `idle_get_status` command, not the loop's internal state
- `idle_started_at` computed as `Utc::now() - Duration::seconds(idle_secs)` — clock-accurate even after sleep

**T033 — `idle_get_status` (`commands/idle.rs`):**
- Reads threshold from `user_preferences`, calls `idle_service::get_current_idle_status(state.platform.as_ref(), threshold)`
- Returns `IdleStatusResponse { is_idle, idle_seconds, idle_since: Option<String> }`

**T034 — `idle_resolve` (`commands/idle.rs`):**
- Four resolution branches: `"keep"` (no-op), `"break"`, `"meeting"`, `"specify"`
- Timer stopped at `idle_started_at` (not at resolution time) — matches decision in decisions.md
- `insert_entry` helper: full 10-column INSERT with `device_id` from `COMPUTERNAME` env var (fallback `"local"`) — matches `timer.rs` exact pattern
- `is_break = TRUE` only for `"break"` resolution; false for `"meeting"` and `"specify"`
- `"specify"` validates `entry_details` present or returns error; inserts `time_entry_tags` junction rows

**cargo check result: PASS** — 17 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-16: T038/T039/T040 — hierarchy.rs, Client/Project/Task IPC Commands (cargo check PASS)

**File created:** `src-tauri/src/commands/hierarchy.rs`
**Files updated:** `commands/mod.rs` (`pub mod hierarchy;`), `lib.rs` (16 new commands in `generate_handler![]`)

**Commands implemented:** `client_list`, `client_create`, `client_update`, `client_archive`, `client_unarchive`, `client_delete`, `project_list`, `project_create`, `project_update`, `project_archive`, `project_unarchive`, `project_delete`, `task_list`, `task_create`, `task_update`, `task_delete`

**Color validation:** `color.len() == 7 && starts_with('#') && [1..].chars().all(is_ascii_hexdigit())` — no regex crate needed.

**Dynamic WHERE without injection:** For `project_list` and `client_list`, used SQL `(?1 IS NULL OR client_id = ?1) AND (?2 = 1 OR is_archived = 0)` — passes `Option<&str>` as ?1 (NULL when None) and `include_archived as i64` as ?2. Safe parameterisation, no string building.

**`client_delete` cascade sequence:**
1. COUNT projects in client → `deleted_projects`
2. COUNT tasks in those projects → `deleted_tasks`
3. COUNT time_entries where project_id or task_id points into deleted scope → `orphaned_entries`
4. UPDATE time_entries SET project_id = NULL for project matches
5. UPDATE time_entries SET task_id = NULL for task matches
6. DELETE client — FK CASCADE (ON DELETE CASCADE) removes projects then tasks automatically

**`project_delete` / `task_delete`:** NULL out time_entries references explicitly before delete; then DELETE entity. FK CASCADE handles sub-entity cleanup.

**Name conflict detection for update:** `WHERE name = ?1 AND id != ?2 AND client_id = (SELECT client_id FROM projects WHERE id = ?2)` — scoped to same client.

**List return shapes:** `client_list` returns `{ "clients": [...] }` (wrapped object per spec); `project_list` and `task_list` return bare JSON arrays (`serde_json::to_value(vec)`).

**Void return:** `project_delete` and `task_delete` return `Ok(serde_json::Value::Null)`.

**`is_archived` read as bool:** `row.get::<_, bool>(n)?` — rusqlite's `FromSql` for bool converts `INTEGER` 0→false / non-zero→true. Confirmed working.

**Inline params for simple id-only commands:** `client_archive(state, id: String)` style used (not a wrapper struct) — cleaner for 1-field inputs.

**cargo check result: PASS** — 18 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-17: T043/T044/T045/T046/T048 — screenshot_service.rs (cargo check PASS)

**Files created/updated:**
- `src-tauri/src/services/screenshot_service.rs` — new file
- `src-tauri/src/services/mod.rs` — `pub mod screenshot_service;` added
- `src-tauri/Cargo.toml` — NO changes needed (all required windows features already present)

**Briefing corrections applied (DO NOT REPEAT):**
- Import: `use crate::commands::AppState;` — never `use crate::commands::mod::AppState;` (invalid syntax)
- `PlatformHooks::get_foreground_window_info` returns `Option<WindowInfo>` — no `.ok()` call needed
- `WindowInfo` field is `title` (not `window_title`) — always check `platform/mod.rs` for exact struct shape
- `screenshots` table has NO `created_at` column — 7 columns: `id, file_path, captured_at, window_title, process_name, trigger, device_id`
- Use `tauri::async_runtime::spawn_blocking` (not `tokio::task::spawn_blocking`) — rule: never use tokio directly
- `query_map` fix in `cleanup_expired`: use `match stmt.query_map(...) { Ok(mapped) => mapped.filter_map(...).collect(), Err(_) => return }` — the `.unwrap_or_else(|_| Box::new(std::iter::empty()))` pattern does NOT compile (type mismatch)
- `HBITMAP → HGDIOBJ` conversion: `HGDIOBJ(hbm.0)` — both wrap `*mut core::ffi::c_void` in windows 0.58
- GDI cleanup: `DeleteObject(HGDIOBJ(hbm.0))` and `DeleteDC(hdc_mem)` return `BOOL` (`#[must_use]`); suppress with `let _ =`

**E0597 State borrow lifetime fixes:**
- `match state.db.lock() { ... }` as last expression in a block → bind to `let x = ...; x` pattern so the `Result` temporary drops before `state` drops
- `if let Ok(conn) = state.db.lock()` → add `;` after the `if let` block close so the `Result` temporary drops before `state` drops at the outer block end
- This pattern applies whenever `app.state::<T>()` binding is in same scope as a `lock()` match at block-end position

**GDI capture notes:**
- `biCompression: BI_RGB.0` — `BITMAPINFOHEADER.biCompression` is `u32` in windows 0.58; `BI_RGB.0` extracts the `u32` from `BI_COMPRESSION` newtype ✓
- Production capture: `GetDesktopWindow` in `Win32_UI_WindowsAndMessaging`; all GDI ops in `Win32_Graphics_Gdi`
- BGRA→RGB: `chunks_exact(4).flat_map(|bgra| [bgra[2], bgra[1], bgra[0]])` — reverses channel order for device (GDI bottom-up stores BGR, biHeight<0 makes it top-down)
- `image::DynamicImage::from(resized)` via `Into` — `RgbImage` into `DynamicImage` before `.write_to()`

**NOT wired into lib.rs** — Reese-B handles `start_screenshot_loop` wiring.

**cargo check result: PASS** — 24 dead_code warnings (all expected stubs, none new errors), zero errors.

---

### 2026-03-17: T047 — screenshot.rs IPC Commands + lib.rs Wiring (cargo check PASS)

**Files created/updated:**
- `src-tauri/src/commands/screenshot.rs` — new file (T047)
- `src-tauri/src/commands/mod.rs` — `pub mod screenshot;` added
- `src-tauri/src/lib.rs` — 2 commands + service loop wired

**Commands implemented:**

- `screenshot_list(request: ScreenshotListRequest) -> Result<Vec<ScreenshotItem>, String>`
  - Query: `SELECT id, file_path, captured_at, window_title, process_name, trigger FROM screenshots WHERE captured_at >= ?1 AND captured_at <= ?2 ORDER BY captured_at DESC`
  - No `created_at` in SELECT — column does not exist in schema (7 columns: id, file_path, captured_at, window_title, process_name, trigger, device_id)

- `screenshot_delete_expired() -> Result<serde_json::Value, String>`
  - Reads `screenshot_retention_days` from `user_preferences` (defaults to 30 on error)
  - Collects `file_path` values for expired rows, calls `std::fs::remove_file` for each (ignores individual failures)
  - DELETEs DB rows; returns `{ "deleted_count": N }`

**lib.rs changes:**
- `.setup()` now calls `services::screenshot_service::start_screenshot_loop(app.handle().clone())` after `start_idle_loop`
- `generate_handler![]` extended with `commands::screenshot::screenshot_list` and `commands::screenshot::screenshot_delete_expired`

**cargo check result: PASS** — 16 dead_code warnings (all pre-existing stubs), zero errors.

---

### 2026-03-17: Cross-Agent Note (from Root T049) — IPC Wrapper Pattern Confirmed

- Root T049 found `screenshot_list` was being called with bare `new { from, to }` instead of `new { request = new { from, to } }`. **Established Phase 2 convention:** all IPC commands taking a Rust struct as input must be invoked from Blazor with `new { request = ... }` wrapper. This is how `window.__TAURI_INTERNALS__.invoke` maps named args to the Rust `#[tauri::command]` struct parameter.
- Single-field inline commands (`client_archive(state, id: String)` style) do NOT use a wrapper — the arg name matches the parameter directly.
- Root corrected `ErrorPayload` from `{ component, message }` to `{ component, event, error }` per `tracey://error` contract. Future Rust event emissions must use this payload shape.
