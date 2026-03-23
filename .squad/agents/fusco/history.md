# Fusco — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** GitHub Actions (windows-latest), Tauri 2.0 build, cargo, dotnet, Playwright, portable exe output
- **My files:** `.github/workflows/`, `src-tauri/tauri.conf.json` (build config portions)
- **Constraint:** Portable `.exe` only — NO installer, NO NSIS, NO MSI, NO admin rights required
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup & Build Notes
- Portable exe constraint is absolute — no installer artifacts in release pipeline
- CI must run: cargo check → cargo clippy (-D warnings) → cargo test → tauri build → dotnet build → dotnet test → Playwright E2E
- `--features test` separate build step required for Playwright runs (enables GDI screenshot test stub)
- `npx playwright install --with-deps` must run before Playwright tests in CI
- Release workflow triggers on `v*.*.*` semver tags, publishes `.exe` as GitHub Release asset
- `[features] test = []` flag in `src-tauri/Cargo.toml` — Reese defines it, I wire it into CI

---

## 2026-03-21: T079 + T085 — CI Pipeline (.github/workflows/ci.yml)

### What was built
Four-job sequential pipeline on `windows-latest`:
1. **lint** — `cargo clippy -D warnings` + `dotnet format --verify-no-changes`
2. **unit-tests** — `cargo test` + `dotnet test` + `cargo audit` (CVE scan)
3. **build-portable** — `dotnet publish` Blazor WASM → `cargo build --release` → portable exe verification → upload artifact `tracey-portable-exe`
4. **e2e-tests** — push-only gate; runs `tsc --noEmit` on the Playwright test suite (full browser E2E deferred to local dev until a Tauri WebDriver harness is set up)

### Key decisions
- Used `cargo build --release` (not `cargo tauri build`) to avoid requiring the Tauri CLI in CI
- Portable check copies exe to a GUID temp dir and asserts `tracey.db` appears beside the exe — confirms T078 path-resolution logic
- E2E job skipped on PRs (`if: github.event_name == 'push'`) to keep PR feedback fast
- `cargo audit` co-located in unit-tests job to avoid extra runner spin-up
- No installer artifact — portable-exe-only, respecting the portable constraint

### Files created / modified
- Created: `.github/workflows/ci.yml`
- Marked done: T079 in `specs/001-window-activity-tracker/tasks.md`
- Created: `.squad/decisions/inbox/fusco-ci-pipeline.md`

---

## 2026-03-21: T085 — CI Pipeline Extended + release.yml

### What changed
Extended the existing `ci.yml` created in T079 and added `release.yml`:

**ci.yml additions:**
- All Rust toolchain steps now specify `targets: x86_64-pc-windows-msvc`
- Added `actions/cache@668228422ae6a00e4ad889ee87cd7109ec5666a7 # v5.0.4` for Cargo registry (`~/.cargo/registry`, `~/.cargo/git`) in lint, unit-tests, and build-portable jobs
- Added Cargo target dir cache (`src-tauri/target`) in build-portable job
- Added `cargo check` step to lint job (before clippy)
- Added `dotnet build src/Tracey.slnx` step to unit-tests job (explicit build check before `dotnet test --no-build`)
- E2E job upgraded from TypeScript-only check to real Playwright test run:
  - `npx playwright install --with-deps chromium` (was `--with-deps` missing)
  - Blazor WASM dev server started in background (`dotnet run --urls http://localhost:5000`)
  - `npx playwright test` runs against localhost:5000 with `continue-on-error: true`
  - Playwright report uploaded as artifact on `if: always()`
  - Dev server stopped in cleanup step

**release.yml created:**
- Triggers on `v*.*.*` semver tags
- Builds: `dotnet publish` → `cargo build --release --target x86_64-pc-windows-msvc`
- Creates GitHub Release via `softprops/action-gh-release@v2`
- `prerelease: true` auto-detected when tag contains `-` (e.g. `v1.0.0-rc.1`)

### Key decisions
- E2E `continue-on-error: true` because `window.__TAURI__` is absent in devserver mode — Tauri IPC calls will fail; HTML/CSS structural tests still run
- Full native E2E pending `[features] test = []` in Cargo.toml (Reese) + tauri-driver CDP harness
- Used `cargo build --release` (not `cargo tauri build`) consistently — avoids Tauri CLI requirement in CI

### Files created / modified
- Modified: `.github/workflows/ci.yml`
- Created: `.github/workflows/release.yml`
- Marked done: T085 in `specs/001-window-activity-tracker/tasks.md`
- Updated: `.squad/decisions/inbox/fusco-ci-pipeline.md`
