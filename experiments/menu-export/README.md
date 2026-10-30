# Native menu export experiments

This separate Rust 1.89 workspace exercises GIO exports and their ownership without enabling a Tarot shell capability. It uses pinned `gio` 0.21.5 and its own lockfile: 69 packages including this experiment, with 68 registry dependencies. Tarot's production dependencies and installed phone package remain independent.

From the repository root, with Rust, `pkg-config`, GIO development files and `dbus-run-session` installed:

```sh
cargo build --manifest-path experiments/menu-export/Cargo.toml --locked --target-dir target/menu-export --bins
dbus-run-session -- target/menu-export/debug/tarot-menu-export-spike
dbus-run-session -- target/menu-export/debug/menu-ownership
cargo clippy --manifest-path experiments/menu-export/Cargo.toml --locked --target-dir target/menu-export --all-targets -- -D warnings
```

Use `cargo +1.89.0 build` with a separate target directory for the minimum-compiler check. Each executable prints a JSON report after its assertions pass. Both require their own private bus; the ownership executable also creates its own private Wayland socket and two producer processes.

## GIO export and completion

The first executable observes initial asynchronous population, a submenu/string target, boolean state after activation, disabled actions, rejected argument types, menu/enabled-state mutation and unique-owner replacement. Its exporter and consumer connections keep GIO objects on separately owned thread-default main contexts.

The stalled-exporter case has an explicit 100 ms transport deadline and verifies the timeout error domain. The handler subsequently executes the action. Timeout therefore leaves the action outcome uncertain; it does not establish cancellation or permission to retry. The report requires actual callback/state observations separately from a method reply. Fixture timings are experimental values.

## Surface and process ownership

`menu-ownership` uses the pinned [GTK protocol](protocols/README.md) for actual `get_gtk_surface` and `set_dbus_properties` requests. Its resources are unmapped `wl_surface` objects on a minimal private server. It binds version 4 with zero global-menu capability flags. It does not render an application, implement general GTK-shell behavior or modify the installed compositor.

Two producers share the same Unix UID and cgroup but have different processes, Wayland connections and real GIO menu/action exports. The server captures the connecting process through `SO_PEERPIDFD`. A dedicated worker obtains the claimed unique D-Bus connection's `ProcessFD`, validates retained live handles and loads that exact export. A successful explicit activation must also produce the expected callback and argument in the correct producer.

The comparison requires 64-bit Linux pidfs identities. It checks filesystem type, device/inode identity and process liveness; it has no numeric-PID, UID or cgroup fallback. Unsupported kernels/credentials cannot validate a claim. The D-Bus credential is optional, and malformed descriptor types/indices are rejected before calling GIO. [D-Bus credential contract](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials) · [Pinned Linux pidfs identity implementation](https://github.com/torvalds/linux/blob/v7.0/fs/pidfs.c).

The scenario requires rejection of another process's export, a well-known name in the GTK unique-name field, retired surface/extension/claim generations, an exporter connection that closes or is replaced in the same process, and process exit. It exercises surface destruction and claim mutation with validation replies pending, then attempts stale dispatch through the worker. Recreated surfaces and fresh exports must work. Exact callback counts reject extra forwarding from denied cases. The fixed known fixture action is intentional; this does not implement a general menu decoder or application SDK.

The first owned-bus attempt exposed a malformed-index panic in the GIO wrapper. Explicit descriptor-list bounds checking fixes it; its original failed source/report remains retained separately. Current and minimum-Rust host checks and strict lint pass. ARM builds and strict ARM lint pass against an isolated phone SDK. Stock phone D-Bus omits the required credential and rejects the first legitimate claim. A controlled private same-version daemon build enables the target feature and passes the full current/minimum-Rust ownership scenarios; GIO regressions pass across both daemon configurations. [Native comparison, retained failure and exact scope](../../docs/recordings/menu-ownership/README.md).

## Phone build inputs and remaining gates

The isolated cross SDK uses the phone's signature-verified development packages (`glib-dev`, `libffi-dev`, `pcre2-dev`, `util-linux-dev`, `zlib-dev`, `libeconf-dev`) and matching runtime libraries. This checkpoint uses GLib 2.88.1 on the phone and 2.86.5 on the host. SDK construction does not install packages or alter the normal session. The root [cross-linker](../../scripts/aarch64-linker.sh) can be reused with `TAROT_SYSROOT` pointing to this separate SDK, `PKG_CONFIG_ALLOW_CROSS=1`, target SDK `PKG_CONFIG_SYSROOT_DIR`/`PKG_CONFIG_LIBDIR`, and the existing AArch64 C compiler wrapper. Build this manifest with `--target aarch64-unknown-linux-musl`, separately from production packaging.

Mapped real GTK/KDE application exports, a cooperating Rust/egui producer, bounded/adversarial menu snapshots, bus-daemon replacement/recovery, permission/delegation rules, activation tokens, an actual shell menu and pressed-row ownership remain required. Retiring a choice cannot revoke an action already dispatched across D-Bus; the timeout result above remains authoritative. Compare GIO with a direct Rust D-Bus adapter before selecting production dependencies. Performance, resource budgets and complete phone UI/PAM/KMS acceptance are not established by these protocol experiments.
