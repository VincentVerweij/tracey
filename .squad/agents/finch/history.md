# Finch — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Tauri 2.0 (Rust, Windows 11) + Blazor WASM .NET 10 (C#, WebView2) + BlazorBlueprint.Components + SQLite (WAL) + optional Postgres/Supabase
- **Team:** Finch (Lead), Reese (Rust), Root (Blazor), Shaw (QA), Fusco (DevOps), Control (Security), Leon (Data)
- **Spec files:** `specs/001-window-activity-tracker/` → spec.md (9 user stories), plan.md, tasks.md, data-model.md, research.md
- **Contracts:** `contracts/ipc-commands.md`, `contracts/sync-api.md`
- **Constraints:** Portable exe, no installer, no admin, no registry. Screenshots local only. Blazor WASM (no server process).
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup
- Team assembled. 7 domain agents + Scribe + Ralph.
- All architecture decisions seeded in `.squad/decisions.md`.
- Constitution check in plan.md: all 7 principles pass (pre- and post-design).
- Tasks in 7+ phases (tasks.md): Phase 1 (setup) → Phase 2 (foundational) → Phases 3-9 (user stories US1-US9).
- Key reviewer gates: Shaw (test coverage), Control (security), Finch (architecture/IPC).
- IPC contract in `contracts/ipc-commands.md` is the source of truth for all Tauri commands.

### 2026-03-15: Phase 1 + 2 Kickoff Review

**IPC Contract Gap Fixed**: `time_entry_autocomplete` response was missing `is_orphaned: boolean`. Field added via inbox decision. Reese (T025) and Root (T028) must both use this field. Contract is now authoritative.

**T082 Ordering Bug**: Win32 polling loop (foreground window detection) placed in Final Phase, but Phase 6 (US4) requires it for window-change-triggered screenshots (T046). Decision filed in inbox for Vincent: split T082 into T082a (polling loop → Phase 2) and T082b (ActivityRecord writes → Final Phase). If Vincent approves, Reese carries T082a as early Phase 2 work and it unlocks US4 fully.

**PlatformHooks Trait Simplified**: T017b originally defined three methods including `trigger_screenshot_capture()`. This created an ownership ambiguity (T082 says ActivityTracker calls ScreenshotService directly, bypassing PlatformHooks). Decision: trait reduced to two methods — `get_foreground_window_info()` and `get_idle_seconds()`. Capture triggering stays in ActivityTracker/ScreenshotService layer.

**T058 File Routing**: Tag commands incorrectly routed to `activity.rs` in T058. Corrected to `tags.rs`. Leon and Reese to note.

**research.md Section 5 Inconsistency**: research.md still says "Blazor Server" as the selected approach. It's a historical artifact from the options phase. Blazor WASM is the final decision (decisions.md, tasks.md). Not a blocker.

**Task Ownership Confirmed**:
- Phase 1: T001(Reese), T002(Root), T003(Shaw), T004(Shaw), T005(Root), T017b(Reese)
- Phase 2: T006-T014(Reese, with Leon on T008/T009 schema), T015-T017(Root), T007 gates on Control review
- TDD gate: Shaw writes T018+T019 and they MUST FAIL before any Phase 3 implementation begins

**Control gate**: T007 (Tauri capabilities) requires Control review before merge. Confirmed permission slug: `fs:allow-write-file` (singular). Wrong slug fails silently.

**Phase 2 strict ordering**: T006 → T007 → T008 → T009 → T012. Everything else in Phase 2 is parallelizable once scaffolding exists.

### 2026-03-15: Phase 2 Adjudications

**Adj 1 — `health_get` shape**: Contract was already correct. `{running, last_write_at, events_per_sec, memory_mb, active_errors, pending_sync_count}` is authoritative. T014 briefing shape `{status, db_open, version, platform, uptime_seconds}` was wrong. Reese's Rust struct and Root's C# record are both aligned. No changes.

**Adj 2 — `preferences_get` / `preferences_update` gap**: Both commands were implemented by Reese (T013) and called by Root (T015) but missing from `ipc-commands.md`. Added "Settings / Preferences Commands" section. Canonical field names from Rust: `local_timezone` (not `timezone`), `page_size` (not `entries_per_page`). Root's C# `UserPreferences` record has three wrong `JsonPropertyName` attributes — **blocking bug** for Settings UI. Root must fix before T015 is done.

**Adj 3 — `sync_queue` columns**: `table_name`, `record_id`, `queued_at` canonical (matching DDL). `payload` NOT added — re-reading from local DB at sync time is correct for last-write-wins. `attempts INTEGER NOT NULL DEFAULT 0` ADDED via migration 003. Model, migrations runner, and data-model.md updated.

**Also fixed**: `is_orphaned` field added to `time_entry_autocomplete` contract output — was in decisions.md and Root's C# but not in the contract file itself.

**Phase 2 checkpoint**: `cargo tauri dev` will NOT work as-is. `devUrl = "http://localhost:5000"` requires a running Blazor server but `beforeDevCommand` is empty. Fix: either set `beforeDevCommand` to launch `dotnet run`, or remove `devUrl` and publish Blazor first with `dotnet publish`. Full detail in `.squad/decisions/inbox/finch-adjudications-phase2.md`.

### 2026-03-18: Phase 9 — US7 Long-Running Timer Notifications

**Pre-implementation findings**:
- `tracey://notification-sent` was ALREADY in the IPC contract (no amendment needed).
- `timer_notification_threshold_hours` was ALREADY in the C# records.
- **Gap found**: `notification_channels_json` was in the IPC contract but MISSING from `UserPreferences` and `PreferencesUpdateRequest`. Fixed by Root.
- MailKit 4.15.1 already in csproj — but SMTP (raw TCP sockets) is unavailable in Blazor WASM. Email channel correctly implemented as a stub throwing `NotSupportedException`.
- TauriEventService already handled `tracey://notification-sent` but only from Rust/JS bridge. Added `RaiseNotificationSent()` public method for C#-originated events.

**Architecture decisions (see finch-phase9-notifications-arch.md)**:
- AD-1: `SendAsync` takes `NotificationChannelSettings` as parameter. Keeps channels stateless and testable.
- AD-2: `IHttpClientFactory` for TelegramNotificationChannel (avoids singleton-holds-scoped-HttpClient issue).
- AD-3: duplicate-notification guard via `_notifiedForEntryId`.
- AD-4: `notification_channels_json` stores JSON array of `{channel_id, enabled, config}` objects.
- AD-5: EmailNotificationChannel is a WASM stub; MailKit reserved for future Tauri IPC relay.
- AD-6: No Rust changes needed for Phase 9.

**Tech debt noted**:
- `FakeTauriIpcService` in tests uses `new` (hides, not overrides) on `PreferencesGetAsync`. Will silently break if the method signature changes. Resolve by extracting `ITauriIpcService` interface in a future phase.
- `BackgroundService` singleton in WASM injecting scoped `ITimerStateService` is fine in WASM (single root scope) but would be incorrect in server-side Blazor. Document this assumption if stack ever changes.

**Constitution gate**: all 7 principles pass for Phase 9.

### 2026-03-19: Phase 4 Final — BbDialog Portal Failure + Pipeline Confirmation

**BbDialog unusable (net10 TFM mismatch):** `BbDialog` (BlazorBlueprint 3.5.2, built for net8.0) relies on `BbPortalHost` registering with `PortalService`. On net10.0, `BbPortalHost` silently fails to register. The RZ10012 warning (previously classified harmless) is NOT harmless — at runtime every `BbDialog.Open()` throws "No <PortalHost /> detected". **Rule:** Never use `BbDialog` or any BlazorBlueprint portal-based overlay in this project. Use plain `@if (_isVisible)` HTML overlay with `position: fixed; inset: 0; z-index: 9999` instead.

**Escaped quotes forbidden in Razor lambda string interpolations:** Using `\"` inside a Razor `@code` lambda or inline `@($"...")` expression causes build errors RZ1027/CS1039/CS1073. Hoist any quoted string expressions to local variables before the lambda. This is distinct from the existing `@bind="..."` backslash-escape issue (Phase 9 build fix) — it applies to `@code` lambdas specifically.

**IdleReturnModal pattern:** Now uses plain HTML overlay with `@if (_isVisible)` guard and `<div class="idle-modal-backdrop">`. All four resolutions (break, meeting, specify, keep) confirmed working end-to-end.

**Diagnostics pattern confirmed:** 4-layer diagnostics (Rust `eprintln!`, JS `console.log`, C# `Console.WriteLine` at RouteEvent, C# at component handler) are the canonical approach for debugging Tauri→JS→C# event pipeline issues. In Phase 4, all 4 layers passed — the sole failure was BbDialog rendering.

### 2026-03-22: Local CI Parity Run

**Task:** Ran all ci.yml steps (Jobs 1–3) locally to surface and fix failures before they reach GitHub CI.

**Single failure found:** `TimerStateServiceTests` — 16 tests failing with `NotImplementedException`. Root cause: `TimerStateServiceStub` was a TDD gate stub (intentionally throwing) written before T020. Root completed T020 (real `TimerStateService` fully implemented) but the stub was never updated to a real working test double.

**Fix applied:** Replaced the throwing stub with an in-memory state-machine test double that maintains all state locally without Tauri IPC. All 16 failing tests now pass. The real `TimerStateService` in the app is unchanged.

**Final state:** 55/55 .NET tests pass, 4/4 Rust tests pass. cargo check, cargo clippy, dotnet format, cargo audit all clean.

**VS Code tasks added:** `cargo test (Rust)` and `dotnet test (.NET)` were missing from `.vscode/tasks.json`. Both added. All 5 CI gates now have corresponding `Run Task` entries.

**Iteration count:** 1 failure, 1 fix, confirmed clean on re-run.

**Decision filed:** `.squad/decisions/inbox/finch-local-ci-parity-2026-03-22.md`

**TDD gate pattern — lesson:** Once an implementation task (T0xx) is done, the corresponding TDD-gate stub MUST be replaced with a real working test double or the tests will remain permanently red in CI. Shaw should verify stubs are updated when tasks are marked complete.
