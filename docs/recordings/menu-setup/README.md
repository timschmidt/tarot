# Bounded menu setup, cancellation and receive framing

Recorded October 30, 2026, Detroit. Revision 48 corrects two reproduced native
menu transport failures and supplies scoped native delivery checks. The full
plan remains active; no complete milestone is closed. The deferred Gold/Fx0
reference changes no current theme or device baseline.

## Retained failures

A private fake bus accepts the old compositor's socket but never answers
EXTERNAL authentication. Entering Card View retires the claim, yet the socket
remains open past the one-second cancellation gate. A second fake completes
SASL but withholds `Hello`; the old worker remains loading past three seconds.
The pinned zbus builder uses a blocking task for Unix connect, and its method
timeout does not bound these setup phases. A stuck retiring worker can hold up
subsequent menu workers.

Another fake announces a 64 MiB body using only the sixteen-byte primary header.
The old compositor allocates the announced buffer before Tarot's accepted-body
limit runs. Observed RSS grows by 67,792 KiB and the connection remains open at
the one-second gate. All three original reports stay failed. Cleanup then closes
the test processes/sockets; a cleanup EOF is not a passing cancellation result.

## Production change

The new Rust socket adapter owns a nonblocking concrete Unix pathname/abstract
connection. It does not submit a blocking connect task to zbus. A two-second
setup budget covers connect/backlog, authentication and the initial `Hello`.
Cancellation is checked between polls with a ten-millisecond park interval and
at I/O progress. This is an ordinary scheduler bound, not a hard real-time
promise. Remote/command transports cannot supply this local process-descriptor
contract and are rejected. An optional address GUID is still validated.

A separate close-on-exec control descriptor allows immediate socket shutdown
when a claim retires. The compositor does not wait for zbus's asynchronous
writer lock. A real full-socket test holds that lock with a pending send and
requires cancellation to return and wake the sender, without a setup timer.
The main thread still never joins an unfinished menu worker.

Receive guards run before zbus can allocate the announced frame:

- Authentication accepts at most 1 KiB per line and 4 KiB in total, with no
  received descriptors. Byte reads separate SASL's final response from `Hello`.
- Each binary primary header is validated before its final bytes are forwarded:
  body at most 64 KiB and header fields at most 16 KiB, plus the fixed primary
  header and alignment padding. Little- and big-endian lengths are checked.
- `recvmsg` has an aligned ancillary buffer with exactly four descriptor slots.
  Alignment slack from the general sizing macro cannot enlarge the budget.
  Truncation rejects the message; received descriptors close on rejection.
  `MSG_CMSG_CLOEXEC` marks accepted descriptors atomically so concurrent app
  launches cannot inherit them. A message retains at most four descriptors;
  across a rejected later read, at most four previously retained plus four
  newly received descriptors can exist briefly before cleanup.

Zbus still decodes messages and assigns receive positions. Tarot does not
reconstruct them or replace the signal/reply ordering barrier. The existing
sixteen-message queue, per-RPC 500 ms deadlines, parser/graph limits, process
credentials, intersected leases, revisions and bus-GUID checks remain active.
A post-dispatch timeout still means uncertain completion, with no action retry.

`async-io` 2.6.0 and `async-trait` 0.1.92 become explicit direct dependencies;
Rustix's existing net feature is selected explicitly. All 446 locked registry
packages, versions and checksums remain the same. There are no new APK runtime
dependencies or platform-policy changes.

## Host acceptance

All eleven final-image cases pass on private Xvfb/Unix sockets. The legitimate
producer connects to a separate real private bus. After the failed socket is
retired, that listener becomes reachable at the compositor's configured path.
Every case requires a fresh available menu and one actual rendered
`choose("night")` callback. The test never changes the host desktop or bus.

| Final case | Observed retirement after test trigger |
| --- | ---: |
| Authentication cancellation | 82 ms |
| Authentication deadline | 2,002 ms |
| Initial Hello cancellation | 73 ms |
| Initial Hello deadline | 2,008 ms |
| Full listener backlog cancellation | 232 ms |
| Full listener backlog deadline | 2,011 ms |
| 64 KiB unterminated auth input | 18 ms |
| 64 offered auth descriptors | 3 ms |
| 64 MiB body announcement | under 1 ms |
| Fragmented big-endian body announcement | under 1 ms |
| 32 MiB header-field announcement | under 1 ms |

These are finite debug/software-renderer observations, not phone latency or
energy measurements. The last header byte can cause closure before the fake's
next observation. The final oversized-body case grows RSS by 2,028 KiB in this
run; frame/body bytes are not transmitted. The two-second budget is not renewed
by partial authentication progress. A minimum-Rust compositor independently
passes authentication cancellation and oversized-body rejection/recovery.

Current and minimum Rust each pass all 239 workspace tests, including the two
real socket/kernel regressions. Strict host and DRM-enabled AArch64 Clippy,
formatting, Python syntax and cross builds pass. The real GTK and Rust/egui
mapped-menu replays pass. Typed string/boolean/integer actions, independent GIO
observations, local fallback controls and the mapped claim/held-input/bus-epoch
matrix pass. The adapter's complete existing GIO matrix also passes all fifteen
malformed producer cases and retains its uncertain-action-without-retry check.

Earlier candidate images/reports are retained separately. The first signed
revision-48 candidate predates the syscall ancillary cap and atomic descriptor
flags; review withheld it before installation. The final package uses a second
clean source/build directory. No passing prototype report is relabeled as final
coverage.

Final host compositor SHA-256:
`edcb174d7ff5833ddc2263beb2c386e6ab543e004212a464f961e62f88155bd7`.
Minimum-Rust compositor:
`31ff05c83043ea0cb87cada5205a2e31d6912d0c38e57c38a88e2dc5c8b34712`.

## Package and native delivery

Final source archive SHA-256:
`afa3f06ddb03577370c8c6964e303657a9133b2031486ace9410c2e39b33d904`.
All 981 project-source archive entries verify, with 446 checksum-verified registry
packages. Final docs are subsequent to the compiler/source freeze. The clean
Alpine build uses Rust 1.96.1 and the AArch64 Alpine target with DRM; it does not
execute cross-built target tests.

APK SHA-256:
`b241e687d4f0993b9e5ee789a05286582239261e6fa7c697d775b5fd6c8a61ff`.

| Program | Installed SHA-256 |
| --- | --- |
| tarot | `22b36fd14cd02ab224df4ee827838756263e98454e6e3f22a7191e5627adf9ff` |
| tarot-session | `b888f052196f5eb949237d810ed086dde2fbac5271f95db0124a2756f9fe5b74` |
| tarot-lock | `6ccb212519f49da02baf67b479e24ff08137ab6c09125a463e57b06c8316b00b` |
| tarot-keyboard | `a630337df4e389c215a6890fb121bc38a1b64f68bd7cf4bd6c0cc805fedf6604` |

All four executable bytes change in the clean rebuild. The other installed-file
change is the README containing prior checkpoints. PAM, session entry,
notices/licenses and runtime dependency lists retain their prior contents.
The device verifies the development signature, tests a private root-owned APK
copy, simulates a Tarot-only transaction and verifies all installed payloads.

Both new cross-built kernel tests pass on the actual PinePhone Pro as UID 10000:
full-writer cancellation and descriptor limits/close-on-exec. These are ordinary
account processes in the SSH controller's cgroup, separately identified from the
subsequent ordinary PAM login's UI checks.

The first final-package native run passes all seven protocol cases/50 driver
requests, real GTK/Qt dialog/text/fullscreen/keyboard checks, GTK and Rust/egui
menus, and the claim lifecycle component in one ordinary login. Actual process
credentials and callbacks, held-input retirement, exporter reconnection,
protected claim changes and actual PAM pass. Protected startup covers four
orientations; Calculator/audio, complete logout and a fresh normal Tarot 48
login pass. Owner preferences, MIME settings, policies and raw controls are
restored. No reboot or kernel sleep is requested. This is injected native
input, distinct from owner finger approval.

The controller freezes 528 inputs in its own namespace. The independent final
normal-session audit passes loaded program hashes, default normal Tarot 48,
diagnostics off, hardware keys, services, Panfrost and exact preference/MIME,
policy, platform/system-bus and raw-control restoration. All 860 collected
archive members and 528 frozen inputs verify locally against package/source
ledgers, the two kernel test results, the seven-case protocol log, widget events,
exact menu/claim callbacks and the audit-program hash. The controller has exited;
normal Tarot 48 remains the phone's default session.

Native archive SHA-256:
`60b7823ff758a079fd40a4bc4e5851567a81de3e7f53bef248f90507d54c255e`.

## Scope and remaining work

The fault injection above is a private host case; it is not a production phone
bus crash/recovery or whole-system performance test. General flood/CPU/latency
budgets, unrelated bus-signal traffic, exhaustive descriptor/message variants, destructive surface/client
lifetime cases, managed-locker exclusion, malformed menus through the full shell,
KDE's separate adapter and broader activation/recovery remain open. No GTK global
menu capability flags are enabled. The retained Firefox activation failure,
current-binary endurance, USB/suspend work, physical/reference/enlarged-text and
accessibility review, services/applications and the remaining plan keep their
existing scope.

The verified host archive has 1,022 members. SHA-256:
`33cd19997be26c25223e1a0c5b366e4aaa5b2042885b2d76ecf5d7228af68675`.
Saved executables are separate and their loaded-image hashes are checked.

Raw binaries, failed attempts, ledgers, captures and signed candidates remain
private under ignored `artifacts/m2/menu-setup/`. See the
[adapter contract](../../NATIVE_MENU_ADAPTER.md) and
[reproduction instructions](../../BUILDING.md#menu-setup-fault-injection).
