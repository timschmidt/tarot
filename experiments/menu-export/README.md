# Exported menu and action feasibility experiment

A separate Rust 1.89 workspace tests GIO menu/action exports and proxies on an owned private D-Bus. It uses pinned `gio` 0.21.5 and its own lockfile. Tarot's production workspace, dependencies and phone package remain independent. The tested native library is GIO/GLib 2.86.5.

From the repository root, with Rust, `pkg-config`, GIO development files and `dbus-run-session` installed:

```sh
cargo build --manifest-path experiments/menu-export/Cargo.toml --locked
dbus-run-session -- experiments/menu-export/target/debug/tarot-menu-export-spike
cargo clippy --manifest-path experiments/menu-export/Cargo.toml --locked -- -D warnings
```

Use `cargo +1.89.0 build` for the minimum-compiler check. A successful run prints a JSON report. Each run creates its own exporter and consumer connections; their GIO objects stay on separately owned thread-default main contexts. The experiment observes initial asynchronous population, a submenu/string target, boolean state after activation, disabled actions, rejected argument types, menu/enabled-state mutation and unique-owner replacement.

The stalled-exporter case has an explicit 100 ms transport deadline and verifies the timeout error domain. The handler subsequently executes the action. Therefore timeout leaves the action outcome uncertain: retry and cancellation policy must account for an operation already dispatched. The report requires the observed state/callback results separately from a method reply. These fixture timings are experimental values.

Wayland surface/client credential binding, real GTK/KDE application exports, a cooperating Rust/egui app, bounded/adversarial snapshots, capability advertisement, application-level acknowledgement, performance and the ARM/phone dependency build remain follow-up work. The experiment adds no shell capability. [Integration plan and pinned implementation research](../../docs/MENUS_PLAN.md).
