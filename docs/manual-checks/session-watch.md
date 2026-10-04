# Manual check: the session watch

The session watch ([ADR-0002](../adr/0002-wts-session-notifications-detect-lock.md)) is the one piece of `Locked` that can't be automated. It owns a message-only window, registers it for WTS session notifications, seeds `Locked` at startup, and is not started under the `test` feature. A real lock can't be scripted, so this checklist verifies it end to end by hand. It is an **accepted manual gap** ([#74](https://github.com/VincentVerweij/tracey/issues/74)). No fake session source or scripted lock is kept.

The baseline is the #73 probe log ([lock-probe.log](https://github.com/user-attachments/files/33012372/lock-probe.log), summarised in [#73's resolution](https://github.com/VincentVerweij/tracey/issues/73)). It showed that `SessionFlags` reads `LOCK` on every locked tick, agrees with each event within about 1 ms, and never returns `UNKNOWN`. On sleep and wake it showed the queued `LOCK` and `UNLOCK` arriving together after the user is already back. The expectations below follow from that log.

Run this checklist whenever the session watch, the seed or the reconcile changes.

## Setup

1. Build and start Tracey once with `cargo tauri dev` from `src-tauri`.
2. In Settings, turn logging on and set the level to **trace**. Trace also writes debug lines, which include the codes the watch ignores. The setting is persisted and applies from the next start too.
3. The log is `tracey.log` next to the executable, which is `src-tauri\target\x86_64-pc-windows-msvc\debug\` for a dev build. Each line is JSON with a UTC `ts`. The database `tracey.db` sits beside it. To follow the session watch only:

   ```powershell
   Get-Content src-tauri\target\x86_64-pc-windows-msvc\debug\tracey.log -Wait |
     Select-String 'session_watch|\(source: '
   ```

4. Note the wall-clock time at the start of each scenario. You can't type while the machine is locked. Leave about 30 s of unlocked desktop between scenarios so the log has clean boundaries.

Each scenario lists the log lines it expects (the `message` field) and what the database should show. Run the queries under [Database checks](#database-checks) with the scenario's start time.

## Scenarios

### A. Startup while unlocked

Start Tracey on an unlocked desktop.

- [ ] Exactly one seed line: `session unlocked at startup; Locked stays clear (source: seed)`.
- [ ] No `WTSRegisterSessionNotification failed` warning.
- [ ] Capture and activity rows appear as before. Startup is not a suspension end, so there is no `suspension_end` screenshot.

### B. Startup while locked

1. Quit Tracey from the tray.
2. From a terminal, start the built executable after a delay and lock straight away:

   ```powershell
   Start-Sleep 15; & src-tauri\target\x86_64-pc-windows-msvc\debug\tracey.exe
   ```

   Press **Win+L** as soon as the command is running. The seed runs in the Rust backend, so it doesn't matter whether the window's frontend loads. To have it load, keep `dotnet watch run --project src/Tracey.App --urls http://localhost:5000` running in another terminal.
3. Wait about 30 s on the lock screen, then unlock.

- [ ] The seed line is `Locked raised (source: seed)`, with no warning about an unknown state or a failed query.
- [ ] On unlock, `Locked cleared (source: event)`.
- [ ] No screenshot and no activity row between the start and the unlock.
- [ ] About 2 s after the unlock, one screenshot with trigger `suspension_end`. One activity row at the unlock.

### C. Win+L and unlock

Leave Tracey running from B.

1. Press **Win+L** and wait on the lock screen for about 60 s. That is longer than the capture interval if it is set to 60 s or less, which tests that an interval due during the lock is absorbed.
2. Press a key to bring up the PIN or password prompt and wait about 10 s on it.
3. Unlock.
4. Stay in the same window you locked from for a few seconds.

- [ ] On lock, `Locked raised (source: event)`.
- [ ] On unlock, `Locked cleared (source: event)`.
- [ ] No `(source: query)` warning. In #73 the query never disagreed with an event, so a warning here means something is out of step.
- [ ] No screenshot and no activity row while locked. In particular, no `LockApp.exe` row.
- [ ] Exactly one `suspension_end` screenshot, about 2 s after the unlock, and no `interval` screenshot alongside it.
- [ ] Exactly one activity row at the unlock, even though it is the same window as before the lock.

Optionally repeat with `rundll32.exe user32.dll,LockWorkStation` from a terminal, or with Ctrl+Alt+Del → **Lock**. #73 showed both emit the same events.

### D. Sleep and wake

This assumes "sign in on wake" is enabled. Sleep and resume handling is out of scope (#76). This scenario only checks that a sleep doesn't leave `Locked` stuck.

1. Start → Power → **Sleep**.
2. Wake the machine after about 30 s and unlock.

- [ ] Nothing is logged from entering sleep until after the unlock. In #73 the process was frozen throughout.
- [ ] On thaw, `Locked raised (source: event)` and `Locked cleared (source: event)` arrive milliseconds apart. If a loop's reconcile runs between the two events, a `Locked cleared (source: query): missed unlock event` warning may appear between them. That is the query winning over a stale event, as intended.
- [ ] `Locked` ends clear: screenshots and activity rows carry on after the unlock.
- [ ] No lock-screen screenshot and no `LockApp.exe` activity row. A single `suspension_end` screenshot and fresh activity row appear only if a loop ticked while `Locked` held. When both events land between ticks, there is no re-entry, and that is fine.

## Database checks

Replace the timestamp with the scenario's start time in UTC, in the same form as the log's `ts`.

```sql
SELECT captured_at, trigger, process_name, window_title
FROM screenshots
WHERE captured_at >= '2026-10-04T12:00:00'
ORDER BY captured_at;

SELECT recorded_at, process_name, window_title
FROM window_activity_records
WHERE recorded_at >= '2026-10-04T12:00:00'
ORDER BY recorded_at;
```

## Log lines

These are the lines the session watch writes (component `tracey_lib::platform::windows::session_watch`). The source in brackets is what each check looks for.

| Level | Message | When |
|---|---|---|
| INFO | `Locked raised (source: seed)` | Startup on a locked session |
| INFO | `session unlocked at startup; Locked stays clear (source: seed)` | Startup on an unlocked session |
| WARN | `Locked raised (source: seed): session state unknown (…), failing closed` | Seed read `UNKNOWN`. Never seen in #73 |
| WARN | `Locked raised (source: seed): session state query failed (…), failing closed` | Seed query failed. Never seen in #73 |
| INFO | `Locked raised (source: event)` | `WTS_SESSION_LOCK` |
| INFO | `Locked cleared (source: event)` | `WTS_SESSION_UNLOCK` |
| DEBUG | `session watch: session change 0x… ignored` | Any other session code |
| WARN | `Locked raised (source: query): missed lock event` | The reconcile found a lock the events missed |
| WARN | `Locked cleared (source: query): missed unlock event` | The reconcile found an unlock the events missed |
| WARN | `session watch: WTSRegisterSessionNotification failed (attempt N): …` | Registration failed, typically at logon. Retried with backoff |
| INFO | `session watch: registered after N failed attempts` | Registration recovered |

## Results

Record each run here, newest first: date, build (commit), Windows version, each scenario's outcome, and any unexpected lines.

| Date | Commit | Windows | A | B | C | D | Notes |
|---|---|---|---|---|---|---|---|
| | | | | | | | |
