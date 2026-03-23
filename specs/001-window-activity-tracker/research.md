# Research: Window Activity Timetracking Tool

**Phase**: 0 — Outline & Research  
**Branch**: `001-window-activity-tracker`  
**Date**: 2026-03-14  
**Status**: Complete — all NEEDS CLARIFICATION resolved

---

## 1. Idle Detection on Windows 11

### Decision
Use `tauri-plugin-system-idle` to query system-wide idle time. This plugin wraps `GetLastInputInfo` + `GetTickCount64` internally and exposes the result as a Tauri IPC command.

**Critical implementation details (validated on Windows 11)**:
- `GetLastInputInfo` + `GetTickCount64` correctly detects system-wide idle time.
- `GetTickCount64` **must** be used instead of `GetTickCount` to avoid 32-bit rollover after ~49 days of uptime.
- Required Cargo features: `Win32_UI_Input_KeyboardAndMouse`, `Win32_System_SystemInformation`.
- If implementing manually (for testing or fallback), null-check for `HWND` uses `std::ptr::null_mut()`, not `== 0`.

### Rationale
`tauri-plugin-system-idle` eliminates manual Win32 boilerplate, is maintained alongside Tauri, and has been tested to work correctly on Windows 11.

### Alternatives Considered
- **Manual GetLastInputInfo**: Works but duplicates what the plugin already does securely.
- **Global keyboard/mouse hooks (SetWindowsHookEx)**: Requires admin rights in some configurations and processes every input event, violating the < 2 % CPU budget.

---

## 2. Active Window Detection on Windows 11

### Decision
Use `GetForegroundWindow` → `GetWindowThreadProcessId` → `GetModuleFileNameExW` from the `windows` crate 0.58+ to silently query the OS for active window title and process name. No hooks, no elevated permissions.

**Critical implementation details (validated on Windows 11)**:
- `GetForegroundWindow` silently queries foreground window; no hooks needed.
- `HWND` in `windows` crate 0.58+ wraps a raw pointer — null check **must** use `std::ptr::null_mut()`, not `== 0`.
- Required Cargo features: `Win32_UI_WindowsAndMessaging`, `Win32_System_ProcessStatus`, `Win32_Foundation`.
- This detection runs on a background polling loop (e.g., every 1 second) inside `tauri::async_runtime::spawn`.

### Rationale
The polling approach is simple, reliable, and requires no elevated privileges. The 1-second poll interval is lightweight (< 1 ms per check).

### Alternatives Considered
- **SetWinEventHook**: Event-driven; harder to cancel cleanly on app shutdown.
- **WMI polling**: Much heavier; requires COM init.

---

## 3. Screenshot Capture Pipeline on Windows 11

### Decision
Capture the monitor where the active window lives using: `MonitorFromWindow` → `GetMonitorInfo` to find the correct monitor bounding rect, then `GetDesktopWindow` + `GetWindowDC` + `BitBlt` + `GetDIBits` to capture. Scale to 50 % with the `image` crate using the **Triangle** filter. Encode as JPEG and write to disk.

**Critical implementation details (validated on Windows 11)**:
- `GetDesktopWindow` is in `Win32_UI_WindowsAndMessaging`, not `Win32_Graphics_Gdi` — import must come from the correct module.
- The entire capture + resize + encode + write pipeline **must** run inside `tauri::async_runtime::spawn_blocking`; running on the main thread causes "Not Responding" / hang.
- Scope the `BitBlt` to the target monitor's bounding rect (from `GetMonitorInfo`), not the full virtual desktop and not hard-coded primary monitor.
- Triangle resize filter is preferred over Lanczos3 — significantly faster with negligible quality loss at 50 % scale.
- Tauri filesystem permission: use `fs:allow-write-file` (singular noun), **not** `fs:allow-write-files` (plural).
- Required Cargo features: `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`, `Win32_Foundation`.

### Rationale
GDI-based capture is validated to work on Windows 11 without elevation. The `spawn_blocking` requirement is non-negotiable for desktop responsiveness.

### Alternatives Considered
- **Windows.Graphics.Capture API (WGC, DXGI)**: More modern; supports hardware acceleration; requires COM init; significantly more complex for a thumbnail capture pipeline.
- **Full virtual-desktop BitBlt**: Captures all monitors; wasteful for multi-monitor setups; not what the user is working on.

---

## 4. Screenshot Debounce

### Decision
Apply a **2-second debounce** to window-change-triggered screenshots. If the window/title changes more frequently than 2 seconds (e.g., a browser tab title polling), the screenshot timer is reset but no capture fires until stability.

### Rationale
FR-012 explicitly requires debouncing, including the 2-second default period. This prevents excessive captures from tab-title-updating browsers.

### Alternatives Considered
- **1-second debounce**: Too aggressive for some browsers.
- **5-second debounce**: Too long; misses genuine rapid app switches.

---

## 5. Blazor Integration with Tauri 2.0 (WebView2)

### Decision
Use **Blazor Server** running locally inside the Tauri process exposed via a local HTTP endpoint, served to the embedded WebView2. The Blazor app calls Tauri IPC via the `@tauri-apps/api` JavaScript interop layer (called from Blazor's JS interop). C# services wrap Tauri IPC calls through `IJSRuntime.InvokeAsync`.

**Architecture**:
```
Rust (Tauri) ←IPC→ WebView2 ←JS Interop→ Blazor (C# / .NET 10)
```

The Blazor app is packaged alongside the Tauri binary via the `tauri.conf.json` `beforeBuildCommand` / `frontendDist` settings.

### Rationale
Blazor Server running locally avoids WASM startup latency (WASM cold-start for .NET 10 WASM is still ~2-3 seconds for large bundles). Blazor Server via local loopback has < 1 ms server roundtrip, well within the 500 ms UX budget. No external network access needed.

### Alternatives Considered
- **Blazor WebAssembly**: Longer cold-start; all state in browser sandbox; harder to call native OS APIs.
- **Blazor Hybrid (MAUI)**: MAUI doesn't target Tauri; too different a deployment model.
- **React / Vue frontend**: Would require TypeScript; user specified Blazor.

---

## 6. BlazorBlueprint.Components

### Decision
Use **BlazorBlueprint.Components** (NuGet: `BlazorBlueprint.Components`) as the primary UI component library. Components used will include: buttons, modals, input fields, dropdowns, lists, timeline/scroll views, and notification badges.

- Documentation: https://blazorblueprintui.com
- Source: https://github.com/blazorblueprintui/ui
- Install: `dotnet add package BlazorBlueprint.Components`

All interactive elements from BlazorBlueprint must be verified to meet WCAG 2.1 AA requirements (Constitution III). Any component that does not meet accessibility requirements will be wrapped with custom ARIA attributes.

### Rationale
Provides a consistent design system; reduces bespoke component authoring; compatible with .NET 10 Blazor.

### Alternatives Considered
- **MudBlazor**: Mature but more opinionated Material Design.
- **Radzen Blazor**: Good component set; not specified by user.
- **Bespoke components**: Highest maintenance cost; inconsistent look.

---

## 7. Local Storage: SQLite

### Decision
Use **SQLite** via `Microsoft.Data.Sqlite` for the local cache. The DB file lives in the app's portable directory (`{exe_dir}/tracey.db`). Schema migrations are managed with a hand-written sequential migration runner (no heavyweight ORM).

### Rationale
SQLite is a single portable file, zero config, runs without admin rights, and is supported natively on .NET 10. Perfectly suited for the portable deployment model.

### Alternatives Considered
- **LiteDB**: Document store; less standard; harder to query for reporting.
- **Entity Framework Core + SQLite**: Adds EF migration tooling complexity; for a portable app with a small schema, raw SQL is simpler.

---

## 8. External Database Sync (Cloud Sync)

### Decision
Support **PostgreSQL** (including Supabase) as the external database via a user-supplied connection URI. Use **Npgsql** on the C# side for database access. Sync is triggered:
- On a background timer (e.g., every 30 seconds when online).
- Immediately after any local write (fire-and-forget).
- On app startup (reconcile pending offline writes).

Conflict resolution: last-write-wins based on `modified_at` (UTC timestamp).

Window activity records are batched and flushed every **30 seconds** to the external DB (FR-061, SC-007).

Screenshots are **never** synced (FR-017, FR-062).

### Rationale
Npgsql is the standard .NET PostgreSQL driver. Supabase uses standard PostgreSQL wire protocol, so no Supabase SDK is required.

### Alternatives Considered
- **Supabase .NET SDK**: Adds client library overhead; Npgsql directly is simpler and just as capable.
- **REST API sync**: More latency; loses transactional guarantees.

---

## 9. Fuzzy Matching (Quick-Entry Bar)

### Decision
Implement fuzzy matching in **C# on the Blazor side** using a custom weighted Levenshtein / prefix-match scorer (no external library). The algorithm:
1. Exact prefix → highest rank.
2. Consecutive character match → second rank.
3. Character-spread match (VS Code style) → third rank.
4. Case-insensitive throughout.

The scoring logic is unit-tested with xUnit.

### Rationale
The VS Code-style Ctrl+P matching is well-understood and implementable in ~100 lines of C#. No NuGet dependency is needed.

### Alternatives Considered
- **FuzzySharp NuGet**: Levenstein-based; not character-spread aware.
- **Server-side (Rust)**: Adds an IPC roundtrip per keystroke; unacceptable latency for real-time dropdown.

---

## 10. Notification Channels (Email, Telegram)

### Decision
Define an `INotificationChannel` abstraction in C#:

```csharp
public interface INotificationChannel
{
    string ChannelId { get; }
    Task SendAsync(NotificationMessage message, CancellationToken ct);
    NotificationChannelSettings DefaultSettings { get; }
}
```

Built-in implementations: `EmailNotificationChannel` (uses `MailKit` / SMTP) and `TelegramNotificationChannel` (uses Telegram Bot API via `HttpClient`).

Timer notification check runs on a background service that fires every minute, checking if the active timer has exceeded the threshold.

### Rationale
The abstraction ensures SC-010: adding a new channel requires implementing one interface, not modifying existing channels.

### Alternatives Considered
- **Plugin system with DI**: Over-engineered for two built-in channels.
- **Webhook-only**: Less user-friendly for the Telegram use case.

---

## 11. Playwright E2E Testing Strategy

### Decision
Playwright tests run against the full packaged Tauri app (not a mocked frontend). The Tauri app is launched as a subprocess in `beforeAll`; Playwright connects to the WebView2 window via its DevTools endpoint.

Test files map 1:1 to User Stories:

| File | User Story |
|------|-----------|
| `timer.spec.ts` | US1 – Start/stop timer |
| `idle-detection.spec.ts` | US2 – Idle return prompt |
| `projects.spec.ts` | US3 – Client/Project/Task management |
| `screenshot-timeline.spec.ts` | US4 – Screenshot timeline |
| `quick-entry.spec.ts` | US5 – Fuzzy quick-entry |
| `tags.spec.ts` | US6 – Tag management |
| `notifications.spec.ts` | US7 – Long-running timer notifications |
| `cloud-sync.spec.ts` | US8 – Two-instance sync |

Idle detection tests use a configurable inactivity timeout (set to 5 seconds during tests). Screenshot tests mock the GDI capture with a test double (a Rust feature flag `#[cfg(test)]` that writes a pre-canned image instead).

### Rationale
Full-stack E2E with Playwright covers the acceptance criteria at the integration boundary where most regressions occur in Tauri + Blazor hybrid apps. Unit tests cover business logic; Playwright covers the IPC contract.

### Alternatives Considered
- **bUnit**: Tests Blazor components in isolation; does not cover the Tauri IPC boundary.
- **WinUI automation**: Platform-specific; does not test the Blazor render output.

---

## 12. Security Threat Model

### Threat: IPC Command Injection
- **Attack**: Malicious web content hosted in the WebView2 issues arbitrary Tauri commands.
- **Mitigation**: Tauri CSP restricts embedded resources; only `tauri://localhost` origin is whitelisted in capabilities. All IPC command inputs are validated in Rust (path validation, length checks, enum-constrained inputs).

### Threat: Path Traversal via Screenshot Storage
- **Attack**: User configures a screenshot path that traverses outside intended directories (e.g., `../../../../Windows/System32`).
- **Mitigation**: Screenshot storage path is canonicalized and validated in Rust before any write. Only paths within user-writable locations are accepted.

### Threat: Connection URI Credential Exposure
- **Attack**: External DB connection URI (containing credentials) is stored in plaintext config file.
- **Mitigation**: Connection URI is stored in the OS credential store (Windows Credential Manager via the `keyring` crate). It is never written to disk in plaintext.

### Threat: Tauri Plugin Surface
- **Attack**: Third-party Tauri plugins introduce RCE or data exfiltration.
- **Mitigation**: Only first-party Tauri plugins used (`tauri-plugin-system-idle`, `tauri-plugin-fs`). All plugins reviewed before adoption. Dependency CVE scan in CI.

### Threat: Log-Based PII Leakage
- **Attack**: Window titles or process names containing sensitive data (passwords, PII) appear in log files.
- **Mitigation**: Window titles in logs are truncated to 50 characters and redacted if they contain any known sensitive-app process names (configurable deny-list). Logs never contain raw screenshot data.

---

## 13. Portability: No Admin Rights

### Decision
The app:
- Uses `AppData\Roaming\Tracey` (or `{exe_dir}` as fallback) for config and SQLite DB — both are user-writable without elevation.
- Does not write to registry.
- Does not require a Windows service.
- Uses `GetForegroundWindow` and GDI screenshot, which work without elevation.
- System tray integration via Tauri's `tauri-plugin-positioner` / `tauri-plugin-notification` (no admin needed).

### Rationale
Validated by spec requirement FR-061, FR-063. Windows 11 allows all required Win32 APIs from a standard user token.

---

## 14. .NET 10 Compatibility

### Decision
Target **.NET 10** for all C# / Blazor projects. This is the latest LTS-candidate release (LTS status expected late 2025). Blazor Server in .NET 10 supports WebView2 as a host renderer.

Build target in `Tracey.App.csproj`:
```xml
<TargetFramework>net10.0</TargetFramework>
```

### Rationale
Net 10 was explicitly required by the user. Blazor has full framework support on .NET 10.

---

## Summary Table of Decisions

| Topic | Decision | Key Risk |
|-------|----------|----------|
| Idle detection | `tauri-plugin-system-idle` | Plugin maintenance lag |
| Window detection | `GetForegroundWindow` polling | 1-second granularity may miss very quick switches |
| Screenshots | GDI BitBlt + spawn_blocking + Triangle JPEG | CPU spike during capture; mitigated by spawn_blocking |
| Debounce | 2-second window-change debounce | High-frequency browsers may still trigger too often |
| Blazor integration | Blazor Server local loopback → WebView2 | Local port may conflict; mitigated by dynamic port selection |
| Component library | BlazorBlueprint.Components | Needs WCAG 2.1 AA audit per component |
| Local DB | SQLite via Microsoft.Data.Sqlite | WAL mode required for concurrent writes from Rust + Blazor |
| External sync | Npgsql to user-supplied Postgres/Supabase | User must provision their own DB |
| Fuzzy match | Custom C# scorer | Coverage of edge cases needs thorough unit tests |
| Notifications | INotificationChannel abstraction | Email deliverability depends on SMTP settings |
| E2E testing | Playwright against full Tauri app | Flakiness risk with GDI screenshot tests → use test doubles |
| Security | CSP + capability least-privilege + keyring | WebView2 CSP configuration is strict by default ✅ |
