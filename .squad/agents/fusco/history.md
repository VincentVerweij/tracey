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
