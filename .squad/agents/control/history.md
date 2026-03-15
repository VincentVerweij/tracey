# Control — Project History

## Core Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Branch:** `001-window-activity-tracker`
- **Stack:** Tauri 2.0 capabilities model, `keyring` crate, `std::fs::canonicalize`, structured JSON logs
- **Security surface files:** `src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`, all `src-tauri/src/commands/` handlers
- **Threat model:** `specs/001-window-activity-tracker/research.md`
- **Created:** 2026-03-15

## Learnings

### 2026-03-15: Team Setup & Security Baseline
- Two capabilities only: `fs:allow-write-file` (singular — note the exact name) and `system-idle:allow-get-idle-time`
- CSP: WebView2 content locked to `tauri://localhost` only
- One credential in the entire system: Postgres/Supabase connection URI → lives in OS keychain (keyring crate), never plain text
- Path traversal: all screenshot paths go through `std::fs::canonicalize` before write, then validated against configured screenshots directory
- No PII in logs: deny-listed process names must not appear in any log field, including window title fields
- No local HTTP/TCP listeners — `health_get` is IPC-only, not a network endpoint
- Screenshots: never in sync queue, never transmitted. This is both a privacy and security requirement.
- Reviewer gate: I must sign off on all PRs touching capabilities, IPC handlers, file system, external network, or credentials
