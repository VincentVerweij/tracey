# Developer Quickstart: Window Activity Timetracking Tool

**Branch**: `001-window-activity-tracker`  
**Date**: 2026-03-14  
**Stack**: Tauri 2.0 (Rust) + Blazor .NET 10 (C#) + Playwright (E2E)

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.77.2+ | `winget install Rustlang.Rustup` then `rustup update stable` |
| .NET SDK | 10.0 | https://dot.net — install .NET 10 SDK |
| Node.js | 20 LTS | `winget install OpenJS.NodeJS.LTS` |
| Tauri CLI | 2.0+ | `cargo install tauri-cli --version "^2.0"` |
| WebView2 | any | Pre-installed on Windows 11; developer bootstrap: https://developer.microsoft.com/en-us/microsoft-edge/webview2/ |
| Playwright | 1.40+ | Installed via `npm install` (see E2E section) |

---

## Verify Prerequisites

```powershell
rustc --version        # rustc 1.77.x or newer
cargo --version
dotnet --version       # 10.0.x
node --version         # v20.x
cargo tauri --version  # tauri-cli 2.0.x
```

---

## Project Structure

```
tracey/
├── src-tauri/          # Rust / Tauri native layer
├── src/                # C# Blazor .NET 10 frontend
│   ├── Tracey.slnx
│   └── Tracey.App/
├── tests/
│   └── e2e/            # Playwright tests
└── docs/
    └── ux/tone.md
```

---

## First-Time Setup

### 1. Clone and Enter Repo

```powershell
git clone https://github.com/VincentVerweij/tracey.git
cd tracey
git checkout 001-window-activity-tracker
```

### 2. Install Rust Dependencies

```powershell
cd src-tauri
cargo fetch
cd ..
```

### 3. Restore .NET Dependencies

```powershell
cd src
dotnet restore
cd ..
```

The project references **BlazorBlueprint.Components** and **MailKit** from NuGet. Run `dotnet restore` to pull them.

> **NuGet package**: `BlazorBlueprint.Components`  
> **Docs**: https://blazorblueprintui.com  
> **Source**: https://github.com/blazorblueprintui/ui

### 4. Install Node / Playwright Dependencies

```powershell
cd tests/e2e
npm install
npx playwright install --with-deps chromium
cd ../..
```

---

## Development Workflow

### Run in Development Mode

Starts Blazor with hot-reload and launches the Tauri dev window:

```powershell
cargo tauri dev
```

This command:
1. Runs `dotnet watch run --project ../src/Tracey.App --urls http://localhost:5000` (configured in `tauri.conf.json` → `build.beforeDevCommand`).
2. Launches the Tauri window pointed at `http://localhost:5000`.
3. Watches for Rust changes and recompiles the native layer.

### Hot Reload (Blazor Only)

If you only change `.razor` / `.cs` files:

```powershell
cd src
dotnet watch --project Tracey.App
```

This is faster than full `cargo tauri dev` for UI-only changes.

---

## Running Tests

### Rust Unit Tests

```powershell
cd src-tauri
cargo test
```

### .NET / Blazor Unit Tests (xUnit)

```powershell
cd src
dotnet test
```

### Playwright E2E Tests

The E2E suite launches the full Tauri app as a subprocess:

```powershell
# Build the app first (required for E2E)
cargo tauri build --debug

# Run all E2E tests
cd tests/e2e
npx playwright test

# Run a specific spec
npx playwright test specs/timer.spec.ts

# Open Playwright UI mode (interactive)
npx playwright test --ui
```

> **Idle detection tests**: The inactivity timeout is overridden to **5 seconds** in the test fixture via a Tauri IPC call at test start. This avoids real 5-minute waits.

> **Screenshot tests**: GDI capture is replaced by a test double in `#[cfg(feature = "test")]` builds. The test double writes a pre-canned 100×100 JPEG instead of calling Win32 APIs.

---

## Build for Distribution (Portable Executable)

```powershell
cargo tauri build
```

Output is in `src-tauri/target/release/`. The result is a **single portable `.exe`** — no installer, no admin rights required. Copy it anywhere and run it.

---

## Key Configuration Files

| File | Purpose |
|------|---------|
| `src-tauri/tauri.conf.json` | App identity, window config, capabilities |
| `src-tauri/capabilities/default.json` | Tauri permission grants (least-privilege) |
| `src-tauri/Cargo.toml` | Rust dependencies and feature flags |
| `src/Tracey.App/Tracey.App.csproj` | .NET 10 project, NuGet references |
| `tests/e2e/playwright.config.ts` | Playwright configuration |

---

## Cargo.toml Key Dependencies

```toml
[dependencies]
tauri    = { version = "2", features = ["protocol-asset"] }
tauri-plugin-fs = "2"
# Idle detection handled via Win32 in platform/windows.rs (no tauri-plugin-system-idle)

[target.'cfg(target_os = "windows")'.dependencies]
windows  = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_System_SystemInformation",
    "Win32_System_ProcessStatus",
    "Win32_System_Threading",
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
] }
image    = { version = "0.25", default-features = false, features = ["jpeg"] }
serde    = { version = "1", features = ["derive"] }
keyring  = "3"
tokio    = { version = "1", features = ["full"] }

[features]
test = []   # enables test doubles for screenshot capture
```

---

## Blazor .csproj Key References

```xml
<PackageReference Include="BlazorBlueprint.Components"                        Version="3.5.2" />
<PackageReference Include="MailKit"                                            Version="4.15.1" />
<PackageReference Include="Microsoft.AspNetCore.Components.WebAssembly"        Version="10.0.4" />
<PackageReference Include="Microsoft.AspNetCore.Components.WebAssembly.DevServer" Version="10.0.4" />
<PackageReference Include="Microsoft.Extensions.Hosting.Abstractions"          Version="10.0.4" />
<PackageReference Include="Microsoft.Extensions.Http"                          Version="10.0.4" />
```

> **Note**: SQLite is provided by `rusqlite` on the Rust side (bundled). There is no `Microsoft.Data.Sqlite` or `Npgsql` .NET package — PostgreSQL sync uses `tokio-postgres` in the Rust layer.

---

## Tauri Capabilities (default.json)

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "identifier": "default",
  "description": "Least-privilege capability grants for Tracey.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "fs:allow-write-file",
    "fs:allow-read-file"
  ]
}
```

> **Note**: Use `fs:allow-write-file` (singular), not `fs:allow-write-files` (plural) — the plural form does not exist and silently fails. Idle detection does not require a Tauri plugin — it is implemented directly via Win32 `GetLastInputInfo` in `platform/windows.rs`.

---

## First Run Behaviour

On first launch the app:
1. Creates `{exe_dir}/tracey.db` (SQLite local cache).
2. Creates `{exe_dir}/screenshots/` directory.
3. Inserts default `user_preferences` row.
4. Shows the onboarding wizard to configure timezone, inactivity timeout, and optionally the external DB URI.

No admin rights are required. The app does not write to the Windows registry.

---

## Adding a New Notification Channel

1. Create `src/Tracey.App/Services/Notifications/MyChannelNotificationChannel.cs` implementing `INotificationChannel`.
2. Register it in `Program.cs`: `builder.Services.AddSingleton<INotificationChannel, MyChannelNotificationChannel>();`.
3. Add the channel config POCO to `NotificationChannelSettings.cs`.
4. Add a settings UI section in `Pages/Settings.razor`.
5. Write a Playwright test in `tests/e2e/specs/notifications.spec.ts`.

No changes to existing channels (`EmailNotificationChannel`, `TelegramNotificationChannel`) are required (ref. SC-010).

---

## Common Issues

| Issue | Fix |
|-------|-----|
| `WebView2 not found` | Install WebView2 runtime from https://developer.microsoft.com/en-us/microsoft-edge/webview2/ |
| `cargo tauri dev` hangs | Ensure the Blazor dev server is running on the port in `tauri.conf.json` |
| `fs:allow-write-files` permission error | Change to `fs:allow-write-file` (singular) in `capabilities/default.json` |
| HWND null check crash | Use `std::ptr::null_mut()` not `== 0` when checking HWND in windows crate 0.58+ |
| Screenshot causes "Not Responding" | Ensure GDI capture runs inside `tauri::async_runtime::spawn_blocking` |
| GetTickCount rollover | Use `GetTickCount64` not `GetTickCount` (32-bit rolls over at ~49 days uptime) |
| `dotnet restore` fails for BlazorBlueprint | Ensure nuget.org source is configured: `dotnet nuget list source` |
