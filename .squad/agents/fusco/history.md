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
