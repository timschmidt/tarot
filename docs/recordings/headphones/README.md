# Physical headset checkpoint

October 30, 2026, using the unchanged signed Tarot revision 52. The owner
identified JLab Studio and made it available for testing. Further physical
Bluetooth testing is now deferred at the owner's request. No complete headset
acceptance or Bluetooth audio pass is claimed.

The native touchscreen selected the exact advertised name and paired the
headphones. Independent ordinary-user BlueZ readback observed Paired=true and a
transient connection with resolved services. A later test changed Trust while
disconnected and independently verified both the enabled and restored values.
The saved pairing remains on the phone, untrusted and disconnected.

Subsequent GUI connection requests failed. A separate ordinary-user GIO client
also received `org.bluez.Error.Failed: br-connection-page-timeout`. That diagnostic
client ran in the SSH controller's cgroup; it does not replace GUI acceptance.
The cause is unproven. The owner confirmed the headphones were on, nearby, and
not connected elsewhere. Bluetooth audio modules were installed and loaded.
The observed idle UART runtime-suspend state alone does not establish a driver
fault. No kernel, firmware, radio, or audio configuration was changed.

A2DP playback and the quiet acoustic check were never reached. No test tones
played. Headset microphone, hands-free/call routing and physical Forget/re-pair
acceptance remain open.

## Retained attempts

| Attempt | Outcome |
| --- | --- |
| 01 | The QA selector could not match a two-word name; corrected to require the complete adjacent name. |
| 02 | Real pairing observed; connection/trust follow-up failed. |
| 03 | Trust enabled/restored; connection failed. |
| 04 | Scoped diagnostic trace recorded the GUI Connect call; its missing reply is not treated as a complete wire trace. |
| 05 | Independent ordinary-user Connect returned page timeout. |
| 06 | Physical-readiness window expired; no connection-stage pass. |
| 07 | Owner deferred further testing; the live controller was canceled without releasing its readiness gate. |

All attempts are retained as incomplete or failed, including the successful
individual observations within them. The cancellation exposed a QA cleanup
selector looking for Stop on a device-details page. The source now stops owned
discovery through the scoped adapter action and preserves its report even when
cleanup fails. That harness correction has syntax validation; it has not been
rerun against the headphones while testing is deferred.

The diagnostic login was ended and the exact original preferences, policies,
audio and backlight were restored. A separate final audit verifies normal
default Tarot52, loaded program hashes, actual UID/session/VT7, Panfrost,
diagnostics off, services, no virtual input, discovery/incoming visibility off,
and the saved untrusted/disconnected pairing. No headphone controllers,
monitors or playback processes remain.

The private first-six-attempt archive has 994 verified members and SHA-256
`6181215aa1eefc800485db826f948e4e56820b31af9bb499540c478554280e3b`.
The cancellation/final-audit archive has 170 verified members and SHA-256
`2fbd5bb06451192d8b2f1f8d87a92590e834711778b84e91df43a9e0d0096791`.
Local reports and frozen inputs remain under ignored `artifacts/m4/headphones/`;
device originals remain in `/home/user/tarot-dev/headphones-r52*`. The public
[evidence index](evidence.json) omits hardware addresses and unrelated devices.

## Reproduction boundary

`tests/phone_headphones_replay.py` is an opt-in, owner-assisted development
harness for this explicitly identified peripheral, using the existing installed
session replay. It requires root only for QA input/observation; controls and
playback use the ordinary user. Its readiness marker must follow an actual owner
reply in a live, unlocked session. The five-minute handoff needs suitably longer
temporary idle preferences and exact restoration afterward. A separate normal
session audit is required, including after failure/cancellation.

Do not resume physical Bluetooth tests, request headphone actions or play tones
until the owner resumes that work. The rest of Tarot development remains active.
