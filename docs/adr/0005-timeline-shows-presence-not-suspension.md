# ADR-0005 — The timeline bar shows presence, not suspension

- **Status:** Accepted
- **Date:** 2026-10-17
- **Deciders:** Vincent Verweij
- **Resolves:** [#105](https://github.com/VincentVerweij/tracey/issues/105), under map [#104](https://github.com/VincentVerweij/tracey/issues/104)
- **Relates to:** [ADR-0001](0001-suspension-is-one-concept-with-reasons.md), [ADR-0003](0003-locked-is-lock-only-and-suspends-window-activity.md)
- **Vocabulary:** [`GLOSSARY.md` § Presence](../../GLOSSARY.md#presence)

## Context

Map #104 redesigns the Timeline bar. Its background replaces screenshot dots with colored spans. The map called them "tracking-state spans" and charted them as green for active, red for locked, and uncolored when not tracked. #105 had to decide what those states mean before #106 decides how to record them.

Suspension (ADR-0001) answers one question: whether Tracey is observing. The user wants the bar to answer a different one: whether the user was working at the device. Idle detection already reports this, and it keeps running under `Locked` (ADR-0003), so the data exists.

## Decision

### 1. The bar shows presence

Each span is in exactly one **presence** state:

| State | Holds when | Color | UI word |
|---|---|---|---|
| `Present` | Tracked, not `Locked`, not idle | green | "Active" |
| `Away` | Tracked, and `Locked` or idle | red | "Away" |
| `Untracked` | Tracey not running, or `Paused` holds | none | "Not tracked" |

Idle is not a third color. It shares red with locked, which is consistent with map #104 ruling idle out of scope as a separate visual state.

### 2. Precedence: `Untracked` > `Away` > `Present`

`Paused` suspends all tracking, idle detection included, so nothing is observed. The bar does not claim `Away` for time it did not watch. Locking the screen while paused stays uncolored.

### 3. Idle `Away` is backdated to the last input

Idle means no input for `inactivity_timeout_seconds`. The span starts at `idle_since` (now minus the idle seconds), the instant the idle prompt already reports. It does not start at the moment the threshold was crossed. As a result, away-time shorter than the threshold stays `Present`.

### 4. `Away` records its cause, and color ignores it

An `Away` span carries its **away cause**: `Locked` or idle. When both hold, the cause is `Locked`, the more specific fact. A change of cause splits the span into adjacent `Away` spans. The color is the same either way. The cause exists so that a hover hint ("Away · locked 12:03–12:41") is possible. #107 decides whether to show it.

### 5. The bar is observation; the idle-prompt resolution never recolors it

`break`, `meeting`, `keep` and `specify` describe **time entries**. A span stays `Away` whatever the user answers. The bar records what was observed at the device and is never rewritten afterwards, which also means recording does not depend on the prompt being answered.

## Consequences

- #106 must record idle transitions as well as lock and pause transitions and process lifetime, and must store the away cause.
- `Locked` will be persisted as part of presence history. ADR-0003 calls it "in-memory; never persisted" as a suspension reason. Whether persisting it as history amends that wording is for #106 to settle.
- A disconnected session is not `Locked` (ADR-0003). It becomes `Away` through idle, assuming the platform's idle reading keeps working while disconnected. This assumption is unverified and is left to #106.
- A meeting resolved as `meeting` still shows red. The time entries carry that interpretation; the bar does not.

## Alternatives considered

- **Show suspension (green = tracking runs, idle stays green).** Rejected: the user wants to see when they were not working, and suspension cannot show that.
- **Start idle `Away` at the threshold crossing.** Rejected: every away-stretch would begin with about five minutes of false green, and the bar would disagree with the idle prompt.
- **Let `keep`/`meeting` turn `Away` back to `Present`.** Rejected: it would mix interpretation into observation, and it would make spans depend on a dialog that may never be answered.
- **Drop the away cause.** Rejected: one small field, and the information cannot be recovered later for days already recorded.
