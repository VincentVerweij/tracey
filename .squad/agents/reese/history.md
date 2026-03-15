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

### 2026-03-15: Phase 2 Session Completion Note (Scribe)

T012 confirmed complete: first-launch init runs inside `db::open()` immediately after migrations. Creates `{exe_dir}/screenshots/` directory (non-fatal on failure) and seeds `user_preferences` with 12 explicit column values. Idempotent — guarded by `COUNT(*)` check. T011, T013, T014, T017b also complete this session. cargo check 0 errors across all tasks.
