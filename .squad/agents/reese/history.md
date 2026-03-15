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
