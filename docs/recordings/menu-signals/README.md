# Native menu broadcast subscriptions

Recorded October 30, 2026, Detroit. Revision 49 fixes a reproduced interaction
between the receive-frame limits and an overly broad broadcast subscription.
This is scoped progress toward the full development plan. No complete milestone
is closed; Nocturne/PinePhone Pro remain the current baseline.

## Retained failure and production change

The unchanged revision-48 host compositor subscribes to every session-bus signal.
A separate GIO observer confirms that the cooperating exporter emits a 96 KiB
byte-array broadcast at `/org/tarot/Noise`, interface `org.tarot.Noise`, member
`Changed`. This traffic is unrelated to its declared menu or actions. It reaches
the menu worker's socket, whose 64 KiB body guard closes the connection before
header relevance filtering can run. The visible menu changes from available to
unavailable, failing the unchanged-menu assertion before any action callback.
The original report remains failed; cleanup does not turn it into a pass.

The worker now asks the broker for at most four typed match rules:

- `NameOwnerChanged` from the bus, at its exact path/interface, with argument zero
  equal to the exporter's unique name.
- `org.gtk.Menus.Changed` from that unique exporter at its declared menu path.
- `org.gtk.Actions.Changed` from that exporter at each declared application/window
  action path. Missing paths are omitted; identical action paths are deduplicated.

The local message iterator remains unfiltered and is created before subscription.
It must still observe method replies to advance the receive-sequence barrier.
Process descriptors, bus GUIDs, surface/claim leases, row revisions and the
pre-allocation frame limits retain their previous checks. This change filters
**broadcasts at the broker**; it does not claim protection from arbitrary directed
traffic, a malicious relevant exporter or a bus-wide flood. Oversized relevant
messages still retire the worker and require a fresh claim.

## Host evidence

The same full traffic/claim replay passes in portrait, landscape and with the
minimum Rust 1.89 build. Each run independently observes eleven broadcasts of
exactly 98,304 payload bytes:

| Cases | Expected and observed result |
| --- | --- |
| Unrelated interface/path; each wrong action path/interface/member; each wrong menu path/interface/member; another exporter's otherwise matching action signal (eight total) | Available menu, source stamp and complete rows remain identical across repeated observations for 0.8 seconds per case; no callbacks |
| Correct exporter and application/window/menu headers (three total) | Oversized body rejected, unavailable menu with no rows, no callbacks; a fresh claim restores availability |

The observer records actual sender/path/interface/member/size, separately from
the emitter's acknowledgement. It never records payload contents or invokes
commands. All three runs then pass foreign-process rejection, release/reclaim,
held pointer/key retirement, 49-claim churn, same-bus reconnect, private bus GUID
replacement with exact unique-name reuse, trusted-keyboard exclusion and
cooperative close. Four actual `choose("night")` callbacks occur per run; the
foreign exporter receives none.

A separate real GTK replay passes a valid `org.gtk.Menus.Changed` replacement:
while Choose is held, the producer removes/reinserts it as **Renamed command**
with a different action. The old release does nothing; a fresh visible selection
invokes the new toggle action. This complements valid application/window action
changes and owner-loss checks; header-correct oversized rejection alone does not
prove valid change delivery.

All 239 workspace tests pass on current Rust and Rust 1.89. Strict host and
DRM-enabled AArch64 Clippy pass. These are host/software-renderer observations,
not physical-finger, phone latency, power or endurance measurements.

Host compositor SHA-256:
`c645a76fbd11dcd3e4d62b3b5f317754473810e60ae4cfe3255cec3a08d4e505`.
Minimum-Rust compositor:
`78630a2d31bfb8fdf458a2149498cfb77d39e2dd6cd6cd6484a222a52d19c75e`.
Retained failing revision-48 compositor:
`edcb174d7ff5833ddc2263beb2c386e6ab543e004212a464f961e62f88155bd7`.

The verified private host archive has 559 members, including 265 frozen inputs,
with SHA-256
`9b429a98cbb455966f6e610ce1632dbac9048e3d7975120d51362c004537ba72`.
The claim reports retain the earlier GTK replay helper's hash; that exact helper
is preserved from the package source archive. The later optional dynamic-GTK QA
case has its own executed-source ledger. No compiler input changed between them.

## Package

One clean offline Alpine build succeeds with Rust 1.96.1, the AArch64 Alpine
musl target, DRM and 446 checksum-verified registry packages. It does not execute
cross-built target tests. All project-source members match the archive manifest;
current Rust/compiler inputs match that frozen source. Documentation and the
optional dynamic-GTK QA helper were updated afterward, without changing runtime
inputs. Locked registry versions/checksums and runtime dependencies are unchanged.

Source archive SHA-256:
`a70c993c96ef262ce2bbf0d99afa9cc2841a6a1b21168f0920c8e097d0950244`.
APK SHA-256:
`9b1263338d9f703021aaedf6f8cbab2556a0e209322d572871a0e5c18de6cbae`.

| Program | Package SHA-256 |
| --- | --- |
| tarot | `917390f0de7ca9d880bf99c11debdb01a97c071d1ec637a966c75bf1f5324175` |
| tarot-session | `2da83ffc5ee5e9470d487f5764f38f7a94e660d5f545cfa24b11843043671a95` |
| tarot-lock | `15002dc3a66042fcc34164d7663363436206d0c1e1a8b19575e73b35518a0fe4` |
| tarot-keyboard | `38a85037d53db2705804d90a38b2967733930bcae689a37ab6282093255ad969` |

All four executable bytes change in this clean rebuild. The other installed-file
change is the README containing earlier checkpoints. PAM, session entry and
notices/licenses retain their earlier contents.

## Native delivery and final state

The actual PinePhone Pro verifies the development signature and the root-owned
private APK copy, simulates a Tarot-only transaction, upgrades 48 → 49 and
verifies all four installed payloads. One combined acceptance login passes:

- Protected startup and all four protected orientations, actual PAM, ordinary
  Calculator/audio and full login-cgroup teardown.
- Actual rendered GTK and Rust/egui menu input: respectively three and four exact
  callbacks, including the typed window target `7`; held-row flip retirement,
  disabled commands, orientation, Card View, fresh PAM and cooperative close.
- The same eleven independently observed broadcasts described above. Eight
  preserve the exact menu/stamp, three fail closed and recover on fresh claims.
  Then foreign-process rejection, claim clear/release/reclaim, held touch and
  held Down-key retirement, same-bus exporter reconnect and claim changes during
  protection pass. Four `choose("night")` callbacks occur; the foreign exporter
  receives none. The observer and both producers exit successfully.

Root observations verify observer/wrapper/exporter executable hashes, process
start identities, UID 10000 and membership in the ordinary authenticated PAM
cgroup. Temporary catalog entries and input devices are removed. Original
settings/MIME contents, ownership/modes, brightness, per-channel sound state,
root policies, platform programs and system bus identity are preserved.
A fresh normal Tarot 49 session passes an independent audit: default session,
actual loaded payloads, UID/VT, Panfrost, services, hardware keys on, diagnostics
and metadata off, no stale fixtures/control socket/RTC alarm. No Phosh restore,
reboot or kernel suspend is requested by this component.

The verified private native archive has 744 members and 527 frozen inputs,
SHA-256
`09bc59f3ab5d9f9f4fb59a2a665450946785fd175d05c8811d82c0f56fbd422b`.
All archive-member hashes and staged source/executable identities were checked
again on the host. Raw captures, reports and binaries remain in ignored
`artifacts/m2/menu-signals/`; they are not included in the public repository.

## Remaining work

General flood/CPU/latency/memory and directed-traffic budgets remain unproven.
Broader destructive surface/socket lifetime, production phone bus crash/recovery,
activation policy, malformed/deep menus through the shell, KDE's distinct
protocol, reusable application APIs and accessibility still require work. GTK
global-menu capability flags remain zero and the Rust example is not a stable
SDK. Earlier revision-48 kernel/protocol/toolkit results retain their own binary
scope; this run does not repeat or recertify them. Physical-finger/visual review,
current-binary endurance, the retained Firefox held-activation failure and the
remaining platform/phone features stay open. The requested DMP flashlight
feature is planned separately under M4/T015; Gold/Fx0 remains deferred until
all existing work.
