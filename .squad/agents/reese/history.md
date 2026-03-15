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
