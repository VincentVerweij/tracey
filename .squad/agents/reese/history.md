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
