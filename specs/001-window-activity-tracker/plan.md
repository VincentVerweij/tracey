# Implementation Plan: Window Activity Timetracking Tool

**Branch**: `001-window-activity-tracker` | **Date**: 2026-03-14 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-window-activity-tracker/spec.md`

## Summary

Build a portable, keyboard-first time-tracking desktop application that silently monitors which application windows are active, captures periodic screenshots, and surfaces an idle-return prompt when the user steps away. The tech stack is **Tauri 2.0** (Rust native layer on Windows 11) with a **C# Blazor .NET 10** WebView2 frontend, using **BlazorBlueprint.Components** for the UI component library. Data is stored in a local SQLite cache and optionally synchronised to a user-supplied external database (Postgres/Supabase). UI regression testing is handled by Playwright.

## Technical Context

**Language/Version**: Rust 1.77.2+ (Tauri native layer), C# / .NET 10 (Blazor WebAssembly or Blazor Hybrid inside Tauri WebView2)  
**Primary Dependencies**:
- `tauri 2.0` — app shell, IPC, capabilities
- `tauri-plugin-system-idle` — OS idle time query (replaces manual GetLastInputInfo plumbing)
- `windows` crate 0.58+ with features: `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_SystemInformation`, `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`, `Win32_System_ProcessStatus`, `Win32_Foundation`
- `image` crate — 50 % resize with Triangle filter, JPEG encode
- `serde` / `serde_json` — IPC serialization
- BlazorBlueprint.Components NuGet — Blazor UI component library
- Microsoft.Data.Sqlite — local SQLite cache
- Npgsql or Supabase client — optional external DB sync  

**Storage**: SQLite (local, in app data dir); external Postgres/Supabase via user-supplied connection URI (optional)  
**Testing**: `cargo test` (Rust unit tests), `dotnet test` (xUnit for Blazor business logic), Playwright (E2E UI regression against complete app)  
**Target Platform**: Windows 11 primary; architecture must allow per-OS swap-in of native hooks  
**Project Type**: Desktop application (portable executable, no installer required)  
**Performance Goals**: Background tracing < 2 % CPU over any 10-second window; memory < 150 MB RSS; user-initiated queries return first result < 500 ms at p95 (local dataset ≤ 1 M events); app ready within 5 seconds of launch  
**Constraints**: Portable executable — no admin rights, no registry, no installer; offline-capable with sync-on-reconnect; screenshots stored locally only, never transmitted; idle-return prompt appears within 3 seconds of user activity resuming  
**Scale/Scope**: Single-user, multi-device (shared connection URI); up to ~1 M window-activity events in local DB over a year; screenshot retention up to 30 days rolling

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Code Quality | ✅ PASS | Functions / modules will be single-responsibility; SRP enforced in Rust commands and Blazor services. Max cyclomatic complexity 10 per function is achievable given the modular design. |
| II. Testing Standards | ✅ PASS | Tests written before implementation (TDD). Playwright covers all user stories (E2E). `cargo test` covers Rust business logic at ≥ 80 % branch coverage. Every user story has at least one E2E scenario mapped. |
| III. UX Consistency | ✅ PASS | BlazorBlueprint.Components provides consistent component primitives. Tone-of-voice guide will be created in `docs/ux/tone.md`. All dialogs, prompts, and confirmations follow the same pattern. |
| IV. Performance | ✅ PASS | Background tracing loop is async; GDI screenshot pipeline runs in `spawn_blocking`; disk writes are batched. Performance benchmarks in CI are planned. |
| V. Privacy First | ✅ PASS | Screenshots stored locally only (never synced). Window activity written to external DB only with user-configured connection URI. No built-in auth/account system. No third-party telemetry. Delete-all-data operation required (< 60 s). |
| VI. Security | ✅ PASS | Tauri capabilities system enforces least privilege. All IPC inputs validated in Rust handlers. Connection URI stored using platform keychain (not plain text). No unauthenticated local endpoints. Threat model recorded in [research.md](research.md). |
| VII. Observability | ✅ PASS | Structured JSON logs throughout (timestamp, level, component, event, trace_id). Health endpoint exposed locally. Metrics: events captured, events dropped, flush latency, memory. No PII in logs. |

**Violations requiring justification**: None — all gates pass.

### Post-Design Re-check (after Phase 1)

| Principle | Post-Design Status | Design decisions that reinforce or affect this principle |
|-----------|-------------------|-------------------------------------------------------|
| I. Code Quality | ✅ PASS | IPC command layer cleanly split from service layer in Rust. C# `INotificationChannel` abstraction keeps channel implementations isolated. SQLite migration runner is sequential and simple. |
| II. Testing Standards | ✅ PASS | Playwright test files map 1:1 to all 9 user stories. GDI capture test double defined (`#[cfg(feature="test")]`). Fuzzy match scored by custom C# algorithm — fully unit-testable. |
| III. UX Consistency | ✅ PASS | All modals (idle-return, overlap warning, tag delete warning, client delete confirmation) use the same BlazorBlueprint modal component. |
| IV. Performance | ✅ PASS | SQLite WAL mode enabled. Window activity batch-flushed every 60 seconds. Screenshot pipeline in `spawn_blocking`. Sync runs every 30 seconds on background task. |
| V. Privacy First | ✅ PASS | Process deny-list stored in `user_preferences.process_deny_list_json`; applied at collection boundary before any DB write. `logo_path` is never synced to external DB. |
| VI. Security | ✅ PASS | screenshot storage path canonicalized and validated in Rust before write (path traversal mitigation). Connection URI in OS keychain (`keyring` crate). `health_get` IPC command is local-only. |
| VII. Observability | ✅ PASS | `health_get` IPC command defined in contracts (returns running state, last-write, events/sec, memory, errors). Push events via `tracey://sync-status-changed` and `tracey://error` events. |

**Post-design result**: All gates continue to pass. No new violations introduced.

## Project Structure

### Documentation (this feature)

```text
specs/001-window-activity-tracker/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── ipc-commands.md
│   └── sync-api.md
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src-tauri/                          # Rust / Tauri native layer
├── Cargo.toml
├── tauri.conf.json
├── capabilities/
│   └── default.json                # Tauri capability grants
└── src/
    ├── main.rs                     # Entry point
    ├── lib.rs
    ├── commands/                   # Tauri IPC command handlers
    │   ├── mod.rs
    │   ├── activity.rs             # Window activity monitoring
    │   ├── screenshot.rs           # Screenshot capture pipeline
    │   ├── idle.rs                 # Idle detection
    │   ├── timer.rs                # Timer control
    │   └── sync.rs                 # External DB sync
    ├── services/                   # Domain services (business logic)
    │   ├── mod.rs
    │   ├── activity_tracker.rs
    │   ├── screenshot_service.rs
    │   ├── idle_service.rs
    │   ├── sync_service.rs
    │   └── notification_service.rs
    ├── models/                     # Rust data structures
    │   └── mod.rs
    ├── db/                         # SQLite local cache
    │   ├── mod.rs
    │   └── migrations/
    ├── platform/                   # Platform-specific implementations
    │   ├── mod.rs
    │   └── windows.rs              # Windows-specific Win32 hooks
    └── notifications/              # Notification channels
        ├── mod.rs
        ├── email.rs
        └── telegram.rs

src/                                # C# Blazor .NET 10 frontend
├── Tracey.sln
├── Tracey.App/                     # Blazor application project
│   ├── Tracey.App.csproj           # References BlazorBlueprint.Components NuGet
│   ├── Program.cs
│   ├── App.razor
│   ├── wwwroot/
│   ├── Components/                 # Reusable Blazor components
│   │   ├── QuickEntryBar.razor
│   │   ├── TimeEntryList.razor
│   │   ├── ScreenshotTimeline.razor
│   │   ├── IdleReturnModal.razor
│   │   └── ...
│   ├── Pages/                      # Route-level pages
│   │   ├── Dashboard.razor
│   │   ├── Projects.razor
│   │   ├── Tags.razor
│   │   ├── Settings.razor
│   │   └── Timeline.razor
│   └── Services/                   # C# services (IPC wrappers, state)
│       ├── TauriIpcService.cs
│       ├── TimerStateService.cs
│       ├── FuzzyMatchService.cs
│       └── ...
└── Tracey.Tests/                   # xUnit tests for Blazor services
    └── Tracey.Tests.csproj

tests/
└── e2e/                            # Playwright E2E tests
    ├── playwright.config.ts
    ├── fixtures/
    └── specs/
        ├── timer.spec.ts           # User Story 1
        ├── idle-detection.spec.ts  # User Story 2
        ├── projects.spec.ts        # User Story 3
        ├── screenshot-timeline.spec.ts  # User Story 4
        ├── quick-entry.spec.ts     # User Story 5
        ├── tags.spec.ts            # User Story 6
        ├── notifications.spec.ts   # User Story 7
        └── cloud-sync.spec.ts      # User Story 8

docs/
└── ux/
    └── tone.md                     # UX tone-of-voice guide (required by Constitution III)
```

**Structure Decision**: Hybrid Tauri + Blazor desktop app. The Rust layer (`src-tauri/`) owns all OS interactions (window tracking, idle detection, GDI screenshots, notifications). The C# Blazor layer (`src/`) owns all UI and user-facing business logic. Communication goes exclusively through Tauri IPC commands. Playwright E2E tests drive the full stack from the UI layer down.
