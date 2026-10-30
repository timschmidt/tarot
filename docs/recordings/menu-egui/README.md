# Cooperating Rust/egui exports and revision 46

October 30, 2026, Detroit. The new ordinary Rust/egui example owns its Wayland
surface and exports actual menus/actions with zbus. The host acceptance is
complete for the scope below. The clean signed revision-46 package is installed;
two native replay failures and their recoveries are recorded separately. The
third native replay passes on the unchanged package, with its independent
normal-session audit and all evidence verified locally. No complete milestone or
full development-plan acceptance is claimed.

## Host acceptance

Current Rust 1.98.1 passes portrait and landscape interaction on one immutable
producer. Rust 1.89 passes the same portrait scenario. Each run requires:

- Actual visible submenu choices invoking string `night`, boolean toggle and
  window integer `7` callbacks, with exactly four shell callbacks.
- Independent GIO menu/action reads, correct single-struct `Describe` reply,
  parameter signatures and live state/enabled signals; the reader never invokes
  an action.
- Card View and return with a fresh worker source; observed check marks;
  actual held pointer release rejected after disable/re-enable restores the
  same semantic rows; a fresh choice succeeds and a disabled one does nothing.
- A second window with no shell claim, two actual in-window button callbacks,
  fallback card controls and a successful cooperative Close card.
- Owner exit removes exported commands and the card.

Both unchanged GTK producer regressions also pass with the updated capture
helper. Every visible target retains the existing 80% recognition gate. The
initial Rust portrait failure is retained: the border/shadow and app text below
the menu confused OCR before disabled selection. The corrected helper crops the
actual panel's inner gradient and records bounds/recognized words. It changes
neither action callbacks nor confidence requirements.

Current/minimum workspace tests each pass 237 checks. The first restricted run
could not create the existing compositor tests' Unix sockets; the unchanged
suite passes with those capabilities enabled. Strict host and AArch64 example
lint, minimum-Rust example build and AArch64 release build pass. No registry
package identity changes; the root dev dependencies now reuse the existing
Wayland-egui renderer. Source packaging includes examples.

The final source/configuration ledger holds 294 files. Its digest is
`25c01bfb70603078265b1f848b30833b9d292d82302a0cdf09b5b40776f2e127`.
The later phone-only QA rotation correction has a separate input delta; it did
not alter compiled Rust or host replay inputs. Frozen copies remain unchanged.

| Retained image | SHA-256 |
| --- | --- |
| Current-Rust producer | `78f15981e8dd4d2bf9597cbe56ceeac2537b4c45d40ef93deef6c936bb8ea7c0` |
| Minimum-Rust producer | `b1182e4303e8468fad6623386cc4599154bda12d0e66c3d54c747c05c1d312bd` |
| AArch64 native producer | `a1df4ac08941f2e4af38bd472671c311c9f245cffc465d12e140dce653d33d83` |
| Host compositor | `64cc2bd9efe36788c348dcff8de263ef81cc0fcee69213176a4c261bad7c446a` |

The verifier compares raw callback files, GIO events, observed generations,
executed helper hashes and all immutable images. Final app/result captures were
inspected. This proves scoped nested input, not physical/reference conformance.

## Signed package and first native run

The new isolated Alpine staging directory builds and signs revision 46 from a
vendored source archive, with a clean source/target/package tree. All four
executable payloads change in this rebuild; the new GTK COPYING notice is the
only additional installed file. PAM and the session-bus package remain unchanged.
The audit verifies 367 frozen runtime Rust/manifest/protocol inputs. The sample's
synthetic report writer was subsequently made a single buffered write; the
separately built/staged native example has its own exact source manifest.
Later native QA helpers are staged separately from the APK's compiler inputs.
Cross-building does not execute the target workspace suite.

APK SHA-256 `f2debda23bbafb429aba66951f9c9f767d11dd3d90ccb34a82952c035d822d33`;
source archive SHA-256
`980bc5d49f59b817f8cb4e5f52016d0c915ad12ca6276d5ce5ab60c8a55143b2`.
Actual signature verification and a simulated Tarot-only transaction precede
installation. No global signing key, greeter or system-bus policy is changed.

The first native run passes protected startup/orientations and actual PAM,
Calculator in the real login cgroup, mapped GTK commands/state, Card View
return, portrait/landscape touches, held flip-back rejection and fresh/disabled
choices. It then fails an incorrect QA expectation that rotating an open menu
closes it. `reconfigure_menus` intentionally keeps an open panel and invalidates
old choices; the retained status shows a usable submenu in the new geometry.
The cleanup controller also trips a process-exit identity race after the
session replay requests termination. Neither failure is relabeled as a pass.

A separate recovery restores exact owner settings, removes only the known test
entries and logs into normal Tarot 46. The independent normal-session audit
passes loaded payloads, PAM session/default Tarot, diagnostics/metadata off,
hardware keys, Panfrost, original policies/preferences/system bus and raw
volume/brightness controls. The first run's 128 inputs remain immutable.
Its 191-member archive verifies every input/file and raw GTK callbacks:
`3687d5fafc39ca2b7764ea604571c05cc2c57a1d3f86f9b5620ec64dbcbca88d`.

The second run uses a separate 129-input namespace and unchanged runtime bytes.
It completes the GTK producer, including actual PAM/menu retirement and close,
and reaches all four Rust/egui callbacks. The phone's Tesseract model then fails
to recognize the dimmed, checked Choose label. On the exact same capture,
excluding the check column recognizes Choose at 92% confidence with the original
80% threshold. A separate cleanup race occurs while opening a process descriptor
after the compositor has exited. The second normal-session recovery and
independent audit pass. Its 263-member archive, including all 129 original
inputs and raw callbacks, verifies as
`bccb7e85a350d0ff4963d66db7745127e70a7a6c3af035fd790e8e6ebf98e954`.

The third run keeps its own 129-input namespace, the same APK and all executable
bytes. Only the phone capture helper, complete cleanup exit handling and audit
namespace change. The helper retries preprocessing of the same pixels before
any selection, records rejected crops and retains exact words/confidence; it
never retries an action. This final run passes both producers on actual native
Panfrost/KMS with ordinary-user catalog launches in the real PAM login cgroup:

- GTK records exactly choose/night, toggle/no argument and choose/night;
  Rust/egui records those plus window/7, selected through a landscape touch scroll.
- Portrait and landscape preserve command hierarchy, observed checks, disabled
  state, fresh selection and Card View/return. Held flip-back releases invoke
  nothing; geometry retires the old menu revision while preserving its panel.
- Each producer survives an open-menu lock and actual PAM unlock with a fresh
  worker source, protected diagnostic redaction and no extra callbacks.
- Both apps close cooperatively. Protected startup covers all four orientations;
  native Calculator/audio, clean session exit and a new normal login also pass.
- An independent audit verifies all four loaded program hashes, default normal
  Tarot with diagnostics/metadata off and hardware keys on, Panfrost, unchanged
  system/session-bus payloads, original policies/preferences and exact raw
  brightness/volume. Test entries/devices and the diagnostic socket are gone.

The final archive is `281f65c3666aa283f07b289e8c874e7a5996bf59a15f83d84d0cdf85c18dc5bc`:
274 safe file members and all 129 frozen inputs, each verified against the local
staging ledger and recorded file hashes. Raw callback counts, current public
QA sources, protected states, package and loaded-role identities are checked
independently of the aggregate pass flag. The native root/grouping and
checked/disabled landscape captures were inspected. This is injected touch
through the real input stack, **not** owner finger confirmation or historical
visual conformance. Zero suspend cycles are requested.

The private host archive also verifies all 564 members:
`f746501cdba2d15151e672d708db7fc1ea6459ed7d09958601bf1afbb59dd79f`.
All executable images remain separately retained and hashed.

| Normal installed program | SHA-256 |
| --- | --- |
| tarot | `52529ba5ed8e4ec226fc581bf4306439614ee433d54544b25f1399c7f346609e` |
| tarot-session | `9180859a415ffc38b5c4584395db46c063bea4b8cd7ba5fd313c06e4c8241fcf` |
| tarot-lock | `b9240e65f453e9b7efce00d37c7527fb6255449d0cffba7bb31241cd21b12e38` |
| tarot-keyboard | `cf48dae71489fba3bc2761b868aff8a6c11ebab1e096d24f6f5e387d33eba741` |

## Remaining scope

GTK capability flags remain zero. Full GTK modal/activation compatibility,
KDE, adversarial surface/claim/protection and daemon-replacement recovery,
held-key mutation, worker cancellation/churn/transport memory budgets,
physical/reference/enlarged-text/accessibility review and all unfinished M0–M6
work remain open. The sample is not a stable app SDK. This checkpoint does not
close the retained Firefox input failure, current-binary endurance, kernel USB
suspend issue or phone/carrier work. No suspend cycles are requested here.

Raw source ledgers, signed APK, binaries, captures, callbacks, failed runs and
private evidence archives remain ignored under `artifacts/m2/menu-egui/`.
[Example and reproduction](../../RUST_APP_MENUS.md) ·
[Menu delivery plan](../../MENUS_PLAN.md).
