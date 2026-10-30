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

The later [revision-47 compatibility checkpoint](recordings/menu-compat/README.md)
repeats both native producers in the same login as the seven protocol cases and
real GTK/Qt dialog/text/fullscreen checks. The exporter remains unchanged; a
separate compositor input fix clears shell hover on native pointer handoff.

## Claim and lifecycle QA

```sh
python3 tests/native_menu_claim_replay.py --xvfb /absolute/path/to/Xvfb \
  --compositor /absolute/saved-tarot \
  --producer /absolute/saved-native-menu-egui \
  --output /absolute/fresh-claims
```

Add `--landscape` for the other layout. This replay owns its private bus; do not
wrap it in a production-session bus replacement. It checks foreign process
claims, clear/release/reclaim, held input, finite claim churn, same-bus exporter
reconnect, exact-name reuse across private bus GUID replacement and the trusted
input-method connection. [Executed host/native scope and limits](recordings/menu-claims/README.md).

The sample's explicit `--qa-stdin` accepts `claim-self`, `clear-claim`,
`release-claim`, `claim-other UNIQUE_NAME`, `close-export`, `reconnect-export`,
`flip`, `disable` and `exit`. A claim mutation acknowledgement follows a
Wayland round trip. These controls never invoke an action; only actual shell or
app input does. `--export-only --qa-stdin` supplies the independent process's
static exports without creating a Wayland window. This remains a synthetic QA
interface, with no claim of production stdin resource isolation.

Add `--traffic` to the host claim replay to send eleven synthetic 96 KiB
broadcasts before the claim matrix. The QA-only `noise CASE` stdin command emits
traffic without activating actions. An independent GIO observer verifies actual
sender/path/interface/member/size: eight unrelated broadcasts must preserve the
complete menu and stamp; three relevant oversized broadcasts must retire it and
allow fresh claims. The observer's broad subscription is test machinery; the
production worker subscribes only to its validated export's endpoints and owner
changes. [Reproduced failure and recorded scope](recordings/menu-signals/README.md).

For the installed runner's `--native-menu-claims` hook, prepare the user-owned
`<output>/native-menu-claims` directory, its mode-0600 `commands.fifo` and
`fixture-audit.json` containing `producer_sha256` and `python_sha256`. A temporary
`test.tarot.NativeMenuClaims.desktop` entry runs `native_menu_stdin.py --fifo
<folder>/commands.fifo -- /usr/bin/python3 <sources>/native_menu_claim_fixture.py
--producer <saved-native-menu-egui> --output <folder>`. The catalog-owned wrapper
starts both producers in the actual login cgroup. Root observations verify their
hashes/start times/cgroup; actual touch/key input supplies every callback.
Preserve exact owner preferences, remove only the temporary entry, end the test
login and independently verify normal/default Tarot and Panfrost afterward.
This device component does not replace its bus or request kernel sleep.

The installed runner's `--native-menu-traffic` flag includes the claim component.
Also prepare an owned mode-0600 `foreign.fifo` in the same directory and add
`--traffic` to the catalog wrapper's `native_menu_claim_fixture.py` arguments.
Stage `native_menu_traffic_observer.py` and `native_menu_traffic_cases.py` beside
the fixture. The wrapper launches the observer in the ordinary PAM cgroup;
the replay verifies its UID, executable hash and process lifetime along with
both exporters. Require all eleven independently observed packets, zero foreign
callbacks and observer/fixture teardown before normal-session acceptance.

Next develop a reusable application API when implementing actual Tarot apps,
cover dynamic menu lifetimes and richer typed actions, and complete broader
native compatibility and accessibility. KDE requires its distinct adapter; GMenu
compatibility does not imply DBusMenu support.
