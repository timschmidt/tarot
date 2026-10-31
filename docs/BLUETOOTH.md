# Bluetooth

Tarot's Bluetooth settings use the standard BlueZ D-Bus API. The implementation
is Rust; the system supplies BlueZ and its Linux drivers. Open the device menu
and choose Bluetooth, or find Bluetooth through Just Type.

## Controls and pairing

The page offers an adapter radio switch, Find devices/Stop searching, device
list and device details. Selecting an unpaired device starts pairing. Paired
devices have connection/disconnection controls, an explicit Trust switch and a
separate Forget confirmation. Pairing does not automatically mark a device
trusted. A successful Connect reply is followed by actual property readback; it
does not prove every profile, audio route or physical peripheral works.

The registered KeyboardDisplay agent supports PIN entry, numeric passkey entry,
code confirmation, displaying a PIN/passkey and entered-digit progress, pairing
authorization and service authorization. Displayed numeric codes have six digits,
including leading zeroes. Input is masked and held in redacted transient buffers;
it is excluded from routine status, egui text history and saved preferences.
Bluetooth PINs accept 1–16 printable ASCII characters; passkeys accept 1–6 digits.
The agent waits at most 60 seconds for an interactive response.

Incoming pairing requires the explicit “Allow new pairings for 2 minutes” switch
and a successful default-agent registration. It sets finite BlueZ discoverable
and pairable timeouts. Keep Bluetooth open to pair. Leaving the page, locking,
radio loss, changing adapter or reaching the deadline cancels the local allowance
and attempts to restore earlier adapter values. Restoration changes only fields
that still equal Tarot's lease values. BlueZ properties are shared: another
writer assigning the same values cannot be distinguished from Tarot's lease.

Paired, explicitly trusted devices may receive service authorization while the
session is unlocked. Other service requests require visible user approval.
New authorization is refused during protection; this initial policy does not
promise reconnection of every Bluetooth peripheral at the lock screen. An
already connected input device still follows the compositor's normal input and
lock enforcement.

## Service boundary and recovery

Bus operations run outside the compositor thread. Short calls have a five-second
deadline; pairing/connect/disconnect calls have a ninety-second bound and do not
block the UI. One long operation can be outstanding. Healthy idle waits for bus
signals, commands or an actual deadline; operation completion uses bounded
polling. The system bus may activate BlueZ through StartServiceByName. Explicit
private diagnostic buses never activate the host's BlueZ service.

Only the current root-owned BlueZ exporter may call the production agent. Each
request is tied to a service epoch, opaque device token, visible page and unlocked
session, checked again before returning an accepted response. Diagnostic buses
require the current test UID instead. Device removal, daemon replacement,
lock/page dismissal and agent cancellation retire requests and clear local code
entry. Tokens are not reused when an object path reappears. Subscriptions are
restricted at the broker to BlueZ object/property changes and its name ownership.
A retired connection's reader cannot clear its replacement's agent.

Discovery belongs to the requesting connection. Tarot stops its own discovery
session; a different client's scan can remain active. A failed StopDiscovery
keeps Tarot's ownership state until readback proves discovery ended or a later
stop succeeds. Daemon/bus replacement reconnects with bounded backoff and new
adapter/device identities. Actions resolve against a fresh inventory; they are
not automatically retried after an uncertain outcome.

## Verification and remaining work

`tests/bluetooth_replay.py` starts its own Xvfb, D-Bus broker and synthetic BlueZ
exporter. It never uses the system Bluetooth bus. Rendered controls exercise
radio changes, owned discovery, confirmation pairing, PIN and passkey entry,
connect/disconnect, trust and two-step Forget. Protocol cases cover spoofed and
concurrent agent callers, stale responses, lock/page cancellation, display
progress, incoming pairing, same-path replacement, daemon replacement and loss
of the whole broker. Synthetic fixed codes may appear in its private fixture
report and intentional framebuffer captures; product status and logs omit them.
Its external-discovery case models a second client's session in the exporter;
it does not claim a second real radio client was used.

Signed revision 51 passes native radio/discovery/visibility controls, independent
ordinary-user property readback, actual PAM and normal-session restoration.
[Exact evidence and limitations](recordings/bluetooth/README.md). No real peripheral pairing, Bluetooth audio, codec/profile routing, call audio,
file transfer, battery reporting, rfkill controls, accessibility acceptance or
multiple physical adapters are claimed by this initial fixture. M4 remains open.

A separate revision-52 check with the owner's JLab Studio observes real pairing
and Trust changes, but subsequent connection attempts fail; an independent
ordinary-user client reports `br-connection-page-timeout`. A2DP/acoustic checks
were not reached. The cause remains unproven. Further physical tests are deferred
at the owner's request; normal Tarot52 is independently restored with discovery
off and the saved pairing untrusted/disconnected.
[Partial results and retained failures](recordings/headphones/README.md).

October 31: with an owner-provided host USB adapter as a dedicated peer, Tarot
pairs by touch end to end on the phone: discovery, numeric comparison (the
displayed code matched the peer's), trust, connect/disconnect and Forget, each
read back from BlueZ. A first run had failed with `Page Timeout`. That was
mostly distance: at close range, BlueZ on the phone paired 6 times out of 6.
Separately, for a time no device could discover or page the visible phone,
because Wi-Fi coexistence on the shared antenna was starving Bluetooth's scan
windows. Reloading the Wi-Fi driver (`brcmfmac`) cleared it, and it could not
be reproduced afterwards. The patch firmware, the 32 kHz clock, UART flow
control and power saving were ruled out. `tests/bluetooth_reachability.py`
with `tests/phone_bluetooth_visible.py` checks the incoming direction from
the host.
[Investigation and evidence](recordings/bluetooth-peer/README.md).

The owner's JLab Studio then passed A2DP stereo (SBC) and HFP wideband voice
(mSBC) in both directions, checked by ear, after re-pairing: it had forgotten
the phone's link key (`br-connection-key-missing`). PulseAudio switches output
to the headset on connect. From revision 65 its play and pause buttons control
the player shown in the dashboard, including next and previous. From revision
66 the Sound page switches the headset between A2DP and HFP, after a fix for
the greeter's leftover PulseAudio claiming the hands-free profile. Call audio
remains untested.
[Evidence and limits](recordings/bluetooth-headset/README.md).

API references: [Adapter1](https://bluez.readthedocs.io/en/latest/adapter-api/),
[Device1](https://bluez.readthedocs.io/en/latest/device-api/) and
[Agent1](https://bluez.readthedocs.io/en/latest/agent-api/).
