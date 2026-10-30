# GTK compatibility and shell pointer handoff

Recorded October 30, 2026, Detroit. This checkpoint extends the mapped-menu
compatibility evidence and corrects a reproduced compositor input defect. The
full development plan remains active; no complete milestone is closed. Fx0 Gold
remains reference material for the deferred M7 phase.

## Protocol and host acceptance

The new ignored `gtk_modal_hints_combine_with_xdg_and_preserve_focus_boundaries`
case uses the pinned GTK version-4 client protocol on real owned Wayland
surfaces. It checks modal hints set before the xdg role, independent GTK/xdg
hints, extension unset/release/recreation, modal blocking of a sibling, focus
requests during overview/menu/protection, redacted protected status and restored
focus after unlock. Eleven labeled snapshots accompany the assertions. GTK
capability flags remain zero. This is scoped compatibility coverage, not full
claim, activation or global-menu conformance.

The lifecycle selection now runs seven non-notification cases with 50 driver
requests: two GTK protection requests and one verified root contact extend the
previous six-case/47-request fixture. The viewport contact checks the actual
surface and local input coordinates; original pixel assertions are retained.
The separately driven notification case remains outside this selection.

Both final-image host scale-1 and scale-2 runs pass all seven cases, exact native
pixels/input, popup overflow, configure/state negotiation, viewport malformed
client isolation and protocol-lock protection. A Rust-1.89 protocol client also
passes the new GTK case. These private Xvfb/Mesa tests use a managed lock probe;
actual KMS/PAM is checked separately below. All 237 workspace tests pass on Rust
1.98.1 and the supported minimum 1.89.0. Formatting, Python syntax, strict host
Clippy and DRM-enabled AArch64 Clippy pass. Upstream Smithay warnings remain
attributed to the vendored dependency. The clean Alpine cross build does not
execute the target regression suite.

## Preserved tooltip failures and fix

The first full scale-2 run fails the original viewport-root-destruction sample:
expected purple `[192,32,192,255]`, observed gray `[36,36,36,255]`. Inspection
shows the Activity menu tooltip over the correct native buffer. A verified root
click added to the fixture does **not** cure the same failure in the second run.
Both attempts remain failed and retained. This is a real compositor hover
handoff defect; moving the pixel sample would hide it.

A focused private-display replay reproduces the defect on the previous
compositor: show the actual tooltip, move the pointer into a green native
client, and require its original pixels to return and remain clear. The old
image fails; the corrected image passes at both scales. The final helper also
records executed Python source hashes and accepts an explicit Xvfb path.

The compositor now tracks whether egui has a shell pointer and sends one
`PointerGone` when native pointer routing takes over. Shell-owned button/touch
sequences retain ownership until completion or cancellation. Completed touch
and explicit cancellation clear the tracked presence. The focused replay also
requires a shell press dragged outside to retain its release, an outside
contact to dismiss without leaking native buttons, and a fresh native click to
produce exactly one press/release pair. The full lifecycle replay then passes
both original scale-2 failure samples. An earlier passing prototype is retained
separately from the final image, which adds the explicit held-shell-button guard.

Final host compositor SHA-256:
`425d6e7e14354a2ca6555246c43ff348d6e79fa4c125c23e4ec79feed2cfc08a`.
Previous failing image:
`64cc2bd9efe36788c348dcff8de263ef81cc0fcee69213176a4c261bad7c446a`.

## Unchanged revision-46 native compatibility

Before installing the input fix, the unchanged signed revision-46 package passes
all seven protocol cases/50 requests on the PinePhone Pro. Its fixture and
applications run as UID 10000 in the ordinary PAM login cgroup. Temporary uinput
supplies real touch/keyboard events; the checks require native Panfrost/KMS and
actual PAM unlock. This is injected native input, not owner finger approval.

Real GTK3 and Qt6 widgets also pass dialog mapping and separate transient roles,
exact touch-keyboard text, popup selection outside the parent allocation,
unsaved-close refusal/card restoration, Cancel/Apply responses and cooperative
close. Actual maximize/fullscreen/restore and bottom-field keyboard geometry
pass; text remains exact through keyboard hide/reopen, overview/card return and
PAM, ending at `tarotxza`. Protected startup covers four orientations; native
Calculator/audio, logout/empty cgroup and fresh normal Tarot pass.

The independent audit verifies loaded program hashes, normal/default Tarot,
diagnostics off, hardware keys, Panfrost, services, unchanged platform/system
bus and exact preference/MIME/policy/raw-control restoration. The 373-member
archive and all 244 frozen input hashes verify locally. Archive SHA-256:
`b08658041539bf138503ed21d1bf40219bdb8f9c48b42c2ca5549b776929ca78`.
No package reinstall, reboot or suspend cycle occurs in this comparison.

## Revision 47 package and native delivery

The clean signed `tarot-0.1.0-r47.apk` contains the pointer fix. Package/source
ledgers verify locally. All four AArch64 executable hashes change in the clean
rebuild; their unchanged helper sources do not imply identical executable bytes.
The other installed-file change is the README incorporating earlier menu/Fx0
reference documentation. PAM, session entry, dependencies and license files
retain their prior contents. The Fx0 reference changes no runtime asset or theme. The portable pointer QA helper and this final documentation are
subsequent QA/docs changes, outside the compiler input freeze.

Source archive SHA-256:
`37f48a099a2e5424a57703cc81f87794a1202b79348e5512d098268a308af652`.
All 974 project-source entries verify against the staged archive.

APK SHA-256:
`546e908111ea1cdcb8fa610e42c5c8a468d7918285f20a25bcf1a2dab0753d19`.

| Program | Installed SHA-256 |
| --- | --- |
| tarot | `bc38576aa7eed12c75d67d9c0c3f2f250890fba343082a9beb0199e708ac0122` |
| tarot-session | `7ddb15e5361787dd11d60e073426ee1c5537e0aca6cbb4da9112889ecf43ade3` |
| tarot-lock | `75c82a900cccdec7e0f8d2cdc20e570d43eaddf134a368461889cbd4cf47c109` |
| tarot-keyboard | `8457aa73c2dff8cd14a33914e5778afab455543c303b0d6c11971358df77bb9d` |

The phone controller verifies the development signature, ends the verified old
session and installs a private root-owned, hash-checked APK copy after a
Tarot-only package simulation. Its own namespace freezes 249 inputs and leaves
the preceding revision-46 fixture/evidence untouched. An initial premature
controller invocation stops at a missing staged-input check before password,
logout or package changes; the transfer completes before the actual run.

The revision-47 controller passes all seven protocol cases/50 requests, actual
GTK/Qt dialog/text/fullscreen checks and the existing GTK/Rust-egui native menu
checks in one login. The producers receive exactly three GTK callbacks and four
egui callbacks, including the typed integer window action. Portrait/landscape,
held-row flip-back retirement, disabled choices, Card View/return, actual PAM,
claim removal and cooperative close pass. Protected startup covers all four
orientations; native Calculator/audio, complete logout and a fresh normal Tarot
login pass. Original preferences, MIME data, root policies, system/session bus
payloads and raw brightness/volume values are preserved. No reboot or kernel
suspend is requested.

The independent normal-session audit passes loaded program hashes, default
Tarot, diagnostics off, hardware keys, services, Panfrost and exact restoration.
All 513 archive members and 249 frozen inputs verify locally against the
separately staged source/binary/package ledger, raw protocol log, eleven GTK
snapshots, widget events, exact producer callbacks and audit-program hash.
Archive SHA-256:
`ee43a2fe808553da74346b959fbbeb3da14b794be47f81e392450da8fdc23fb9`.
The package builder is stopped and the controller has exited. Normal revision
47 remains the phone's default session.

The host archive retains 627 verified members, including the failed attempts,
259 final source/configuration inputs, helper hashes and passing reports. Its
SHA-256 is
`72cb88c0e3f3b7f297b9f665cab940d6323ff14a26dcf7da698b0dc7b61e4ff0`.
Saved executable files are separate and their hashes are checked in the reports.

## Remaining scope

The fixed tooltip is verified by real pointer input on the nested backend;
native phone acceptance covers touch and the affected shell/app boundaries.
A physical phone mouse/hover check remains distinct. The retained browser
activation failure, current-binary endurance, USB/suspend investigation,
physical/reference/enlarged-text/accessibility review and unfinished M0–M6
remain open. Broader GTK claim/replacement/trusted-client exclusion, held-key
mutation, session-bus replacement/recovery, worker churn/setup cancellation and
resource bounds still need actual-shell acceptance. KDE requires its separate
adapter; the cooperating Rust example remains a development sample.

Raw packages, binaries, manifests, captures, callbacks and both failed host runs
remain ignored/private under `artifacts/m2/menu-compat/`. Reproduction and native
staging are described in [BUILDING](../../BUILDING.md) and the
[Rust menu example](../../RUST_APP_MENUS.md).
