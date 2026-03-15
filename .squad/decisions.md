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
**Why:** Tags are not activity data. Placing them in `activity.rs` is semantically wrong and will confuse future readers.

## 2026-03-15: T082 — No Split (Option B)
**By:** Vincent Verweij (via Coordinator)
**What:** T082 (Win32 foreground window polling loop) stays in the Final Phase as originally planned. Phase 6 (US4 — Screenshot Timeline) delivers interval-based screenshots only. Window-change-triggered screenshots arrive in the Final Phase when T082 is implemented alongside T083 (ActivityRecord writes) and the sync queue. Shaw's US4 E2E tests do NOT need to cover window-change-triggered captures during Phase 6.
**Why:** Accepted gap. Interval-based captures are sufficient for Phase 6 delivery. The window-change trigger is a Final Phase enhancement.
