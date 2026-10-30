# Text-input state reset across consecutive enables

Recorded October 30, 2026, Detroit. The Firefox activation investigation exposed
a separate, reproducible text-input-v3 state bug in pinned Smithay. This change
does not resolve the retained Firefox held-key cancellation or close a complete
development-plan milestone.

## Failure and correction

A real ordinary Wayland client sends this single uncommitted batch:

1. Enable text input, then set synthetic surrounding text `STALE`, a
   sensitive-data hint, terminal purpose, change cause and cursor rectangle.
2. Enable again and commit, without supplying replacement fields.

The revision-49 compositor forwards `STALE`, hint 128, purpose 13 and change
cause 1 to the trusted input method. These fields precede the last enable and
should have been discarded. Both the original fixture and final-QA comparison
fail on that actual delivery, before any composition callback. The failed
reports remain failed.

The [text-input-v3 enable contract](https://raw.githubusercontent.com/wayland-mirror/wayland-protocols/main/unstable/text-input/text-input-unstable-v3.xml)
resets previous enable/disable and associated state requests. The local
[Smithay correction](../../../third-party/smithay-text-input-enable-reset.patch)
replaces the complete pending state at each enable, retaining only that enable
until subsequent requests supply fresh fields. It does not defer or suppress
activation, alter key submission timing, forward canceled held input or infer
DOM field identity from surface/serial equality.

A separate case supplies `FRESH`, new selection bounds, normal purpose/hints
and change cause after the last enable. Only those fields reach the method.
The dispatcher still produces one committed activation/done pair per batch.
The patch and the existing metadata patch apply in either order to the original
Smithay 0.7.0 file and reproduce the vendored source exactly. Smithay's MIT
license remains in place; this is a local correction, not an upstream backport.

## Host scope

The corrected current and minimum-Rust compositors pass both reset cases and
the complete real protocol replay: Unicode preedit/commit, UTF-8 deletion,
stale serial rejection, disabled/background and cross-client isolation,
untrusted privileged-global rejection and helper exit. A separate rendered
terminal replay passes modifiers, navigation, hide/purpose/landscape behavior
and the existing held-key context-reset cancellation with a fresh key afterward.
These tests retain their original expectations and deadlines.

Both compilers pass 239 workspace tests. The first ordinary sandbox run could
not send into its private test socket (`EPERM`); its result is retained. Running
with the required private-socket permission passes. This is separate from the
real protocol failures above. Strict host and DRM-enabled AArch64 Clippy pass.

The host uses private displays and synthetic text. These results do not prove
physical-finger behavior, native KMS, battery or current-binary suspend endurance.

Current host compositor SHA-256:
`179e3d42a6ef03780b6b5491e2a14dab04d756548f8d77b3f746444a33d23437`.
Minimum-Rust compositor:
`81e956facdce4f81ebf2bb4662fce8baae7dd92fc16d99e56aac4b5da5b48fc5`.
Retained failing revision-49 compositor:
`c645a76fbd11dcd3e4d62b3b5f317754473810e60ae4cfe3255cec3a08d4e505`.

The final-QA before/after comparison uses identical fixture/replay hashes. The
first reproduction's earlier fixture is preserved separately. The independently
verified private host archive has 646 members, including 524 frozen
runtime/QA/patch inputs, SHA-256
`8543959ae0a7a0938e3f1db31663328a6d8c2ae92b67ff253e86827356792ff0`.

## Package

One clean offline AArch64 Alpine/DRM build succeeds with 446 checksum-verified
registry packages. The source archive verifies all 987 project-source entries;
current runtime/QA inputs match the package snapshot. Final documentation follows
that freeze. No target tests are executed by the cross build. Runtime
dependencies, PAM, session entry and installed license files are unchanged.
All four executable bytes change in the clean rebuild; the installed README and
THIRD-PARTY documentation also change. The latter contains the new local-patch
attribution and is verified against its frozen source.

Source archive SHA-256:
`20866e9b4befdb87dfe145c1870a9974004cd0fe5ac3487de00e042effd153c4`.
Revision-50 APK:
`b0a8091088f5d5f3aad2586b0eeb32c7eae5411d73fb1e70ab0e7fd6ee18bab6`.

| Program | Package SHA-256 |
| --- | --- |
| tarot | `a22c3762e60202b43f49910bc323f30a2f90bdf3795fb136267f767c5eed2e3b` |
| tarot-session | `16d27def8b8ddd98032030a3fdefe80e7d0b75d83a10102539d3806f1cb59a59` |
| tarot-lock | `4884b187381daa900944e84bfb16634e0a281915be8a37d891632fd8a728fed2` |
| tarot-keyboard | `387fd4b335894f86ebaca1a2b4bb64bc5e0ff7e42793829ff070beb062a8cd69` |

## Native delivery and final state

The phone verifies the development signature and private root-owned APK copy,
simulates a Tarot-only upgrade, installs 49 → 50 and verifies the installed
payload hashes. The first native controller passes both reset cases with the
ordinary Rust keyboard. Each starts from an actual 288-unit terminal keyboard
and returns to the 240-unit normal keyboard after the final enable, with fresh
touch text accepted. A third fresh character succeeds after actual PAM.

The fixture runs as UID 10000 in the real authenticated login cgroup; its loaded
executable hash and process start identity are checked. The parent also passes
protected startup/all four protected orientations, Calculator/audio and complete
login-cgroup cleanup. The fixture closes cooperatively, its temporary catalog
entry is removed and normal Tarot 50 is started through real PAM.

An independent audit verifies loaded payloads, default Tarot/ordinary-user VT7,
Panfrost, services, hardware keys on, diagnostics/metadata off, no stale fixture,
control socket, replay device or RTC alarm. Exact owner preferences/MIME data,
ownership/modes, brightness/per-channel audio, root policies, platform programs
and system-bus identity are unchanged. No reboot, kernel suspend or routine
Phosh restore is requested.

The collected native archive has 547 safe members and 527 frozen inputs,
SHA-256
`d3307a14e2e3c3bdf67ef664f842133bec0aadd3478a1db339166c3f50e62928`.
All collected file hashes and source/executable identities match the immutable
staging ledger and were verified again on the host. Raw evidence remains private
under ignored `artifacts/m2/text-reset/`.

## Unchanged-package GTK/Qt and Firefox comparison

Two subsequent fresh authenticated sessions use the exact same signed revision
50, without installing or changing packages. Both pass their original checks:

- GTK 3.24.52-r0 and Qt 6.11.1-r0: separate modal dialogs, exact native touch
  text, child-popup selection outside parent bounds, refused unsaved close,
  Cancel/Apply, maximize/fullscreen restoration, keyboard hide/reopen/Card View,
  fullscreen field text `tarotxza`, actual PAM and cooperative teardown.
- Firefox ESR 140.12.0-r0 with mobile-config-firefox 5.4.1-r0: queued native
  keyboard taps, 21 exact-text card returns, text/Backspace, PAM, masked password,
  history, popup entry/close and fullscreen `café`. Its observer records 160
  heartbeats, zero reported transport failures and a maximum receipt-minus-
  browser timestamp of 547.9 ms. Input remains native; no WebDriver input,
  browser-sandbox change, typing slowdown or relaxed deadline is introduced.

The executed browser fixture/replay match their exact bytes at the retained
revision-44 failure. The two sessions retain separate results and do not become
one combined full-plan acceptance run. Their fixed-schema keyboard metadata
contains 662/1,124 records respectively, with 16/54 context resets and **zero
held-contact resets**. Consequently the passing browser run does not exercise
the earlier held-reset overlap or establish a fix for that intermittent failure.

The controller returns to normal default Tarot 50. Independent loaded-program,
ordinary-user VT7, Panfrost, service, preference/MIME/control/policy/platform/
system-bus and fixture cleanup audits pass; diagnostics/metadata are off and
hardware keys remain on. No reinstall, reboot, Phosh restore or suspend cycle
is requested. The independently rechecked private comparison archive has 882
members and 524 frozen inputs, SHA-256
`c029c806785b4681ba42d31996d8b1abef2d2926011824234c90b6cf23dfd878`.

## Remaining work

The Firefox held-activation failure remains unresolved. Equal surface or layout
metadata cannot identify a DOM field; there is no unsafe retained-key replay or
serial-validation exception in this correction. The initial native reset
component uses an audited synthetic app; the separate GTK/Qt/Firefox comparisons
above extend its scope without resolving the prior overlap. Wider application coverage,
physical/reference/accessibility review, current-binary endurance, platform
services, phone applications and the remaining full development plan stay open.
The DMP double-chop features remain planned; Fx0/Gold retains its deferred order.
