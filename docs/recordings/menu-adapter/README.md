# Direct Rust native-menu adapter checkpoint

October 29, 2026, Detroit. The development compositor captures real accepted
Wayland peers and retires client authority on disconnect. A shared Rust library
implements typed, bounded GMenu/action reads and guarded activations using the
existing zbus dependency. The installed phone stays in normal **revision 45**;
no replacement compositor package or app-menu UI is accepted by these results.
[Contract, supported subset and remaining work](../../NATIVE_MENU_ADAPTER.md).

## Executed scope

Current Rust and Rust 1.89 pass **234 workspace tests each**, including an actual
Wayland-server disconnect while the client process survives. Strict host and
AArch64 DRM lint, both host/ARM adapter builds and formatting pass. No production
Cargo dependency or lockfile changes.

The modified compositor's rendered portrait and landscape menu replays pass
on the exact final host image. Real mapped native clients have live peer
ownership metadata. Existing menu hierarchy, keyboard selection, cooperative
and refused close, focus and client death checks pass. These are nested host
results; they do not establish phone KMS acceptance for the new compositor.

The same production-library acceptance client passes with both compiler builds
on isolated host buses and with both unchanged ARM executables on the phone's
**actual normal Tarot 45 session bus**. Each run uses separate real GIO producer
processes, actual Unix peer handles, real action callbacks, distinct app/window
action paths and exported submenu/section models.

The passing matrix checks:

- Same-UID wrong-process rejection and unique-name requirements.
- Exact string `night` and integer `7` arguments, boolean state observation,
  unavailable actions and an unsupported tuple target left disabled.
- Changed/disabled choices, foreign snapshots, intersected authority retirement,
  and retirement while a real `Start` reply is pending.
- A surviving process replacing its D-Bus connection, stale old bindings, fresh
  replacement binding, connection loss while the process survives, and exit.
- A 500 ms activation timeout followed by its real callback, with no retry and
  exactly five total accepted callbacks across the complete scenario.

Fifteen malformed/resource cases reject without an action callback: excessive
actions; short/wrong-type links; cycles; depth; group count; expanded shared
graph size; oversized replies; long labels; item/attribute limits; combined reply
bytes; foreign group; attached descriptors; and a silent method. Actual GIO
models test supported behavior; manual wire producers supply malformed cases.

Real empty roots and empty linked submenus also pass. The initial decoder
incorrectly treated an omitted menu as a missing-link error. An added real-GIO
regression fails on that source with `missing linked menu`; the retained binary,
fixture, source hashes and failure remain separate. GLib's group subscriber
omits empty menus by design. The corrected decoder returns a successful empty
snapshot after a successful group reply, while failed calls still return errors.
The earlier missing-root negative was a protocol misclassification; a genuine
foreign-group reply replaces it, with all fifteen current malformed cases still
rejecting and the two real empty cases passing on host and phone.

An independent phone audit verifies the same normal session/default, loaded
revision-45 binaries, active ordinary-user Wayland login, running system
services, Panfrost ownership, diagnostics/metadata off, hardware keys enabled,
unchanged stock system bus/files and no replay devices/alarm/helper. The test
runner separately verifies exact compositor/session/keyboard process identities,
settings/MIME/platform hashes before/after and removal of all test processes.
No package/policy is changed and no suspend is requested.

## Identities and evidence

Fourteen frozen compiler/test/configuration inputs have canonical SHA-256
`acfb5b7b7d21b03d817abae5830063c1e56618eee9e695e763d7d763a8c5ac15`.

| Executable | SHA-256 |
| --- | --- |
| Host current adapter | `337869cc3fa4ebc796b809fa2d1d2f233653c68667946858b25bff495e2aac94` |
| Host Rust-1.89 adapter | `ece3f9e2bc9f3f5fbd12e49da6826765a4a861d2e0e594a2015f00a84decd676` |
| ARM current adapter | `f5805a565049be1ffd6b3c5fb97667900ef97991ce785921d45a092f7d7b5555` |
| ARM Rust-1.89 adapter | `177c07244c25d49c57688fd42df0b6222e8e06dd7f2bdc784739182ac448f862` |
| Nested compositor | `29b1d4913620a82d0b9f77aba431ec0eb3f8e05ad8d4694c4e3380d9edb3fcde` |

The collected phone archive is
`545fcbb7a1ab8cabdfd6531eacfdd57d39d9b57fcb339f90ab2433f3f7d7dc72`
(70,427 bytes, 119 safe members). Local verification checks frozen source/runner
hashes, exact staged ELF hashes, both reports, each actual callback file, empty
malformed/unsupported producer callback files, pending-load retirement, baseline
equality and the fresh independent audit. ELF files are retained separately
with the frozen manifest. Raw evidence stays private.

The first phone preflight stops before any case because its staging manifest
also lists host-only files. That failed namespace/log is retained. A new
phone-only manifest runs the unchanged ARM binaries; it does not alter their
source or retract the original failure. Earlier host matrices and the nonempty phone archive
`561201bc2f71b98ffb3316e8700894ff547f375c68e69f88eacf2643448a0259`
remain historical scopes. The final corrected matrix includes empty-model,
pending-load and composed-authority checks.

## Remaining acceptance

The native adapter tests use Unix peer sockets and exported GIO objects; they
have no mapped phone Wayland surface or egui menu. The new real compositor
acceptance path is tested only on the host here. No GTK shell global/capability,
KDE claim protocol, general menu worker or visible exported command ships.

Next connect actual surface/claim lifetimes, signal subscriptions and bus epochs
to immutable presentation rows and pressed-row guards. Then exercise real mapped
GTK/KDE and cooperating egui producers, activation policy, worker/transport
budgets, protection, phone touch/rotation/PAM/restart and physical/accessibility
review. The snapshot parser bounds do not establish a transport-wide memory
limit, and callback/transport success does not certify document completion.
The complete plan, Firefox issue and current-binary endurance remain open.
