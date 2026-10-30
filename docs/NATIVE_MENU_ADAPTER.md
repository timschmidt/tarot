# Native menu transport and client ownership

The development compositor now retains a Linux process handle for each ordinary
accepted Wayland client. Its client authority retires on actual Wayland
disconnection, independently of process exit. Trusted locker and keyboard
socketpairs have neither ordinary peer authority nor eligibility for exported
app commands. A missing kernel capability leaves ordinary native windows and
card controls available.

The shared [Rust library](../src/lib.rs) supplies process identity and a direct
`zbus` menu transport. It adds no Cargo dependency and does not link GIO into the
adapter. The installed phone remains revision 45: this development compositor
and adapter have not yet been packaged as a new phone UI. The subsequent
[mapped GTK checkpoint](recordings/menu-ui/README.md) adds surface claims, a
signal-driven worker and visible app commands on the host, while global-menu
capability flags remain zero.
[Executed evidence and remaining integration](recordings/menu-adapter/README.md).

## Ownership and lifecycle

A binding holds the original D-Bus connection, a syntactically valid unique
exporter name, validated object paths, retained Wayland and exporter process
handles, and an authority lease. `GetConnectionCredentials` must provide a real
`ProcessFD`; numeric PID, UID and cgroup equality never substitute. Both handles
must identify the same live 64-bit Linux pidfs object. The peer implementation
checks the filesystem, device/inode identity and liveness, and retains ownership
of its close-on-exec descriptor.

Dropping or retiring a client/surface/claim authority invalidates its leases.
Leases can intersect so any participating authority can retire pending work.
The development compositor wires client authority into acceptance/disconnect
and claim authority into GTK property replacement, release and surface
retirement. Foreground/protection changes retire the active worker and its
published choices. A snapshot belongs to one binding, and another binding
cannot activate it. The adapter never reconnects an old binding to a new bus.

All RPCs require a connection configured with at most a 500 ms method timeout.
The connection helper requires an explicit `DBUS_SESSION_BUS_ADDRESS` and does
not discover a different desktop's bus. Successful credential replies must come
from the bus driver; successful menu/action replies must come from the exact
unique exporter. UI/render callbacks must never call this blocking transport.

## Wire subset and bounds

The implementation follows the private `org.gtk.Menus` and `org.gtk.Actions`
interfaces in GLib 2.88.1's [menu exporter](https://github.com/GNOME/glib/blob/2.88.1/gio/gmenuexporter.c)
and [action exporter](https://github.com/GNOME/glib/blob/2.88.1/gio/gactiongroupexporter.c).
It loads `DescribeAll`, follows menu `Start` groups, and sends a matching `End`
even when loading fails. A failed/invalid cleanup closes that connection rather
than accepting the candidate. Referenced links must have exactly two unsigned
integer fields; an unchecked library tuple conversion is deliberately avoided.

| Accepted data | Limit |
| --- | --- |
| Each menu/action reply body before typed decoding | 64 KiB, no attached descriptors |
| Combined accepted snapshot reply bodies | 256 KiB |
| Subscribed menu groups / menus | 16 / 64 |
| Wire items and expanded display items | 256 each |
| Link depth | 8; cycles rejected |
| Attributes per item / attribute-name bytes | 16 / 128 |
| Display label, action name, scalar string bytes | 512 |
| Combined action descriptions / state values per action | 256 / 1 |
| Endpoint paths / unique name bytes | 1,024 / 255 |

GLib omits empty menus from successful group replies. An omitted root or linked
menu therefore becomes a valid empty model after its group is subscribed; an
RPC failure remains an error. Real GIO empty-root and empty-submenu regressions
cover this distinction. Reply entries from a different requested group are
rejected.

Plain UTF-8 labels, submenu/section links, enabled actions, string/integer targets
and boolean/scalar state are represented as owned Rust data. Supported scalar
signatures are `b`, `s`, `i`, `u`, `x`, `t`, and finite `d`. Unsupported parameter,
state or target types leave their rows unavailable; arguments are not coerced.
Action lookup routes `app.` and `win.` through their respective declared paths
on the same validated unique connection. Unknown actions remain unavailable.

These are accepted-message and parser/graph bounds. They do **not** bound memory
before zbus has received a complete frame. Transport queue/framing budgets,
connection-setup cancellation and complete worker teardown still require their
own integration and adversarial checks.
The timeout is per RPC. Snapshot loading also checks an overall two-second
deadline between RPCs, with additional bounded RPC/barrier cleanup waits; this
is not a hard transport deadline. Automatic recovery and reconnect/backoff
acceptance remain open.

## Choosing an action

Before activation the adapter checks lease/process liveness, reloads the
candidate, compares the complete displayed tree and action metadata, and checks
the selected row's supported/enabled state and target signature. Disabled,
changed, retired and foreign-binding choices are rejected before dispatch.

`Replied` means a method reply arrived, not that a document operation finished.
Once a call is attempted, a timeout, disconnect or invalid response returns
`Uncertain`. There is no automatic retry. The real producer test demonstrates a
callback executing after the waiting deadline. Retirement cannot revoke a call
already sent; application acknowledgement/idempotency is a separate contract.

The transport is used by an owned foreground menu worker in the mapped GTK
checkpoint. Its remaining acceptance gates are recorded separately. The compositor
must subscribe before loading, track menu/action changes and bus epochs, retire
pressed-row generations even if state later changes back, publish immutable
presentation rows, and dispatch only after the existing menu-retirement frame.
Snapshot comparison alone does not establish that complete interaction contract.
The standalone reader retains its temporary Start/End mode. `watched_snapshot`
keeps one baseline reference per current group; the worker registers its signal
reader before loading and balances unused/extra references on refresh. A D-Bus
match rule alone cannot receive menu changes after the exporter has no subscribers.

## Replays

Build the production-library acceptance client:

```sh
cargo build --locked --offline --example native-menu-transport
DBUS_SESSION_BUS_ADDRESS=... target/debug/examples/native-menu-transport \
  --fixture tests/native_menu_export_fixture.py --output /absolute/fresh-output
```

Use a private `dbus-run-session -- …` on a supported host. On the phone, use
Tarot 45's actual session environment or explicitly select its packaged session
daemon; a fresh default stock bus still lacks the credential. The Python test
fixture needs GI/GIO and creates only owned producer processes, Unix sockets
and report files. It installs no desktop/service files and changes no settings.
It exercises actual GIO exports as well as deliberately malformed menu replies.

The existing menu input replay also accepts `--require-export-peer`, asserting
that the real mapped native client's identity appears in the owner-only
unprotected diagnostic metadata. Its ordinary fallback mode remains usable on
hosts without pidfs support. Protection continues to redact that metadata.

The mapped GTK development checkpoint supplies surface claims, combined modal
hints, the owned signal worker and presentation models. Next prove cooperating
egui exports and native phone behavior, complete adversarial claim/protection
and recovery checks, and implement KDE's distinct adapter. Preserve card
controls while capabilities are absent.
[Complete delivery plan](MENUS_PLAN.md#next-delivery).
