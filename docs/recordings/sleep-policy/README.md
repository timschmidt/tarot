# System sleep policy from Settings

October 31, 2026. Signed revision 67 adds a **Let the system sleep when idle**
switch to the Automatic sleep section of the Power page. Before this, the
system policy (elogind's `IdleAction=suspend`) came only from installing the
separate `tarot-automatic-sleep` package. This is T016 progress under M4; it does
not close automatic sleep.

## Design

- **Helper.** `/usr/libexec/tarot-sleep-policy` runs behind the Polkit action
  `org.tarot.sleep-policy` (administrator authentication, active local session
  only), like the [boot destination](../boot-destination/README.md) helper.
  `status` prints JSON for anyone; `set on|off` requires root.
- **Drop-in.** The helper owns
  `/etc/elogind/logind.conf.d/50-tarot-automatic-sleep-setting.conf`
  (`IdleAction=suspend`, `IdleActionSec=10s`), separate from the package's
  `50-tarot-automatic-sleep.conf`. While the package's file is present, the
  switch shows on and cannot be changed, with a note.
- **Restart.** elogind 255 arms its idle timer only at startup. The drop-in
  records the boot it was written in, so the page can say "Restart the phone
  to finish turning this on." until the next boot.
- **Off at once.** Turning it off removes the drop-in and sends elogind SIGHUP.
  The helper finds elogind through `/run/elogind.pid`, holds it with a pidfd and
  checks its executable before signalling.
- **Observed, not assumed.** The page shows what the files say, rereads them
  after every change, and refuses a second request while one is pending.
- The user's own **Sleep after the screen turns off** setting still decides
  whether Tarot ever reports the session idle; the system switch only allows
  the system to act on it.

## Device results

| Step | Result |
| --- | --- |
| Status as the ordinary user | `{"enabled":false,"package":false,"restart_required":false}` |
| `set on` without root | Refused: "requires administrator authorization" |
| `pkexec … set on` from SSH | Refused by Polkit without a prompt (exit 127): the action allows only an active local session |
| `set on` as root | Drop-in written with the boot ID; status `restart_required`; elogind still `IdleAction=ignore` |
| Reboot | elogind `IdleAction=suspend`, `IdleActionUSec=10000000`; `restart_required` cleared |
| `set off` as root | Drop-in removed; elogind (same PID) logged "Config file reloaded" and reported `ignore` at once; a second `off` changed nothing |

**By the owner's finger** in the normal r67 session, through pkexec and Tarot's
own Polkit agent: switched on (drop-in written, restart note shown), canceled a
request to switch off (Polkit: "Request dismissed"; nothing changed, "Not
changed" message shown), then switched off (drop-in removed, elogind reloaded).
The owner confirmed every message appeared as described.

That run also found a stale-state bug: the session had read the policy as on
when it started, and an off change made from SSH afterwards was not reflected,
so the first tap asked to turn off a policy that was already off. The helper is
idempotent, so nothing went wrong. The source now rereads both system settings
(this switch and At startup) whenever the Power page opens; that fix is not in
the r67 package.

### The login screen with the policy on

With the policy armed after the reboot and no other session (SSH disconnected),
an observer script logged:

| Time | Event |
| --- | --- |
| 11:21:38 | The greeter session (Phrog) reported itself idle |
| ~11:21:48 | elogind suspended the phone (kernel suspend count 0 → 1) |
| 11:26:22 | The 5-minute RTC safety alarm woke it |
| 11:26:42 | Idle again, and the phone suspended again |
| 11:28:54 | The owner woke it and logged in; elogind's greeter cleanup ended the greeter session |

So with the policy on, the phone sleeps at the login screen about half a minute
after it is left alone, driven by Phrog's own idle hint. That matches phone
expectations; it also means remote access to a phone at the login screen ends
when it sleeps.

## Evidence

`artifacts/m4/sleep-policy/`: signed `tarot-0.1.0-r67.apk` (SHA-256
`bd5fd160c6c4d0a39362b82a576752160501c6cd36c073efc0d73ed772c3592f`, compositor
`ec6ac410b76fd0f7a7902b73595908067da5cf3c6409f0a2b4b3810d6d635831`) and its
audit, the greeter observation log, the switch monitor log and the Polkit/pkexec
journal lines.

## Limits

- The frozen native replay `tests/phone_sleep_policy_replay.py` (enable,
  cancel, disable by OCR taps) is written but has not run; the owner's own taps
  covered the same path.
- An actual automatic sleep from Tarot with this switch (rather than the
  package) was not repeated; the earlier [package run](../automatic-sleep/README.md)
  covers elogind's behavior.
- Battery-only behavior, other users' sessions and the policy for calls and
  active work remain open.
