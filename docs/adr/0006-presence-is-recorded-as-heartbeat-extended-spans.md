# ADR-0006 — Presence is recorded as heartbeat-extended spans by a presence recorder

- **Status:** Accepted
- **Date:** 2026-10-18
- **Deciders:** Vincent Verweij
- **Resolves:** [#106](https://github.com/VincentVerweij/tracey/issues/106), under map [#104](https://github.com/VincentVerweij/tracey/issues/104)
- **Relates to:** [ADR-0002](0002-wts-session-notifications-detect-lock.md), [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md), [ADR-0005](0005-timeline-shows-presence-not-suspension.md)
- **Vocabulary:** [`GLOSSARY.md` § Presence](../../GLOSSARY.md#presence)

## Context

ADR-0005 decided that the Timeline bar shows presence (`Present` / `Away` / `Untracked`) and that an `Away` span carries its away cause. To draw any day, past or present, the bar needs the intervals of each state. Today only point events are persisted (`screenshots.captured_at`, `window_activity_records.recorded_at`). `Locked` is an in-memory bit, idle state lives in the idle loop's locals, and "Tracey not running" leaves no trace. Tracey writes nothing at shutdown, so a kill looks like a clean exit.

Gaps in the existing rows can't be used to infer presence. Idle doesn't stop window-activity records, and a lock gap looks the same as Tracey not running.

## Decision

### 1. A span table with a heartbeat-extended open span

`presence_spans(id, started_at, ended_at, state, away_cause)`, with UTC RFC 3339 timestamps and an index on `started_at`. Spans never overlap.

- Only tracked states are stored: `Present` and `Away`. `Untracked` is the gaps between rows. "Tracey not running" and, once #62 lands, `Paused` therefore need no marker.
- A transition closes the open span at the transition instant and opens the next one there.
- While a span is open, a heartbeat rewrites its `ended_at` to now every ~30 s. A crash, kill or power loss leaves the span ending at the last heartbeat, at most ~30 s late. No recovery pass is needed.
- On a clean exit (`RunEvent::Exit`), the open span is closed at the last tick, best effort.
- An idle transition is backdated (ADR-0005 decision 3). It truncates the open `Present` span to `idle_since` and opens `Away` there. The backdate is clamped to the open span's start: `Away` starts at `max(idle_since, span start)`, so it never reaches back into an `Untracked` gap, for example before Tracey started.
- A change of away cause (idle → `Locked` or back) closes one `Away` span and opens another (ADR-0005 decision 4).

### 2. A presence recorder writes the spans

A new **presence recorder** is the only writer. It runs its own 1 s tick. It is not a tracking loop and is never suspended, because it observes suspension rather than being subject to it.

- Each tick reads one snapshot: `get_idle_seconds()`, `inactivity_timeout_seconds`, `Suspension::is_locked()` and, once #62 lands, `Paused`. From that snapshot it derives the presence state and away cause, applying ADR-0005's precedence and cause rules.
- The derivation is a pure function `(open span, observation) → writes`, unit-tested without DB or platform. A thin loop applies the writes and runs the heartbeat.
- On each heartbeat it reconciles `Locked` through the existing `SessionStateQuery` (ADR-0002 decision 5), so a missed WTS event costs at most one heartbeat.
- The session watch **wakes** the recorder (a `Notify`) on each lock and unlock, so lock instants are exact rather than up to 1 s late. The recorder still reads its own snapshot. The wake only brings the tick forward.
- After each transition it writes, the recorder emits **`presence-changed`**: an in-process broadcast plus a Tauri event to the UI. Future consumers should subscribe to this derived event rather than to raw lock or idle signals.
- The idle loop is unchanged and remains the owner of the idle prompt. The recorder computes idle independently from the same platform call and threshold.
- A threshold change applies from the next tick and never rewrites recorded spans.

### 3. Sleep and hibernate are `Untracked`

The process is frozen while the machine sleeps. On resume, `get_idle_seconds()` includes the time asleep, so without special handling sleep would render as backdated `Away`. That would claim observation Tracey didn't make (ADR-0005 decision 2).

The recorder keeps its last tick instant in memory. If a tick finds more than ~10 s since the previous one, it closes the open span at the **previous tick's instant**, leaves the gap uncovered (`Untracked`), and opens a fresh span. Together with the backdating clamp, an untouched machine is `Away` from resume onward, never during the sleep.

### 4. `Paused` writes nothing

When #62 lands, the recorder closes the open span when `Paused` is raised and opens none until it clears. Paused time is a gap, and therefore `Untracked`.

### 5. The UI reads spans with `presence_list`

`presence_list(request: { from, to }) → Vec<PresenceSpan>`, following `screenshot_list`'s convention.

- Spans overlapping `[from, to)` are returned (`started_at < to AND ended_at > from`), sorted ascending by `started_at`.
- Spans are **unclipped**. The UI clips them to the bar, so a hover hint can show a span's true extent across midnight.
- `PresenceSpan { started_at, ended_at, state: "Present" | "Away", away_cause: "Locked" | "Idle" | null, open: bool }`. `open` marks the span currently being extended, which only happens on today. The UI grows it to now and redraws it on `presence-changed`.
- `Untracked` is never returned. Every uncovered stretch is `Untracked`.

### 6. `Locked` as a suspension reason stays in-memory

ADR-0003 and `GLOSSARY.md` describe `Locked` as "in-memory; re-seeded at startup, never persisted." That describes the *reason*: live state that gates the tracking loops. It still holds. Presence spans record lock *history*, and nothing ever reads them back to decide whether `Locked` holds now. ADR-0003's decision is neither amended nor superseded. It gains a pointer to this ADR.

## Consequences

- A new migration adds `presence_spans`. Retention, cleanup and sync of span data remain open on map #104 ("Span storage lifecycle").
- Days recorded before this ships have no spans. How they render remains open on map #104 ("Historical days").
- Today's bar grows live from `open` plus `presence-changed`, without polling.
- **Disconnected sessions** become `Away` through idle. `GetLastInputInfo` reports session-specific input for the calling session only (Microsoft documentation), and a disconnected session receives none, so its idle count keeps growing. This is confirmed by documentation, not by test.
- The idle reading has a pre-existing 49.7-day wraparound bug: the 32-bit `LASTINPUTINFO.dwTime` is subtracted from the 64-bit `GetTickCount64()`. On long uptimes it would paint the whole bar `Away`. It is tracked as its own bug.

## Alternatives considered

- **Append-only transition event log, spans derived at query time.** Rejected: a crash leaves the last state without an end, so it needs a heartbeat anyway. It then becomes this design with an extra derivation step.
- **Infer presence from gaps in screenshots and activity rows.** Rejected: idle doesn't stop activity records, and a lock gap looks the same as Tracey not running.
- **Fold recording into the idle loop.** Rejected: `Paused` will suspend idle detection (#62), but the recorder must keep running to close spans and leave paused time untracked.
- **Event-driven pushes from the session watch and idle loop into a writer.** Rejected as the primary mechanism. Idle has no OS event and is polled anyway. Two producers raise ordering questions, such as a backdated idle transition landing before an already-recorded lock. Its benefits are kept through the wake (exact lock instants) and the `presence-changed` broadcast (extensibility at the derived level).
- **Let sleep be `Away`.** Rejected: Tracey doesn't observe while the machine sleeps. A closed laptop overnight would become a 14-hour red bar.
