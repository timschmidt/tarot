# Installed Nura session

Tarot's first package targets the verified Nura 26.06 / Alpine 3.24 AArch64 OpenRC/elogind baseline. It installs alongside Phosh. This is a development session; rotation, broader power/wake policy, persistent authentication policy and release reliability remain separate gates.

## Session boundary

`tarot-session` is a Rust executable started by the display manager after normal PAM authentication. It refuses root/set-ID execution and asks the root-owned login1 manager for the session of its actual process. That session must belong to the current user, be active/local, have class `user`, type `wayland`, seat `seat0` and a VT. Setting environment variables in an SSH shell does not create such a session.

The launcher verifies the PAM runtime directory and root-owned installed programs, detects the single connected DSI panel, and starts a new private D-Bus session. It selects libseat's `logind` backend. Routine execution does not start seatd, change VT ownership, create an elogind session, or need an administrator terminal. The compositor always receives the actual locker, trusted keyboard, notification/system services and sleep coordination. The launcher opts into persistent idle preferences, defaulting to a two-minute idle lock and three-minute panel off. Saved rotation is loaded before protected frame one. Revision 8 verifies actual timeout controls, restart and native panel timing; explicit diagnostic CLI intervals take precedence.

The OpenRC preview launcher starts the standard unprivileged PulseAudio daemon once per login, alongside protected graphics startup, with a five-second bound. The audio adapter itself never autospawns a daemon during reconnect. A failed startup is logged and remains an unavailable audio state in the shell. `--audio-service external` leaves startup to a separately managed provider and retains its supplied Pulse server environment; a complete PipeWire service integration is not implied. A newly started daemon belongs to this PAM cgroup and is included in session teardown.

Startup remains protected and requires the real PAM locker even after display-manager authentication. This currently means another authentication step. A future authenticated handoff may improve that interaction; no unauthenticated startup exception is used. No development timer, synthetic locker, platform-bus override or framebuffer capture is enabled by the normal desktop entry.

Private session logs live under `$XDG_STATE_HOME/tarot/sessions` or `~/.local/state/tarot/sessions`, with private directory/file permissions. They record session identity, the compositor PID and exit reason; they do not dump environment variables or credential contents. Log rotation and diagnostic export remain release work. `--diagnostics` explicitly enables the existing restricted control socket at `$XDG_RUNTIME_DIR/tarot-session/control.sock`; it supplies no unlock operation.

The supervisor holds an exclusive user-session lock, removes only an owned stale diagnostic socket before startup, and removes that entry again after compositor exit. A socket destructor alone is insufficient after SIGKILL. Files, symlinks and wrong-owner entries are preserved. The supervisor does not restart a dead compositor into an ordinary session. It observes exit before reaping its process-group leader, cleans that group while its ID cannot be reused, then asks elogind to terminate its verified PAM session, including the private bus and applications. Greetd should return to Phrog, where another login requires authentication. Actual device crash and cleanup evidence determines whether this contract is met; compositor startup protection alone is insufficient.

## Source package

The [APKBUILD](../packaging/nura/APKBUILD) installs four ordinary root-owned executables, the [Wayland session entry](../packaging/nura/tarot.desktop), the existing [PAM reauthentication service](../packaging/pam/tarot-lock), project licenses and source attribution. It makes no greeter configuration or autologin changes. Dynamically loaded PAM, EGL/GLES, Wayland client/EGL, GPU-driver and keyboard-data dependencies are explicit; abuild also derives ELF library dependencies.

Create a source snapshot from the exact lockfile, including vendored registry sources:

```sh
cargo vendor --locked --versioned-dirs /absolute/vendor
python3 scripts/package-source.py --vendor /absolute/vendor --output /absolute/aports/local/tarot
```

The snapshot command verifies vendor package/file checksums, excludes device artifacts/git/build outputs/user Cargo credentials, fixes tar ownership/timestamps and stages a recipe with the actual source archive SHA-512. It also writes a SHA-256 source manifest. The recipe builds offline/frozen from that archive. A package and a source archive are separate artifacts; retain both with build logs and the compiler/platform record.

Use abuild's complete build sequence when changing snapshots. Do not retain a previous extracted source tree's Cargo target directory while overwriting it with another fixed-timestamp archive: Cargo can miss changed source contents when mtimes stay fixed. A fresh source/target tree and clean package staging are required; verify expected launcher options and binary hashes in the resulting artifact. Incremental rootpkg-only revisions can also retain old automatically generated command-provider metadata if the package staging tree is reused.

On a native AArch64 Alpine/Nura builder, use the staged recipe with normal `abuild` dependency installation/signing. It uses Alpine's native compiler/system libraries. The repository's Rustup-based phone build remains available for ordinary development.

The workspace minimum is Rust 1.89, verified with host tests and an AArch64 DRM check. The recorded Alpine source builder uses Rust 1.96.1; keep compiler, target standard library and sysroot identities with each artifact.

The verified cross-build setup uses an isolated Alpine 3.24 builder with `cargo`, `abuild`, `clang`, `lld`, `llvm` and `pkgconf`; the same Alpine Rust version's AArch64 standard library; a read-only phone SDK; and a target package database for automatic dependency lookup. The relevant variables are:

```sh
CBUILD=x86_64-alpine-linux-musl
CHOST=aarch64-alpine-linux-musl
CARCH=aarch64
CTARGET=aarch64-alpine-linux-musl
CBUILDROOT=/absolute/aarch64-package-root
TAROT_RUST_TARGET=aarch64-alpine-linux-musl
TAROT_SYSROOT=/absolute/phone-sdk
TAROT_GCC_RUNTIME=/absolute/gcc-root/usr/lib/gcc/aarch64-alpine-linux-musl/15.2.0
AR_aarch64_alpine_linux_musl=llvm-ar
STRIP=llvm-strip
```

Alpine Rust uses the `aarch64-alpine-linux-musl` target, rather than the Rustup toolchain's `aarch64-unknown-linux-musl`. Alpine's `libgcc-static` target package supplies `crtbeginS.o`, `crtendS.o` and `libgcc.a`; the linker accepts their build-only directory explicitly. The native and target Rust standard libraries must match compiler version. The target package database must contain the providers of the linked SONAMEs; a directory of copied `.so` files alone is insufficient for abuild's metadata phase. Cross-building skips target test execution, so retain host checks and actual device verification separately.

## Installation and recovery evidence

On this authorized spare development phone, install the reviewed local APK through the package manager. A development signing key need not be added to the device's global trust store; a hash-checked local package can be installed with a one-command `--allow-untrusted` exception. Normal release signing/distribution, updates/rollback and a maintained distribution UI meta-package remain unfinished.

Keep Phosh and the existing Phrog/greetd configuration installed. Select Tarot's desktop entry through the greeter, or use the root-only QA login helper with `--session tarot --diagnostics` to test its fixed installed command through actual greetd PAM. The latter validates the launched command/session, not manual session-picker interaction.

The owner now requests Tarot as the phone's default session and final state after routine QA. Phrog stores its selection in the greeter account's `mobi.phosh.phrog last-session` GSettings key; use the greeter's actual environment/account when changing it and verify a fresh greeter reads `tarot`. This selects the installed desktop entry; it does not enable autologin or change PAM policy. Finish tests in the normal Tarot entry unless a specific recovery or stock-comparison workload requires otherwise.

Revision 9 adds `--hardware-keys` to the installed compositor command. Its clean package/signature/payload audit, Tarot-only local update, native power/volume and actual PAM cancellation replay pass. The owner confirms the physical buttons; the final normal session audit verifies loaded hashes and the persisted Tarot default with diagnostics absent. The update preserves the existing `HandlePowerKey=ignore`, greetd/PAM policies and user settings. Earlier revision 8 login/suspend endurance and rollback evidence retain their earlier binary scope. [Revision 9 evidence](IMPLEMENTATION_STATUS.md#hardware-buttons)

`tests/phone_installed_session_replay.py` expects that authenticated diagnostic session on VT7. It reads the account password at a hidden prompt, creates temporary uinput input, checks protected startup and actual PAM, launches the installed GTK Calculator through Tarot's catalog, verifies the app shares the login cgroup, and can exercise real RTC-woken suspend. Its `--crash` case kills only the verified installed compositor and requires the old session cgroup to empty and Phrog to return. `--detach` keeps the controller alive across USB pauses without storing credentials. `--rotate-protected` also requires each protected orientation to present a full-size lock surface without exposing an ordinary frame or allowing capture. Its password path requires a confirmed current portrait layout and stops if orientation changes during typing. It does not certify manual sensor input or all wake sources. Use the independent stock login/output checks to verify the final recovery desktop.

`tests/phone_installed_session_cycles.py` reuses those checks for ten normal greetd/PAM login and clean-exit cycles without kernel sleep. Each cycle requires actual touch/PAM unlock, a catalog-launched native application, audio readiness and complete login-cgroup teardown. It obtains the password once at a hidden prompt and can detach with credentials retained only in controller memory. It stops at the first failure and leaves Phrog ready for recovery login.

The independent `tests/phone_installed_session_cleanup.py` checks current stock-session identity, all old recorded login cgroups, test resources, services, network, brightness and policies. Its explicit `--allow-failed-evidence` mode verifies cleanup after an interrupted attempt without changing that attempt's failed result. Preserve any manual repair separately.

`tests/phone_package_switch.py` prepares a local rollback or reinstallation from an archived APK and its package audit. It refuses a live Tarot session, verifies a private root-owned copy against the recorded package digest, and simulates a Tarot-only transaction with networking disabled before applying it. Installed payload hashes, ownership/modes, policies and the original settings digest are checked afterward. A package transaction does not prove that its session works: run an actual greetd/PAM/app/audio/exit replay on the retained revision, restore the current audited package, repeat that session check and verify the final normal session. The revision 8 → 5 → 8 round trip and both actual session checks now pass; revision 8 is restored and the normal Tarot entry is left running as requested. General schema rollback and release distribution remain separate gates.

See [current implementation evidence](IMPLEMENTATION_STATUS.md) for actual outcomes. The full M3 and development-plan criteria remain authoritative.
