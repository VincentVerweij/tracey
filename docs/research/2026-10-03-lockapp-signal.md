# Does the `LockApp.exe` foreground signal hold outside the default lock configuration?

Research for [#72](https://github.com/VincentVerweij/tracey/issues/72), part of map [#65](https://github.com/VincentVerweij/tracey/issues/65).
Feeds the mechanism decision in [#69](https://github.com/VincentVerweij/tracey/issues/69).
Date: 2026-10-03. Plan-only: no production code was touched.

**Question.** Is `LockApp.exe` as the foreground process a lock signal that holds beyond the
default consumer lock screen? If not, what fronts the lock screen instead?

**Starting evidence.** The #66 probe (`prototypes/lock-probe/findings-2026-09-21.log`), one
Windows 11 machine with default settings, Win+L. Polling `GetForegroundWindow` → owning process,
it saw `NULL` for one tick, then `LockApp.exe` / "Windows Default Lock Screen" for the whole steady
lock, then `explorer.exe` / "UnlockingWindow", then `NULL` throughout credential entry and unlock.

**Source policy.** Each claim is labelled:

- **Documented**: Microsoft Learn, Microsoft Support, or another first-party Microsoft page.
  Microsoft Q&A answers are *not* counted as documented unless a Microsoft employee wrote them and
  they cite docs. They are community content on a Microsoft domain.
- **Community-observed**: forums, Q&A threads, third-party blogs, other projects' issue trackers.
- **GAP**: something I looked for and could not establish. I did not reason around it.

Every claim has a URL.

---

## Verdict

**No. Outside the default local lock, the `LockApp.exe` signal is not something you can rely on.**

Microsoft does name `LockApp`, in exactly one place I found: a Microsoft Support article. That
article separates two lock-time surfaces:

- the **user lock screen (LockApp)**, shown after Win+L, which "runs in the user session"
- the **Secure Lock screen (LogonUI)**, which "runs on the Winlogon secure desktop under the SYSTEM
  account". It appears "after restart, during initial sign-in, or after pressing Ctrl + Alt + Delete".

This confirms the probe's picture for the Win+L path. It also says the LogonUI surface is on the
secure desktop. A user-desktop process cannot see that surface as a foreground window, which is
consistent with the `NULL` the probe saw. What it implies:

1. **The LockApp phase is optional.** Microsoft documents a policy (`NoLockScreen`) under which the
   user goes "straight to their selected tile". There is then no LockApp curtain, and the lock is
   all `NULL`/LogonUI. On Azure Virtual Desktop with Entra single sign-on, the default response to a
   lock is to *disconnect* the session rather than show any remote lock screen.
2. **The LockApp phase can end before the lock does.** Pressing Ctrl+Alt+Del, or starting
   credential entry, moves to the LogonUI secure desktop while the session is still locked. The
   probe saw this as `NULL` for about three minutes.
3. **LockApp can show while the session is *not* locked.** Another project, on Windows 11 in 2026,
   saw `LockApp.exe` stay the foreground window for 46 minutes of real work after an unlock.
   (Community-observed.)

Microsoft's own API reference says there is no function to ask whether the workstation is locked.
It sends you to `WTSRegisterSessionNotification` instead. No Microsoft source treats LockApp as a
contract.

---

## 1. What Microsoft documents about LockApp at all

| Claim | Label | Source |
| --- | --- | --- |
| "the user lock screen (LockApp) shown after pressing Windows key + L runs in the user session and uses a dynamic timer that refreshes at the next minute boundary" | **Documented** (Microsoft Support) | <https://support.microsoft.com/en-us/windows/secure-lock-screen-clock-may-appear-up-to-30-seconds-behind-7093a752-93b3-423a-9558-902bceb2ae47> |
| "On the Windows Secure Lock screen (for example, after restart, during initial sign-in, or after pressing Ctrl + Alt + Delete) … The Secure Lock screen runs on the Winlogon secure desktop under the SYSTEM account … This behavior is specific to the Secure Lock screen (LogonUI)." Applies to Windows 11 and Windows 10. | **Documented** (Microsoft Support) | same |
| A UWP "lock screen app" concept exists: `LockApplicationHost` "Allows the lock screen app to request that the device unlocks". It has been in the API since Windows 10 10.0.10240 (UniversalApiContract v1). | **Documented** (Learn) | <https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.lockscreen.lockapplicationhost> |
| `LockWorkStation`: "There is no function you can call to determine whether the workstation is locked. To receive a notification when the user locks the workstation or logs in, use the `WTSRegisterSessionNotification` function". It also says the call "has the same result as pressing Ctrl+Alt+Del and clicking **Lock**." | **Documented** (Learn) | <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-lockworkstation> |
| `GetForegroundWindow`: "The foreground window can be NULL in certain circumstances, such as when a window is losing activation." The page says nothing about lock, LockApp, or secure desktops. | **Documented** (Learn) | <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow> |
| "Winlogon always starts the process Logon UI after it receives a secure attention sequence event." Credential providers draw their tiles "on the secure desktop". | **Documented** (Learn) | <https://learn.microsoft.com/en-us/windows-server/security/windows-authentication/credentials-processes-in-windows-authentication> |
| The Learn Win32 / API reference names neither the `LockApp.exe` process nor its window title ("Windows Default Lock Screen"), and makes no promise about either. | **GAP** (null result) | Searched learn.microsoft.com. The only hits were Q&A threads such as <https://learn.microsoft.com/en-us/answers/questions/5684812/windows-default-lock-screen-app> |
| The package lives at `C:\Windows\SystemApps\Microsoft.LockApp_cw5n1h2txyewy`. Renaming that folder stops LockApp loading and "you'll still be able to sign in normally". | **Community-observed** (independent advisor on Q&A) | <https://learn.microsoft.com/en-us/answers/questions/5684812/windows-default-lock-screen-app> |
| The LockApp lock screen window is on the user's **Default** desktop, not a separate lock desktop. | **Community-observed** (Q&A asker's own testing) | <https://learn.microsoft.com/en-us/answers/questions/668625/creating-windows-on-the-lock-screen> |
| LockApp "only does something when you're at the lock screen" and suspends after sign-in. With the lock screen disabled, users go "straight to the sign-in screen". | **Community-observed** (How-To Geek, 2018) | <https://www.howtogeek.com/366271/what-is-lockapp.exe-on-windows-10/> |

The Support article's wording is "after pressing Ctrl + Alt + Delete". It does not say whether this
means pressing Ctrl+Alt+Del *on a locked screen* (to reach credential entry) or the *Ctrl+Alt+Del →
Lock* path. `LockWorkStation` is documented as equivalent to that second path. **GAP: which surface
fronts a lock started from the Ctrl+Alt+Del menu, or by `LockWorkStation`, is not stated.** The
probe used Win+L only.

---

## 2. Per-configuration findings

### 2.1 Ctrl+Alt+Del required for sign-in (`DisableCAD` = 0 / "Interactive logon: Do not require CTRL+ALT+DEL" = Disabled)

- **Documented.** If the policy is disabled, "any user is required to press CTRL+ALT+DEL before
  logging on" (unless using a smart card). The registry value is `DisableCAD` under
  `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System`.
  <https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/interactive-logon-do-not-require-ctrl-alt-del>
- **Documented (archived page).** "This policy is disabled by default on workstations and servers
  that are joined to a domain. It is enabled by default on stand-alone workstations." In other
  words, **domain-joined machines require Ctrl+Alt+Del by default.**
  <https://learn.microsoft.com/en-us/previous-versions/ms814164(v=msdn.10)>
  The current Windows 10/11 page's default table lists "Client Computer Effective Default
  Settings: Disabled" without separating domain from stand-alone. The two pages do not obviously
  agree. **GAP: the current default per join state is not stated unambiguously.**
- **Documented.** "Do not display the lock screen" (below) describes its effect only for "users
  that aren't required to press CTRL + ALT + DEL". This shows that Microsoft treats the
  Ctrl+Alt+Del-required case as a different lock-screen experience. It does not say what that
  experience is.
  <https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay#cpl_personalization_nolockscreen>
- **Documented.** Pressing Ctrl+Alt+Del is a secure attention sequence, and "Winlogon always
  starts the process Logon UI after it receives a secure attention sequence event"
  (Credentials Processes page, §1). The Secure Lock screen shown "after pressing Ctrl + Alt +
  Delete" is LogonUI on the secure desktop (Support article, §1).
- **Community-observed.** With Ctrl+Alt+Del enforced, a woken Windows 10 machine shows "the lock
  screen, but no Sign In button until I press Ctrl+Alt+Del". So a lock-screen image still appears
  first. The thread does not say which process draws it.
  <https://learn.microsoft.com/en-us/answers/questions/4080450/why-do-i-have-to-press-ctrl-alt-del-to-get-the-sig>
- **GAP.** Whether a Win+L lock with Ctrl+Alt+Del required still shows a `LockApp.exe` curtain
  ("Press Ctrl+Alt+Delete to unlock") in the user session, or goes straight to LogonUI on the
  secure desktop. No Microsoft source says, and I found no community trace that names the process
  in this configuration. Two third-party write-ups (askvg, elevenforum) look relevant but returned
  HTTP 403 and could not be read. **This is the single most decision-relevant gap for a corporate
  fleet, and it can only be answered empirically**: rerun the #66 probe with `DisableCAD=0`.

### 2.2 Group Policy that removes or alters the lock screen

- **Documented: `NoLockScreen` removes the LockApp phase.** "Do not display the lock screen"
  (`CPL_Personalization_NoLockScreen`; `HKLM\Software\Policies\Microsoft\Windows\Personalization`
  → `NoLockScreen`): "If you enable this policy setting, users that aren't required to press CTRL +
  ALT + DEL before signing in will see their selected tile after locking their PC. If you disable
  or don't configure this policy setting, … [they] will see a lock screen after locking their PC.
  They must dismiss the lock screen using touch, the keyboard, or by dragging it with the mouse."
  It applies to Pro, Enterprise, Education and IoT Enterprise editions: Windows 10 2004/20H2/21H1
  with KB5005101 and later, and Windows 11 21H2 and later.
  <https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay#cpl_personalization_nolockscreen>
  The "selected tile" is a credential-provider tile, which credential providers draw on the secure
  desktop through LogonUI (§1). So with this policy a lock goes **straight to the `NULL`/LogonUI
  phase, with no `LockApp.exe` foreground at any point.** The docs state the two halves of that
  chain. The joined-up "no LockApp foreground" conclusion is my inference, and an empirical check
  would close it.
- **Documented.** The embedded/IoT unattend setting `Microsoft-Windows-Embedded-EmbeddedLogon` →
  `NoLockScreen` = 1 "Disable[s] the lock screen functionality and UI elements."
  <https://learn.microsoft.com/en-us/windows-hardware/customize/desktop/unattend/microsoft-windows-embedded-embeddedlogon-nolockscreen>
- **Community-observed.** People recommend `NoLockScreen` on consumer machines as a way to hide
  LockApp. Q&A advice applies it even on Home (registry only).
  <https://learn.microsoft.com/en-us/answers/questions/5684812/windows-default-lock-screen-app>
- **Documented, but not relevant to the signal.** "Force a specific default lock screen and logon
  image" and "Prevent changing lock screen and logon image" change only the *image*. Nothing in
  their text suggests they change which process hosts the lock screen.
  <https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-admx-controlpaneldisplay>
  **GAP: no source confirms one way or the other that they leave LockApp in place.**
- **"Force legacy logon UI".** **GAP.** I found no current Microsoft policy by that name for
  Windows 10/11, and no source on how it would affect LockApp.

### 2.3 Third-party credential providers (smart card, Windows Hello for Business, SSO agents)

- **Documented.** Credential providers plug into LogonUI and enumerate tiles "on the secure
  desktop", including for unlock: "The logon and authentication architecture lets a user use tiles
  enumerated by the credential provider to unlock a workstation." Organisations can control
  "workstation lock/unlock policies, by using customized credential providers."
  <https://learn.microsoft.com/en-us/windows-server/security/windows-authentication/credentials-processes-in-windows-authentication>
- **Implication, not a finding.** Credential providers live on the LogonUI side. For the
  foreground-process mechanism they show up as the `NULL` phase, whoever wrote them.
- **GAP.** Whether any third-party credential provider or SSO agent replaces or suppresses the
  LockApp curtain. Microsoft documents that `LockApplicationHost` serves "the lock screen app",
  which suggests the lock app was designed to be replaceable. A search snippet from that API area
  also mentions an app being "removed as the user's default lock app". I could not find that
  sentence on the class page or the `Unlocking` event page I fetched, so treat it as
  **unverified**. I found **no** documentation that a non-Microsoft lock app can be installed on
  desktop Windows today, and no vendor that does it.
- **GAP.** Smart-card removal ("Interactive logon: Smart card removal behavior" → Lock
  Workstation): which surface appears. Not researched to a source.
- **Windows Hello / Hello for Business.** **GAP.** No Microsoft source describes Hello changing
  which process fronts the lock. Hello face sign-in is commonly seen working from the LockApp
  curtain, but I found no source I could cite for that.

### 2.4 Windows Server, RDP, and multi-session hosts

- **Documented: AVD / Windows 365 with Entra SSO do not show a lock screen at all by default.**
  "You can choose whether the session is disconnected or the remote lock screen is shown when a
  remote session is locked." Defaults: "Single sign-on using Microsoft Entra ID → Disconnect the
  session". "Legacy authentication protocols → Show the remote lock screen". This applies to
  Windows 11 single/multi-session (KB5037770+), Windows 10 21H2+ (KB5039211+), and Windows Server
  2022 (KB5037782+). The policies are "Disconnect remote session on lock for Microsoft identity
  platform authentication" / "… for legacy authentication".
  <https://learn.microsoft.com/en-us/azure/virtual-desktop/configure-session-lock-behavior>
  In the disconnect case the process that observes the foreground has no presented desktop.
  **GAP: what `GetForegroundWindow` returns in a disconnected session is not documented.** It is
  the same open question as the disconnected-capture GAP in
  `docs/research/2026-09-21-wts-session-notifications.md` §5, on branch
  `research/wts-session-notifications`.
- **Documented.** The "remote lock screen" exists as a concept, but the AVD page does not name the
  process that draws it. **GAP: whether the remote lock screen in an RDP/AVD session is
  `LockApp.exe` or LogonUI.**
- **Community-observed.** Since Windows 10 1803 / Server 2019, an NLA RDP session that
  auto-reconnects after a network drop is restored "to a logged-in desktop rather than the login
  screen" (CERT/CC VU#576688, 2019-06-04). Microsoft said this "does not meet the Microsoft
  Security Servicing Criteria". So in RDP a lock can be undone without any unlock UI appearing.
  <https://www.kb.cert.org/vuls/id/576688>,
  <https://www.sei.cmu.edu/blog/expectations-of-windows-rdp-session-locking-behavior/>
- **Community-observed.** Windows Server 2016 shows an "Unlock the PC" screen when a user locks
  the server. The process is not named.
  <https://learn.microsoft.com/en-us/answers/questions/539568/how-to-remove-unlock-the-pc-screen-from-server-201>
  Third-party write-ups say Server 2012 R2 / 2016 and later display a lock screen
  (<https://techjourney.net/disable-lock-screen-in-windows-10-8-1-windows-server-2016-2012-r2/>).
- **Documented.** The `NoLockScreen` CSP lists only client editions (Pro/Enterprise/Education/IoT).
  Server is outside MDM CSP scope, so that page says nothing about Server either way.
- **GAP.** Whether `LockApp.exe` / `Microsoft.LockApp_cw5n1h2txyewy` is present and runs on
  Windows Server 2016/2019/2022/2025 (Desktop Experience) or on Windows 10/11 Enterprise
  multi-session. A search for the package name together with Server versions returned nothing.
  Server defaults to Ctrl+Alt+Del required (§2.1), which on its own puts Server into the §2.1
  unknown.

### 2.5 Windows 10 vs 11, and servicing changes

- **Documented.** The Support article that names LockApp and LogonUI "Applies To: Windows 11,
  Windows 10". As of that article, the Win+L → LockApp-in-user-session split is the same on both.
- **Documented.** Windows 11 keeps changing lock-screen *content*, for example lock-screen widgets
  (<https://blogs.windows.com/windowsexperience/2025/10/16/new-experiences-currently-rolling-out-for-windows-11/>).
  None of the release notes I found says lock-screen hosting moved to another process.
- **GAP.** No Microsoft commitment that lock-screen hosting stays in `LockApp.exe` across releases,
  and no record of whether it ever moved. When LockApp replaced earlier hosting (Windows 8/8.1) is
  not documented by Microsoft. Community blogs disagree ("Windows 8" vs "Windows 10") and cite
  nothing.
- **Community-observed.** On Windows 11 25H2 (build 26200.7462), the "Windows default lock screen"
  app briefly appears on the taskbar after wake and closes. That is LockApp surfacing in the user
  session outside a steady lock.
  <https://learn.microsoft.com/en-us/answers/questions/5684812/windows-default-lock-screen-app>

### 2.6 False positives: LockApp in the foreground while *not* locked

- **Community-observed (directly relevant).** On Windows 11, ActivityWatch's window watcher saw
  `LockApp.exe | Windows Default Lock Screen` as the foreground for 15:49:32–16:21:41 and
  16:22:09–16:36:04, while screenshots showed live work. "LockApp had stuck as the foreground
  window after an unlock; the session was not locked." Opened 2026-09-14, still open.
  <https://github.com/Cordedmink2/activity-to-timesheet/issues/77>
  For tracey this is the opposite failure from the "missed signal" one: a false *lock* would
  suspend capture during real work. It also means "LockApp in foreground ⇒ locked" cannot be
  treated as an invariant, even on the default configuration.

### 2.7 Dynamic lock and `LockWorkStation`

- **Documented.** `LockWorkStation` "has the same result as pressing Ctrl+Alt+Del and clicking
  Lock". It is asynchronous, and a nonzero return "does not indicate whether the workstation has
  been successfully locked."
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-lockworkstation>
- **Documented.** "The dynamic lock feature only locks the device if the Bluetooth signal falls
  **and** the system is idle." The page does not say which lock UI is shown.
  <https://learn.microsoft.com/en-us/windows/security/identity-protection/hello-for-business/hello-feature-dynamic-lock>
- **GAP.** Which surface (LockApp or LogonUI) fronts a lock started by dynamic lock or by
  `LockWorkStation`. The only lead is the Support article's "after pressing Ctrl + Alt + Delete →
  Secure Lock screen (LogonUI)", together with "LockWorkStation = Ctrl+Alt+Del → Lock". Read
  together they *suggest* a programmatic lock might land on LogonUI, but the article's wording is
  ambiguous (§1) and **this must not be treated as established.**

---

## Could not establish

Do not let these get rounded up into facts.

- **Ctrl+Alt+Del-required lock path.** Whether Win+L with `DisableCAD=0` still shows a
  `LockApp.exe` curtain before LogonUI. This is the decisive unknown for domain-joined machines,
  where the archived Learn page says Ctrl+Alt+Del is required by default. It needs an empirical
  rerun of the #66 probe.
- **Which surface fronts a `LockWorkStation`, Ctrl+Alt+Del → Lock, dynamic lock, smart-card
  removal, or idle-timeout ("machine inactivity limit") lock.** The probe covered Win+L only.
- **RDP / AVD remote lock screen process.** Whether it is LockApp or LogonUI. Also what
  `GetForegroundWindow` returns in a disconnected session, which is the AVD Entra SSO default on lock.
- **LockApp on Windows Server and multi-session SKUs.** Whether it is installed and runs.
- **Any Microsoft commitment** that `LockApp.exe` or its window title is stable across Windows
  servicing. None found. A null result on Learn.
- **Third-party lock apps / credential providers** suppressing or replacing the LockApp curtain.
  The concept of a pluggable "lock screen app" exists in the UWP API, but I found no shipping
  case.
- **The cause of the LockApp-sticks-after-unlock false positive** in the ActivityWatch report, and
  how often it happens. One report, no reproduction steps.
- **The Ctrl+Alt+Del default for domain vs stand-alone** on current Windows 10/11. The archived and
  current Learn pages differ in how they state it.

---

## What this hands #69

This informs the decision. It does not make it.

- **Coverage.** The foreground-process signal sees `LockApp.exe` only during the *user lock
  screen* phase. Microsoft documents configurations with no such phase: `NoLockScreen` on
  Pro/Enterprise/Education, and AVD/Windows 365 Entra SSO, which disconnects on lock. It also
  documents that the phase ends once the user reaches the LogonUI secure desktop, while the
  session is still locked. A design that keys on `LockApp.exe` must therefore treat `NULL` as
  lock-adjacent, as the #66 probe already suggested. That is the case where `GetForegroundWindow`
  is documented only as "can be NULL … such as when a window is losing activation". It is not a
  lock indicator.
- **False locks are now a known failure mode** (§2.6), not just missed locks. The two mechanisms
  fail in different directions here. WTS gives explicit `WTS_SESSION_LOCK`/`UNLOCK` events.
  Foreground polling has to infer unlock from LockApp going away, which reportedly can fail to
  happen.
- **Documented guidance points one way.** Microsoft's `LockWorkStation` reference says there is no
  lock-state query and names `WTSRegisterSessionNotification` as the way to track lock and unlock.
  `LockApp.exe` has exactly one first-party mention (a Support article about a clock), with no
  contract attached.
- **Capture side effect worth carrying forward** (from the #66 log, not new research): during the
  LockApp phase `BitBlt` *succeeded* and returned a non-black frame (mean ≈ 110, i.e. the lock
  screen image). It only failed with `ERROR_INVALID_HANDLE` during the `NULL`/LogonUI phase. So
  the existing reactive `BitBlt` net does **not** cover the LockApp phase. Whatever mechanism #69
  picks has to cover that phase itself.
- **If foreground-process stays in the running**, the cheapest evidence to gather next is a probe
  rerun under `DisableCAD=0`, under `NoLockScreen=1`, with a `LockWorkStation`-initiated lock, and
  inside an RDP session. Those four runs would close most of the GAPs above.
