# Work Routing

How to decide who handles what.

## Routing Table

| Work Type | Route To | Examples |
|-----------|----------|----------|
| Architecture, IPC contracts, design gates, constitution check | Finch | New features, IPC contract changes, phase boundaries |
| Rust code, Tauri IPC commands, Win32 APIs, SQLite write path, sync engine, keychain | Reese | Window tracking, screenshot pipeline, idle detection, sync |
| C# / Blazor WASM UI, BlazorBlueprint, quick-entry, fuzzy match, idle prompt, Settings | Root | UI components, quick-entry bar, timer display, timeline |
| Tests, Playwright E2E, xUnit, cargo test, test-first, edge cases | Shaw | All acceptance scenario tests, TDD gate |
| GitHub Actions, Tauri build, CI/CD, portable exe, versioning, release | Fusco | Build pipeline, CI gates, release artifacts |
| Security review, Tauri capabilities, IPC validation, keychain, path traversal | Control | PR security review, capability changes, credential handling |
| SQLite schema, migrations, WAL, Postgres sync strategy, data modeling | Leon | Schema design, migration runner, sync contract |
| Session logging | Scribe | Automatic — never needs routing |
| Backlog, issue triage, PR monitoring | Ralph | Work queue management |

## Multi-Domain Task Routing

| Task | Primary | Secondary |
|------|---------|----------|
| Window activity tracking | Reese (Rust polling loop) | Leon (schema for `window_activity_records`) |
| Idle detection → UI prompt | Reese (IPC: `system_idle_check`) | Root (idle-return modal) |
| Screenshot capture pipeline | Reese (GDI + JPEG pipeline) | Leon (schema for `screenshots`) |
| External sync engine | Reese (Rust background task) | Leon (Postgres strategy, sync_queue schema) |
| Quick-entry fuzzy match | Root (UI + C# algorithm) | Shaw (xUnit tests for algorithm) |
| New PR review | Shaw (test coverage) → Control (security) → Finch (architecture) | — |
| New feature | Finch (architecture review) → Reese or Root (implement) → Shaw (tests) | Control (if security-sensitive) |
| Schema changes | Leon (design) → Reese (Rust impl) | Shaw (migration tests) |
| Settings persistence | Leon (preferences schema) → Reese (IPC) | Root (settings UI) |

## Issue Routing

| Label | Action | Who |
|-------|--------|-----|
| `squad` | Triage: analyze issue, assign `squad:{member}` label | Finch (Lead) |
| `squad:{name}` | Pick up issue and complete the work | Named member |

## Reviewer Gates

- **Shaw**: Must approve all PRs (test coverage gate)
- **Control**: Must approve all PRs touching Tauri capabilities, IPC handlers, file system access, external network, or credentials
- **Finch**: Must approve all PRs with architecture or IPC contract changes
4. When `squad:copilot` is applied and auto-assign is enabled, `@copilot` is assigned on the issue and picks it up autonomously.
5. Members can reassign by removing their label and adding another member's label.
6. The `squad` label is the "inbox" — untriaged issues waiting for Lead review.

### Lead Triage Guidance for @copilot

When triaging, the Lead should ask:

1. **Is this well-defined?** Clear title, reproduction steps or acceptance criteria, bounded scope → likely 🟢
2. **Does it follow existing patterns?** Adding a test, fixing a known bug, updating a dependency → likely 🟢
3. **Does it need design judgment?** Architecture, API design, UX decisions → likely 🔴
4. **Is it security-sensitive?** Auth, encryption, access control → always 🔴
5. **Is it medium complexity with specs?** Feature with clear requirements, refactoring with tests → likely 🟡

## Rules

1. **Eager by default** — spawn all agents who could usefully start work, including anticipatory downstream work.
2. **Scribe always runs** after substantial work, always as `mode: "background"`. Never blocks.
3. **Quick facts → coordinator answers directly.** Don't spawn an agent for "what port does the server run on?"
4. **When two agents could handle it**, pick the one whose domain is the primary concern.
5. **"Team, ..." → fan-out.** Spawn all relevant agents in parallel as `mode: "background"`.
6. **Anticipate downstream work.** If a feature is being built, spawn the tester to write test cases from requirements simultaneously.
7. **Issue-labeled work** — when a `squad:{member}` label is applied to an issue, route to that member. The Lead handles all `squad` (base label) triage.
8. **@copilot routing** — when evaluating issues, check @copilot's capability profile in `team.md`. Route 🟢 good-fit tasks to `squad:copilot`. Flag 🟡 needs-review tasks for PR review. Keep 🔴 not-suitable tasks with squad members.
