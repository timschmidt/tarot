# Bluetooth development evidence

October 30, 2026. This is initial M4 service/agent work. It does not complete M4,
the development plan, Bluetooth audio or physical peripheral acceptance.
[Behavior and service contract](../../BLUETOOTH.md).

## Host checks

Current Rust 1.98.1 and minimum Rust 1.89.0 each pass all 241 workspace tests.
Strict host and aarch64-musl/DRM workspace lint pass. The existing private
NetworkManager creation/secret-agent regression also passes with the shared
credential keyboard changes.

The final private BlueZ replays use separately frozen current/minimum compositor
images. Each passes rendered radio/discovery controls, denied discovery-stop
ownership, confirmation pairing with six-digit display, leading-zero PIN and
numeric passkey input, connect/disconnect, explicit trust, confirmed Forget,
spoof/concurrent caller rejection, stale replies, page cancellation, five
lock/unlock cancellation cycles, passkey display progress, device path reuse,
daemon replacement, incoming pairing and complete broker replacement. No real
radio or physical peripheral participates in these host cases. The external-scan
case models another client's session in the synthetic exporter.

| Retained image | SHA-256 |
| --- | --- |
| Current compositor | `d6b3ef756e037b0eda2ed87ae7c6f94e8e123196d3997b896706adeaa8737ef1` |
| Minimum compositor | `73e26d716bacbeb391e90114fa05a5379da951a83506e4ad9b3625841ce0d3ff` |
| Host evidence archive | `cfe056c047b9ebae7e5470baca4f4985b345b0779459e5466e8131f9d10db9f2` |

The verified private host archive has 849 members, including 602 frozen runtime
and QA inputs, the accepted images/reports/captures and earlier failure evidence.
Build/test outputs can replace Cargo's target executable; final fixture runs
therefore use immutable evidence copies and verify their exact hashes.

Earlier failures remain recorded: one numeric-keyboard Enter OCR target was not
recognized; one run timed out waiting for a compositor response after mock
unlock. The latter's cause is unproven. Subsequent five-cycle runs on both
compilers pass, without relaxing response deadlines. It is not claimed fixed by
that result or by the separate stale-rejection/reader-retirement corrections.
One existing minimum-Rust URI-helper test failed once; its isolated and full
workspace rechecks pass, with no URI code or test changes. An initial host DRM
lint invocation lacked libseat development metadata; the configured ARM sysroot
lint passes. These failures do not become successful checks retroactively.

Follow-up on the retained timeout: a frozen current compositor passes 100 consecutive private PIN → lock/unlock → display-passkey → capture cycles. That focused sequence does not reproduce the earlier response timeout or prove its cause. The original failure remains open; the new private report retains its exact producer and image identities. The phone was not involved.

## Signed native candidate

The corrected source archive has SHA-256
`c02b74056e7513b2f3188c72c428249375886f87d9bd128e09d9f105c097d5b3`.
The clean Alpine cross build vendors and checks all 446 locked registry packages.
Its signed revision-51 APK has SHA-256
`bd8802500efc7193ba028c2a9ab54d67aebbdd311dba49ebdd2c5b434336f2a5`.
The package audit verifies all four aarch64 ELF programs, 421 frozen runtime
inputs, the added BlueZ dependency, absent install scripts and unchanged PAM and
session entry. An earlier uninstalled candidate predates the final reader
retirement correction and is excluded from native acceptance.

Signed revision 51 is installed and passes native controls in a normal
greetd/elogind/PAM session. Actual KMS/libinput touch replay changes the radio
off/on, starts/stops discovery, enables finite incoming-pairing visibility and
locks the phone. Independent UID-10000 GIO calls verify each actual property,
including restoration of pairable/discoverable flags and their original
timeouts during protection. Protected Bluetooth commands are denied. Real PAM
unlock restores the same Calculator card and its login cgroup. No root bus-call
substitute or fallback control recovery is used to make those checks pass.

The independent final audit verifies loaded revision-51 programs, default Tarot,
Panfrost, ordinary session ownership, diagnostics/metadata off, hardware keys on,
unchanged system bus/services and exact adapter/preferences/policies/audio/
brightness restoration. Temporary input and diagnostic control are gone. All
559 collected archive members and 532 frozen native inputs verify locally. The
private native archive SHA-256 is
`d12d3673c488cbcdfb15049c311aebc27d646582d39ec570c24f202519d49653`.
The native package was cross-built; target unit tests were not run by abuild.
No suspend cycle or full application compatibility/endurance matrix is claimed.

Physical peer pairing, acoustic output, codecs/profiles and call audio remain
open. No nearby peripheral is selected without the owner's identification.
