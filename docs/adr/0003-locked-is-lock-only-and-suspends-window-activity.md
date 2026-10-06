# ADR-0003 — `Locked` responds to lock only, and suspends window activity tracking too

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** Vincent Verweij
- **Resolves:** [#70](https://github.com/VincentVerweij/tracey/issues/70), under map [#65](https://github.com/VincentVerweij/tracey/issues/65)
- **Amends:** [ADR-0001](0001-suspension-is-one-concept-with-reasons.md) decision 5 (one sentence), [ADR-0002](0002-wts-session-notifications-detect-lock.md) decisions 1 and 5
- **Vocabulary:** [`GLOSSARY.md` § Suspension](../../GLOSSARY.md#suspension)

## Context

ADR-0002 chose WTS session notifications to raise and clear `Locked`. It left two questions to #70: which *other* WTS codes affect the reason, and whether `Locked` should stay capture-only, as map #65 charted it.

WTS delivers more than lock and unlock. Console and remote connect/disconnect, logon/logoff, remote control, and session create/terminate all arrive on the same registration. `0xC`–`0xE` are undefined (#67). Only the disconnect pair is lock-like. `REMOTE_CONNECT` is the opposite of a lock.

The capture-only scope left a leak, raised as ADR-0001's open question. While the session is locked, window activity tracking records the lock screen (`LockApp.exe`) as if it were an application the user worked in. #66 showed `LockApp.exe` holds the foreground for the whole steady lock. The tracker writes a row on each window change, so every lock writes one such row. `classification_loop` then classifies it like any other row, and `auto_create_or_extend_time_entry` either extends a running auto entry to the lock instant or creates a stray zero-length one for whatever project the lock screen classifies to.

## Decision

### 1. `Locked` responds to lock and unlock only

`WTS_SESSION_LOCK` and `WTS_SESSION_UNLOCK` are the only codes that raise or clear `Locked`. Every other code is logged at debug level and ignored:

| Code | Action |
|---|---|
| `SESSION_LOCK` / `SESSION_UNLOCK` | raise / clear `Locked` |
| `CONSOLE_CONNECT` / `CONSOLE_DISCONNECT`, `REMOTE_CONNECT` / `REMOTE_DISCONNECT`, `REMOTE_CONTROL` | debug log only |
| `SESSION_LOGON` / `SESSION_LOGOFF`, `SESSION_CREATE` / `SESSION_TERMINATE` | debug log only (not lock-like; logoff ends the process) |
| `0xC`–`0xE`, or any unrecognised value | debug log only |

This makes ADR-0002 decision 1's "treated as unknown, never as no change" concrete. An unrecognised event **makes no claim** about lock state, so it neither raises nor clears `Locked` on its own. Decision 3 below ensures the reason is correct before any loop acts on it.

Tracey runs locally, at the console of the machine it tracks. Widening to the disconnect pair would mean reopening ADR-0002 decision 5, because a disconnect-raised `Locked` would be cleared again by a `SessionFlags` query that still reports `UNLOCK`. It would also make the unresolved question of whether a disconnected session's `BitBlt` returns a black frame load-bearing. Neither cost buys anything for local use.

### 2. `Locked` suspends screenshot capture **and window activity tracking**

| Loop | Under `Locked` |
|---|---|
| Screenshot capture | suspended |
| Window activity tracking | **suspended** |
| Idle detection | runs |
| Classification | runs |

The reason is **correctness, not privacy**. While the session is locked, the only window the tracker can observe is the lock screen. That is a session state, not user activity, and recording it as activity is wrong data.

ADR-0001 decision 5's privacy rationale stands unchanged: a lock is an absence, not a statement that nothing should be observed. That is why idle detection keeps running. A lock is exactly what should drive the idle prompt, and map #65's ruling that stopping a time entry on lock is out of scope relies on it. Classification observes nothing and only processes rows. With the tracker suspended, no lock-screen rows reach it.

Only ADR-0001 decision 5's sentence "`Locked` suspends screenshot capture alone" is amended. This also closes ADR-0001's open question. When a pause deadline expires during a lock, only idle detection and classification resume, and neither records the lock screen.

### 3. The reconcile runs before any loop records an observation under `Locked`

ADR-0002 decision 5 re-queries `SessionFlags` before each capture, so that a missed event or a dead registration recovers itself. With window activity tracking now under `Locked`, the same reconcile also runs **before each activity write**. The tracker writes only on window change, and a lock *is* a window change, so this costs one query per change and catches exactly the case that matters: a missed lock event followed by `LockApp.exe` coming to the foreground.

The rule becomes: **the query reconciles `Locked` before any loop records an observation under it.** A loop that opts into `Locked` later inherits the reconcile with it.

## Known gaps, left deliberately unhandled

- **Remote and disconnect-instead-of-lock setups.** If Tracey runs inside an RDP or AVD session, the session may disconnect where a local session would lock (AVD with Entra SSO does this, per #72), and capture continues against a desktop nobody sees. Unhandled because Tracey runs locally at the console.
- **Fast user switching.** Assumed to lock the original session first (so `WTS_SESSION_LOCK` fires and `SessionFlags` reports `LOCK`), but this is unverified. Not a use case today: the machine has one user. A fast-user-switch step was added to the `SessionFlags` probe ([#73](https://github.com/VincentVerweij/tracey/issues/73)). If it shows FUS does *not* lock, this comes back as a ticket.

## Consequences

- Lock-screen rows stop appearing in `window_activity_records`, and with them the stray auto entries classification derived from them.
- Existing lock-screen rows are left as they are. Nothing has been synced and classification has been off, so no migration is planned.
- When `Locked` clears and the user returns to the same window they left, the tracker's change detection may write no row. The lock then leaves no boundary in the activity data. Whether clearing `Locked` should force a fresh row belongs to resume semantics ([#71](https://github.com/VincentVerweij/tracey/issues/71)). **Settled by [ADR-0004](0004-suspension-ends-with-a-per-loop-re-entry.md):** when suspension ends, the tracker forces one fresh row.

## Alternatives considered

- **Keep `Locked` capture-only and add `LockApp.exe` to the process deny-list.** Smaller change. Rejected: the deny-list is a user preference, so editing it brings the leak back. It keys on a process name #72 showed to be unreliable, and it treats a session state as an uninteresting application. It would also mean two mechanisms for one fact.
- **Widen `Locked` to all tracking, as `Paused` does.** Rejected: suspending idle detection would remove the idle prompt, which is what catches a timer left running across a lock.
- **Treat `CONSOLE_DISCONNECT` / `REMOTE_DISCONNECT` as lock.** Rejected under decision 1: no local use case, and it would reopen ADR-0002's query-wins reconcile.
