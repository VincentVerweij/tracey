# Glossary

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
| **Locked** | The session watch: WTS lock/unlock notifications, reconciled by a session-state query ([ADR-0002](docs/adr/0002-wts-session-notifications-detect-lock.md)) | **Screenshot capture and window activity tracking** ([ADR-0003](docs/adr/0003-locked-is-lock-only-and-suspends-window-activity.md)) | In-memory; re-seeded at startup, never restored. Lock *history* is kept as presence spans ([ADR-0006](docs/adr/0006-presence-is-recorded-as-heartbeat-extended-spans.md)) | No — logged only |

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

---

## Presence

Settled by [ADR-0005](docs/adr/0005-timeline-shows-presence-not-suspension.md);
recording by [ADR-0006](docs/adr/0006-presence-is-recorded-as-heartbeat-extended-spans.md).

**Presence** — what the Timeline bar shows over time: whether the user was
working at the device. Distinct from **Suspension**, which says whether Tracey
is observing. Internal vocabulary.

**Presence span** — a maximal stretch of time in one presence state.

| State | Holds when | UI word | Color |
|---|---|---|---|
| **`Present`** | Tracked, not `Locked`, not idle | "Active" | green |
| **`Away`** | Tracked, and `Locked` or idle | "Away" | red |
| **`Untracked`** | Tracey not running, or `Paused` holds | "Not tracked" | none |

Precedence when several hold: `Untracked` > `Away` > `Present`. Nothing is
observed while `Paused`, so locking while paused stays `Untracked`.

**Idle** (for presence) — no input for `inactivity_timeout_seconds`. An idle
`Away` span starts at the last input (`idle_since`), not when the threshold is
crossed, so away-time shorter than the threshold stays `Present`.

**Away cause** — why a span is `Away`: `Locked` or idle. `Locked` wins when
both hold, and a change of cause splits the span. The cause never changes the
color.

**Presence recorder** — the sole writer of presence spans. It runs its own
1 s tick, reading idle seconds, `Locked` and `Paused` as one snapshot, and is
never suspended, because it observes suspension rather than being subject to
it. It stores only `Present` and `Away` spans. Every gap, including Tracey not
running, `Paused`, and sleep or hibernate, is `Untracked`. It emits
`presence-changed` after each transition. Settled by ADR-0006.

**Open span** — the span the presence recorder is still extending. A heartbeat
moves its end to now every ~30 s, so a crash loses at most that much.

The idle-prompt resolution (`break`, `meeting`, `keep`, `specify`) belongs to
time entries and never changes a presence span. The bar records what was
observed, not how it was interpreted.

### Retired terms

- **"Tracking-state span"**: charting shorthand from map #104. Say presence
  span.
- **"Active" as an internal state name**: it is only the UI word for
  `Present`. "Active" already means a running timer in code.
