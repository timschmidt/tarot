# Native session idle hints on the PinePhone Pro

October 30, 2026. Signed revision 58 publishes the actual authenticated GUI
session's login1 `IdleHint` from protected physical panel-off readiness. The
experiment needs explicit `--diagnostics --session-idle` launcher flags. The
normal installed session leaves it disabled, and the phone defaults to normal
Tarot58. The manager's `IdleAction` stays `ignore` throughout, so no automatic
or requested system sleep occurs. This is scoped T016 progress. It does not
complete T016 or M4. [Contract](../../SESSION_IDLE.md).

## Native acceptance

The root-only controller verifies the signed package and its payload hashes, then
starts two real greetd/PAM diagnostic logins on VT7. An independent GIO observer
resolves the compositor's own login1 session and checks that the manager is
root-owned, the session belongs to UID 10000, and it is an active local Wayland
`user` session on seat0/VT7. The observer subscribes to that session's
`PropertiesChanged` and `SessionRemoved` signals. Each read confirms that the
manager still has the same owner and that `IdleAction` is `ignore`.

**External power allowed** (`--session-idle-on-external-power`, phone on USB power):

- An owned null-sink stream holds Tarot's audio sleep block (one manager grant,
  UID 10000, compositor PID). A temporary 15 s lock / 20 s off policy locks the
  session and turns the panel off. The driver then reads DPMS Off and backlight
  power 4. Readiness is `Ready`, and the independent hint reads true while the
  audio block remains held.
- Corking the last stream while the panel is off releases the grant. The hint
  stays true.
- A neutral wake contact followed by actual PAM restores the same card and view.
  The hint clears to false, and readiness returns to `Active`. The original
  idle preferences are restored and saved.
- A uinput power key turns the panel off, and the hint is true again. The
  controller verifies the locker's identity, then kills it through a pidfd.
  A new owner and lock generation arrive while the panel stays off. The off
  counter and historical protected-off proof stay unchanged, the session shows
  no unprotected presentation, and the hint stays true.
- The panel is woken, the replacement locker presents, and actual PAM
  authenticates. The hint clears. A final power-off sets it true. Diagnostic
  exit then writes an explicit false **before** the manager removes the
  session. The observed sequence is
  true → false → true → false → true → false → session removed.

**Battery-only default** (no external-power flag, same USB power): the same
automatic and manual power-off steps report `ExternalPower` readiness. The hint
stays false, and the observer records no hint change before session removal.

Both sessions leave the kernel suspend counters unchanged. No Suspend method is
called, and no physical audio, Bluetooth or SIM is used. The replay unloads only
its verified null sink and restores every original card mode, channel, mute,
route and default. Afterward the controller returns to normal Tarot58. Policies,
rules, network profiles, the active connection, system-bus and NetworkManager
identities, backlight, Tarot/MIME preferences and feedback settings all match
their baseline. An independent audit of the normal session then verifies:
loaded r58 hashes, Panfrost, hardware keys on, diagnostics and metadata off,
no `--session-idle` arguments, `IdleHint` false, `IdleAction` ignore, no Tarot
audio block and no QA resources.

## Retained failure

The first native controller (failed1) passed automatic off with audio, PAM
wake, manual power-off and the owner-replacement proof. It then waited
25 seconds for lock phase `Locked` before sending the wake contact. A
replacement owner that arrives while the panel is off cannot present, so the
compositor correctly keeps it protected in `Preparing` until wake. The QA
helper now accepts `Awaiting`/`Preparing` with the output off and a live,
non-restarting locker. After the wake contact it still requires a fully
presented `Locked` generation before any credential input. No runtime code or
package changed. The failed run's restoration passed, and a separate post-failure
normal audit passed. Its evidence is kept apart from the accepted run.

## Host and package scope

Rust 1.98.1 and minimum 1.89.0 each pass 280 workspace tests, with nine ignored.
Both compilers' binaries pass the fourteen-case private idle-publisher matrix
again on this source. Its first rerun attempt could not find a system Xvfb; that
log is retained, and the project's bundled Xvfb is used instead. The clean
Alpine cross-build uses 446 locked registry packages. All 1178 project files in
the staged archive match the working tree. The package audit verifies AArch64
executables, unchanged dependencies, PAM configuration and session entry, and
the absence of installation scripts. Only the four executables and README change
from r57. Target unit tests are not executed by abuild.

| Artifact | SHA-256 |
| --- | --- |
| Canonical source archive | `1441f159bb2755b5391ba929cde95efeed39470716f0654334e38e64186bcaea` |
| Signed APK `tarot-0.1.0-r58.apk` | `5de0f9dc6bb0eaa0c7847e0d9e025e09faba69860e5cd53ef9efc7376ee84e28` |
| `tarot` | `e8bed4890e12092598c49a58b425ba918de9e785c691309f6de1c0b49736905b` |
| `tarot-session` | `18cdb48a371fc34a9885ad733ba0e85ca89741b070a2bd7bad5d61a79065dbfd` |
| `tarot-lock` | `6f7420e83b27ff8318e5ea35b453377cb514113c27a1f3b923635c77a5269488` |
| `tarot-keyboard` | `1a2209533d17ac4f594dc7096baacf82cb150a520d0712012e80a0f908419c9c` |
| Accepted native payload (637 inputs) | `d6faa9e40880b1078ead67517c2cf26eeed2abb671a0816556a86b871d64834e` |
| Accepted native evidence (656 members) | `bee02d3b3c269a1f71321ca84838507921c200db999562456f088d591413e5d0` |
| Failed1 proof archive | `6a8d4ab78231a9d203eede49d06a2e9a0cf003aae9a7b4d3c971f70bda1f40ff` |

The ignored `artifacts/m4/session-idle-native/` directory keeps the build
script, package audit, both payloads, controllers, audits, collectors, both
private matrices and logs. [Evidence index](evidence.json).

## Limits and next work

The manager action was `ignore`, so this proves hint publication, retirement and
teardown ordering. It does not prove the manager's idle timing or its enforcement
of all-UID blocks. Because the phone stayed on USB power, the battery-only result
is a policy refusal, not a measurement on battery. Next in T016: persistent Power
controls for automatic sleep and external power, an explicit packaged
manager-policy path with rollback, global-inhibitor qualification, and genuine
RTC-woken automatic sleep. That last step requires the retained `fe800000.usb`
`-11` kernel failure to be resolved first. Physical Bluetooth stays deferred.
