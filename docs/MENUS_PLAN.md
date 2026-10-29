# App and device menus: implementation plan

Research checkpoint: October 29, 2026. Revision 39 implements the initial shared shell menus after the Just Type checkpoint, with scoped native component and separate page/restart evidence. Physical/visual acceptance, remaining interaction coverage and native exported-action adapters remain pending. [Evidence and limits](recordings/r39/README.md). [Current implementation contract](MENUS.md). It refines M1/M2 and the original packet's [device menu](Tarot_Design_Packet/screens/png/08-device-menu.png), [app menu](Tarot_Design_Packet/screens/png/09-app-menu.png) and fidelity capture G.

## Product contract

The left title area opens the current app/activity menu; the right status area opens the device menu. Both descend from their anchors, use Nocturne's dark material, separators and touch row rhythm, and leave a dimmed foreground visible. They are overlays over the existing activity rather than replacement app views. A second opening changes the active root instead of stacking independent scrims. Repeated activation of the same title dismisses its menu.

Use the packet's proposed 56-unit row as the portrait starting point. Clamp panel width/height to the actual output and keep long menus scrollable above the gesture strip. In landscape, preserve readable rows and expose submenus in the same bounded panel with an explicit return control. Long labels, enlarged text and reduced motion need their own layouts. These dimensions are proposed adaptations until real-panel/reference calibration.

Outside contact dismisses and is consumed. Escape returns from a submenu, then dismisses the root. Back follows that menu hierarchy without inventing application navigation. Opening cancels an owned card/icon gesture and retires underlying client/shell text focus. Closing restores eligible input only for the same surviving active card/activity; protection, client replacement or a changed view cancels restoration. Existing stale-held-input guards remain authoritative. A press on an old menu item cannot activate a replacement row after a service, client or menu revision changes.

## Initial functional coverage

| Surface | First implementation | Later dependencies |
| --- | --- | --- |
| Device root | Actual date and battery state, Wi-Fi summary/submenu, brightness control and links to existing sound/display/locking controls | Bluetooth, coordinated airplane mode and call/ringer policy after their real service adapters |
| Device details | Reuse current asynchronous observed/pending/error behavior, network credentials and atomic preferences | Full Settings app and permission agents |
| Home/Card activity menu | Just Type, existing favorite organization and relevant existing card/stack actions | Additional shell-owned activities |
| Launcher/Just Type | Existing page, search and favorite commands acquire the shared material/anchor/dismissal behavior | Provider preferences and remaining Just Type sources/actions |
| Ordinary native app | Existing shell-owned card navigation and cooperative close, clearly presented as card controls | Actual exported application actions through validated adapters |
| Cooperating Rust app | Registered commands with stable IDs, enabled/state snapshots and declared parameter types | Reusable public application SDK after one real client proves the contract |

The illustrative packet's mail commands, Bluetooth state, network name and battery value are not runtime defaults. Do not present New message, Preferences, Copy or Paste as supported app commands without a corresponding capability. A cooperative close may open the app's real unsaved-changes dialog or be refused; it must not silently force termination. Native clients continue to own their content and internal controls.

## Native menu interoperability

GTK/GIO menu models carry sections, submenus, named actions and state/target data, with application and window action namespaces. Menu content alone is insufficient: enabled state, typed action parameters and the appropriate action group must be observed too. [GIO menu model](https://docs.gtk.org/gio/class.MenuModel.html).

GTK's own Wayland extension can associate a surface with application/window objects, menu paths and a unique D-Bus name. It also defines global-menu capability flags. This is GTK-specific cooperation, separate from core xdg-shell. The inspected XML is version 7 at file commit `b3d08d3` (February 19, 2026), not an installed-client compatibility claim. [Pinned GTK protocol source](https://github.com/GNOME/gtk/blob/b3d08d3e60136da2522de3313cf4479e1d6fe20f/gdk/wayland/protocol/gtk-shell.xml).

KDE's appmenu protocol associates a Wayland surface with a D-Bus service/object implementing `com.canonical.dbusmenu`; an adapter for it is a distinct protocol/data-model task. The inspected XML is version 2 at file commit `d5e0289` (September 23, 2025). [Pinned KDE protocol source](https://github.com/KDE/plasma-wayland-protocols/blob/d5e0289c156c8c07156d5d228e817c368525e2e7/src/protocols/appmenu.xml).

GIO explicitly identifies its exported-menu D-Bus interface as private. Before choosing a Rust implementation, compare a Rust GIO-binding worker against a version-pinned direct Rust D-Bus adapter. The former uses the allowed standard GLib dependency; the latter incurs private-wire-format maintenance. Neither requires replacing egui or adding GTK widget rendering to Tarot. Do not commit to a wire implementation until a real export/state/action/lifecycle spike passes. [GIO exporter contract](https://docs.gtk.org/gio/menu-exporter.html).

Do not advertise a global-menu capability until its complete supported behavior is usable. GTK also has shell-menu/menubar settings, so capability advertisement must be tested against actual in-app presentation rather than assumed harmless. [GTK app-menu property](https://docs.gtk.org/gtk3/property.Settings.gtk-shell-shows-app-menu.html) · [GTK menubar property](https://docs.gtk.org/gtk3/property.Settings.gtk-shell-shows-menubar.html).

Bind exported commands to the owning surface/client and current D-Bus owner. Resolve well-known names to unique owners and validate process identity; shared UID/session cgroup alone is not an app boundary. Helpers and sandbox proxies need explicit ownership/permission rules. Owner replacement, surface destruction or a revised menu invalidates old choices. Keep D-Bus work off the compositor loop, bound snapshots/nesting/strings and action deadlines, and expose recoverable failures without blocking native controls. Limits must be measured against real apps and recorded as adapter limits rather than universal menu conformance.

## Delivery and acceptance

1. Introduce shared overlay coordination and versioned item identities in the core model. Prove mutual exclusion, stale held-row rejection, view/client replacement, protection and submenu cancellation. Keep native surface lifecycle and existing card arrangement separate from app command capability.
2. Render the shared menu material/anchors and move the existing Launcher/Just Type menu onto it. Test real widget hit regions, scrollers, enlarged/long text, portrait/landscape, reduced motion and dismissal with no underlying action.
3. Add the compact device root while retaining current detail pages and backend/credential semantics. Verify brightness and volume against independent service snapshots, preserve observed/pending/error distinctions and restore exact original device settings after QA.
4. Add the app-title/card-controls menu, including cooperative unsaved/refused close and client death while open. Preserve real native text input, password masking and popup ownership before/after menu dismissal. Keep the retained Firefox input failures as independent open gates.
5. Spike one real exported native menu and one cooperating Rust app. Test stateful and disabled actions, changed typed targets, submenus, owner replacement, wrong-client claims, malformed/oversized data, timeout and disconnection. Record support per application before enabling an adapter generally.
6. Freeze/build/sign and test the installed ordinary-user session with actual touch, four orientations, real PAM and saved restart. Re-run affected favorite/page/card, toolkit, terminal and keyboard cases; independently verify exact preference/policy restoration, normal default Tarot, diagnostics off and Panfrost. Capture packet G for a separate visual/physical decision.

Menu composition does not close notification well/rail/dashboard, lock presentation, service permissions, broader app compatibility, current-binary endurance or other remaining M0–M6 gates.

## Next native-export experiment

October 29 follow-up: start the GTK/GIO comparison with a separate, pinned host experiment using `gio` 0.21.5, without enabling a compositor capability. Its upstream workspace declares Rust 1.83, compatible in declared minimum with Tarot's 1.89 policy; actual dependency resolution and minimum-compiler builds must still pass. The inspected tag resolves to commit `8b654067dcac04ebb3c7cce6a7aa5926155776fa`. Both menu and action proxies are exposed by that binding. [Pinned workspace manifest](https://github.com/gtk-rs/gtk-rs-core/blob/8b654067dcac04ebb3c7cce6a7aa5926155776fa/Cargo.toml) · [Menu proxy](https://github.com/gtk-rs/gtk-rs-core/blob/8b654067dcac04ebb3c7cce6a7aa5926155776fa/gio/src/auto/dbus_menu_model.rs) · [Action proxy](https://github.com/gtk-rs/gtk-rs-core/blob/8b654067dcac04ebb3c7cce6a7aa5926155776fa/gio/src/auto/dbus_action_group.rs).

Keep the GIO objects on one owned worker/main context. GIO requires both menu proxy access and its signals to use the original thread-default context. Action proxies can start empty; subscribe to changes before taking the initial action list. Send bounded immutable Rust snapshots and typed requests across the compositor boundary. This worker arrangement is the proposed implementation, not a demonstrated adapter. [Menu context contract](https://docs.gtk.org/gio/type_func.DBusMenuModel.get.html) · [Action initialization/context contract](https://docs.gtk.org/gio/type_func.DBusActionGroup.get.html).

The current host reports GIO/GLib 2.86.5 through `pkg-config`. The existing phone cross sysroot has no `gio-2.0.pc`, `glib-2.0.pc` or `gobject-2.0.pc`, and the APK recipe lists the GLib runtime without its development package. Therefore a successful host binding experiment alone cannot establish phone buildability. Add the distribution's appropriate development files to the isolated cross-build inputs, preserve the existing runtime policy, and repeat minimum-compiler/ARM/package checks before introducing a production binding. No dependency or build recipe has changed for this experiment yet.

Use an owned session bus and an actual exporter with a submenu, a disabled action, a boolean state and a typed target. Require initial population, mutation signals, exact activation arguments, disable-after-press rejection, unique-owner replacement, timeout and teardown. Then associate that export with a real Wayland surface and prove the wrong-client rejection rules above. A cooperating Rust/egui client is the second required producer. Preserve ordinary native controls throughout; advertise no global-menu capability until real client behavior has been verified. Compare the measured worker/dependency costs with the direct Rust D-Bus prototype before selecting the production adapter. GIO's private wire interface remains the maintenance reason for this comparison, rather than a public protocol guarantee. [Exporter contract](https://docs.gtk.org/gio/menu-exporter.html).

## Revision 40 component checkpoint

Revision 40 implements proposed menu motion with a presented-frame input-retirement gate, held-key ownership cleanup, vector glyphs and a separated brightness rail, pinned/truncated submenu headings and landscape credential scrolling. Host checks and one fourteen-component native/page/restart controller pass. An additional host native-to-menu held-key regression fails in portrait and landscape; its original zero-held-key gate requires correction before complete menu keyboard acceptance. The timings remain an adaptation requiring packet-G calibration. Enlarged text and the exported-menu experiments above remain independent work. [Candidate evidence](recordings/r40/README.md).

### Exported action completion and time bounds

The GLib 2.86.5 implementation adds a constraint to the GIO-worker comparison: its action proxy sends Activate/SetState without a result callback, and initial action loading uses the default D-Bus timeout. An initial loading error leaves an empty action table. Thus a successful proxy invocation cannot establish application completion, and an empty list alone cannot distinguish a valid empty export from failed loading. This is a source-derived limitation, not a tested Tarot adapter. [Pinned implementation](https://github.com/GNOME/glib/blob/2.86.5/gio/gdbusactiongroup.c).

The experiment must compare a bounded explicit call path with the proxy path. Define separate loading, available, unavailable and retired states; impose Tarot's own load/action deadlines and cancel stale generations. Only observed state changes may confirm a stateful result. A method reply establishes transport/application dispatch, not completion of a document operation. Keep proxy objects and callbacks on the owned worker context, and keep usable card controls available throughout. Measure real refusal, disconnected and silent exporters before selecting the production approach.
