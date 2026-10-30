# Mapped native-menu claim and bus lifecycle acceptance

Recorded October 30, 2026, Detroit. This extends acceptance of the existing
revision-47 compositor. No production compositor/session code, package,
dependency, theme or platform policy changes. The full development plan and
unfinished M0–M6 milestones remain open; Gold/Fx0 stays deferred to M7.

## Fixture changes

The cooperating Rust example's opt-in `--qa-stdin` now handles claim mutations
on its Wayland thread. Each acknowledgement follows a real server round trip.
Commands set/clear/release/recreate the example's own GTK claim, name another
exporter, close its bus connection or reconnect while retaining synthetic
callback state. They never activate an action. The bounded command queue keeps
at most sixteen pending strings; the stdin reader remains an explicit trusted
QA facility, not an application IPC interface or general input-resource bound.
`--export-only --qa-stdin` creates a separate exporter without a Wayland window.
Normal sample controls, static exports and in-window fallback remain available.
The sample still is not a stable app SDK.

## Actual-shell host matrix

The replay owns Xvfb, its compositor, both producers and a private D-Bus daemon.
It never sends input to the host desktop or replaces the host's session bus.
Current-Rust portrait and landscape runs and a minimum-Rust producer run pass:

- A legitimate mapped claim supplies visible commands; real rendered-label
  input invokes exactly four `choose("night")` callbacks across the scenario.
- A second exporter with the same UID, login cgroup and executable hash but a
  different process is rejected. A held menu contact cannot activate its
  replacement or the original row. Neither producer receives a foreign action;
  ordinary card controls remain visible.
- Clearing a claim, using a well-known name, naming a nonexistent unique owner,
  releasing/recreating the GTK extension and making a fresh legitimate claim
  produce the expected absent/unavailable/available states.
- Holding Down across an export revision clears the selection. Waiting through
  the hold, releasing and pressing Enter do not activate the retired choice.
  Fresh rendered input works afterward.
- Forty-nine alternating claim/release requests end in a fresh valid claim.
  The held release invokes nothing. Sampled worker/signal-reader counts peak at
  one each in these runs; the replay allows at most two during thread exit.
  Descriptor observations are retained. This finite sample does not prove
  general memory, setup-cancellation, flood or latency bounds.
- Closing the exporter retires commands while its Wayland window remains.
  Reconnecting on the same bus obtains a new unique name and needs a fresh
  surface claim before commands become available.
- Terminating the private bus retires commands and a held pointer choice. The
  replacement daemon has a different GUID. Deliberate connection ordering then
  gives the same process **exactly the original unique name** on the new bus.
  Card View/return starts another worker, which rejects the old claim's bus
  epoch. An explicit new claim restores commands. Comparing only process and
  unique name would fail this case.
- A separate compositor launches the producer through the actual trusted
  input-method socket. Its mapped claim remains absent, its accepted client has
  no ordinary export peer, and card controls work. This checks the trusted
  keyboard connection, not the distinct managed-locker claim path.
- Cooperative close removes the mapped card and claim; both producers exit.

All evidence records loaded executable/source hashes, menu-generation changes,
actual callbacks and exact OCR targets at the existing 80% confidence gate.
Two initial setup failures are preserved: a 40-unit inner crop clips Preferences
and then unmarked Choose before any action occurs. The final helper tries the
12- and 40-unit crops of the same captured pixels, retaining rejected crops.
It never retries an action or lowers recognition confidence. A later complete
pre-churn run also passes and remains a separate scope from the final matrix.

The earlier GTK/egui interaction replay also passes with the expanded producer:
string/boolean/integer targets, independent GIO descriptions/signals, held-row
retirement, disabled choices, owner exit and no-claim in-window controls remain
accepted. All 237 workspace tests pass on Rust 1.98.1 and minimum Rust 1.89.0;
strict host/AArch64 example Clippy, both compiler builds, the AArch64 release
build, formatting and Python syntax checks pass. The cross build does not
execute the native target test suite.

| Saved executable | SHA-256 |
| --- | --- |
| Revision-47 host compositor | `425d6e7e14354a2ca6555246c43ff348d6e79fa4c125c23e4ec79feed2cfc08a` |
| Current-Rust producer | `a4787e837f78e2a9f85d191a432ef80998b76a9a1d6888152e910d15bba4e649` |
| Minimum-Rust producer | `b6805db554ed5e138304bb64207f88b09f7a922f612ecbd2a6dedde319c87715` |
| AArch64 producer | `b863e7270f1245d67aa7fb7085995f665827af6fdf9a510517fa265cc91d5324` |

## Unchanged revision-47 phone

The native fixture launches both Rust processes through one temporary catalog
entry in the ordinary UID-10000 PAM login. Independent `/proc` observations
check executable hashes, process start times and the shared login cgroup.
A root controller supplies only temporary uinput and observations; the shell,
producers, keyboard, locker and bus remain ordinary login processes.

The first native run passes legitimate rendered selection, foreign-exporter
rejection during a held landscape touch, clear/invalid/missing-owner claims,
extension release/reclaim, held-key selection retirement and fresh input.
Exporter loss requires a fresh claim after same-bus reconnection. Release and
reclaim during actual lock protection remain redacted; real PAM restores the
new claim and permits fresh selection. Exactly four synthetic callbacks reach
the legitimate producer and zero reach the foreign producer. Both children and
the wrapper exit; the prior Calculator card is restored. Protected startup in
four orientations, Calculator/audio, complete login teardown and a fresh normal
Tarot login pass. No bus replacement, churn matrix, package reinstall, reboot
or kernel suspend is attempted on the phone.

Independent normal-session audit passes normal/default Tarot, diagnostics off,
hardware keys, services, Panfrost, loaded package payloads and exact preference,
MIME, root policy, platform/system-bus and raw-control restoration. All 282 collected archive members and 203 frozen source/package/fixture inputs
verify locally, including raw callbacks, the distinct producer processes,
protected snapshots and the independent audit's loaded program hashes.
Archive SHA-256:
`c3d2785b193f2856f2925edb74607de0167efd9d95f8ff637c8ce9b86dfd35f5`.
The controller has exited and normal revision 47 remains the default. This is injected native touch/key input, not
owner finger approval or physical keyboard calibration.

## Remaining work

Duplicate-extension/protocol-error and socket/surface destruction variants,
managed-locker exclusion, deeper malformed menus through the shell, setup/auth
cancellation, adversarial floods, real resource/latency bounds and wider
activation/recovery remain open. The bus-replacement result is a private host
case; production phone session-bus crash/recovery still needs separate scoped
acceptance. KDE needs its separate adapter. No GTK global-menu flags are enabled.
The retained Firefox activation failure, current-binary endurance, USB/suspend,
physical/reference/enlarged-text/accessibility review and remaining applications
and services retain their scope.

The sealed host archive has 623 verified members, including the source
snapshots for earlier/final helpers, all reports and retained setup failures.
SHA-256:
`e0438bb0ba992070b20876c5adbef05a4de2a55a28f23b1e4269a7846cfa31b5`.
Saved executables remain separate, with their hashes verified by the reports.

Raw binaries, source ledgers, failed attempts, reports, callbacks and captures
remain private in ignored `artifacts/m2/menu-claims/`. Commands and native
staging are described in [the Rust example](../../RUST_APP_MENUS.md).
