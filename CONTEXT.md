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
| **Locked** | The Windows session-lock signal | **Screenshot capture only** | In-memory; re-seeded at startup, never persisted | No — logged only |

**Paused** — the user-initiated reason. The UI word is *pause*: "Pause
tracking", "Continue paused tracking".

**Locked** — the involuntary reason. Maintained continuously, including while
`Paused` also holds: the lock signal keeps updating it, it simply stops
mattering, because suspension is already in force.

Its capture-only scope is inherited from map #65 and is **not final** —
[#70](https://github.com/VincentVerweij/tracey/issues/70) carries an open
question about widening it to window activity tracking, which records
`LockApp.exe` throughout every lock today. The detection mechanism is
[#69](https://github.com/VincentVerweij/tracey/issues/69)'s.

**Resume** — clearing the `Paused` reason. There is no user resume for
`Locked`; unlocking is not a resume.

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
