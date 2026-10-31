# Real automatic sleep through the session manager

October 30, 2026. Signed revision 61 completes the first real end-to-end
automatic sleep on the PinePhone Pro. Tarot reports the authenticated session
idle after a protected screen-off, and elogind's own idle action suspends the
phone. Tarot then performs its protected resume, and actual PAM restores the
session. This is T016 steps 2–4 progress on external power. It does not complete
T016 or M4.
[Contract](../../SESSION_IDLE.md) · [Plan](../../DEVELOPMENT_PLAN.md#m4-addition-automatic-suspend-policy).

## What revision 61 delivers

- **The installed launcher always starts the idle publisher.** With
  **Automatic sleep** off, which is the default, the hint stays false. Only the
  external-power override remains a diagnostic flag.
- **The opt-in subpackage `tarot-automatic-sleep`** installs
  `/etc/elogind/logind.conf.d/50-tarot-automatic-sleep.conf` with
  `IdleAction=suspend` and `IdleActionSec=10s`. It depends on the exact Tarot
  revision and elogind, has no install scripts, and removing it restores the
  distribution's `ignore`. The main package does not change manager policy.

## Platform finding: enabling needs an elogind start

elogind 255.24 arms its idle-action timer only at startup (`manager_startup` →
`manager_dispatch_idle_action`) and from that timer. If `IdleAction` is `ignore`
at startup, the dispatcher returns without arming, and a later SIGHUP reload to
`suspend` never starts the timer.
[Source](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind.c).

The first native controller (failed1) proves this. It enabled the policy by
SIGHUP, then:
- the sole-session window opened
- Tarot reported `Ready` with the hint true
- the gate was released
- no suspend followed within 100 s

**Enabling therefore needs an elogind restart or a reboot. Disabling by SIGHUP
works:** the armed timer next sees `ignore` and stops re-arming. The shipped
r61 drop-in comment says SIGHUP is enough for both; that comment is wrong, and
it is corrected in source for the next package revision.

## Native acceptance (check5)

The signed subpackage was installed, an RTC safety alarm armed, and the phone
rebooted with owner authorization. elogind started with `IdleAction=suspend`.
A real greetd/PAM normal login preceded the root controller.

The controller moved itself to the root cgroup. Otherwise its SSH login session
would never count as idle: elogind treats a session with no TTY as never idle.
It then started a diagnostic Tarot session. The replay:

1. Held a root `idle` block from the start, so the manager could not act before
   the RTC alarm was armed.
2. Saved Automatic sleep (on, immediately, external power allowed) and
   15-second lock and screen-off timeouts.
3. Waited until this GUI session was the manager's **only** session and no
   other sleep or idle block existed. That required closing all SSH sessions;
   the distribution's SSH sleep-inhibitor plugin also releases then.
4. Required Tarot `Ready` with independent `IdleHint` true and the panel off.
5. Armed RTC +120 s and released its gate, with no further input or Suspend
   call.

The results:

- The manager suspended the phone. Kernel success went 0→1 with failures 0,
  and the phone was asleep for 102.7 s (boot-time minus monotonic clock).
  Ping from the host dropped for the same window.
- A root `idle` block taken immediately after resume prevented repeated sleep.
  It was held until the policy was removed.
- Tarot's protected sleep coordination counted one more protected preparation
  and one more protected resume. Unprotected presentations were identical from
  the `Ready` state through resume, and the lock stayed `Locked`.
- After resume the hint cleared to false and readiness returned to `Active`.
- Actual touch-typed PAM restored the same cards and view. Settings were
  restored, and the observer saw an explicit false before session removal.
- The controller removed the subpackage and reloaded elogind (`ignore`), then
  released its block. It restored preferences, audio and controls exactly and
  logged normal Tarot61 back in.
- An independent audit verified normal Tarot61 with the publisher running, the
  hint false, `IdleAction=ignore`, the drop-in absent, Panfrost, hashes and
  services.

Both compilers pass 288 workspace tests. The r61 package audit verifies an
unchanged main payload layout and a subpackage containing only the drop-in.

| Artifact | SHA-256 |
| --- | --- |
| Canonical source archive | `4bd7658ab270b267b8d86ba2e0c1410ff1e67eb894364ab797c88ec2eb54e154` |
| Signed APK `tarot-0.1.0-r61.apk` | `a870ec2c136db7d95596ff27ee23967878a38fb1b8bfec214052d3119c476a10` |
| Subpackage `tarot-automatic-sleep-0.1.0-r61.apk` | `de6dbc8012d39856146736316c7963c3c63cf40b12da99337fa5c48e6be8ad2b` |
| Drop-in file | `87eeacc05c5bd9e19b0f705e87fe05fa7de3c234a31f500849f56f1b6d3a9394` |
| Accepted check5 evidence (652 members) | `ff2cb30fe8fd88ee06fe20a0008fb61b2eec1e64833b16b0c7e40c83a58fef7e` |

## Retained failures

None of these runs changed runtime code. Each restored the phone to normal
Tarot and passed an independent post-failure audit. Check2 and check3 stopped
before testing.

| Run | Cause |
| --- | --- |
| failed1 | SIGHUP did not arm the idle timer (the platform finding above). |
| check2 | The controller hard-coded the previous boot's system-bus PID. |
| check3 | The controller asserted `ignore` before a policy active from boot; its restoration removed the policy. |
| check4 | The manager **did** suspend (success 0→1, 99.2 s asleep), with a protected preparation and resume. The replay then compared unprotected presentations to the unlocked baseline, not the protected `Ready` state. The counts at `Ready` and resume were equal (342). |

## Limits

- **Power source:** the phone stayed on USB power, so this covers
  external-power automatic sleep only. Battery automatic sleep, charging
  transitions and power measurements remain open.
- **Other sessions:** any SSH command session, any non-graphical session
  without an idle TTY, or another graphical session without an idle hint
  correctly prevents the action. The greeter's own idle behavior with the
  policy installed, other users' blocks, audio/call/work policy and multi-cycle
  endurance remain to be qualified.
- **No Settings page for the policy:** installing the subpackage and rebooting
  are manual today.

Physical Bluetooth remains deferred.
