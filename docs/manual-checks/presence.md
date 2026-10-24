# Manual check: the presence recorder

The presence recorder ([ADR-0006](../adr/0006-presence-is-recorded-as-heartbeat-extended-spans.md)) writes `presence_spans`. Its derivation is unit-tested, but a real lock, real idle input, a real sleep and a real kill can't be scripted. This checklist verifies them end to end by hand. Run it whenever the recorder, its derivation or the session watch's wake changes.

## Setup

1. Build and start Tracey with `cargo tauri dev` from `src-tauri`. Leave the idle threshold (Settings → Inactivity timeout) at a value you can wait out, such as 60 s.
2. The database `tracey.db` is next to the executable unless that folder isn't writable (then `%APPDATA%\tracey\`). For a dev build that is `src-tauri\target\x86_64-pc-windows-msvc\debug\`. Open it read-only with any SQLite client, for example `sqlite3 -readonly tracey.db`. WAL mode lets you read while Tracey runs.
3. Note the wall-clock time (UTC) at the start of each scenario. Timestamps are stored as UTC RFC 3339, such as `2026-10-20T09:00:00.000Z`.

All scenarios use this query, with `:since` set to the scenario's start time:

```sql
SELECT started_at, ended_at, state, away_cause,
       ROUND((julianday(ended_at) - julianday(started_at)) * 86400) AS seconds
FROM presence_spans
WHERE ended_at > :since
ORDER BY started_at;
```

Spans never overlap. This should always return no rows:

```sql
SELECT a.id, b.id FROM presence_spans a JOIN presence_spans b
  ON a.id < b.id AND a.started_at < b.ended_at AND b.started_at < a.ended_at;
```

## Scenarios

### A. Lock and unlock give an exact `Away(Locked)` span

1. Work normally for at least a minute.
2. Note the time, press **Win+L**, wait about 60 s, note the time again and unlock.
3. Use the desktop for another minute.

- [ ] A `Present` span ends at the lock and an `Away` / `Locked` span starts at the same instant, within about a second of the noted lock time.
- [ ] The `Away` / `Locked` span ends at the unlock, and a `Present` span starts at the same instant.
- [ ] No gap between the three spans.

### B. Going idle backdates `Away(Idle)` to the last input

1. Note the time, then stop touching mouse and keyboard and wait at least the idle threshold plus 30 s.
2. Move the mouse.

- [ ] The `Present` span ends at about the noted time (the last input), **not** at the moment the threshold was crossed.
- [ ] An `Away` / `Idle` span starts where `Present` ends and lasts about the time you were idle.
- [ ] A `Present` span starts when you moved the mouse.

### C. Sleep leaves a gap

1. Note the time and put the machine to sleep (Start → Power → Sleep). Leave it for at least two minutes.
2. Wake it, sign in, and don't touch anything for a few seconds; then use it normally.

- [ ] The span that was open before the sleep ends within a second or two of the noted time.
- [ ] Nothing covers the sleep: the next span starts after the wake, leaving a gap of about the sleep's length.
- [ ] No span covering the sleep is `Away`. If you stayed on the lock screen after waking, the first span after the gap is `Away` from the wake onward.

### D. A kill leaves the span at most ~30 s late

1. Use the desktop normally for at least a minute.
2. Note the time, then kill Tracey: `Stop-Process -Name tracey -Force`.
3. Start Tracey again after about a minute.

- [ ] The span open at the kill ends no more than about 30 s before the noted time, never after it.
- [ ] A fresh span starts at the restart, leaving the time Tracey was not running as a gap.

### E. A clean exit closes the open span

1. Note the time and quit Tracey from the tray menu (**Exit**).

- [ ] The span open at the exit ends within about a second of the noted time.
