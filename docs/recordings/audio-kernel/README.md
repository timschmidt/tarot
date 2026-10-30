# Audio-block suspend requests on the PinePhone Pro

October 30, 2026. Normal installed Tarot57 rejects the explicit, qualified
suspend requests described below while audio is active. The kernel control
fails in the USB controller. This records scoped progress and an unresolved
native failure; the full development plan remains active. No runtime update,
package installation, preference/policy change or physical Bluetooth test occurs.

## Outcomes

| Check | Result |
| --- | --- |
| Silent null-device playback | Manager denies suspend with Tarot as the sole non-root sleep block |
| Owned null-monitor recording | Same denial; samples discarded without inspection/storage |
| Playback and recording together | Same denial with one shared Tarot grant |
| All streams corked, identical requester/method/flags | Manager accepts request; deep sleep and s2idle fallback both fail in USB |
| Final refusal-only replay | Passes all three denials, with no kernel-control request |
| Cleanup and independent normal-session audits | Pass after both the failed control and final refusal replay |

Every denied request returns the manager's specific root/block AccessDenied
error, with no PrepareForSleep signal or change in kernel success/failure
counters. Independent ListInhibitors reads identify Tarot UID 10000 and the
loaded compositor PID. Its separate sleep-delay grant remains present.
The streams use zero PCM on an owned null sink and recording from that sink's
monitor. No physical speaker, microphone, headset or Bluetooth device is used.

The control is **failed**, not a successful sleep/wake sample. The manager
attempts deep sleep, then fallback s2idle. Both log
`dwc3 fe800000.usb: ... dwc3_plat_suspend returns -11`; the final kernel failure
counter increases **2→4**, success remains **0**, and no meaningful suspended
time occurs. The first observation catches failure 3 while fallback is still
running; final cleanup records 4. The final replay now waits for the entire
manager preparation to finish before recording a future control's final counts.
That revised positive-control path has not been requalified on working hardware.

## Exact request semantics

The phone runs `elogind-255.24-r0`. Its ordinary D-Bus suspend authorization
ignores inhibitors belonging to the requesting UID; an ordinary root request
also bypasses other users' blocks. Consequently an unchecked root test or a
Tarot-UID ordinary request cannot establish the intended audio policy.
The new replay uses root **SuspendWithFlags(uint64 1)**, the installed version's
ROOT_CHECK_INHIBITORS flag. Root-owned SSH blocks remain untouched and exempt;
Tarot's UID-10000 block is the sole block relevant to this request. This proves
explicit manager refusal attributable to Tarot, while preserving SSH access.
It does not establish global automatic-suspend policy or an inhibitor-free
successful kernel cycle.
[Flag definition](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/basic/login-util.h),
[request authorization](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-dbus.c),
[UID exemption](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-inhibit.c).

The existing Rust root observer also changes to this explicit request and
rejects a non-root sleep block before trying its protected-KMS positive test.
Its reports record method, flags and the UID-exemption scope. Targeted current
and minimum-Rust checks, formatting and strict host/AArch64-musl DRM Clippy pass.
The updated observer is not installed or exercised by this Python phone replay;
its actual protected-presentation/reliability acceptance remains separate.

## Restoration and evidence

Both independent audits verify the same normal/default Tarot57 session and all
four loaded executable hashes, Panfrost, hardware keys, diagnostics/metadata off,
GUI-owned audio/feedback providers, original preferences/audio controls/policies,
NetworkManager/system-bus identities and service health. Test streams, null
modules, audio grants and owned RTC alarms are removed. The phone remains in
normal Tarot; there is no new unlock/PAM or physical-button acceptance here.

| Artifact | SHA-256 |
| --- | --- |
| Unchanged installed revision-57 APK | `9287b6d723209e9cdd8fc2eca0436de79ca7d008703b7d862eb00b2601059753` |
| Final frozen refusal-only QA payload | `068e55e00b33708c82454b73c9593740d9f6424886671733fa778ae6adc62f2e` |
| Collected native archive | `7bc9169be2d11a0f5c6109a581edd7afd33bb5c087cc7d493dd69f1c72f38928` |

All 52 archive members and 51 file digests verify; final phone QA source bytes
match the repository. Device originals are in `audio-kernel1` through
`audio-kernel4` under `/home/user/tarot-dev/`. Full evidence, exact-version
research, targeted compiler logs and the independent verifier remain under
ignored `artifacts/m4/audio-kernel/`. The public [index](evidence.json) preserves
accepted refusals and failed control as separate outcomes.

Three earlier fixture/preflight failures remain retained. The first uses an
APK info form that prints descriptions rather than the package version. The
second cannot read a helper below a root-private input directory. The third
programs an alarm but refuses its overly narrow deadline check. None requests
suspend, and each changed-resource attempt cleans up. Independent RTC-only
qualification measures a 16.730-second future alarm from `rtcwake -s 15`, with
RTC/system mapping below three seconds. The corrected test records that mapping
and requires an owned alarm 3–20 seconds ahead. The root-owned input directory
is searchable/readable by the test user's group; result directories stay private.

## Replay and remaining work

Use the root-only [phone replay](../../../tests/phone_audio_suspend_replay.py)
with an exact signed-package audit and verified current compositor PID. Stage
its two imported helpers alongside it, with read access for UID 10000, and use
a new private output directory. It refuses another phone, another elogind
version, mismatching binaries, existing streams, unrelated non-root sleep blocks
or an existing RTC alarm. By default it checks refusals only. Explicit
`--kernel-control` adds one real RTC-woken sleep request after corking; `--detach`
keeps the controller independent of the SSH channel. An uncertain request keeps
its wake alarm through a bounded recovery window before cleanup.

Automatic suspend is still absent: display off does not request system sleep.
[T016](../../DEVELOPMENT_PLAN.md#m4-addition-automatic-suspend-policy) qualifies
an actual GUI-session idle hint and daemon-owned idle action before enabling
power policy. The inspected internal idle-action path checks all UIDs, avoiding
the ordinary-request exemption; a client-side precheck alone has a grant race.
[Idle dispatch](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind.c),
[internal action enforcement](https://raw.githubusercontent.com/elogind/elogind/v255.24/src/login/logind-action.c).
Current USB recovery, unplugged tests, protected resume/actual PAM on the current
binary, work/call policy, physical audio and power/endurance remain required.
