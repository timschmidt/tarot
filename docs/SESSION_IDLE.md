# Session idle reporting

Tarot has an opt-in publisher for the authenticated GUI session's login1 idle
hint. This is T016 integration work. Signed revision 58 is installed as the
phone's normal default session, and its normal launcher leaves the publisher
disabled. The publisher does not implement automatic sleep by itself, and the
phone keeps its existing `IdleAction=ignore` configuration. [Full automatic-suspend plan](DEVELOPMENT_PLAN.md#m4-addition-automatic-suspend-policy).

`--session-idle` requires sleep coordination, the trusted locker, system services
and an idle policy. The local readiness calculation requires all of the following:

- A protected session and actual panel off, with a protected presentation for
  the current off counter. That historical proof survives a new locker
  generation or output geometry while the panel stays off. A pending wake retires
  readiness immediately, even before the physical panel turns on.
- An active seat, a healthy armed sleep-delay coordinator, available audio
  observation, and a held audio block when uncorked audio needs one.
- A known battery/external-power observation. The default allows readiness only
  on battery; `--session-idle-on-external-power` explicitly permits known external
  power. Unknown power stays unready under either policy.

Audio playback can coexist with an idle hint once its sleep block is held.
The manager must still enforce that block. Locking and display off keep their
existing independent behavior. The publisher calls no Suspend method and
changes no manager configuration. Persistent Power controls, packaging the
manager's actual idle-action policy, calls/work integration and native acceptance
remain required.

## Identity and lifetime

The worker uses Tarot's existing guarded local bus transport: cancellable setup,
per-call deadlines, pre-allocation frame limits and close-on-exec received FDs.
Bus I/O stays outside the compositor thread. Readiness changes and property
notifications coalesce; a healthy worker does not poll.
Diagnostic status separates local readiness from the publisher's availability
and confirmed hint. Local Ready alone does not mean the manager accepted it.

It observes the manager's unique name and requires root ownership on a physical
session, or the ordinary test user's ownership on a private fixture. Before
hint operations it resolves its actual process through GetSessionByPID and
checks the returned session's UID, local Wayland/user class, seat0 and nonzero
VT. Inactive sessions publish false. Properties are fetched afresh; signal
payloads and session environment variables do not supply that authority.
The root-owned manager's own SetIdleHint authorization permits the session's UID.
[Installed-version authorization](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-session-dbus.c).

Each connection clears a previous hint before considering current readiness.
Acknowledgements for an older readiness observation do not publish a successful
current state. Denial, timeout, owner replacement or broker loss report unknown
state and reconnect; reconnect begins with another explicit false write and
readback. Normal shutdown attempts a false write before destroying the protected
display/sleep coordination. Unsupported manager idle actions are rejected;
`suspend` also requires at least a five-second configured idle interval. The
publisher accepts `ignore` for qualification without enabling sleep.

**Idle hints are not leases.** Unlike the audio inhibitor descriptor, closing a
D-Bus connection does not clear a hint. A timed-out write may still take effect.
The private asynchronous manager deliberately applies one after the client has
exited; the final client report is unknown, not a promise that the hint cleared.
A later connection clears that stale value. The actual managed-session teardown,
installed manager behavior, failure recovery and default-enablement contract
still need native qualification. Same-UID interference is outside the Wayland
lock boundary and can also change this session-owned hint.
[Graphical-session hint storage](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-session.c).

The manager's internal idle action checks sleep blocks for every UID, while its
ordinary user-requested Suspend path exempts the requesting UID on elogind255.
This is why T016 integrates session readiness with manager policy instead of
relying on a same-user Suspend call or a non-atomic client-side inhibitor check.
[Internal action check](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-action.c).

## Verification and next steps

Both frozen current/minimum builds pass the private idle-publisher matrix.
It exercises real D-Bus method/property traffic, actual caller PID lookup,
readiness/active changes, no-poll behavior, readback after external changes,
late/denied/timed-out writes, invalid session rejection, unsupported manager
policy, owner/broker replacement and healthy/uncertain shutdown. Nested
compositor wiring remains false because a synthetic lock supplies no physical
panel-off proof. These checks do not substitute for native KMS/PAM integration.
[Evidence and retained failures](recordings/session-idle/README.md).

Signed revision 58 passes native qualification in two real diagnostic greetd/PAM
sessions, with manager action `ignore`. With external power allowed, the hint
follows automatic and manual protected panel-off, coexists with a held audio
block, and survives locker replacement while the panel is off. It clears on wake
and PAM, and an explicit false precedes session removal. Under the battery-only
default on USB power, the hint stays false with `ExternalPower` readiness. Kernel
counters are unchanged, and normal Tarot58 is independently restored with the
hint false. [Native evidence and limits](recordings/session-idle-native/README.md).

Next: persistent Power controls, then resolution of the recorded USB kernel
sleep failure. After that, complete T016's global-inhibitor, policy
packaging/rollback and actual automatic-sleep tests. Bluetooth stays
paused at the owner's request. No complete milestone is closed here.
