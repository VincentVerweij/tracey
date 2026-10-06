# ADR-0004 — When suspension ends, each loop runs a defined re-entry

- **Status:** Accepted
- **Date:** 2026-10-03
- **Deciders:** Vincent Verweij
- **Resolves:** [#71](https://github.com/VincentVerweij/tracey/issues/71), under map [#65](https://github.com/VincentVerweij/tracey/issues/65)
- **Builds on:** [ADR-0001](0001-suspension-is-one-concept-with-reasons.md), [ADR-0002](0002-wts-session-notifications-detect-lock.md), [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md)
- **Vocabulary:** [`GLOSSARY.md` § Suspension](../../GLOSSARY.md#suspension)

## Context

ADR-0001 to ADR-0003 settle when `Locked` holds and what it suspends. None of them says what a suspended loop does when it is allowed to observe again.

Left to itself, that behaviour falls out of whatever state the loop held when it stopped:

- `start_screenshot_loop` ([`screenshot_service.rs:306`](../../src-tauri/src/services/screenshot_service.rs#L306)) captures on an elapsed interval **or** a 2s-debounced change of `last_window_key`. A lock longer than the interval leaves an interval capture already due. Returning to a different window also arms the debounce, so one unlock can yield two near-identical captures. Returning to the same window yields at most the interval one.
- `start_activity_loop` ([`activity_tracker.rs`](../../src-tauri/src/services/activity_tracker.rs)) writes a row only when `(process, title)` changes. Returning to the window you left writes nothing. ADR-0003 recorded this as a consequence: the lock then leaves no boundary in the activity data.

Activity rows are point events with only `recorded_at`. `auto_create_or_extend_time_entry` extends an auto entry when a new row arrives within `auto_classification_group_gap_seconds` (120 by default) of its `ended_at`. Without a row after unlock, the time spent back in the same window is not counted until the next window change.

`GLOSSARY.md` reserves **Resume** for clearing `Paused`, and states that unlocking is not a resume. Unlocking also does not end suspension while `Paused` still holds. So the edge this ADR needs is neither "resume" nor "unlock".

## Decision

### 1. Suspension ending is an explicit edge, with a defined re-entry per loop

Each suspended loop keeps one bit, *suspended last tick*. On the first tick where it was suspended and is not any more, the loop runs its **re-entry** before observing anything. Nothing about the post-suspension behaviour is left to state frozen at the moment suspension began.

### 2. The edge is *suspension ends*, per loop, whichever reason cleared last

**Suspension ends** for a loop on the first tick where no reason that suspends *that loop* still holds. It does not matter which reason cleared, or how:

- unlock after a pause has expired,
- a tray resume onto an unlocked screen,
- the pre-observation `SessionFlags` reconcile clearing a missed `Locked` (ADR-0002 decision 5, ADR-0003 decision 3).

All three go down the same path. Unlocking while `Paused` still holds is not an edge, because the loop stays suspended.

Under `Locked`, the edge applies to screenshot capture and window activity tracking, the two loops it suspends. Idle detection and classification reach it only when a pause ends. Their re-entry is [#62](https://github.com/VincentVerweij/tracey/issues/62)'s to define, on this edge.

Startup is not a suspension end. If the seed reports `Locked`, the first unlock is an ordinary re-entry. If it reports unlocked, today's first-tick behaviour stands.

### 3. A suspended tick skips everything, including the foreground query

While suspended, a loop does not query the foreground window or update its change-detection state. This follows ADR-0003 decision 3: the reconcile comes before any observation. The state a loop freezes this way never matters, because re-entry resets it.

### 4. Window activity tracking's re-entry: one forced fresh row

On re-entry the tracker resets `last_window` to `None`. The existing rule that the initial state counts as a change then writes a row on that tick, even when the user returns to the window they left.

- **The deny-list still applies.** A forced row has no special status. If the window is denied, nothing is written, as on any tick.
- **No marker row when suspension starts.** The gap between the last pre-suspension row and the re-entry row already records the suspension. A sentinel would be a new kind of row that sync and classification would both have to learn to skip.
- **Suspensions shorter than `group_gap` are bridged.** A 90s lock in the middle of an auto entry merges into it, exactly as a 90s break without locking does today. This is accepted, not a new inaccuracy.

### 5. Screenshot capture's re-entry: one settled capture, then the interval restarts

On re-entry the capture loop resets `last_window_key` and arms the existing 2s debounce. When that debounce fires, it:

- captures with a new trigger value, **`"suspension_end"`**,
- resets `last_interval_capture` to that instant, so regular cadence runs from the re-entry capture.

So one capture lands about 2s after suspension ends, whether or not the window changed, and an interval that came due during the suspension is absorbed by it rather than firing as well.

- **2s, not immediate.** The settle skips the unlock transition (desktop repaint, a stray `explorer.exe` "UnlockingWindow" tick seen in #66), and it reuses the debounce rather than adding a timer.
- **Not the next interval boundary.** The unlock is when the screen has changed most since the last good capture. Classification reads the most recent screenshot at or before a row's `recorded_at`, so with a long interval, waiting would leave the re-entry row from decision 4 without evidence for up to that interval.
- **Re-suspending within the settle** is safe. Suspended ticks skip everything, so the armed debounce cannot fire. The next re-entry arms it again.
- **The new trigger value** is honest for the same-window case, where `"window_change"` would be false. The Timeline badge ([`Timeline.razor:443`](../../src/Tracey.App/Pages/Timeline.razor#L443)) falls back to showing the raw string, so nothing breaks before it gets a label of its own. The `models` comment listing trigger values gains it.

### 6. No credential-suppression window

`WTS_SESSION_UNLOCK` is delivered after the credential is accepted, once the secure desktop has gone. #66 showed `BitBlt` failing for the whole credential UI. By the time `Locked` clears, there is no credential entry left to capture, so no extra suppression is added.

## Consequences

- Every suspension longer than a tick leaves exactly one boundary in each suspended loop's output: one activity row (unless denied) and one `"suspension_end"` capture.
- Unlock behaviour no longer depends on whether the user returns to the same window, or on where the interval clock stood.
- #62 gets the edge without having to re-derive it. Its tray resume is one more way suspension can end, and only its re-entries for idle detection and classification remain to be decided.
- Short lock/unlock cycles each cost one row and one capture. That is accepted. The `SessionFlags` reconcile already runs before each of them.

## Alternatives considered

- **Emergent re-entry: just stop skipping.** No new state. Rejected: behaviour would be an accident of where the check sits and of whether the window matches, and a same-window unlock leaves no boundary at all.
- **Re-entry on unlock, rather than on suspension ending.** Rejected: unlocking during a pause would run a re-entry for loops that are still suspended. Tying it to `Locked` alone would also give #62 a second, parallel edge.
- **Capture at the next interval boundary.** Keeps the cadence perfectly even. Rejected under decision 5: it delays evidence for the most-changed moment by up to a full interval.
- **Capture immediately on the first unsuspended tick.** Rejected: it risks capturing the unlock transition, and it needs separate handling to avoid a second, debounce-driven capture 2s later.
- **A marker row at suspension start.** Rejected under decision 4: it is a new row kind every consumer must skip, and the timestamps already carry the gap.
