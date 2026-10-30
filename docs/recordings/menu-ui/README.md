# Mapped GTK application menus: development checkpoint

October 30, 2026, Detroit. The development compositor now displays commands from
a real mapped GTK application. GTK surface properties bind the export to the
actual client process and surface lifetime. An owned Rust worker publishes
immutable egui menu rows, retains menu subscriptions and retires stale choices.
The phone remains on installed **revision 45**; this new compositor is not yet
packaged or accepted on native phone graphics/input.

## Resulting behavior

The activity menu shows supported app commands above separately labeled card
controls. Submenus and sections retain their hierarchy, unsupported/disabled
actions stay unavailable, GTK mnemonic underscores are normalized, and observed
boolean/radio state supplies selection marks. The device menu remains separate.
Loading, unavailable and uncertain outcomes have concise status text while card
controls remain usable. App command addresses and parameters never enter the
presentation model.

The worker subscribes before loading and keeps group references after `Start`.
Refreshes balance old references without dropping the baseline subscription.
A relevant exporter signal immediately retires the published choice authority
and advances its revision. A receive-sequence barrier makes the worker process
signals preceding a method reply before accepting that reply's view or choosing
an action. Data that changes back to the same values still retires a held row.
The compositor cancels held input when an open app menu receives a new version.

Dispatch waits for the existing matching menu-retirement presentation, then
checks foreground family/surface, source identity, revision and liveness again.
The worker rechecks its lease and the complete current tree immediately before
sending typed arguments. Once attempted, uncertain calls never retry
automatically. Each surface claim remembers its original bus GUID across
foreground worker restarts. A new daemon cannot silently inherit that claim.
Fault-injection acceptance of daemon replacement remains outstanding.

GTK version 4 is exposed with **zero global-menu capability flags**. Existing
app-owned controls are retained. GTK and xdg-dialog modal hints combine; GTK
modal hints declared before the toplevel are applied when its role appears.
Untrusted present/startup/focus hints do not bypass the shell's focus policy.
The pinned XML and upstream COPYING are included in source/package notices.
This does not certify the complete private GTK protocol or KDE support.

## Executed acceptance

The real GTK3 fixture creates a `Gtk.ApplicationWindow`, exports actual GIO
menus/actions, and uses the normal GTK Wayland property requests. The compositor
loads commands through the same process-credential checks as the earlier
transport. No diagnostic endpoint invokes an app action in this replay.

Both portrait and landscape runs pass:

- A visible Preferences submenu invokes the actual `choose` callback with
  string target `night` through real pointer input.
- Card View and app return stop/restart the foreground worker and revalidate
  the same claim on the same bus with a fresh source identity.
- A visible toggle invokes the actual callback and its exported state updates
  the check mark.
- A real pointer remains down while the exporter disables then re-enables the
  action. The menu revision changes, the held release invokes nothing, and a
  fresh submenu selection succeeds.
- A disabled item stays unavailable, and owner exit removes its commands/card.

Each final fixture records exactly three callbacks: choose/night, toggle/no
argument, choose/night. Captures of the root grouping and checked/disabled
submenu were inspected. This is host nested rendering/input, not historical
visual conformance, physical review or native phone/PAM acceptance.

The no-export fallback replay also passes on the same immutable image, retaining
ordinary card/device menus, hierarchy, keyboard selection, cooperative/refused
close, focus and client death. The prior transport matrix passes with valid
empty models, typed/stateful actions, stale/lifecycle cases, fifteen malformed
producers and a callback executing after timeout without retry. Current and
minimum-Rust workspace tests each pass 237 checks, including three new model
checks for opaque choices, matching presented retirement and revision changes.
Strict host and AArch64 DRM lint pass.

The fixture uses a private bus with no activation service directories while
retaining standard GTK/Glycin resource discovery. Initial runs instead stalled
on unrelated host portal activation, and an empty resource search path exposed
a GTK image-loader failure before mapping. Those failed directories/logs remain
retained. No host package or service configuration was changed; the earlier
private-bus helper processes have exited.

A verification check also caught the fallback replay selecting a default binary
rebuilt by workspace tests. That passing run remains a separate image scope.
Both replays now accept an explicit immutable compositor path; the final
portrait, landscape and fallback reports must match that image's digest.

The final verifier checks all 237 frozen source/configuration files against the
worktree, raw fixture callbacks against both GTK reports, changed generations
with unchanged semantic rows, the fallback helper ledger and all three image
digests. The only source changes since the image freeze were the two replay
scripts accepting an explicit compositor path. Another 44 unchanged build inputs,
including the acceptance client, design assets and compiler configuration, match
base commit `30b605fe5d9a842ef1dd0aa19dddd006c3a315e9` byte for byte.

| Retained identity | SHA-256 |
| --- | --- |
| Compositor used by all three final UI replays | `64cc2bd9efe36788c348dcff8de263ef81cc0fcee69213176a4c261bad7c446a` |
| Transport acceptance client | `096e45e0d4e38e9576b47f1731e0e387fb7f6bb8f95efe1012a996d39eb6f7eb` |
| Sorted, compact JSON source-hash ledger | `778dc3b231dbfb164fba69f7efb5d4ce63b3f3d1f0ad6d45e73a6685a64fc765` |
| Private verified evidence archive, 454 members | `659641b40a19881939de7e5486158b5c847c81bd4213e0eb5e0ea7b727b0f304` |

Every archive member is verified against its retained source/report file;
executables remain separate in the immutable image directory. Raw data and
captures are private and ignored by Git.

## Reproduction and limits

```sh
cargo build --locked --offline --bin tarot --example native-menu-transport
dbus-run-session --config-file=tests/native_menu_session.conf -- \
  python3 tests/native_app_menu_replay.py --output /absolute/fresh-output
```

Pass `--landscape` for the other pose and `--compositor /absolute/saved-image`
when other builds may run concurrently. The fixture needs GTK3 GI/GIO, standard
GTK image loaders, a private X server, software EGL and the replay's existing
input/OCR helpers. Source/binary ledgers, logs, captures and exact callback files
are retained privately under `artifacts/m2/menu-ui/`.

Next add the cooperating Rust/egui producer, phone packaging and native
pointer/touch/rotation/PAM/restart acceptance. Also require adversarial surface
claim/release/replacement and protection cases, GTK modal compatibility, bus
replacement and recovery, held-key mutation, worker churn and transport resource
budgets. Blocking connection setup cancellation is not yet fully bounded; the
compositor retains at most one stopping worker and starts no replacement until
it finishes, keeping UI/card controls responsive. KDE needs its own claim/wire
adapter. No complete M0–M6 milestone is closed.
