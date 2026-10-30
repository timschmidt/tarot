# Cooperating Rust/egui menu example

[`native-menu-egui`](../examples/native-menu-egui.rs) is an ordinary Wayland
application. It owns its xdg-shell surface, renders egui through the existing
Rust EGL/GLES integration and exports menus/actions with Rust/zbus. No GIO or
GTK code is linked into the producer. Standard Linux graphics and session-bus
services remain dependencies.

```sh
cargo build --locked --offline --example native-menu-egui
# Inside the intended desktop's actual session environment:
cargo run --locked --offline --example native-menu-egui
```

The example uses the current `DBUS_SESSION_BUS_ADDRESS` and its connection's
unique name. It declares that name and its menu/application/window object paths
on a GTK extension of its **own** Wayland surface. Tarot validates the actual
accepted-client and exporter process handles; matching an application ID or
Unix UID does not authorize a claim. The producer keeps the export connection
alive until the window exits. It releases its GTK extension and xdg role during
cooperative close.

Preferences contains a string-target theme action, a boolean toggle, a disabled
action and an integer-target window action. Successful callbacks update the
app's own state, and `org.gtk.Actions.Changed` publishes observed changes to
readers. Theme state also supplies the menu's selected mark. The same action
handler serves the in-window Choose and Toggle controls. `--no-claim` keeps
those controls while omitting the shell claim; the example still uses a session
bus. Capabilities remain zero, preserving app-owned controls.

The [export module](../examples/native_menu_egui/export.rs) implements the
GLib 2.88.1 private wire formats for menu `Start`/`End` and action
`List`/`Describe`/`DescribeAll`/`Activate`/`SetState`/`Changed`. Its menu group is
static, so it needs no dynamic-menu subscriber table. Both app/window action
groups share owned state; typed arguments and allowed theme values are checked
before callbacks. This intentionally small example is **not** a stable app SDK
or a general-purpose dynamic menu exporter. It has pointer/touch buttons, not a
complete application keyboard, IME or accessibility implementation. It currently
uses buffer scale 1; handling each output's scale remains application API work.
[Menu exporter source](https://github.com/GNOME/glib/blob/2.88.1/gio/gmenuexporter.c)
· [Action exporter source](https://github.com/GNOME/glib/blob/2.88.1/gio/gactiongroupexporter.c).

## Executed interaction checks

The [recorded checkpoint](recordings/menu-egui/README.md) uses immutable binaries
and real rendered-label input. A separate GIO reader checks the Rust producer's
hierarchy, descriptions, argument types and live signals without activating
anything. Actual callbacks and counts come from the producer, independently of
the shell's presentation metadata.

```sh
dbus-run-session --config-file=tests/native_menu_session.conf -- \
  python3 tests/native_app_menu_replay.py \
    --compositor /absolute/saved-tarot \
    --producer /absolute/saved-native-menu-egui \
    --output /absolute/fresh-output
```

Add `--landscape` for the other pose. The harness provides a private X display,
software EGL, ordinary Wayland connection and private bus. The program's
`--report` and `--qa-stdin` options are explicit synthetic QA facilities.
`flip`/`disable` stdin commands mutate exports and never invoke actions.
The native catalog wrapper supplies only an owned 0600 FIFO as stdin, preserving
the ordinary app's login-session ownership. Raw reports/captures remain private.

For the installed parent replay's `--native-menus` case, create its
`<output>/native-menus` directory for UID 10000 before starting the diagnostic
login. Stage catalog entries `test.tarot.NativeMenuGtk.desktop` and
`test.tarot.NativeMenuEgui.desktop`, each executing `native_menu_stdin.py --fifo
<output>/native-menus/Gtk.fifo -- /usr/bin/python3 <sources>/native_app_menu_fixture.py
--report <output>/native-menus/Gtk.jsonl` or the equivalent Egui command using
the audited native example with `--qa-stdin`. The FIFOs must belong to UID 10000
with mode 0600. `fixtures.json` maps `Gtk` and `Egui` to their actual
`executable_sha256` values. Use the login's real bus/Wayland environment; the
catalog launch preserves its PAM cgroup. The controller must retain and restore
the owner's settings, remove only its created entries after logout, and verify
the final normal session independently. This case requests no suspend cycles.

Next develop a reusable application API when implementing actual Tarot apps,
cover dynamic menu lifetimes and richer typed actions, and complete broader
native compatibility and accessibility. KDE requires its distinct adapter; GMenu
compatibility does not imply DBusMenu support.
