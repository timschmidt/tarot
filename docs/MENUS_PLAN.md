# App and device menus: implementation plan

Research checkpoint: October 29, 2026. Revision 42 passes fourteen scoped native shell components, full pages and saved restart after correcting touch lifecycle and scrolling. Physical/visual acceptance, broader interaction coverage and native exported-action adapters remain pending. Revision 39 retains the initial menu checkpoint. [Current evidence and limits](recordings/r42/README.md). [Current implementation contract](MENUS.md). This plan refines M1/M2 and the original packet’s [device menu](Tarot_Design_Packet/screens/png/08-device-menu.png), [app menu](Tarot_Design_Packet/screens/png/09-app-menu.png) and fidelity capture G.

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

## Revision 41 correction and open search gate

Host investigation reproduces one remembered native key after releasing Shift inside a menu. The shared keyboard retirement routine now clears seat/forwarded state only after removing client focus and guards the physical tails. Both exact failing replays pass unchanged, and expanded tests cover holding a key through dismissal. Both native runs pass these directions through libinput/KMS, then fail rotated Just Type scrolling after seven passing components. Paced QA input does not resolve the phone failure. Remaining components/pages/restart were not reached; exact restoration and independent normal/Panfrost audits pass. Search drag ownership and shell wheel polarity require focused correction and fresh native acceptance. Current evidence is [recorded separately](recordings/r41/README.md); revision-40 failures remain historical evidence.

## Isolated GIO export experiment

The separate [Rust experiment](../experiments/menu-export/README.md) now builds and runs on Rust 1.98.1 and 1.89.0 with GIO/GLib 2.86.5 and strict host lint. Its own lockfile contains 49 packages; Tarot's production dependencies remain unchanged. Distinct exporter/consumer connections and owned thread-default contexts observe initially empty proxies, submenu/string targets, boolean state after activation, disabled handlers, wrong argument-type rejection, menu/enabled-state mutation and unique-owner replacement. This establishes a host feasibility result for the binding, with no shell capability enabled.

A deliberately stalled exporter produces a confirmed 100 ms transport timeout, then executes the action after dispatch resumes. Thus a deadline bounds waiting without establishing cancellation or nonexecution. Treat an in-flight timeout as an uncertain outcome, observe state where available, and require application-level acknowledgement/idempotency before automatic retry. The 100/350 ms fixture timings are experimental values.

The next gate remains association with a real owned Wayland surface and rejection of wrong-client claims, followed by actual GTK/KDE exports and a cooperating Rust/egui producer. Bound and test snapshots/nesting/strings, changed or disabled targets after a press, malformed producers, worker teardown, app acknowledgement, performance and ARM/phone build inputs. The existing phone cross SDK still needs its GIO development input evaluation. Compare this measured binding path with the direct Rust D-Bus approach before selecting the production adapter; the private-interface maintenance constraint remains.

### Stable process ownership precheck

D-Bus credentials can include an optional `ProcessFD`, which avoids numeric PID reuse; missing credentials must be handled explicitly. Linux can provide the corresponding socket peer handle through `SO_PEERPIDFD`. [D-Bus credential contract](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials).

An isolated host bus on D-Bus 1.16.2 supplies a process descriptor whose live PID matches the connecting process. Host and phone Unix-socket probes also obtain peer process descriptors. These are capability observations: the phone's session-bus descriptor support and real Wayland/menu association remain untested. Private executed source and aggregate observations are retained separately.

The next Rust experiment must use distinct producer processes with the same UID/session, actual Wayland surfaces and standard menu claims. Compare live, retained process handles from both endpoints; reject absent or mismatched identity rather than accepting PID/UID equality. Test owner exit/replacement and surface destruction while validation is pending, then reject any stale row before dispatch. Delegated helpers and sandbox proxies require an explicit separate ownership contract. This proposes the initial adapter's supported subset; no production capability is advertised.


## Touch lifecycle prerequisite for input acceptance

Revision 42 addresses the retained rotated search scroll before further menu delivery. A completed touch emits release then pointer departure. Favorite/page handlers process those events in order so the completed intent survives departure. Interrupted input marks departure before synthetic cleanup releases, preventing cleanup from selecting a page or committing a drop. Application tiles leave ordinary drag sensing to the result scroll area while the independent hold engine owns favorite organization. Linux wheel axes are converted only at the shell-to-egui boundary; native axis delivery retains its convention.

The first signed candidate is withheld after genuine release-plus-departure tests expose canceled drops/swipes. The corrected source passes current/minimum-Rust tests, strict host/ARM checks, rendered search/favorite/menu suites and protected recovery checks. The complete fourteen-component native shell/page/restart replay and independent restoration/Panfrost audits pass. The owner also confirms repeated landscape result scrolling and Sound selection with a finger. This does not close browser input, physical calibration or endurance. [Current evidence](recordings/r42/README.md).
