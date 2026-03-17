# Reese — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Tauri 2.0 (Rust, Windows 11) + Blazor WASM .NET 10 + SQLite (WAL, `rusqlite`) + optional Postgres/Supabase
- **My files:** `src-tauri/src/` (commands/, models/, db/, platform/)
- **Contracts:** `specs/001-window-activity-tracker/contracts/ipc-commands.md` — source of truth for all IPC commands
- **Created:** 2026-03-15

### Critical Rules
- `GetTickCount64` only — never `GetTickCount` (32-bit rollover after ~49 days)
- `HWND` null: `hwnd == HWND(std::ptr::null_mut())` — not `== 0`
- `GetDesktopWindow` is in `Win32_UI_WindowsAndMessaging`; GDI ops in `Win32_Graphics_Gdi`
- Screenshot pipeline: `tauri::async_runtime::spawn_blocking` always — never `tokio::task::spawn_blocking` directly
- `fs:allow-write-file` (singular noun) — plural fails silently
- `logo_path` on Client: NEVER synced to external DB
- Process deny-list: applied at Rust collection boundary BEFORE any DB write
- E0597 fix: bind `query_map` result to named variable before block end (so `MappedRows` is consumed before `stmt` drops)
- MutexGuard NEVER held across `.await` — inner block pattern required
- `HBITMAP → HGDIOBJ`: `HGDIOBJ(hbm.0)`; `DeleteObject`/`DeleteDC` return `#[must_use]` BOOL → suppress with `let _ =`
- `use crate::commands::AppState;` — never `crate::commands::mod::AppState` (invalid)
- `WindowInfo` field is `title` (not `window_title`) — check `platform/mod.rs` for exact shape
- `biCompression = BI_RGB.0` — `BITMAPINFOHEADER.biCompression` is `u32`; `.0` extracts from newtype
- `insert_new_timer` and similar `#[tauri::command]` fns cannot call each other directly — extract private helpers

### Files Implemented (Phases 1–6)
- `platform/windows.rs`: `WindowsPlatformHooks` — `get_foreground_window_info`, `get_idle_seconds`
- `db/migrations.rs`: WAL+FK PRAGMA, `schema_migrations`, embedded SQL via `include_str!`
- `services/logger.rs`: JSON-line stderr logger, `DENY_LIST: OnceLock`, log macros
- `services/timer_tick.rs`: 1s poll; MutexGuard dropped before `.await`; emits `tracey://timer-tick`
- `services/idle_service.rs`: 1s poll; `tracey://idle-detected` only when `had_active_timer=true`; silent when no timer
- `services/screenshot_service.rs`: GDI full capture (Triangle 50% scale, JPEG), `start_screenshot_loop`
- `commands/timer.rs`: All timer + time_entry commands; ULID IDs; `stop_running_timer` shared helper
- `commands/idle.rs`: `idle_get_status`, `idle_resolve`; `ended_at = idle_started_at` (not resolution time)
- `commands/screenshot.rs`: `screenshot_list`, `screenshot_delete_expired`
- `commands/hierarchy.rs`: Full client/project/task CRUD (16 commands)
- `lib.rs`: All commands registered + service loops wired in `.setup()`

### IPC Return Shapes (authoritative)
- `client_list` → `{ "clients": [...] }` (wrapped)
- `project_list` → `{ "projects": [...] }` (wrapped — Bug 1 fix; old Phase 5 doc said "bare array" — WRONG)
- `task_list` → `{ "tasks": [...] }` (wrapped — Bug 2 fix)
- `project_delete` → `{ "deleted_tasks": N, "orphaned_entries": N }` (Bug 1 fix)
- `time_entry_autocomplete` suggestion: has `is_orphaned: bool`
- IPC wrapper pattern: Blazor calls `new { request = new { ... } }` for Rust struct params; single-field inline commands pass arg name directly
- `ErrorPayload` shape: `{ component, event, error }` — not `{ component, message }`

### Config (current state)
- `tauri.conf.json`: `assetProtocol { enable: true, scope: ["**"] }`; CSP includes `http://asset.localhost`
- `capabilities/default.json`: `"fs:allow-read-file"` + `"fs:allow-write-file"` (both required)
- `Cargo.toml` tauri features: `["protocol-asset"]` (required when assetProtocol.enable = true)

### hierarchy.rs — Key Patterns
- Color validation: `color.len() == 7 && starts_with('#') && [1..].chars().all(is_ascii_hexdigit())` — no regex crate
- Dynamic NULL WHERE: `(?1 IS NULL OR client_id = ?1) AND (?2 = 1 OR is_archived = 0)` — safe parameterisation
- `client_delete` sequence: COUNT orphans → NULL time_entries refs → DELETE client (FK CASCADE cleans children)
- Name conflict for update: `WHERE name = ?1 AND id != ?2 AND client_id = (SELECT client_id FROM projects WHERE id = ?2)`
- `is_archived` read as bool: `row.get::<_, bool>(n)?` — rusqlite converts INTEGER 0→false

### screenshots Table
- 7 columns only: `id, file_path, captured_at, window_title, process_name, trigger, device_id` — no `created_at`
- GDI BGRA→RGB: `chunks_exact(4).flat_map(|bgra| [bgra[2], bgra[1], bgra[0]])`
- `screenshot_delete_expired` is manually invoked (IPC call from Blazor); no background schedule yet

---

## Learnings

### 2026-03-17: Bug Fixes — JSON shape, assetProtocol, capabilities (cargo check PASS)

**Files updated:**
- `commands/hierarchy.rs`: `project_list` → `{ "projects": [...] }`; `task_list` → `{ "tasks": [...] }`; `project_delete` → `{ "deleted_tasks": N, "orphaned_entries": N }`
- `tauri.conf.json`: CSP + `assetProtocol { enable: true, scope: ["**"] }` block added
- `capabilities/default.json`: `"fs:allow-read-file"` added
- `Cargo.toml`: `"protocol-asset"` added to tauri features

**Key lesson:** `assetProtocol.enable=true` in `tauri.conf.json` MUST be paired with `"protocol-asset"` in Cargo.toml tauri features — build script validates the match and errors if they diverge.

**cargo check: PASS** — 15 dead_code warnings (all pre-existing), 0 errors.

---

## Archived Sessions (condensed)

### 2026-03-16: T038/T039/T040 — hierarchy.rs (cargo check PASS)
Created `commands/hierarchy.rs` with 16 client/project/task CRUD commands. Color validation inline (no regex). Dynamic SQL NULL trick for filters. `client_delete` cascade sequence. `project_delete`/`task_delete` explicit NULL-out. Name conflict detection via scoped WHERE. Registered in `mod.rs` and `lib.rs`. See IPC shapes in Core Context above.

### 2026-03-17: T043–T048 — screenshot_service.rs (cargo check PASS)
Created `services/screenshot_service.rs` with GDI capture pipeline + `start_screenshot_loop`. Critical corrections: no `created_at` column, `tauri::async_runtime::spawn_blocking`, `HGDIOBJ(hbm.0)`, `let _ =` for BOOL returns, `match stmt.query_map(...)` pattern for cleanup. E0597 state borrow lifetime pattern documented.

### 2026-03-17: T047 — screenshot.rs IPC Commands + lib.rs wiring (cargo check PASS)
Created `commands/screenshot.rs` with `screenshot_list` (7-column SELECT) and `screenshot_delete_expired` (reads retention days, removes files, DELETEs rows). Wired `start_screenshot_loop` in `.setup()`. Two commands added to `generate_handler![]`.

### 2026-03-17: Cross-Agent Note (from Root T049)
IPC wrapper pattern confirmed: `new { request = new { ... } }`. `ErrorPayload` corrected to `{ component, event, error }`. Both documented in Core Context above.

