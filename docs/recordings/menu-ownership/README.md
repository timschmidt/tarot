# Native menu ownership and the phone bus feature

October 29, 2026. The [Rust experiment](../../../experiments/menu-export/README.md) now passes its complete ownership scenario on PinePhone Pro with a privately built, feature-enabled D-Bus daemon. The stock installed daemon still omits the required credential. This record preserves the revision-44 experiment. [Revision 45](../r45/README.md) subsequently delivers a separate session-only daemon to the normal login; actual app-menu UI integration remains open.

## Executed scope

Two Rust producer processes create real, unmapped Wayland surfaces and GTK version-4 claims, with zero global-menu capabilities. Each exports an actual GIO menu/action group. They share UID 10000 and a cgroup but have different processes and connections. A separate worker validates live kernel identities from the Wayland peer handle and D-Bus `ProcessFD` before using that exact unique owner's export.

The native scenario passes wrong-client rejection, unique-name requirements, exporter connection replacement within a surviving process, surface destruction/recreation, extension release, revised claims, late validation retirement and stale dispatch rejection. Valid activations require the actual producer callback with target `night`; exact counts are four callbacks in the owner and one in the other producer. Missing, malformed and non-process descriptors are rejected. The known fixture action does not implement a general menu decoder.

Both current Rust and Rust 1.89 host cases pass, with strict host/ARM lint and both AArch64 builds. The unchanged native ownership executables are:

| Build | SHA-256 |
| --- | --- |
| Current Rust | `2d0b358f060e72142174db5861249c416cadb8a43bac63f5219bccc82ac2c87c` |
| Rust 1.89 | `e80452276d4ea3038645892beddd366ed1543967d919b68b8b102c2e697fcb0e` |

Nine source/manifest/protocol inputs have canonical source-set digest `017f7999f2acb6c3c4faa92d96ed7b60e15a53828bd8cd95d528bed60d5f6c71`. The independent experiment lockfile includes 69 packages: the experiment and 68 registry dependencies. Production Rust dependencies are unchanged.

## Dependency comparison

The original stock-phone run fails its first legitimate ownership validation. An independent GIO probe sees `ProcessID`, `UnixGroupIDs` and `UnixUserID`, with `ProcessFD` absent. Target kernel 7.0.0 supports peer process handles, but the target C socket header omits `SO_PEERPIDFD`. Upstream D-Bus 1.16.2 guards collection with that macro. [Upstream code](https://dbus.freedesktop.org/doc/api/html/dbus-sysdeps-unix_8c_source.html) · [Credential contract](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials).

The controlled comparison builds official D-Bus 1.16.2 twice against an isolated, signature-verified phone SDK. All source files remain unmodified. Normalized configuration headers match, and Meson's only differing build option is host `c_args`: empty before, `-DSO_PEERPIDFD=77` after. The value matches the target Linux UAPI header. Internal D-Bus libraries link statically into each private daemon; their dynamic dependencies are Expat and musl, so the comparison does not load the stock D-Bus library.

| Private daemon | SHA-256 |
| --- | --- |
| Before | `2af32c7bbf94e16072b106d332f9f874b6e1339354055c5c1c5ff67dd6f3d3e9` |
| Feature enabled | `592cadafe17f3faeb9a2d9253eab5c1bd58ebbbacd3e473f607f09296ea40cf7` |

The source archive's SHA-512 is `5c26f52d85984bb9ae1dde8d7e73921eacbdf020a61ff15f00a4c240cb38a121553ee04bd66e62b28425ff9bc50f4f5e15135166573ac0888332a01a0db1faa2`, matching the [Alpine recipe's source checksum](https://github.com/alpinelinux/aports/blob/3.24-stable/main/dbus/APKBUILD). This proves the build-feature effect in the controlled pair; the installed daemon's original compiler command has not been reconstructed.

| Case on phone | Before | Feature enabled |
| --- | --- | --- |
| Current-Rust ownership | Expected missing-credential rejection | Full scenario passes |
| Rust-1.89 ownership | Expected missing-credential rejection | Full scenario passes |
| Current-Rust GIO export/state/timeout regression | Passes | Passes |
| Rust-1.89 GIO export/state/timeout regression | Passes | Passes |

All eight comparison predicates pass. The two before-build ownership processes remain failed native scenarios, with exit 101 at the original missing-credential guard. Four GIO regressions and both feature-enabled ownership scenarios exit zero. Independent probes confirm the absent/present credential difference and resolve the returned descriptor to the correct live process. No numeric-PID fallback is introduced.

## Evidence and normal-session state

The original failed native archive is `719735ac8685f51aba44579fc6e23981ef9957ac0e9f33beb0f093e6bc88adf0`, with 19 safe members and all nine source inputs verified. The comparison archive is `5d307b7b315abf9619e6ceb854e4733a90ea9a9e321d99de47464f2f9327859c` (1,579,627 bytes, 24 safe members). Local verification checks archive integrity, actual runner/daemon/source hashes, identical Rust binaries, the sole build-option difference, all eight outcomes and observed callback data.

Ordinary-user execution independently verifies normal Tarot 44's exact compositor/session/keyboard processes and payloads, saved preferences/MIME state, runtime-library hashes, diagnostics disabled, hardware keys enabled and removal of all experiment processes. These processes are the same ones observed by the preceding revision-44 normal-session audit. Installed packages, the stock daemon, login policy and system services are unchanged. No suspend cycle or phone UI/PAM/KMS acceptance is established by this unmapped-resource experiment.

The initial host malformed-descriptor failure remains retained separately. Bounds checking prevents its GIO assertion, and the native feature-enabled run includes those negative cases. Raw sources/logs/binaries/SDKs and captures stay private; this record contains aggregate evidence.

## Remaining delivery

[Revision 45](../r45/README.md) subsequently packages and verifies the normal-session dependency fix. Next, integrate real compositor client/surface/bus generations with bounded menu state and stale pressed-row rejection. Prove mapped GTK/KDE exports and a cooperating Rust/egui producer, compare GIO with the direct Rust D-Bus path, and run installed touch/rotation/PAM/restart and physical/accessibility checks. The [menu plan](../../MENUS_PLAN.md) and complete development plan retain their original scope.
