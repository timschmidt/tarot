# Favorite organization by touch

Revision-28 component checkpoint, October 28, 2026. The signed package is installed and its favorite/shell component replay passes, including saved restart. Full native/browser acceptance remains open. This implements the packet’s hold-and-drag Quick Launch organization; named Launcher pages, native icon resolution, motion calibration and the remaining design/plan gates retain separate work.

## Interaction contract

Hold a visible Launcher application or a resting Quick Launch favorite for the shared proposed 420 ms interval. Movement beyond tap slop before that interval remains ordinary scrolling. During a recognized hold the tray appears, an icon follows the pointer, and the proposed order appears without changing saved preferences. Launcher entries remain linked to their desktop identities. The original entry dims while it is held. The lifted icon and its instruction avoid the global status area and each other.

Release over one of the four editable positions to insert or move that identity. Unoccupied positions append to the compact ordered list. Existing favorites can move in a full tray; a new fifth identity is rejected with visible full-tray feedback. Apps remains fixed at the right. A stationary hold cannot create a favorite merely because revealing the tray placed it beneath a low Launcher row.

A held dock favorite also exposes an explicit “Remove from Quick Launch” target. Removing its shortcut preserves the installed application and any running card. Release outside a valid target, over Apps, or after cancellation leaves the order unchanged. The existing named buttons in the favorite editor remain available for adding, moving and removing entries.

The pure Rust gesture captures view, query, favorites, catalog identities/availability and output dimensions. Changes invalidate the gesture. Keyboard hiding is deliberately allowed during the same drag. The UI adapter uses clipped, actually painted widget regions; it processes input once per egui frame and retains the same release intent through additional layout passes. Cancellation retires the held widget press, including synthetic cancel events from the compositor. Native input ownership, protection and authentication remain compositor responsibilities.

## Keyboard and landscape

At the phone’s landscape height, search uses a compact toolbar and 56-unit result row above the existing shell keyboard. A result stays in place through its press; a recognized hold then hides the keyboard and reveals the tray. Ordinary taps still activate available applications or explain unavailable entries. A pending/failed launch can replace the result row with its existing dismissible message. Portrait layout retains the established geometry. Broader small-window, long-label, font-scale and keyboard target-size work remains open.

## Verification and scope

All 161 ordinary Rust tests pass on current Rust and Rust 1.89, strict host/ARM Clippy and minimum-compiler ARM checking pass. Actual-widget tests cover queued letters, Shift and symbol-page changes, cancellation followed by new taps, and one-shot terminal modifiers. Favorite tests cover transactional persistence, full/fixed positions, removal, interruption, early scrolling, stationary low-row holds, extra layout passes and compact search results.

Build the compositor, helpers and examples before private-display acceptance:

```sh
cargo build --workspace --bins --examples
python3 tests/favorite_drag_replay.py --xvfb /absolute/path/to/Xvfb \
  --scale 1 --output /absolute/new/favorites-portrait-1
python3 tests/favorite_drag_replay.py --xvfb /absolute/path/to/Xvfb \
  --scale 2 --output /absolute/new/favorites-portrait-2
python3 tests/favorite_drag_replay.py --xvfb /absolute/path/to/Xvfb \
  --scale 2 --landscape --output /absolute/new/favorites-landscape-2
```

The runner owns its X server, catalog and preferences. It checks actual pointer input, rendered controls, OCR, native click isolation, reversible preview bytes and saved restart. The three favorite suites and terminal/protected-virtual-key/helper-death suites pass on the final host binaries. Python/Pillow/Tesseract/Xvfb are QA dependencies; Node is optional for the report-queue test. None is a Tarot runtime dependency.

The signed revision-28 favorite/shell component run passes all 28 native holds, normal/left-up/bottom-up/right-up/return-to-normal placement, a filtered source above the landscape keyboard, same-identity full-tray reordering, explicit removal preserving the running Calculator, multi-touch/protection/rotation cancellation and actual PAM. Existing Quick Launch, rounded-card, stack-matrix, real editor Cancel/Save and hardware-key replays pass, followed by saved restart. The controller explicitly records `full_native_acceptance = false`: browser acceptance is a separate open gate.

Independent final audits verify normal default Tarot on UID 10000/VT7, exact loaded binaries, diagnostics disabled, hardware keys, original preference bytes/ownership/mode and system policies, absent fixture processes/input/RTC alarm, and a live Panfrost descriptor. Both component sessions request zero suspend cycles. [Exact identities, unmodified captures and retained failures](recordings/r28/README.md) keep this scope distinct from earlier browser/protocol/toolkit/terminal evidence on the same payloads. No new physical-finger calibration, full visual conformance, current-binary endurance or complete M0–M6 milestone is claimed.

## Native keyboard and observer findings

A real-widget regression and native Firefox both reproduce three queued taps for `caf` producing only `f` on revision 27. egui exposes one widget click per frame; the native helper had drained all pending contacts into a single frame. Revision 28 takes input through the first primary-button release, leaves later contacts queued and requests another frame until they are consumed. Later motion cannot move an earlier click’s target. Focus/purpose/protection teardown still clears the queue. The native regression briefly stops only the verified ordinary-user keyboard, queues real touchscreen contacts and always resumes it; revision 28 retains exact `caf`.

Repeated browser checks still produce password-field DOM mismatches. One retained frame shows six masked characters while the last received report says four; another run observes an unexpected value through independent read-only heartbeats with zero reported transport failures. These observations do not establish that the queued-tap fix resolves every input issue. Full browser/IME acceptance remains open, and revision 26 remains the last complete accepted native replay.

QA also exposed out-of-order HTTP snapshots, a small TCP accept backlog, stale toolkit resize reports, search-caret placement inside an existing query and rotation dismissing shell search focus. The corrected observer uses document identities/source sequence/time, persistent HTTP, a larger accept backlog, one report in flight and coalesced read-only heartbeats. It records known synthetic-state indices, lengths and geometry; optional protocol metadata drops string arguments. Eight host checks cover ordering, redaction, real socket bursts and the shared producer queue; the phone passes seven and skips the optional Node-only check. Exact text assertions and native input timing remain intact. Toolkit bounds must follow the triggering action; search replacement requires an empty field and exact new query; landscape search reopens through an actual oriented touch.

All original failed outputs and executed sources remain immutable under ignored `artifacts/m1/favorite-drag-r27/` and `artifacts/m1/favorite-drag-r28/`. Final QA/documentation follows source freezing; all 335 frozen Rust files match the installed package manifest.

The later [focused observer comparison](BROWSER_COMPATIBILITY.md#revision-28-observer-diagnosis) on these same payloads proves that at least one post-PAM discrepancy is stale reporting: the expected text is visible in the retained native frame. A persistent WebSocket observer passes one complete focused browser case, then exposes fresh unchanged text after overview-return input in a run without verbose diagnostics. This preserves the open native-input gate. Sixteen current host observer checks pass; the phone passes thirteen and skips three optional Node checks. These later QA results do not enlarge the earlier passing favorite component run's scope.

The subsequent [revision-31 keyboard checkpoint](KEYBOARD_REACTIVATION.md) verifies two further native delivery corrections and preserves a separate remaining Firefox activation failure. Revision 28’s favorite component acceptance and source identities above remain historical; they do not certify revision 31’s complete native suite.
