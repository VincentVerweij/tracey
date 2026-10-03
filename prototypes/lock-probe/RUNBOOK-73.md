# PROTOTYPE runbook — ticket #73 (map #65)

Throwaway. This answers one question: does `WTSSessionInfoEx` → `SessionFlags` track the lock state second by second, and how does it line up with `WM_WTSSESSION_CHANGE`?

## Start

```powershell
cd prototypes\lock-probe
cargo run --release
```

Everything is appended to `target\x86_64-pc-windows-msvc\release\lock-probe.log`. Type a line and press Enter at any time to add `MARK <text>` to the log. Mark the start of each scenario, e.g. `A start`. While the machine is locked you can't type, so jot down rough wall-clock times for anything you do then.

What the lines mean:

- `tick`: once a second. `wts=LOCK|UNLOCK|UNKNOWN conn=… sid=…`, then the #66 columns (foreground, BitBlt, idle).
- `EVENT`: each `WM_WTSSESSION_CHANGE`, with the ms timestamp and `SessionFlags` re-queried inside the handler. `on=msgonly|toplevel` shows which window received it. Both are registered, so you should normally see each event twice.

Leave **about 30 s of unlocked desktop between scenarios**, so the log has clean boundaries.

## Scenarios

Required: **A–C**. Ideally also **D–F**, which close gaps that #72 left open. **G** is the fast-user-switching step added from #70.

**A. Startup seed while locked.** Ctrl-C any running probe, then run:

```powershell
Start-Sleep 15; cargo run --release
```

Press **Win+L** straight away and wait on the lock screen for about 30 s, then unlock. Check that the `startup-seed` line reads `LOCK`. *(Leave this probe running for B onwards.)*

**B. Default Win+L cycle, including the credential-UI excursion.**
1. `B start`, then **Win+L**.
2. Wait on the lock screen for about 60 s.
3. Press a key to bring up the PIN/password prompt and wait about 10 s, then press **Esc** to go back to the lock screen.
4. Wait about 30 s, then bring up the prompt again and **unlock**.

**C. Lock by API.** Type `C start`, then run `rundll32.exe user32.dll,LockWorkStation` from a second terminal. Wait about 30 s and unlock.

**D. Ctrl+Alt+Del required.** *(Needs admin; revert afterwards.)*
1. Open `netplwiz` → Advanced → tick **Require users to press Ctrl+Alt+Delete**. Or set `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System\DisableCAD` = `0`.
2. `D1 start`, then **Win+L**. Wait about 30 s, press Ctrl+Alt+Del, then unlock.
3. `D2 start`, then press Ctrl+Alt+Del and choose **Lock**. Wait about 30 s and unlock.
4. Revert the setting.

**E. RDP.** *(Needs another device. Windows 11 Pro can host.)* Enable Settings → System → Remote Desktop. Keep the probe running in the console session.
1. `E start`. From the other device, connect as yourself. This takes over the console session, so expect console-disconnect and remote-connect events.
2. Inside the RDP window, press **Win+L** (or use Start → user icon → Lock). Wait about 30 s, then unlock.
3. Close the RDP window **without** signing out, so the session is disconnected. Wait about 30 s.
4. Sign back in at the physical machine.

**F. Sleep and wake.** *(Optional.)* `F start`, then Start → Power → Sleep. Wake it after about 30 s and unlock. This assumes "sign in on wake" is enabled.

**G. Fast user switching.** *(Needs a second local account.)* `G start`, then Start → user icon → **Switch user**. Sign in to the other account, wait about 30 s, then switch back and unlock.

## Hand back

Ctrl-C, then paste or attach `lock-probe.log` on #73, along with any wall-clock notes and the scenarios you skipped.
