# CONTEXT

Domain vocabulary for Tracey.

Terms are added here lazily, as decisions actually resolve them — see
`docs/agents/domain.md`. Each term names the ADR that settled it. When your
output names one of these concepts (an issue title, a test name, UI copy, a
commit message), use the term as defined here rather than a synonym.

---

## Suspension

Settled by [ADR-0001](docs/adr/0001-suspension-is-one-concept-with-reasons.md).

**Tracking** — the loops that observe the user and record what they find:
screenshot capture, window activity tracking, idle detection, and
classification. *Not* the running time entry, *not* sync, *not* the UI.

**Suspension** — application-level state. Tracking is suspended while at least
one *suspension reason* holds. Internal vocabulary: the word never appears in
the UI.

**Suspension reason** — a named cause of suspension. There are exactly two
today. Reasons are independent, and each may be cleared only by its own owner.

| Reason | Raised by | Suspends | Durability | Visible |
|---|---|---|---|---|
| **Paused** | The user, from the tray | **All tracking** | Persisted as an absolute deadline; survives restart | Yes — tray and in-app indicator |
| **Locked** | The session watch: WTS lock/unlock notifications, reconciled by a session-state query ([ADR-0002](docs/adr/0002-wts-session-notifications-detect-lock.md)) | **Screenshot capture and window activity tracking** ([ADR-0003](docs/adr/0003-locked-is-lock-only-and-suspends-window-activity.md)) | In-memory; re-seeded at startup, never persisted | No — logged only |

**Paused** — the user-initiated reason. The UI word is *pause*: "Pause
tracking", "Continue paused tracking".

**Locked** — the involuntary reason. Maintained continuously, including while
`Paused` also holds: the lock signal keeps updating it, it simply stops
mattering, because suspension is already in force.

Only a session *lock* raises it. A disconnected session is not `Locked`.
It suspends window activity tracking as well as capture, because the only
window visible while locked is the lock screen, which is a session state, not
activity. Idle detection keeps running, since a lock is exactly what the idle
prompt is for. Settled by
[ADR-0003](docs/adr/0003-locked-is-lock-only-and-suspends-window-activity.md).

**Session watch** — the sole owner of `Locked`. It raises and clears the
reason from WTS session notifications, seeds it at startup, and reconciles it
from a session-state query before each capture and each activity write, and
on each tick a loop spends suspended, so a wrong `Locked` cannot hold it shut. Settled by
[ADR-0002](docs/adr/0002-wts-session-notifications-detect-lock.md). The
foreground process (`LockApp.exe`) is *not* a lock signal, and the reactive
`BitBlt` skip only drops a frame; it never raises or clears `Locked`.

**Resume** — clearing the `Paused` reason. There is no user resume for
`Locked`; unlocking is not a resume. A resume is one way *suspension ends*,
but only when no other reason still holds.

**Suspension ends** — the edge, per loop, on the first tick where that loop
was suspended and no longer is, whichever reason cleared last and however it
cleared (unlock, resume, or the session-state reconcile). Unlocking while
`Paused` holds is not a suspension end. Startup is not one either. Settled by
[ADR-0004](docs/adr/0004-suspension-ends-with-a-per-loop-re-entry.md).

**Re-entry** — what a loop does when its suspension ends, before observing
anything. Window activity tracking forces one fresh row (subject to the
deny-list). Screenshot capture takes one settled capture about 2s later, with
trigger `suspension_end`, and restarts its interval clock from it. Re-entry
for idle detection and classification belongs to #62.

### The test for a new loop

A loop must consult suspension if it would otherwise **record new observation
data while `Paused` holds** — to disk or in memory. This is the rule; the list
of four tracking loops above is only its current answer.

### Retired terms

- **"Gated" / "the lock gate"** — informal shorthand from charting. Say the
  `Locked` reason.
- **"Stopped"** — belongs to time entries (an entry is stopped) and to the
  application. Tracking is *suspended*, never stopped.
- **"Paused" as a synonym for suspended** — `Paused` is one reason, not the
  state. Tracking suspended under `Locked` alone is not paused.
