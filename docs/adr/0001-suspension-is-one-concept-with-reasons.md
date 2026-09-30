# ADR-0001 — Suspension is one concept carrying reasons

- **Status:** Accepted
- **Date:** 2026-09-30
- **Deciders:** Vincent Verweij
- **Resolves:** [#68](https://github.com/VincentVerweij/tracey/issues/68), under map [#65](https://github.com/VincentVerweij/tracey/issues/65)
- **Constrains:** [#62](https://github.com/VincentVerweij/tracey/issues/62) (Pause tracking ability)
- **Vocabulary:** [`CONTEXT.md` § Suspension](../../CONTEXT.md#suspension)

## Context

Two unrelated efforts each want to short-circuit background work.

[#62](https://github.com/VincentVerweij/tracey/issues/62) asks for a user-chosen pause from the tray — 15 minutes through 8 hours, until next day, or until manually resumed. Map [#65](https://github.com/VincentVerweij/tracey/issues/65) adds an involuntary one: suspending capture while the Windows session is locked.

If both land as separate `continue` statements in `start_screenshot_loop` ([`screenshot_service.rs:306`](../../src-tauri/src/services/screenshot_service.rs#L306)), the questions that follow have no defined answer — what happens when the device locks during a manual pause, what the tray renders while both hold, and what "resume" means when two causes overlap. #62 is unstarted, which makes this the cheap moment to settle it.

Relevant prior findings: [#66](https://github.com/VincentVerweij/tracey/issues/66) established that the `Locked` reason must *latch* rather than equality-test the foreground process, and [#67](https://github.com/VincentVerweij/tracey/issues/67) established that there is no runtime way to detect that a WTS registration has stopped delivering.

## Decision

### 1. One concept, carrying reasons

Suspension is a single domain concept. Tracking is suspended while at least one **suspension reason** holds. There are two reasons today, `Paused` and `Locked`, defined in `CONTEXT.md`.

The effect is an OR, so "the device locks during a manual pause" needs no rule: suspension is already in force, and the additional reason changes nothing.

### 2. It is application-level state, in a module of its own

The state lives in `AppState`, alongside the existing `sync_state` ([`commands/mod.rs:41`](../../src-tauri/src/commands/mod.rs#L41)), not inside `screenshot_service`.

This is forced rather than chosen: #62 needs three readers outside the capture loop — the tray label, the in-app indicator, and the tray menu handler that sets a pause. None of them can see a variable local to `start_screenshot_loop`, whereas all of them *can* reach `AppState` through `app.state::<AppState>()`, as the Tauri commands already do. (`tray.rs` touches no state at all today — it builds two static menu items — so #62 introduces its first state access either way.)

It gets a dedicated type and module exposing intent-named operations rather than public fields, so that rule 4 below is enforced by the type rather than by discipline at each call site.

### 3. `Locked` is maintained continuously; it stops mattering, not updating

The lock signal keeps updating the `Locked` reason while `Paused` also holds. Maintaining it is free — it is an event callback that fires only on transitions — and it means the reason is already correct when a pause ends.

The alternative, tearing lock tracking down during a pause, would require re-seeding lock state on every resume. #65 lists "whether `WTSQuerySessionInformation` connect-state can serve as the startup seed" as unresolved; that question stays confined to startup rather than becoming load-bearing on every resume, where a wrong seed means capturing a locked screen.

### 4. Each reason is cleared only by its owner

A tray resume clears `Paused`. It cannot clear `Locked`. Unlocking clears `Locked`. It does not clear `Paused`, so a pause that spans a lock survives it.

### 5. `Paused` suspends all tracking; `Locked` suspends capture only

`Paused` suspends screenshot capture, window activity tracking, idle detection and classification. `Locked` suspends screenshot capture alone.

The rationale for the asymmetry is privacy, and it is the user's: a pause is a statement that nothing should be observed. Stopping screenshots while window activity records keep accumulating in the database means data the user believed was not being collected is there to be found later. A lock is not that statement — it is an absence, and the desktop behind it is still the user's session.

`ocr_service` needs no suspension check: it is a function on the capture path, not a loop, so suspending capture stops it. `classification_loop` consults suspension explicitly rather than merely being left without input, so that it also stops draining the backlog captured before the pause.

### 6. A pause ends a running time entry, after confirmation

If a time entry is running when the user picks a pause duration, the app warns that it will be stopped. **Cancel aborts the pause entirely**; OK begins the pause and ends the entry at the pause start. Resuming does not reopen it.

This exists because decision 5 suspends idle detection, and the 300s idle prompt was the only mechanism that would have caught a forgotten timer. Without this, an 8-hour pause could produce an 8-hour time entry with no screenshots and no activity records behind it, and nothing to catch it.

No equivalent rule is needed under `Locked`: idle detection keeps running there, so the idle prompt still fires.

### 8. This ADR constrains #62; it does not merely stay compatible with it

#62 was unstarted when this was decided, which is precisely why it was decided now. Decisions 5, 6 and 7 change what #62 must build — its scope, a confirmation dialog it did not specify, and the copy for its indicator — and decisions 2 and 4 change how. Recording them as merely "compatible" would leave #62 free to re-derive them differently, which is the coordination failure this ticket was raised to prevent.

The constraints are posted as a comment on #62 rather than folded into its body, so that the original ask and what this map imposed on it stay separately legible.

What this ADR does **not** decide, and #62 still owns: the tray submenu's structure and copy, the wording of the confirmation dialog, the shape of the in-app indicator, and the exact columns backing persistence.

### 7. `Locked` is not surfaced to the user

The tray and the in-app indicator reflect `Paused` only. `Locked` is never shown to the user.

The two reasons never visually collide, because nobody can see the tray while the screen is locked, and telling the user after unlock that capture paused while their screen was locked states the obvious.

This decides the **UI** branch of #65's open "observability of suspend/resume" question — *"whether lock/unlock transitions are logged, emitted as a `tracey://` event, or surfaced in the UI"* — and only that branch. Whether transitions are logged, emitted as a `tracey://` event, or both remains [#69](https://github.com/VincentVerweij/tracey/issues/69)'s to settle, because what can be observed depends on the signal it chooses. Today the capture loop already logs a lock skip at debug level; that is the floor, not the decision.

## Amendments to standing decisions

Both are deliberate, and both were settled with Vincent during the #68 grilling.

1. **#65: "Suspension covers screenshot capture only. `activity_tracker`, `idle_service`, the running timer and the OCR/classification loops are untouched."** — Amended **for the `Paused` reason only**, by decision 5. The `Locked` reason remains capture-only exactly as charted.

2. **#65 out-of-scope: "Stopping a running time entry on lock."** — Amended **for the `Paused` reason only**, by decision 6. Stopping a running entry on *lock* remains out of scope; `idle_service` still covers it there.

## Consequences

- The capture loop's suspension check becomes a call to a shared predicate rather than a local condition, and three further loops gain the same call.
- `Paused` requires persistence on the `user_preferences` singleton, via `ALTER TABLE ADD COLUMN` as migrations `008`–`010` do. Note those four columns are all `NOT NULL` with a default, so a nullable deadline would be a departure from that precedent rather than a continuation of it — #62 owns the choice. Two things must be representable: a deadline, and "until manually resumed".
- The deadline must be **absolute wall-clock**, not a `tokio::time::Instant`. An `Instant` does not advance across system sleep, so "For 1 hour" on a laptop that is closed would not expire correctly. This one is not #62's to choose.
- Widening later is cheap: a new loop opts in by consulting the same predicate. Whether it must is decided by the test in [`CONTEXT.md` § The test for a new loop](../../CONTEXT.md#the-test-for-a-new-loop), which is the normative statement — not restated here, so the two cannot drift.
- A third reason (sleep/resume, an enterprise policy) can be added without renaming anything or revisiting the overlap semantics.
- This ADR does not choose the lock-detection mechanism, which remains #69's decision. It assumes only that *some* signal raises and clears `Locked`.

## Open question this creates

If a pause deadline expires while the screen is locked, `Paused` clears but `Locked` holds — so capture stays shut while window activity tracking, idle detection and classification resume against a locked screen, recording `LockApp.exe` as window activity. That is observation data gathered during a lock, which sits awkwardly beside the privacy rationale in decision 5.

Deliberately left open rather than settled here. Graduated to [#70](https://github.com/VincentVerweij/tracey/issues/70) ("Which session states suspend capture"), which owns the scope of the `Locked` reason.

## Alternatives considered

- **Two independent short-circuits.** Cheaper today, and it keeps the involuntary reason physically separate from user intent so no UI path can reopen it. Rejected because the overlap is undefined: the tray label, the meaning of resume while locked, and #71's resume semantics would each need an ad-hoc answer, and #62 would re-derive them.
- **A user-facing pause concept only, with lock as an invisible capture precondition outside the domain vocabulary.** Rejected because it pre-answers #65's observability question as "never observable" from the wrong ticket.
- **Plain fields on `AppState`.** Matches the `SyncState` precedent and adds no module. Rejected because decision 4 would then be unenforced, and the one place it fails is capture continuing against a locked screen.
- **Umbrella term "Paused", with lock as a second kind of pause.** Fewer terms. Rejected because "paused" would mean two things with different scopes, which is the interchangeable-vocabulary problem #68 was raised to fix.
