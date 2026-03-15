# Ceremonies

> Team meetings that happen before or after work.

## Architecture Review

| Field | Value |
|-------|-------|
| **Trigger** | auto |
| **When** | before |
| **Condition** | new feature, IPC contract change, or Phase boundary |
| **Facilitator** | Finch |
| **Participants** | Reese, Root, Leon, Control |
| **Enabled** | ✅ yes |

**Agenda:**
1. Review architecture decisions and IPC contract alignment with `contracts/ipc-commands.md`
2. Verify constitution gates (plan.md principles I–VII)
3. Decompose feature into implementation tasks for Reese, Root, and Leon
4. Identify security-sensitive surfaces (Control flags capability changes)

---

## Security Review

| Field | Value |
|-------|-------|
| **Trigger** | auto |
| **When** | before |
| **Condition** | PR touching Tauri capabilities, IPC handlers, keychain, file system, or external network |
| **Facilitator** | Control |
| **Participants** | Finch, Reese |
| **Enabled** | ✅ yes |

**Agenda:**
1. Verify Tauri capabilities are least-privilege
2. Confirm IPC handler inputs are validated in Rust before processing
3. Check keychain usage and path traversal mitigations
4. Finch signs off on any capability additions

---

## Test Gate

| Field | Value |
|-------|-------|
| **Trigger** | auto |
| **When** | before |
| **Condition** | Implementation task begins (any T-prefix task in tasks.md) |
| **Facilitator** | Shaw |
| **Participants** | Reese, Root |
| **Enabled** | ✅ yes |

**Agenda:**
1. Confirm failing tests exist for all acceptance scenarios (committed, CI confirms red)
2. TDD gate — no implementation starts until tests are failing and committed

---

## Retrospective

| Field | Value |
|-------|-------|
| **Trigger** | manual |
| **When** | on-demand |
| **Facilitator** | Finch |
| **Participants** | Reese, Root, Shaw, Fusco, Control, Leon |
| **Enabled** | ✅ yes |

**Agenda:**
1. What worked well this phase?
2. What slowed us down?
3. Process improvements — record in decisions.md
4. Assign action items

---

## Retrospective

| Field | Value |
|-------|-------|
| **Trigger** | auto |
| **When** | after |
| **Condition** | build failure, test failure, or reviewer rejection |
| **Facilitator** | lead |
| **Participants** | all-involved |
| **Time budget** | focused |
| **Enabled** | ✅ yes |

**Agenda:**
1. What happened? (facts only)
2. Root cause analysis
3. What should change?
4. Action items for next iteration
