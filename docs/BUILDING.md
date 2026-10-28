# Building and inspecting Tarot

## Desktop development

The nested backend needs a Linux graphical session, Rust 1.89 or newer, a linker, and libxkbcommon development files. The native keyboard additionally uses the system Wayland client/Wayland EGL and EGL/GLES runtimes through glutin; it renders through egui_glow. The optional `drm` feature additionally needs GBM/Mesa, libinput, libseat, and libudev development files. The inspected host uses the nested backend; direct-device builds use the isolated phone SDK.

```sh
cargo build --workspace --bins
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

Run an empty shell or launch a client inside its private Wayland session:

```sh
cargo run --bin tarot
cargo run --bin tarot -- -- gtk3-demo
```

The compositor prints its Wayland socket. It supplies that socket to the requested child without replacing the parent desktop's environment. F1 opens overview; F2 opens the installed-app launcher. Normal app keyboard events remain with the focused client.

The present NVIDIA rendering failure is recorded in [implementation status](IMPLEMENTATION_STATUS.md). On the inspected Fedora host, the following opt-in path selects Mesa software EGL without changing system configuration:

```sh
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh -- gtk3-demo
```

This wrapper currently uses Fedora's Mesa GLVND vendor-file location. It is a development workaround, not the intended phone renderer.

The wrapper also creates a private D-Bus session and passes `--publish-session-environment`. This publishes Tarot's display details to that private bus so activated applications and portal helpers can find the correct Wayland server. Use this flag only with a deliberately private bus; it changes that bus's activation environment. A bare `cargo run` does not change it. Full portal support remains pending.

## Native lifecycle regression

The ignored integration test sends actual xdg-shell/shared-memory requests. It checks initial configure, first-buffer mapping, focus preservation while a second window is unbuffered, multiple windows, cooperative close, unmap/remap identity, explicit destruction and abrupt disconnect. It verifies that a visible client's hidden sibling receives responsive callbacks and that a fully hidden client receives paced callbacks. It also requests an unwritable capture followed by a valid capture to check session survival and diagnostic recovery. It requires a running **empty Tarot development session**, rather than a mock server or the parent desktop.

Start an empty session with a private control socket, then run the test in a second terminal:

```sh
mkdir -p /tmp/tarot-control
chmod 700 /tmp/tarot-control
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --exit-after-ms 90000 --control-socket /tmp/tarot-control/lifecycle.sock
# Second terminal, while the session is running:
TAROT_TEST_CONTROL_SOCKET=/tmp/tarot-control/lifecycle.sock \
  cargo test --test wayland_lifecycle -- --ignored --nocapture
target/debug/tarotctl --socket /tmp/tarot-control/lifecycle.sock exit
```

The test resolves Tarot's display from its private control socket; no `WAYLAND_DISPLAY` override is required. `XDG_RUNTIME_DIR` must match the running session. Ordinary `cargo test --workspace` leaves this test ignored and runs the independent model/UI/metadata tests.

The expanded case caches a small SHM texture, commits a larger buffer and then a smaller buffer while hidden, returns to the app, and verifies fresh pixels. `tests/lifecycle_session_replay.py --output /absolute/new/lifecycle-evidence` starts the isolated Mesa session and also rejects GL error diagnostics; debug rendering can consume a GL error before a later capture sees it, so capture success alone is insufficient. Build the compositor first with `cargo build --bin tarot`.

Tarot carries Smithay 0.7.0 as a local Cargo patch with the upstream accumulated-SHM-damage bounds fix. Source, original MIT notice, exact upstream revision and the two-line diff are retained under [third-party attribution](../third-party/README.md#smithay-gles-damage-backport). Do not replace this with a moving upstream branch or ignore GL upload errors. The unpatched release produced an out-of-bounds upload in this regression and on the phone after keyboard/launcher transitions.

## Compositor captures

The capture flag reads Tarot's own composed framebuffer, after a configurable delay. It does not capture another application's desktop session. A failed capture records `capture_error` in development status and keeps the session alive. A new capture clears the previous error. Exiting with an unresolved capture error returns a failure status, so a diagnostic smoke test cannot report success without its requested artifact.

```sh
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --screenshot artifacts/m0/nested-gtk.png --screenshot-after-ms 1200 \
  --exit-after-ms 2200 -- gtk3-demo
```

`artifacts/` is ignored scratch output. Visual inspection is required in addition to a successful process exit. The parent desktop can constrain a requested window, so use the captured dimensions rather than assuming the default 360 by 720 request was granted. Use `--width 720 --height 1440 --scale 2 --fullscreen` for the intended phone-sized nested test.

For interactive development, build both binaries with `cargo build --bins`, then enable the optional owner-only control socket:

```sh
mkdir -p /tmp/tarot-control
chmod 700 /tmp/tarot-control
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --control-socket /tmp/tarot-control/session.sock -- gtk3-demo
# In a second terminal:
target/debug/tarotctl --socket /tmp/tarot-control/session.sock status
target/debug/tarotctl --socket /tmp/tarot-control/session.sock overview
target/debug/tarotctl --socket /tmp/tarot-control/session.sock capture /tmp/tarot-overview.png
target/debug/tarotctl --socket /tmp/tarot-control/session.sock exit
```

The socket requires a private directory owned by the current user and uses mode 0600. Control requests are bounded JSON lines handled by a worker thread; the compositor applies commands on its own event loop. Capture reports a pending request, with completion recorded in the compositor log. This is an opt-in development interface, not a production management or unlocking API.

## Compiler minimum

The workspace declares Rust 1.89 centrally. The locked `uuid` 1.29.0 dependency through zbus rejects Rust 1.88. Actual Rust 1.89 passes all 104 host tests and the complete AArch64/musl DRM workspace/all-target check. Release package builds use the separately recorded Alpine Rust 1.96.1 toolchain. Minimum-version compilation does not substitute for device testing or the ordinary strict Clippy gates.

## AArch64 cross-compilation

The cross-build uses Clang/LLD, Rust's AArch64 musl target and CRT objects, and an isolated sysroot under `target/phone-sysroot`. The sysroot contains the phone's runtime/development libraries and matching musl development files. It does not alter the host's system libraries.

```sh
rustup target add aarch64-unknown-linux-musl
./scripts/sync-phone-sdk.sh user@172.16.42.1
./scripts/build-phone.sh --release
```

The target linker is `scripts/aarch64-linker.sh`; `TAROT_SYSROOT` can override its sysroot path. A sysroot must first be prepared from the selected phone release. The current SDK uses the final 26.06/Alpine 3.24 runtime files and `musl-dev-1.2.6-r3`, resolved through the phone's configured official repositories. It replaced the earlier 25.12 SDK after the final reboot. The build wrapper builds every workspace binary, including `tarot-keyboard`. SDK synchronization now includes the Wayland client, Wayland EGL and libffi runtimes. A release workspace build against a fresh `/tmp/tarot-keyboard-sdk-verify` SDK passed. The installed development source APK now has a separate [Alpine build/session procedure](PACKAGING.md); release signing and distribution remain unfinished.

The SDK script was also verified against a fresh `/tmp` SDK, followed by a successful DRM-enabled build and AArch64 unit-test execution through QEMU. It requires the phone's development packages already installed: `libinput-dev mesa-dev libseat-dev eudev-dev pkgconf`, plus its existing libxkbcommon runtime. It downloads the matching musl development archive using APK's configured repositories and copies system headers/libraries through SSH. `TAROT_SSH_CONTROL` optionally selects an existing authenticated SSH control socket. Use a dedicated SDK directory, and resynchronize after distribution updates. The build wrapper scopes cross-pkg-config settings to its process. It also selects `scripts/aarch64-cc.sh` for target C probes, using the same Clang/musl SDK headers as the linker. This avoids silently treating GBM capabilities as absent because a separately named cross GCC is missing. The tiny probe compiler disables host ccache writes. Standalone cross-test/Clippy commands should set `TAROT_SYSROOT` and `CC_aarch64_unknown_linux_musl` to that wrapper as well.

For the initial development setup, the official musl development package was resolved through the phone's APK repositories. Its archive was extracted with GNU tar's `--ignore-zeros --warning=no-unknown-keyword` options, preserving the package's relative libc symlink. Runtime libraries were copied with dereferencing tar into the isolated sysroot. Linker aliases `libxkbcommon.so` and `libgcc_s.so` point to the corresponding `.so.0` and `.so.1` runtime files. Rust's target `self-contained` directory supplies `crtbeginS.o` and `crtendS.o`.

The output is dynamically linked to `/lib/ld-musl-aarch64.so.1`, libxkbcommon, libgcc_s, and the selected backend's native libraries. It runs on the actual phone. QEMU validation additionally needs the isolated runtime library search path:

```sh
qemu-aarch64-static -L target/phone-sysroot \
  -E LD_LIBRARY_PATH="$PWD/target/phone-sysroot/usr/lib:$PWD/target/phone-sysroot/lib" \
  target/aarch64-unknown-linux-musl/debug/tarot --version
```

## Direct phone experiment

`--drm-device /dev/dri/card1 --scale 2` selects the experimental single-panel backend in a `drm` build. Identify the actual connected display node each boot; card numbering is not an interface guarantee. This backend uses libseat, GBM/EGL, libinput, DRM page flips and vblank completion. It shares the native-client/egui scene with the nested backend. It currently handles one selected output and does not implement hotplug, rotation, a production lock, or a packaged device session.

For the verified phone, `scripts/phone-drm-smoke.sh` runs through an authenticated root terminal after copying the build to `~/tarot-dev/bin` and staging the script on the phone. It stops greetd temporarily, starts a temporary seatd instance, and runs Tarot **as user** on VT8. The default test lasts 45 seconds (`TAROT_DRM_SECONDS` accepts 10–300). A compositor timer, launcher timeout and recovery guard bound the experiment and restore greetd on VT7; SSH remains enabled. The harness checks the compositor's own exit result, not just `openvt`'s status, and removes only its own seatd socket.

The tested phone also has `gtk+3.0-demo grim wtype seatd` installed. Logs, captures and child exit status go to `~/tarot-dev/artifacts`. The script is development tooling tied to this phone's account and OpenRC setup; it is not a production session installer.

## Phone gesture replay

`tests/phone_gesture_replay.py` exercises the actual kernel/uinput → libinput → compositor → egui/native-render path. Stage it in `~/tarot-dev/` and run it through the authenticated root terminal while the direct-session harness owns VT8. It checks the verified phone model, foreground VT, active compositor PID and direct presentation counter before injecting input. It creates a temporary two-contact touchscreen and destroys it on exit; it does not grant the compositor access to `/dev/uinput` or install a persistent service.

In the root terminal, after staging the script and current phone binary:

```sh
TAROT_DRM_SECONDS=180 /home/user/tarot-dev/phone-drm-smoke.sh &
tarot_replay_harness=$!
sleep 4
python3 /home/user/tarot-dev/phone_gesture_replay.py \
  --output /home/user/tarot-dev/artifacts/motion-replay
wait "$tarot_replay_harness"
```

The replay expects the harness's initial GTK demo and installed `foot.desktop`. It captures held overview/browse/dismiss states, checks cancellation and launcher scrolling, browses two independent clients, then closes GTK through the ordinary close protocol while retaining foot. It records status snapshots and PNGs in its output directory. This is a regression tool for a spare device, not a substitute for one-handed physical gesture review, a recording of animation timing, or the five-minute idle/power budgets.

`--reduced-motion` makes settle transitions immediate; direct finger tracking is retained. F1/F2 select overview/launcher, and Left/Right browse cards while overview owns keyboard focus. Quick-launch hold and stack/reorder gestures remain pending.

## Native client coordinate replay

`examples/input-probe.rs` is an independent Rust Wayland client with a fixed oversized buffer, nonzero window-geometry origin, and a child subsurface. It records received protocol events to JSON lines. It is a diagnostic client, not the external egui app required by M2.

```sh
cargo build --bins --example input-probe
./scripts/build-phone.sh --release --example input-probe
```

Stage `target/aarch64-unknown-linux-musl/release/examples/input-probe` as `~/tarot-dev/bin/input-probe`. Stage both `tests/phone_native_input_replay.py` and `tests/phone_gesture_replay.py` in `~/tarot-dev/`. During the same bounded VT8 harness used above, run:

```sh
python3 /home/user/tarot-dev/phone_native_input_replay.py \
  --output /home/user/tarot-dev/artifacts/native-input-after
```

The root harness creates the temporary touchscreen but starts the protocol client under the control socket owner's ordinary account. It verifies root/child coordinates, motion beyond the child's bounds, both root-first and child-first multitouch grabs, cancellation after a two-contact frame has been sent, suppression of held contacts after navigation, and motion during card expansion. Each touch sequence retains its initiating surface, matching the pinned Smithay grab policy; additional contacts use that surface's coordinate space. Logs include the actual client events and shell state. The old compositor was shown to fail the root-drag coordinate check before the correction.

Add `--moving-child` to make the probe move its child during a held contact. The fixture commits the child and parent, then waits for a presented frame before checking the new coordinate origin. The replay also removes its virtual touchscreen with a contact held, checks client cancellation, and verifies a replacement touchscreen can deliver a fresh sequence. This single-touchscreen backend cancels all touch routing when a touch-capable input device disappears; independent simultaneous touch devices remain future work.

This coverage does not certify rotated outputs, viewporter/buffer-scale combinations, relative pointers, toolkit text input or accessibility.

## Phone popup regressions

Stage `tests/phone_popup_replay.py` and `tests/phone_toolkit_popup_replay.py` alongside `phone_gesture_replay.py`, and use the updated `input-probe` binary. Run each under the bounded VT8 harness described above:

```sh
python3 /home/user/tarot-dev/phone_popup_replay.py \
  --output /home/user/tarot-dev/artifacts/popup-after
python3 /home/user/tarot-dev/phone_toolkit_popup_replay.py \
  --output /home/user/tarot-dev/artifacts/gtk4-popup
```

The protocol fixture checks keyboard focus, actual touch targets, nested menus reusing an active parent's serial after the recent-action timeout, child destruction and parent restoration, reverse-order dismissal from the gesture strip, rejection of an unrelated serial, fitted-coordinate edge constraints, and isolation of a client that requests a grab after mapping. A popup remains part of its parent's card. Logs and framebuffer captures accompany status assertions. The deliberate invalid-client step exits the probe unsuccessfully; the replay succeeds only if that failure leaves the compositor and other cards alive.

The toolkit replay opens Calculator 50.0's GTK4 menu, dismisses it by tapping the parent, reopens it and navigates to overview. Its tap coordinates depend on the inspected scale-2 phone layout; captures before opening and on timeout help diagnose layout changes. Run it in its own session, or close its Calculator card before running a replay that assumes only GTK and foot are present.

The recovery harness records the exact compositor PID before exec. Cleanup can terminate that verified unprivileged executable if it blocks beyond the launcher timeout, then releases the temporary seat and restores greetd. An existing control socket prevents another run: inspect the previous process and socket before removing a stale socket left by a forced termination.

## Native text-input experiment

Tarot now bridges text-input-v3 and input-method-v2 through the pinned Smithay implementation. The input-method global is visible only on a private socketpair connection inherited by the explicitly launched helper. Other clients cannot bind it even if they learn its global ID. This is an initial native text bridge, not a finished on-screen keyboard.

Build the synthetic diagnostic client:

```sh
cargo build --bins --example text-probe
./scripts/build-phone.sh --release --example text-probe
mkdir -p /tmp/tarot-text
chmod 700 /tmp/tarot-text
: > /tmp/tarot-text/method.commands
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --control-socket /tmp/tarot-text/control.sock --exit-after-ms 90000 \
  --input-method "$PWD/target/debug/examples/text-probe" \
  --input-method-arg=--role --input-method-arg=method \
  --input-method-arg=--report --input-method-arg=/tmp/tarot-text/method.jsonl \
  --input-method-arg=--commands --input-method-arg=/tmp/tarot-text/method.commands
```

In a second terminal while that session is running:

```sh
python3 tests/text_input_replay.py \
  --socket /tmp/tarot-text/control.sock \
  --probe "$PWD/target/debug/examples/text-probe" \
  --method-report /tmp/tarot-text/method.jsonl \
  --method-commands /tmp/tarot-text/method.commands \
  --output /tmp/tarot-text/replay
target/debug/tarotctl --socket /tmp/tarot-text/control.sock exit
```

The replay sends actual protocol messages and records app-applied text. It checks Japanese preedit, an accented/emoji commit, byte-based surrounding deletion, rejection of stale transactions, disable/enable, overview deactivation, isolation between two client connections, denial of an explicit untrusted input-method bind, and compositor/client survival after the helper exits. The test helper intentionally logs synthetic surrounding text. Use it only with the empty fixture apps and development demos, never personal fields. Production text content is not logged by the compositor bridge.

For the phone, stage `text-probe` in `~/tarot-dev/bin/`, and both text replay scripts alongside `phone_gesture_replay.py` in `~/tarot-dev/`. Start the bounded root harness with `TAROT_TEXT_PROBE=1`; it launches the helper as the ordinary session user. In a separate authenticated root terminal while VT8 is active:

```sh
XDG_RUNTIME_DIR=/run/user/10000 python3 /home/user/tarot-dev/text_input_replay.py \
  --socket /run/user/10000/tarot-drm.sock \
  --probe /home/user/tarot-dev/bin/text-probe \
  --method-report /home/user/tarot-dev/artifacts/text-method.jsonl \
  --method-commands /home/user/tarot-dev/artifacts/text-method.commands \
  --output /home/user/tarot-dev/artifacts/text-input-replay
```

The replay exits its helper, so start a fresh harness for the GTK4 check. The phone now has `gtk4.0-demo` 4.22.4 installed. Launch `gtk4-demo --run=entry_undo` as `user` with the harness's `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY` from `tarotctl status`, under its own `dbus-run-session`. Wait until the `Undo and Redo` card is mapped and active; initial portal startup can take several seconds. Then run:

```sh
python3 /home/user/tarot-dev/phone_gtk_text_replay.py \
  --output /home/user/tarot-dev/artifacts/gtk4-text-replay
```

This uses a temporary touchscreen, the inspected empty GTK entry, the same trusted helper and actual GTK surrounding-text responses. Captures show preedit, `café 🙂` committed, and the unchanged text after navigating away and returning. It does not certify candidate popup geometry, a visible keyboard, app resizing around a keyboard, secure/password mode, shell search entry, Qt/terminal fallback, or international layout coverage. Helpers are monitored/reaped, but automatic replacement after an input-method crash is not implemented.

## Native stack interaction replay

In overview, hold a card for 450 ms to arrange it. Drag onto another card to join/reorder its stack. Keep the finger near an edge to page through the deck, or drag to the rail below the previews to drop before/after a stack as a separate card. A second finger cancels. The selected card's ellipsis menu also offers move, stack and separate actions. Closing one card requests only that native window's close.

Stage `tests/phone_stack_replay.py` alongside `phone_gesture_replay.py`, and stage the current `tarot-egui-probe` binary in `~/tarot-dev/bin`. Run a fresh bounded phone harness with its default GTK3 client and no keyboard option, then execute:

```sh
python3 /home/user/tarot-dev/phone_stack_replay.py \
  --output /home/user/tarot-dev/artifacts/stacks-replay
```

The replay launches two ordinary egui windows and uses real uinput/libinput touches for hold activation without motion, cancellation, edge paging, cross-application grouping, exposed member selection, the menu alternative, within-stack ordering, extraction and standalone reordering. It finally flicks GTK out of the stack, checks the egui sibling and stack identity survive, and closes its own remaining windows. Status snapshots and captures distinguish preview from committed order. Device/PID/VT guards restrict injection to the bounded development session.

## Ordinary egui application fixture

`tarot-egui-probe` is an ordinary Wayland/xdg-shell application using pinned eframe 0.33.3 with Glow and Wayland. It has a primary text/password form and a second independent native window. It uses the shared native-text host adapter for the pinned egui-winit bare-commit issue; password focus explicitly sets the native IME purpose. It is a synthetic compatibility fixture, not Settings or a shipped phone application.

```sh
cargo build --workspace --bins
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --input-method "$PWD/target/debug/tarot-keyboard" -- \
  "$PWD/target/debug/tarot-egui-probe"
```

For a noninteractive multiwindow check, start a separate empty Tarot session with a private control socket, then run:

```sh
python3 tests/egui_lifecycle.py --socket /tmp/tarot-control/session.sock \
  --probe target/debug/tarot-egui-probe --output /tmp/tarot-egui-lifecycle
```

Use a fresh output directory. The fixture creates a mode-0600 JSON-lines file only with explicit `--report`; it records synthetic normal-field text, widget geometry and password character counts. File writes run on a bounded worker channel. The lifecycle replay opens two actual native windows, captures their cards, activates the primary and verifies closing the secondary preserves the process before closing the primary.

On the phone, stage `tarot-egui-probe` in `~/tarot-dev/bin` and `tests/phone_egui_replay.py` alongside `phone_gesture_replay.py`. In a fresh bounded `TAROT_KEYBOARD=1` harness, run:

```sh
python3 /home/user/tarot-dev/phone_egui_replay.py \
  --output /home/user/tarot-dev/artifacts/egui-replay
```

This replay checks touch-focused Unicode entry and deletion, masked synthetic password entry, typing into the second window, retained independent text, overview, cooperative close and isolation after killing a fresh fixture process. It derives taps from the fixture's current widget geometry and Tarot's card placement. `--protocol-log` enables a Wayland trace for this synthetic fixture only; such traces include fixture text. The fixture does not establish clipboard, accessibility, arbitrary dialogs or every external egui application's compatibility.

## Native egui keyboard

`tarot-keyboard` is a separate Rust process using a real restricted layer-shell surface, input-method-v2 and a restricted virtual keyboard for non-text keys. It draws with glutin/EGL/GLES and egui_glow; no CPU-generated key bitmap or compositor-owned imitation window is used. Its phone renderer was confirmed as Mali-T860/Panfrost. All three privileged globals are filtered to the inherited trusted connection. The current layer policy accepts only the `tarot-keyboard` overlay namespace with bottom/left/right anchors and no keyboard interactivity; this is not general layer-shell compatibility.

```sh
cargo build --workspace --bins
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --input-method "$PWD/target/debug/tarot-keyboard" -- gtk4-demo --run=entry_undo
```

The keyboard appears when a native text field enables text-input. Its 240-logical-pixel surface sits above the 28-pixel gesture strip. Tarot configures the app to use the remaining area and clips keyboard drawing to its reserved rectangle. Taps go to the keyboard's surface without moving the application's keyboard focus. Hide dismisses it; the strip's Keyboard button reopens it on the same active field. Letters and symbols use atomic text-input commits; Backspace uses UTF-8 surrounding selection or previous grapheme boundaries, with a restricted virtual key when surrounding context is unavailable. Enter uses a virtual key press/release. Shift, two symbol pages, a numeric layout and a private-entry presentation are implemented. The `#+=` key switches to additional punctuation; `123` returns to the first symbol page and `ABC` returns to letters. Together with Space and letter case, the shared keyboard covers all 95 printable ASCII characters. A pointer-driven private-entry test exercises every character through the actual widgets. The ordinary egui fixture now verifies password-purpose presentation and Backspace without surrounding text; numeric/PIN and broader private-field/toolkit behavior remain unverified.

Launcher search uses the same key view inside Tarot's shell scene. Tap the search field to edit, then type and tap a filtered application tile. Hide or Enter retains the query and restores the results area; tapping search resumes editing. Shell text does not pass through the native input-method process.

For the phone, stage both release binaries in `~/tarot-dev/bin`, the latest `text-probe` fixture there, and `phone_keyboard_replay.py` plus `phone_gesture_replay.py` in `~/tarot-dev/`. Use the bounded harness with `TAROT_KEYBOARD=1` instead of `TAROT_TEXT_PROBE=1`:

```sh
TAROT_KEYBOARD=1 TAROT_DRM_SECONDS=120 /home/user/tarot-dev/phone-drm-smoke.sh &
tarot_keyboard_harness=$!
sleep 4
python3 /home/user/tarot-dev/phone_keyboard_replay.py \
  --output /home/user/tarot-dev/artifacts/keyboard-replay
wait "$tarot_keyboard_harness"
```

The replay checks actual client-received letters, accented text and emoji, deletion, Enter press/release, two-contact cancellation, geometry reservation/restoration, overview/return and same-field hiding/reopening. It also kills the owned keyboard while a key is held, waits for a supervised replacement, verifies the old release cannot type and verifies a fresh key works. It leaves the replacement running until the bounded session ends.

Stage `phone_launcher_keyboard_replay.py` alongside those scripts to exercise shell search in a fresh keyboard-enabled harness. Run it with `--output /home/user/tarot-dev/artifacts/launcher-keyboard`. It types `calc`, checks Unicode deletion and cancellation, hides/reopens search, edits the retained query and launches the actual Calculator tile by touch. It closes only the Calculator card created by that replay.

The configured helper is supervised with delays of 250, 500, 1000, 2000 and 4000 milliseconds. A sixth consecutive failure stops automatic retries; surviving thirty seconds resets the history on the next failure. The strip's Keyboard action can request another attempt. `tarotctl status` reports the helper PID, generation, retry state and error without field contents. A separate inherited nonblocking control socket carries show/hide/quit commands. Ordinary clients receive no input-method capability. A diagnostic helper that intentionally exits will now be restarted too; end its bounded test session after recording that fixture.

For visual GTK4 evidence, use a fresh keyboard-enabled harness and launch the empty `gtk4-demo --run=entry_undo` field as described above. Stage and run `phone_gtk_keyboard_replay.py --output /home/user/tarot-dev/artifacts/gtk4-keyboard-replay`. This script checks focus/geometry and captures typing. Its images were inspected to verify `qé🙂`, `qé` after deletion and `qéw` after overview/return. It does not read GTK text through an accessibility API; the separate raw-client replay supplies machine-checked text evidence.

The keyboard is an initial implementation. Candidate popups, non-text client fallback beyond Enter/Backspace, password/purpose validation, locale/layout selection, long-press variants, key repeat, manual retry after exhaustion and accessibility remain open. No claim of a complete M2 text-input gate is made.

## Native notifications

`run-nested.sh` and `phone-drm-smoke.sh` now pass `--notifications` on their private D-Bus sessions. Bare launches leave the service disabled. Enabling it requires `--publish-session-environment`; use a deliberately private bus. Existing notification-name owners are never replaced or queued behind. The Alerts strip button opens the dashboard; Escape or Alerts closes it.

Build and run the real desktop protocol/failure tests from a graphical session:

```sh
cargo build --workspace --bins --example notification-probe
python3 tests/notification_session_replay.py
```

The runner starts separate private buses for protocol, name collision/release, bus restart, settings restart, unsupported settings, save failure/retry, shutdown while offline, and initial startup before a bus exists. It uses the verified Fedora Mesa software vendor path and checks clean compositor exits. `--output /absolute/new/evidence-directory` retains captures and validation reports at a chosen location. Each case uses fresh owner-only runtime sockets under `/tmp` and its own settings file, avoiding desktop preferences and Unix-socket pathname limits. The synthetic probe writes only fixture content to its explicit trace files. Settings commands are asynchronous; the replay observes the applied/saved snapshot before sending a notification that depends on them.

For the phone, cross-build the example with the compositor:

```sh
./scripts/build-phone.sh --release --offline --example notification-probe
```

Stage `notification-probe` in `~/tarot-dev/bin`, and `notification_replay.py`, `phone_notification_replay.py`, `phone_gesture_replay.py` plus the updated harness together in `~/tarot-dev`. The touch replay also uses the existing `text-probe`, native keyboard and installed `notify-send`. In an authenticated root terminal on the verified spare phone, launch a bounded keyboard-enabled harness and record its actual exit inside the background wrapper:

```sh
(
  TAROT_DRM_SECONDS=180 TAROT_KEYBOARD=1 sh /home/user/tarot-dev/phone-drm-smoke.sh
  tarot_notification_result=$?
  echo "$tarot_notification_result" > /home/user/tarot-dev/artifacts/notification-harness.exit
) > /home/user/tarot-dev/artifacts/notification-harness.log 2>&1 &
# Once the private control socket exists and the GTK card has mapped:
python3 /home/user/tarot-dev/phone_notification_replay.py \
  --output /home/user/tarot-dev/artifacts/notification-ui-new
python3 /home/user/tarot-dev/notification_replay.py \
  --socket /run/user/10000/tarot-drm.sock \
  --probe /home/user/tarot-dev/bin/notification-probe \
  --output /home/user/tarot-dev/artifacts/notification-protocol-new
```

Use fresh evidence directories. These replays require an isolated session and clear its fixture notifications. The touch replay creates a temporary uinput touchscreen, starts native test clients as the socket owner, checks VT8 throughout, and destroys/terminates its fixtures in cleanup. It tests installed libnotify delivery, banner hide/retention, default/dashboard actions, DND/privacy/clear controls, arrival during card hold, banner hold isolation, and real text input before and after dashboard dismissal. The protocol replay checks standard calls, directed signals, sender isolation, replacement/stale actions, expiry, critical/transient/resident policies and 64-entry capacity. Control requests inspect metadata without notification text. The diagnostic action endpoint is owner-only test machinery; user interaction remains the actual touch path.

After the bounded harness ends, inspect both compositor and wrapper exit records, greetd/VT7, remaining fixture processes, temporary sockets and replay input devices. The wrapper exit must be recorded before the shell reaps its job; a later `wait` on an already-reaped job can return 127 and does not establish the harness's exit status.

## Durable notification settings

The initial versioned shell preference schema covers the two notification settings:

```toml
version = 1

[notifications]
dnd = false
hide_previews = false
```

The normal location is `$XDG_CONFIG_HOME/tarot/settings.toml`, falling back to `$HOME/.config/tarot/settings.toml`. Relative XDG paths are ignored. `--config-file /absolute/path/settings.toml` selects another file and requires `--notifications`. The phone harness deliberately uses `~/tarot-dev/artifacts/config/settings.toml`; its settings now survive successive development sessions. Normal notification history remains in memory.

Loads and atomic saves happen on the service worker. Missing files use defaults without creation; a UI change creates the file with mode 0600 and any new directories with 0700. File and parent directory are synced. Invalid/newer-schema, oversized, symlink, foreign-owned or nonregular files detected on startup remain untouched. Settings then apply for the session with an error; repair the file and restart to enable persistence. An ordinary save failure leaves session behavior intact and a later settings change retries. Files are read once at startup, with no live external-edit watcher. Existing directory permissions are retained. The schema currently rejects unknown fields, preserving them on failed startup load rather than silently dropping them. [XDG location rules](https://specifications.freedesktop.org/basedir/0.8/)

Stage `tests/phone_preferences_replay.py` beside the existing phone replay helpers. Run `--phase save --output /home/user/tarot-dev/artifacts/prefs-save-new` in a keyboard-enabled bounded session. It taps both settings on, checks the private versioned TOML and verifies DND suppression. End that session and start a separate harness without deleting the config. Run `--phase restore --output /home/user/tarot-dev/artifacts/prefs-restore-new`; it verifies restored values before any input, checks notification suppression, then taps both settings off and verifies the saved defaults. Keep separate harness/compositor exit records for each session.

The notification worker reconnects to the originally supplied session-bus address with bounded backoff. In notification-enabled sessions, private activation-environment publication also happens on that worker on each connection; a missing initial bus leaves the shell available while recovery continues. The restart test uses a stable private Unix address without a fixed GUID. An address pinned to a dead bus's GUID cannot identify a replacement bus with a different GUID; recovery does not discard that identity constraint. Reconnect requires the configured address to become valid again. The phone's ordinary `dbus-run-session` harness still uses its generated private bus; the actual bus-restart experiment is the nested test, while real-device evidence covers settings persistence and notification/input behavior.

## Power and display controls

`--system-services` enables the UPower/login1 and NetworkManager adapters. The top-right status area opens its modal power/display menu, with a button for Wi-Fi controls. Battery presence, percentage or coarse level, charge state and external power are independent readings. Brightness uses the driver's actual level and maximum; buttons request 5–100 percent through the active local session, show pending/error state, and require readback. A 60% request on the phone's 0–51 range confirms as 31/51, displayed as 61%. Display-off and suspend are separate future controls.

The phone harness enables system services. A bare nested launch leaves them disabled. For real desktop system access, explicitly pass `--system-services`; `--platform-bus ADDRESS --backlight-root DIRECTORY` selects isolated diagnostic services/readback files instead. The adapter only reads those files; brightness writes always go through login1. Multiple backlights currently produce an unavailable state until an output-selection policy is implemented.

Build and run the isolated protocol/failure replay:

```sh
cargo build --workspace --bins --example power-probe
python3 tests/power_replay.py --output /absolute/new/power-evidence
```

It starts a private bus and fake services/backlight, checks initial bus absence and property updates, coarse/invalid levels, delayed/denied writes, readback mismatch, remote/inactive sessions, UPower name release/reacquisition, bus replacement, queued offline action and invalid driver range. It checks that denial leaves the actual level unchanged and that success clears the prior action error. No host system settings are changed.

The ordinary SSH-launched DRM harness inherits a remote session, so login1 correctly refuses its brightness requests. The optional root-only development launcher registers a real temporary local session for the hardware experiment:

```sh
./scripts/build-phone.sh --release --offline --example local-session
# Stage examples/local-session in ~/tarot-dev/bin and the updated harness.
# Stage phone_power_replay.py with phone_gesture_replay.py and notification_replay.py.
```

From an authenticated root terminal on the verified spare phone:

```sh
(
  TAROT_DRM_SECONDS=120 TAROT_KEYBOARD=1 TAROT_LOCAL_SESSION=1 \
    sh /home/user/tarot-dev/phone-drm-smoke.sh
  tarot_power_result=$?
  echo "$tarot_power_result" > /home/user/tarot-dev/artifacts/power-harness.exit
) > /home/user/tarot-dev/artifacts/power-harness.log 2>&1 &
# Once the control socket exists and the initial app is mapped:
python3 /home/user/tarot-dev/phone_power_replay.py \
  --output /home/user/tarot-dev/artifacts/power-ui-new
```

The launcher is restricted to root, PinePhone Pro, VT8 and the unprivileged `su` command path. It moves only its own newly forked, gated child out of the inherited SSH cgroup, then uses `CreateSession`/activation for UID 10000, seat0 and VT8. It does not move the SSH session or change device/Polkit permissions. The actual compositor runs as the normal user and uses the standard session API. This fixture does not replace production greeter/PAM authentication or provide a general privileged application service.

The native replay compares a fresh UPower reading, checks compositor UID, taps both 40/60% buttons, independently reads the real backlight, checks modal keyboard isolation and types again into the same field after dismissal. It restores the original level. The launcher also restores only its pre-launch brightness during experiment cleanup, using the session API when available or root recovery if elogind has already removed the ended session. A missing session on release is accepted; other release errors remain failures. The harness records/checks `local-session.exit` independently of `drm-client.exit` and `drm-launcher.exit`, so a helper error cannot hide behind openvt's known deallocation status.

Keep separate evidence directories and verify helper/compositor/harness exits, original brightness, released local session, greetd/VT7, temporary sockets and input-device cleanup. Brightness preferences, output selection/hotplug, native service-outage tests, full Settings, idle/display-off, inhibitors and suspend remain outstanding.

## Audio controls

`--system-services` also enables playback controls through the current user's PulseAudio-compatible server. Open the top-right status menu, then Audio. Each observed output has volume and mute controls; another output can become the default for new playback. Existing applications can retain their own routes. Volume writes preserve channel balance and accept 0–100%; muting is independent. Device profiles, microphone controls, per-app routing and call-audio policy remain separate work.

The Rust adapter uses `libpulse-sys` 1.23.0 and the platform's libpulse, without spawning `pactl` or an audio daemon. Its worker owns a threaded mainloop, bounded requests, subscriptions, coalesced snapshots and reconnect backoff. No healthy periodic polling is required. Completion needs observed server state after acknowledgement; stale output IDs and values above 100% are rejected. Native hardware volume is reported through the sound server, rather than claimed as acoustic or mixer-driver verification. The session package must arrange the distribution's normal audio service startup.

Native build dependencies include the distribution's libpulse development package (`pulseaudio-libs-devel` on the inspected Fedora host). The phone SDK script copies the existing runtime/common library and extracts a matching `pulseaudio-dev` archive without installing it on the phone. Rust builds remain dynamically linked to the platform client library. [Control API](https://www.freedesktop.org/software/pulseaudio/doxygen/introspect.html), [threaded-mainloop ownership](https://www.freedesktop.org/software/pulseaudio/doxygen/threaded_mainloop.html)

For a private real-server regression, build Tarot and provide an installed or separately extracted PulseAudio daemon. The test creates only synthetic null outputs in a private temporary runtime directory. It covers external updates, balance, zero volume, mute, default selection, hotplug, stale IDs, delayed replies, timeout/recovery, daemon replacement and exit during a pending request. It never connects to the host audio server:

```sh
cargo build --offline --bin tarot
python3 tests/audio_replay.py --output artifacts/m4/audio/nested
```

If the daemon was extracted without installation, pass `--pulse-daemon`, `--pulse-modules` and `--pulse-libraries` with its binary, module and common-library directories. `--audio-server` can target an isolated server explicitly. When `--platform-bus` is set without an audio override, audio targets an unavailable diagnostic endpoint rather than the host server. Diagnostics omit output names/descriptions; captures can contain them and remain private ignored evidence.

The native replay requires a running bounded phone harness with the keyboard and a GTK startup client, plus a running user audio service with exactly one playback output. A normal user `pactl subscribe` client can keep an autospawned development service alive for the experiment; stop that recorded client afterward. Run through the existing authenticated administrative terminal:

```sh
cd /home/user/tarot-dev
python3 phone_audio_replay.py --output artifacts/audio-phone
```

The root QA process runs every sound-server command as UID 10000. The startup client may still be preparing its first buffer when the status menu opens; mapping must preserve that modal. It tests actual UI touches, compares libpulse server state independently with `pactl`, adds/removes only one temporary null sink for default selection, and restores the exact original channel volumes, mute and default. It also checks that the modal cannot type into an underlying native field and restores typing afterward. This replay does not play or record sound and does not establish acoustic speaker, microphone, headset or call-audio behavior. Copy the replay plus the shared `phone_gesture_replay.py` and `notification_replay.py` helpers; keep earlier failures separate from final passing evidence.

## Quick Launch

The local Quick Launch candidate has a separate isolated replay:

```sh
cargo build --offline --locked --workspace --bins --examples
python3 tests/quick_launch_replay.py --output /absolute/new/quick-launch-evidence --xvfb /path/to/Xvfb
```

It supplies its own display, catalog and config file, exercises real native app/keyboard clients and actual favorite editing, then restarts to check persistence. Captures contain synthetic fixture content only. It does not establish phone KMS/PAM results. Favorites use schema version 3; old versions migrate lazily, but revision 9 cannot load the new schema. Keep rollback/schema acceptance separate from the earlier retained-package test. Read the [current checkpoint](IMPLEMENTATION_STATUS.md#quick-launch-development-checkpoint) before treating local results as installed behavior.

The rounded-card GPU replay uses the same private-display boundary and requires Python Pillow, Xvfb and a built `input-probe` example:

```sh
python3 tests/rounded_cards_replay.py --output /absolute/new/rounded-opaque --scale 2 --opaque --xvfb /path/to/Xvfb
python3 tests/rounded_cards_replay.py --output /absolute/new/rounded-alpha --scale 1 --xvfb /path/to/Xvfb
```

It stacks two changing native SHM clients through real input, checks pixels and corner selection in every orientation, and verifies cooperative close, refusal and isolated client exit. `phone_installed_session_replay.py --rounded-cards --cycles 0` adds native KMS/touch checks inside an already authenticated diagnostic session. It requires Pillow on the phone (`py3-pillow`), the two temporary catalog entries and exact audited fixture paths described in `phone_rounded_cards_replay.py`. The fixture must launch through Tarot as the ordinary user in its login cgroup. The native wrapper must remove those entries and preserve original preferences on failure as well as success. This option does not request a kernel-suspend test or establish manual physical-finger acceptance.

The larger matrix and real GTK document-close check use their own private display. Build the current `input-probe` example first. The GTK replay also requires GNOME Text Editor, Tesseract with English data, GLib settings tools and `dbus-daemon`:

```sh
python3 tests/stack_matrix_replay.py --output /absolute/new/stack-matrix --xvfb /path/to/Xvfb
python3 tests/gtk_close_replay.py --output /absolute/new/gtk-close --xvfb /path/to/Xvfb
```

The editor uses private HOME/XDG directories, a private session bus and a synthetic file. It checks document readiness, actual card reactivation, rendered Cancel/Save responses, exact saved bytes and cooperative exit. OCR searches the response-button area so the “Save Changes?” title cannot substitute for the Save button. The native `--stack-matrix` and `--gtk-close` options require the five temporary entries and isolated editor home supplied by the audited native wrapper; they preserve the original Calculator and login cgroup. Phone OCR packages are QA dependencies, not Tarot runtime dependencies.

`capture_motion.py` optionally records actual completed compositor PNGs with timestamps, output/pose metadata and hashes. It never interpolates frames and stops on protected capture denial. Readback/encoding perturbs rendering, so these recordings are visual evidence and must not be used as frame-pacing or energy benchmarks. The raw PNGs and timing sidecars remain the source for any smaller published animation. `python3 scripts/export-motion.py /path/to/motion/gtk-minimize --output /path/to/minimize.gif --package-audit /path/to/package-audit.json` verifies every source image hash before export; its JSON preserves the observations, quantization and explicit endpoint display hold.

Other X11 pointer replays can use `tests/with_xvfb.py --xvfb /path/to/Xvfb --log /absolute/new/server.log -- python3 tests/orientation_replay.py --output /absolute/new/evidence`. This keeps host focus changes out of the replay; owned-PID window discovery works without a window manager and keeps its focus guard.

## Hardware buttons

The installed session enables `--hardware-keys`. Diagnostic launches require a managed locker and either `--idle-preferences` or `--idle-lock-ms`; unlocked lock-probe mode is rejected. Power locks in nested mode and toggles protected panel off/wake on DRM. Held volume repeats after 400 ms, every 150 ms, without catch-up bursts. Seat/focus/geometry interruption cancels held actions until release; changing the default audio output also cancels volume holds. The feedback reports observed values, preserves mute on volume steps and exposes no application/device metadata while locked.

`tests/hardware_replay.py --output /absolute/new/evidence` starts its own Xvfb and PulseAudio processes. It needs those binaries and `pactl`; extracted binaries can be supplied with `--xvfb`, `--pulse-daemon`, `--pulse-modules` and `--pulse-libraries`. Its XTEST connection explicitly targets that private display, so power/media keys do not reach the host desktop. This replay proves nested input/audio behavior, not physical buttons, native panel transitions or authentication.

On the verified spare phone, the installed-session controller's `--hardware-keys` option additionally invokes `phone_hardware_replay.py`. It requires the actual authenticated diagnostic Tarot session and existing shared QA helpers. It compares DRM DPMS/backlight, exercises stale held contacts/keys and off-screen volume, pauses an owned real rejected PAM worker, requires power-off to kill/reap it, and then requires fresh real touch/PAM authentication with the same native app restored. Credentials use the controller's hidden prompt and stay out of artifacts. Original raw audio channel values and mute are restored. Physical GPIO-button acceptance remains a separate manual check.

## Wi-Fi controls

With `--system-services`, open the top-right status menu and tap Wi-Fi. NetworkManager supplies the device state, software/hardware radio availability, permissions, scan results, saved-profile compatibility and global system connectivity. The first implementation supports one Wi-Fi device. It can scan, activate an existing compatible profile, disconnect that Wi-Fi device, and change only `WirelessEnabled`. It leaves USB and the global networking switch alone. It never requests stored secrets or displays them in routine metadata.

Scan completion requires an advanced `LastScan`; activation requires the returned active connection, requested profile and connected AP group. Pending state lasts until confirmation, failure or timeout, rather than ending with the RPC reply. The scan/radio/disconnect deadline is 20 seconds; connection is 45 seconds, extended while a password prompt is active. Authentication-required permissions are distinct from allowed permissions and disable the affected actions. Personal-network passwords and a restricted secret agent now exist; Polkit authorization prompts, hidden networks, enterprise credential prompts, profile editing/forgetting and multiple adapters are unfinished. System connectivity can come from another interface.

Build and run the isolated real D-Bus replay on a desktop Wayland session:

```sh
cargo build --offline --bin tarot --example network-probe
python3 tests/network_replay.py --output /absolute/new/network-evidence
```

The fixture owns NetworkManager's interfaces on a private bus, without host system-network access. It tests initial bus absence, scan acknowledgement versus completion and timeout, permission denial/authentication, profile/security matching, activation confirmation while the old connection remains active, disconnect isolation, credential-needed state, radio/hardware state, disappearing APs, owner replacement, stale tokens and bus recovery. A fixture `GetSecrets` call fails the check. Captures/status distinguish errors from confirmed state.

For the verified spare phone, stage `phone_network_replay.py` beside `phone_gesture_replay.py` and `notification_replay.py`, use the [bounded local-session harness](#power-and-display-controls), and allow 180–300 seconds. A saved, currently connected Wi-Fi profile and USB networking are replay preconditions; the expected saved row must sort first and fit the initial viewport. Run from the authenticated root terminal only after the compositor, mapped GTK card and local-session permissions are ready:

```sh
python3 /home/user/tarot-dev/phone_network_replay.py \
  --output /home/user/tarot-dev/artifacts/network-ui-new
```

The replay injects real libinput touch events while the compositor runs as UID 10000. It scans, disconnects, taps the matching saved profile, checks the exact original profile, toggles Wi-Fi off/on and observes autoconnection to that same profile. It checks USB at each stage, preserves the native view and records separate status/capture evidence. Its failure cleanup may use administrative NetworkManager commands to restore only the original radio/profile; `admin_recovery_used` then records that recovery and is not UI success evidence. The passing phone run required no administrative connection recovery. The current replay expects the original profile to autoconnect after radio enable.

Real network captures reveal nearby SSIDs; keep their evidence directories private and ignored. Routine control metadata omits names, UUIDs, BSSIDs, IP addresses and service paths. Verify the wrapper/compositor/helper exits independently, original network/radio/brightness, stock greetd/VT7, released temporary session and removed processes/sockets/replay device before finishing. The native radio test does not prove hardware-switch operation or airplane-mode policy. Service/bus-failure coverage is isolated, not a forced outage of the phone's system bus.

### Connection passwords

A visible unsaved open/personal network supplies a new-profile action. Open networks show a confirmation; personal networks show masked entry with the shared private touch keyboard. Remember this network requests NetworkManager disk persistence for a private user profile. Clearing it uses a volatile profile, removed after disconnection. Password/key-management compatibility is validated again at dispatch. PSK accepts 8–63 ASCII characters or 64 hex digits; SAE accepts nonempty input up to the current 128-byte entry bound. Pure SAE is implemented but has no native interoperability evidence. The account name currently comes from the effective UID's `/etc/passwd` entry.

The secret agent accepts personal-network requests only from the live NetworkManager owner and only for a user-initiated connection. Missing-password requests open a modal private form; Cancel returns UserCanceled, while service cancellation/loss, expiry and shutdown retire it. Requests without interaction permission or for another profile are rejected without a prompt. Entry expires after 90 seconds. Password values are absent from status/debug output and egui TextEdit history. Tarot's owned entry/control buffers zeroize on drop; this does not cover copies in input libraries, D-Bus, the OS or the Python QA process. Persistent agent-owned credential storage and enterprise/VPN prompts are unsupported.

The separate private-bus replay tests creation options, permission rechecks, masked entry/cancel, wrong senders, noninteractive/wrong-profile requests, stale replies, user/daemon cancellation, owner-loss cleanup and shutdown with a populated pending prompt:

```sh
cargo build --offline --bin tarot --example network-probe
python3 tests/network_credentials_replay.py --output /absolute/new/credential-evidence
```

Stage `phone_network_credentials_replay.py` beside the other phone helpers and run it in a fresh 300-second keyboard/local-session harness. Its QA-only root process reads the spare phone's original personal-network password into memory, never into evidence files or process arguments. It verifies touch letter/deletion and masked rendering, supplies the known password through private development control, and touches the keyboard Enter key to create a separate volatile profile. It then restores the original profile through the shell. A distinct memory-only profile with no stored password forces a real NetworkManager secret-agent request; the same form supplies the password and restores the original connection afterward. Only those test profiles are removed, and administrative failure recovery is recorded separately from UI success. This is not proof that every character of the existing password was entered by touch.

```sh
python3 /home/user/tarot-dev/phone_network_credentials_replay.py \
  --output /home/user/tarot-dev/artifacts/credential-ui-new
```

Keep separate evidence for unsuccessful and passing runs, verify all test profiles are gone, and retain independent replay/helper/compositor/harness exits and stock-session cleanup. Never copy an unmasked password, secret request body or keyfile into the evidence record. The previous saved-network replay remains a separate scan/radio regression.

If an active local session still gets unexpected authorization results, inspect the running authority executable as well as installed packages. This upgraded phone had an old deleted ConsoleKit-backed process despite an installed elogind-backed package; no permission rules needed changing. See the [recorded diagnosis](DEVICE_BASELINE.md#policy-authority-recovery).

### Remembered Wi-Fi profiles across sessions

Stage `phone_network_persistence_replay.py` and `phone_touch_keyboard.py` beside the other replay helpers. On the verified spare phone, start a bounded keyboard/local-session harness, then run from its authenticated root terminal:

```sh
python3 /home/user/tarot-dev/phone_network_persistence_replay.py \
  --phase save --ledger /home/user/tarot-dev/artifacts/persistence-ledger-new.json \
  --output /home/user/tarot-dev/artifacts/persistence-save-new
```

The replay expects one compatible saved profile on the original personal network. It selects a new-profile form on that controlled AP through development control, toggles Remember off/on by touch, enters the entire existing password through real touchscreen key presses and submits with keyboard Enter. It does not use the diagnostic credential-text command. Root QA reads the existing credential into memory only; no password is put in arguments, reports or the ledger. It verifies the new connection, private-user permission, autoconnect behavior, root-owned 0600 keyfile, saved state and an in-memory comparison of the newly written password. GLib handles keyfile escaping; file contents are never copied into evidence.

Save restores the original connection and intentionally retains the one test profile for the next session. The ledger contains only original/test UUIDs and the first compositor PID, with root ownership and mode 0600. End the first harness, verify its independent helper/compositor/launcher/wrapper exits, stock greetd/VT7, absent old process/sockets/replay device, and retained test profile. Start a new bounded harness, then run:

```sh
python3 /home/user/tarot-dev/phone_network_persistence_replay.py \
  --phase restore --ledger /home/user/tarot-dev/artifacts/persistence-ledger-new.json \
  --output /home/user/tarot-dev/artifacts/persistence-restore-new
```

Restore requires a different compositor PID. It verifies the saved private keyfile again, taps the corresponding saved-profile action without entering a password, confirms the exact profile connected, restores the original through its visible action and removes the test profile/keyfile/ledger. Both phases check USB and retain the GTK view. Administrative failure recovery is recorded separately from normal UI success. NetworkManager remains running throughout; this proves persistence across a Tarot session restart, not a phone reboot or daemon restart.

If interrupted after a successful save, run `--phase recover` with that same ledger and a fresh output directory. Recovery validates the root-only UUID ledger, restores the original connection if needed and removes only the recorded test profile. A fresh save refuses an existing ledger. Do not archive the ledger or keyfile with the ordinary captures/status evidence.

The native keyboard regression now also requires `phone_touch_keyboard.py`. It enters all printable ASCII into the synthetic native text client after the existing Unicode/Enter/cancellation/helper-restart checks. Its reports deliberately contain known fixture text, never the real Wi-Fi credential. The host private-entry test and native replay complement the full-touch password test; none establishes additional layouts, full IME editing, accessibility or purpose coverage.

## Session-lock protocol regression

The `--lock-client PROGRAM` launcher grants session-lock permission only to its private inherited Wayland connection and reserves startup protection by default. Repeat `--lock-client-arg=VALUE` for arguments. Without a configured trusted client the interface is hidden. Synthetic protocol replays explicitly pass `--lock-probe-unlocked-start` to retain an unlocked initial state and manual recovery; the phone harness does this only for `TAROT_LOCK_PROBE=1`. The current `lock-probe` is an explicitly controlled synthetic fixture with no authentication; use it only in an isolated development session. It is separate from the actual Rust/egui/PAM client described below and supplies no normal Lock button.

For the Mesa desktop test:

```sh
cargo build --workspace --bins --examples --offline
python3 tests/lock_replay.py --output /absolute/new/lock-evidence
python3 tests/lifecycle_session_replay.py --output /absolute/new/lifecycle-evidence
python3 tests/network_credentials_replay.py --output /absolute/new/credential-evidence
```

The lock replay starts its own nested compositor, private locker and ordinary app. It checks hidden/global-ID bind denial, full opaque black/synthetic-color/fallback audits, redacted diagnostics, normal command and capture denial, a scheduled pre-lock capture, cached clipboard offers, destroyed role, invalid second unlock, crash and explicit trusted recovery. The nested acknowledgement follows EGL submission; physical output presentation is a separate native test. Ordinary lifecycle and credential replays cover the unprotected behavior after the policy changes.

`--verify-lock-blank` enables the optional QA framebuffer audit; `--verify-lock-surface-rgba=336699ff` also checks the synthetic surface. Only results/generations are exposed, never protected pixels. These flags are off by default. They are not production lock performance measurements. While protected, control status contains only `ok`, `pid` and `lock`; ordinary commands/captures fail. `restart_locker` is accepted only for an orphaned protected session and launches the same configured trusted program. It cannot unlock. Developer Exit remains available for bounded-session recovery.

Cross-build the fixture using the same SDK configuration as the phone build:

```sh
TAROT_SYSROOT=/absolute/phone-sdk ./scripts/build-phone.sh \
  --release --offline --example lock-probe
# Restore the canonical workspace-bin feature set after an example/test build:
TAROT_SYSROOT=/absolute/phone-sdk ./scripts/build-phone.sh --release --offline
```

Stage the canonical compositor, keyboard, local-session helper and `examples/lock-probe` in `~/tarot-dev/bin`, plus the current harness and `tests/phone_lock_replay.py`, `tests/lock_replay.py`, `tests/phone_gesture_replay.py` and `tests/notification_replay.py` in `~/tarot-dev`. The existing local-session/hardware prerequisites still apply. In an authenticated root terminal on the verified spare phone:

```sh
cd /home/user/tarot-dev
TAROT_DRM_SECONDS=240 TAROT_LOCAL_SESSION=1 TAROT_KEYBOARD=1 TAROT_LOCK_PROBE=1 \
  sh ./phone-drm-smoke.sh > artifacts/lock-harness.log 2>&1 &
tarot_lock_harness=$!
# Once the control socket exists and the initial GTK card is mapped:
python3 ./phone_lock_replay.py --output artifacts/lock-phone-new
wait "$tarot_lock_harness"
```

The replay closes the initial GTK card and uses its own unprivileged SHM client. Temporary root-created uinput devices check held keys/touches on both entry and exit, fresh locker input and suppression of normal F1/F2 navigation. It checks a new KMS presentation after VT reacquisition, idle presentation count, clipboard/capture denial, role destruction, invalid unlock and trusted recovery after owner death. A normal app lifecycle and keyboard-helper replacement must preserve the locker contact. The harness automatically ends and restores stock greetd/VT7; retain its compositor/local-session/launcher statuses as well as replay output. Confirm no remaining test clients, sockets or input devices, and original services/network/brightness before further testing. The synthetic fixture does not authenticate. The actual client below adds PAM. The normal mode now supplies protected compositor startup, reusable locking and bounded recovery; idle/suspend and authenticated session packaging remain unfinished.

## Authenticated locker experiment

`crates/tarot-lock` is now an actual Rust/egui session-lock client with a private embedded keyboard and same-user Linux-PAM authentication. `crates/tarot-wayland-egui` shares the existing EGL/GLES host with the native keyboard. Build both compositor and helpers:

```sh
cargo build --workspace --bins --examples --offline
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
python3 tests/locker_session_replay.py --output /absolute/new/locker-evidence
```

The desktop replay exercises the real GL lock cover, restricted status/commands, opaque crash fallback and trusted replacement. It does not attempt to authenticate the host account. The separate explicit integration test creates private temporary PAM configurations using the host's real permit/deny modules, checking both authentication and account rejection:

```sh
cargo test -p tarot-lock --offline auth::tests::actual_pam -- --ignored --nocapture
```

That test needs Linux-PAM and audit-socket access. In a restricted network sandbox the audit netlink socket is denied and PAM reports system error 4; run the integration test outside that sandbox. Ordinary unit tests cover private UTF-8 erasure, repeated-hidden-prompt rejection, worker timeout/reaping and an actual Unlock click while pending/unacknowledged/in cooldown. They do not require a working host PAM service.

To inspect a real nested lock cover with the same Mesa environment as the other desktop examples:

```sh
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh \
  --lock-client "$PWD/target/debug/tarot-lock"
```

With a locker configured, the compositor reserves protection before backend initialization, then starts the client. The system menu supplies a reusable Lock action that starts a fresh trusted client, and unexpected exit/connection/role loss triggers bounded replacement. Startup/no-buffer readiness has a five-second deadline, with presentation waits deferred across a paused VT. These guarantees do not yet cover an installed greeter/session-supervisor failure path or idle/suspend policy. Root/set-ID graphics/worker execution is refused. Authentication resolves the current real UID and fixed PAM service `tarot-lock`; password and service/user overrides are not accepted on argv. Each attempt uses a bounded private stdin pipe, a 15-second child deadline/process group and parent-death kill. Failures clear entry and apply a per-instance 2/4/8/16/30-second cooldown. Account restrictions and requests to change an expired password retain protection. Extra hidden-factor prompts are unsupported and fail closed.

`packaging/pam/tarot-lock` targets Nura's `base-auth`/`base-account` stacks. The verified phone has the dedicated file installed at `/etc/pam.d/tarot-lock`, root:root 0644. It opens no additional PAM session and changes no passwords, greeter policy or database. Other distributions need a corresponding service configuration. Cross-build with the existing SDK; no PAM development archive is needed because the worker loads the installed runtime SONAME:

```sh
TAROT_SYSROOT=/absolute/phone-sdk ./scripts/build-phone.sh --release --offline
```

Stage canonical `tarot`, `tarot-lock`, `tarot-keyboard`, existing `local-session` and the updated harness. Stage `phone_locker_replay.py`, `phone_lock_replay.py`, `phone_touch_keyboard.py`, `phone_gesture_replay.py`, `lock_replay.py` and `notification_replay.py` together. The existing hardware/local-session prerequisites apply. In an authenticated root terminal on the authorized spare phone, with no current Tarot test session:

```sh
cd /home/user/tarot-dev
python3 ./phone_locker_replay.py --output artifacts/locker-phone-new
```

The script prompts privately before starting its bounded 180-second harness. It verifies real unprivileged wrong/correct PAM results, protected KMS presentation, kills the locker during an actual in-flight attempt and checks that its worker dies, checks automatic trusted replacement, rejects a wrong physical-keyboard entry, checks command/capture restrictions, enters the correct password by touch then Cancels, re-enters it and authenticates, confirms normal capture afterward, then taps the actual system-menu Lock button and authenticates again with the same cards/view restored. Credentials never enter its files/argv/environment/JSON. Python/PAM/kernel copies are outside a complete zeroization guarantee. Its evidence directory is owned by the compositor's user so the post-unlock capture is writable. Inspect replay/harness/compositor/local-session/launcher results and stock-session cleanup separately.

The optional `TAROT_LOCKER=1` harness mode selects the real locker and is mutually exclusive with `TAROT_LOCK_PROBE=1`. It explicitly requests QA-only captures of the lock client's own scene into a new private `artifacts/locker-ui` directory. Archive that directory before another run. The actual client has no capture by default; `--capture-dir DIRECTORY` must be supplied explicitly through its launch arguments and requires an existing directory. It writes each own-scene state once per process with mode 0600. It cannot read other apps or the compositor scene, and ordinary compositor captures remain denied while protected.

The final phone test and a separate normal-session notification/power/native-typing regression passed. The normal regression's openvt returned its known VT-deallocation status 8 while compositor, local-session helper and harness returned 0; the harness released seatd, restored VT7 and the following authentication run successfully reused VT8. The final authentication launcher also reported the recognized deallocation status 8 while its compositor, local-session helper and harness returned 0; explicit post-run deallocation initially remained busy despite no in-use reference bit/user descriptor; clearing kernel console selection allowed deallocation to return 0. Preserve these distinct results rather than normalizing the raw launcher code. [Evidence and remaining integration gates](IMPLEMENTATION_STATUS.md#authenticated-rustegui-locker)

## Phone access and provisioning

The current development phone is `user@172.16.42.1` over USB. Authenticate normally; no password is stored in the repository. The owner has approved OS/build/runtime package installation, persistent SSH startup, and reboots on this spare device.

See [device baseline](DEVICE_BASELINE.md) for observed storage and OS details. Upgrades run inside a root-owned tmux session named `tarot-upgrade`, retaining distribution-tool logs under `/var/lib/postmarketos-release-upgrade/`. Additional technical recovery backups and the resumed 25.12 upgrade log are under `/var/lib/tarot-provision/`.

Do not apply the APK script-archive repair indiscriminately. It addresses a reproduced legacy-checksum pathname incompatibility; it writes a separate output archive, and the original installed archive was backed up before replacement. The full procedure and evidence are recorded in implementation status. No SPI or raw-storage flashing has been performed.

## Protected startup and reusable lock regression

Run the complete isolated host policy replay against current workspace binaries:

```sh
cargo build --workspace --bins --examples --offline
python3 tests/lock_policy_replay.py --output /absolute/new/lock-policy-evidence
python3 tests/lock_replay.py --output /absolute/new/protocol-evidence
python3 tests/locker_session_replay.py --output /absolute/new/real-locker-evidence
```

The policy replay checks an initially silent trusted fixture while startup is already opaque/redacted, fresh clients across repeated Lock requests, a stale five-second timer, role destruction, crash replacement, six-failure exhaustion from both process exits and missing executable, an unresponsive process, and an acknowledged owner that never supplies a buffer. Synthetic requests do not authenticate. The real-client replay separately renders the Rust/egui cover and checks automatic crash replacement. Both use nested EGL submission; the native replay proves the KMS startup/reauthentication path.

The updated `phone_locker_replay.py` described above also asserts that the first actual KMS frame was protected and that zero ordinary frames were presented before initial authentication. It kills the locker during a PAM attempt, observes automatic replacement and a presented opaque orphan frame, tests rejection/Cancel/authentication, then taps the real menu Lock button and authenticates in a new process. Repeated lock requests while protected must not replace the current owner. It restores the same app deck/view with panels closed.

On an explicitly enabled private control socket, operator commands are:

```sh
tarotctl --socket /private/runtime/control.sock lock
tarotctl --socket /private/runtime/control.sock status
tarotctl --socket /private/runtime/control.sock restart-locker
```

`lock` reserves protection and requests a client; its reply is not a presentation acknowledgement. Wait for the lock phase/protected-presentation evidence before implementing suspend. `restart-locker` is permitted only for orphaned protection, including after the automatic budget stops. Neither command can authenticate/unlock. The normal mode is protected; `--lock-probe-unlocked-start` intentionally opts a diagnostic fixture out of that startup/recovery policy. Production session packaging must omit that diagnostic option.

### Protected recovery after retry exhaustion

After six consecutive failed starts, the normal managed mode displays a separate opaque recovery view. Try again, Enter, keypad Enter or Space resets the budget and starts the same configured trusted program. The action never unlocks. Input becomes active after that generation's recovery presentation; held contacts/keys are suppressed and canceled/multitouch/device-loss contacts cannot click. Intermediate attempts remain black. Normal control captures, navigation, app metadata and cached clipboard access remain blocked.

The host integration uses the Rust `recovery-client` QA example to deny startup until a marker exists, then exec the synthetic protocol client with the private connection preserved:

```sh
cargo build --workspace --bins --examples --offline
python3 tests/recovery_replay.py --output /absolute/new/recovery-evidence
```

It covers isolated recovery drawing/capture, privacy, operator replacement, owning unlock and app/clipboard restoration. A second case makes the own-view diagnostic capture unwritable and requires protection/retry to remain active. Physical retry input and actual PAM are separate native checks. `--recovery-screenshot PATH` writes only the dedicated renderer's offscreen texture to a new mode-0600 PNG, once. It requires a configured locker. This optional QA flag does not enable normal protected screenshot access. `--verify-lock-blank` additionally audits the final recovery frame's opacity/text presence, with result flags only; neither diagnostic is a production performance measure.

For the authorized spare phone, cross-build `recovery-client` with the same SDK/linker environment used for the other examples, then rerun the canonical `scripts/build-phone.sh --release --offline` workspace build. Example feature unification can rebuild the compositor with different features; stage the canonical compositor only after that final build. Stage `bin/recovery-client`, the unchanged actual `bin/tarot-lock`, updated `phone-drm-smoke.sh`, and these Python helpers together: `phone_recovery_replay`, `phone_power_replay`, `phone_touch_keyboard`, `phone_lock_replay`, `phone_gesture_replay`, `notification_replay`, `lock_replay`. The existing `text-probe` and `lock-probe` fixtures are also required.

With stock greetd restored and no development socket, archive the previous `artifacts/locker-ui` directory and any `artifacts/recovery-own.png` first. The replay refuses existing recovery markers/captures, obtains the account password only through its administrator terminal's hidden prompt, and starts its own bounded 180-second local-session/keyboard harness:

```sh
python3 phone_recovery_replay.py --output /home/user/tarot-dev/artifacts/new-recovery-run
```

Run this script in the authenticated root terminal for temporary uinput devices; compositor/app/locker run as UID 10000. It authenticates startup, maps a known native app/clipboard, exhausts the locker, checks held and canceled input/device removal, retries via actual Enter and touch, replaces the client and requires fresh real PAM authentication. It then verifies original app/view/clipboard and ordinary power/modal/text-keyboard behavior. The password is never put in arguments, environment or evidence; the Python fixture makes no comprehensive memory-zeroization guarantee. This is development QA, not a production launcher or alternate authenticator.

`TAROT_LOCK_RECOVERY=1` is the harness's dedicated QA mode, mutually exclusive with `TAROT_LOCKER=1` and `TAROT_LOCK_PROBE=1`. It selects `recovery-client` with the fixed marker `artifacts/recovery-allow` and actual PAM locker as its eventual exec target. Do not install this marker-controlled gate as a production session. Preserve separate raw compositor/helper/launcher/harness exits and verify stock VT7, absent VT8 allocation, services, network, brightness, processes, sockets and input-device cleanup. [Current evidence and limitations](IMPLEMENTATION_STATUS.md#protected-recovery-after-exhausted-retries)

### Sleep preparation and resume presentation

`--sleep-coordination` enables the login1/elogind delay-inhibitor boundary. It requires `--system-services` and a normal configured `--lock-client`; it conflicts with the synthetic unlocked-start mode. Real-manager operation requires the direct DRM backend. The only nested exception is an explicit `--platform-bus` whose manager is a same-user non-root fixture. The worker checks its bus UID before calling Inhibit, so nested EGL submission cannot release a real root manager's inhibitor.

The worker holds a `sleep`/`delay` FD, listens to the unique manager's `PrepareForSleep`, and reads the initial preparation property before acquiring an inhibitor. Preparation reserves protection and requests a new frame. The matching queued ticket/generation must complete before closing the FD. Resume requires another fresh protected frame and a new next-cycle inhibitor. Initializing, unavailable, preparing or re-arming coordination blocks unlock. Elogind can still reject an inhibitor with `org.freedesktop.login1.OperationInProgress` immediately after its resume signal: Tarot retries that specific response in the same connection for three seconds, with 25–400 ms backoff. Other failures and an expired retry window reconnect with the session protected. An already-preparing manager is observed without requesting an impossible initial inhibitor. No renderer/UI callback performs a bus call. A settled connection waits for events without a periodic polling timer.

Build the workspace examples and run the private-bus regression:

```sh
cargo build --workspace --bins --examples --offline
python3 tests/sleep_replay.py --output /absolute/new/sleep-evidence
```

The `sleep-probe` Rust fixture implements only a private login1 manager and never calls Suspend. It passes actual Unix FDs and reports their EOF, emits authoritative/foreign signals, rejects inhibitors during preparation, and allows a bounded post-resume busy period, denial and name replacement. The test covers held FD lifetime while the compositor is stopped, protected submission, preparation/re-arm unlock refusal, same-epoch transient retry, retry-deadline protection/recovery, duplicate signals, name/bus loss and replacement, initial preparation discovered without its signal, and original app/clipboard restoration. This is nested EGL evidence, not a native sleep cycle.

On the verified spare PinePhone Pro, stage `phone_sleep_replay.py` with the existing phone helpers and updated harness. Archive the previous `artifacts/locker-ui` directory. In the authenticated root terminal:

```sh
python3 phone_sleep_replay.py --output /home/user/tarot-dev/artifacts/new-sleep-barrier-run
```

It starts its own bounded 180-second local-session/keyboard/actual-locker harness with `TAROT_SLEEP_COORDINATION=1`. The account password is read only through getpass, never arguments/environment/files. The actual compositor/locker/PAM worker run as UID 10000. The script verifies the real daemon's inhibitor tuple, authenticates startup, pauses VT8, and uses elogind's privileged `SendPrepareForSleep` helper to emit a true signal. With no new KMS completion possible, the FD must remain listed. Returning to VT8 must complete a protected frame before releasing it. A false signal while paused checks the distinct resume barrier and next-cycle FD, then fresh PAM authentication and original card/view/capture restoration. Cleanup sends false, destroys the replay device, exits the harness, and checks that Tarot's inhibitor disappeared. A detached bounded false-signal guard covers interruption of the administrator terminal.

The pinned elogind helper emits signals without executing kernel sleep, but other system subscribers receive them. Always retain cleanup evidence for services, network, VT, brightness, processes and inhibitors. This test **does not count as a kernel suspend/resume cycle**. The phone's five-second delay maximum remains unchanged: the daemon may proceed after that bound even if a client still holds a delay FD. Production supervision, actual wake sources/reliability, idle/display-off and application/call inhibitor policy remain unfinished. [Pinned helper implementation](https://github.com/elogind/elogind/blob/v255.24/src/login/logind-dbus.c), [current evidence](IMPLEMENTATION_STATUS.md#sleep-preparation-and-resume-presentation)

### Actual RTC-woken kernel suspend

These additional QA tools are restricted to the verified spare PinePhone Pro. `phone_stock_login.py` authenticates the existing user through normal local greetd IPC/PAM, ends only the verified greeter compositor, and waits for actual UID-10000 Phoc and Phosh on VT7. It changes no greeter policy. A running Phrog greeter is a recovery login screen, not an authenticated Phosh session. Stage the Python tools with their existing replay dependencies and build/stage the root-only Rust observer with the phone SDK:

```sh
TAROT_SYSROOT=/absolute/sdk \
CC_aarch64_unknown_linux_musl="$PWD/scripts/aarch64-cc.sh" \
PKG_CONFIG_ALLOW_CROSS=1 PKG_CONFIG_SYSROOT_DIR=/absolute/sdk \
PKG_CONFIG_LIBDIR=/absolute/sdk/usr/lib/pkgconfig:/absolute/sdk/usr/share/pkgconfig \
cargo build --target aarch64-unknown-linux-musl --features drm --release --offline --example suspend-request
```

Run the canonical workspace phone build last before staging the compositor, because example feature unification can change its artifact. In the authenticated root phone terminal, with no existing RTC alarm or Tarot session:

```sh
python3 phone_stock_login.py --output artifacts/new-stock-login
python3 phone_suspend_probe.py --phase alarm --seconds 6 --output artifacts/new-awake-alarm
python3 phone_suspend_probe.py --phase stock --seconds 15 --alarm-evidence artifacts/new-awake-alarm/status.json --output artifacts/new-stock-suspend
```

The awake alarm must pass first on the same boot. The stock test requires actual user Phosh, calls the normal login1 `Suspend(false)` method, and verifies RTC wake, unchanged boot/session process identity, kernel success/failure counters and at least three seconds of suspended time. It does not force a kernel sleep mode. Preserve its private evidence before starting Tarot. Use a detached local controller for USB loss across suspend; inspect its terminal status rather than assuming an interrupted SSH command failed or succeeded.

Archive previous `artifacts/locker-ui` captures before the next harness. Stage current `phone-drm-smoke.sh`, `phone_suspend_replay.py`, `phone_suspend_probe.py`, `phone_suspend_cleanup.py` and the usual keyboard/lock replay dependencies. The existing original-network baseline at `artifacts/persistence-restore-final/status.json` is required. Then:

```sh
python3 phone_suspend_replay.py --output artifacts/new-suspend-twenty --stock-evidence artifacts/new-stock-suspend/status.json --cycles 20 --seconds 15 --detach
# After its controller has exited and status.json reports success:
python3 phone_suspend_cleanup.py --evidence artifacts/new-suspend-twenty/status.json --output artifacts/new-suspend-cleanup.json
```

The password is read at a hidden prompt before detaching, kept only in test-process memory and entered through uinput/libinput into the actual PAM locker. The independent Rust observer holds an additional real delay FD until a matching protected Tarot KMS completion is seen within two seconds of `CLOCK_BOOTTIME`, then releases it so normal elogind suspend can proceed. Every cycle requires kernel success without failures, at least three seconds suspended, protected resume/re-arm in the same manager epoch, fresh PAM unlock, the original app/view/clipboard, new native input, a completed normal capture, and original Wi-Fi profile/USB restoration. After authentication the script requires an ordinary KMS completion and normal capture before pressing the visible app; it waits for client down acknowledgement before releasing and requires the matching up, with no contact retry. Held input is additionally tested across the first cycle. Only this run's RTC alarm is cleared.

`TAROT_SUSPEND_RELIABILITY=1` explicitly extends the harness's maximum from 300 to 900 active seconds and requires sleep coordination, the actual locker and a local session; ordinary harness runs keep their existing bound. The replay explicitly sets the synthetic app's `--exit-after-seconds` to the harness duration too; its ordinary 180-second default is insufficient for this regression. The detached replay and stock-session login are QA tools, not production session management. Suspended duration is `CLOCK_BOOTTIME` elapsed minus `CLOCK_MONOTONIC` elapsed. Preserve failed attempts separately; do not add partial attempts together to claim twenty consecutive passes. Automated touch evidence exercises the real libinput/compositor/PAM path through a temporary uinput device; manual touch-sensor, physical power-key and carrier wake remain separate checks. [Linux clock semantics](https://docs.kernel.org/core-api/timekeeping.html)

The harness restores greetd/Phrog on VT7. If the final desired state is the authenticated desktop, run `phone_stock_login.py` again and retain its result. `phone_stock_check.py --output artifacts/new-stock-output` then verifies the same authenticated shell identity, sends fresh uinput Shift and obtains a 720×1440 stock capture for visual inspection. It changes no authentication policy and does not itself unlock Phosh. Independent cleanup preserves raw launcher status, checks absent VT8 allocation/resources/inhibitors/alarm, and verifies stock services/network/brightness. For an interrupted test, `--allow-failed-evidence` permits inspecting cleanup while explicitly retaining the failed native validation.


### Idle locking and panel power

`--idle-lock-ms N` enables automatic locking after monotonic seat inactivity. It requires a configured normal managed locker and conflicts with the synthetic unlocked-start option. `--idle-off-ms M` additionally powers off the direct DRM panel after inactivity; `M` must be at least `N`. It is rejected on the nested backend. Both flags are opt-in, accept 100–86,400,000 milliseconds and preserve requested brightness. Final persisted preferences/defaults remain pending.

A newly queued protected frame must complete before power-off. Wake input is consumed, its held releases remain suppressed, and another protected frame must complete before input/unlock resumes. A valid owning authenticated unlock remains necessary. The `lock.idle` diagnostic distinguishes policy phase, observed output-off flag and off/wake counts. Inhibitor counts are available only while ordinary status is permitted; protected status returns null for that metadata. Control requests and app commits do not reset inactivity.

Build and run the real Wayland protocol test:

```sh
cargo build --workspace --bins --examples --offline
python3 tests/idle_replay.py --output /absolute/new/idle-evidence
```

It tests native surface-scoped inhibitors, duplicate reference counts, release grace, hidden/restored/unmapped/dead surfaces, an opaque child covering its parent, automatic protected locking and valid owning unlock. A second compositor with automatic idle disabled verifies dead-handle cleanup and continued unlocked behavior. Nested evidence covers the lock/policy protocol, not panel DPMS or physical wake input.

For the verified phone, stage the updated compositor and `lock-probe` fixture, current `phone-drm-smoke.sh`, `phone_idle_replay.py` and its existing Python dependencies (including `phone_sleep_replay.py`). Archive old `artifacts/locker-ui` captures. In the authenticated root terminal:

```sh
python3 phone_idle_replay.py --output /home/user/tarot-dev/artifacts/new-idle-panel-run
```

The script starts its own bounded 180-second local-session/keyboard/PAM/sleep harness. `TAROT_IDLE_PANEL=1` selects six-second lock and ten-second panel-off intervals and requires `TAROT_LOCKER=1`. These are QA values. The compositor, clients and PAM worker stay UID 10000; root creates temporary input devices and calls only elogind's signal helper. Password entry uses getpass and private touch entry, never files/arguments/environment. The test checks actual DPMS and backlight-power readback with requested brightness preserved, presentation counts while off, held-release isolation, touch/Enter wake consumption, device removal, fresh real PAM after wakes, original app/view/clipboard and interaction with the real sleep coordinator. It emits one signal-only prepare/resume pair with bounded detached false-signal recovery; no kernel suspend occurs.

Retain separate raw compositor/helper/launcher/harness exits and independently check stock VT7/console deallocation, services, original network/radio/brightness, no test clients/sockets/input devices/inhibitors and no graphics errors. Physical mouse, persistent idle preferences, power/proximity keys, application/audio/call inhibitors, real suspend/wake and long-run power gates remain unverified or unfinished. [Current evidence](IMPLEMENTATION_STATUS.md#idle-locking-and-panel-power)

## Output rotation and sensors

`--orientation normal|left-up|bottom-up|right-up` sets the initial panel orientation in either backend. An explicitly enabled control socket accepts `{"command":"rotate","orientation":"left-up"}`. Status exposes the requested/current orientation and geometry/presentation revisions, including while protected. Ordinary app/capture diagnostics remain redacted while locked.

With `--system-services`, the shell reads the standard SensorProxy service on the system bus. The Power & display menu supplies a persistent rotation lock through the shared preferences worker. Missing/undefined sensors retain the current orientation. Sensor work stays on its own thread, uses property signals and releases the claim while rotation is locked, the panel is off, the seat is inactive or sleep is preparing. Revision 8 verifies persistence and protected landscape restart; revision 5 separately observes accepted panel-off claim release/reclaim. Broader physical wake checks remain open. Native rotation/touch/keyboard and physical-sensor observations are recorded in [implementation status](IMPLEMENTATION_STATUS.md#rotation-and-orientation-checkpoint).

Build the workspace binaries plus `input-probe`, `text-probe`, `lock-probe`, `sensor-probe` and `sleep-probe` examples, then run the explicit host replays:

```sh
cargo build --workspace --bins --example input-probe --example text-probe --example lock-probe --example sensor-probe --example sleep-probe
python3 tests/orientation_replay.py --output artifacts/m3/orientation/nested-new
python3 tests/sensor_replay.py --output artifacts/m3/orientation/sensors-new
```

The orientation replay requires the host's X11/XTEST connection and software Mesa path. It identifies only its own nested window by child PID, raises that window's frame, and restores prior focus/pointer position if the test still owns focus. Run it without another test opening overlapping windows. It checks actual client coordinates/pixels and keyboard text; its locker is a synthetic protocol fixture and never authenticates. The sensor replay uses private D-Bus fixtures, never hardware services or kernel suspend. Preserve failures separately from passing runs.

`tests/phone_orientation_replay.py` is root-only spare-phone QA for an installed diagnostic session. It creates two temporary catalog entries, authenticates through the actual touch/PAM locker, verifies client coordinates and native keyboard input in all four orientations, and removes its entries on exit. Review the physical 720×1440 captures against the client-coordinate colors independently; a passing controller is not itself a pixel review. `--physical-seconds 0` skips physical observation; a nonzero window requires the owner to turn the phone both ways and back to portrait for a completed physical sequence. Leave the phone upright for later fixed-layout credential replays.

`tests/phone_session_picker.py --session tarot --output /absolute/new/evidence` exercises Phrog's actual visible picker without the auxiliary greetd IPC client. Run it from a root device terminal. Review each pre-credential frame before supplying the requested row coordinates and confirmations; its bounded numeric-password path obtains credentials through a hidden prompt and sends only uinput events. The default Tarot entry must launch ordinary-user processes with no diagnostic socket. A successful Tarot picker login and an independently verified stock recovery are separate checks. Revision 5 now has reviewed successful Tarot and Phosh picker runs; retain the exact helper source and images with each result.


For the native sensor/panel gate, `phone_installed_session_replay.py --cycles 0 --require-audio --observe-sensors --panel-off --detach` requires the installed diagnostic session. It waits through the normal idle deadline; the root-only `phone_sensor_claims.py` observer correlates only SensorProxy claim/release call headers and owner replies to the verified compositor PID. It checks actual panel DPMS/backlight readings, accepted release/reclaim, protected wake and fresh PAM/app/audio restoration. This does not measure power consumption or exercise a physical power key. Pass each additional successful native record to `phone_installed_session_cleanup.py --extra-replay` so its old login cgroup is independently checked too.


## Persistent rotation and idle preferences

The current source uses version-2 `settings.toml` under `$XDG_CONFIG_HOME/tarot`, or `~/.config/tarot`. Notification-only version-1 files migrate without an eager write. Rotation lock preserves its current orientation across restart. Invalid/newer files remain untouched; changes then apply until logout and the UI reports that saving failed. Stop Tarot before repairing or moving aside such a file, then restart.

`--idle-preferences` requires a locker and enables saved timeouts with defaults of 120-second locking and 180-second panel off. The installed launcher supplies this option. `--idle-lock-ms`/`--idle-off-ms` override saved timeouts; `--orientation` overrides the saved initial locked orientation. Only direct DRM applies panel power-off. An explicit diagnostic `--platform-bus` uses temporary preferences unless `--config-file` is supplied, so isolated service tests cannot alter host settings.

Build the binaries and `lock-probe`, `sensor-probe`, `notification-probe` examples first. Then run `tests/display_preferences_replay.py --output /absolute/new/evidence` and the existing notification-session suite. `tests/locker_orientation_replay.py` requires X11/XTEST and must run without competing window-input tests; it checks the real Rust locker's entry/keyboard in every orientation without attempting PAM. Its empty-entry captures produce independent key-center templates. `tests/phone_display_preferences.py` uses those templates on the authorized spare phone to test actual touch/PAM and persistent idle timing, restores the original settings and returns to the normal Tarot session by default. It can begin from either Tarot or Phosh; `--restore-session phosh` is reserved for an explicit stock workload. Installed revision 8 passes this replay, including actual timeout dropdowns selected with `--touch-idle-controls`; consult the revision-specific status for exact scope. Endurance and independent final recovery remain separate checks.


`tests/lock_resize_callback_replay.py` deliberately holds the old-size buffer until its callback, checks that the gap remains black and uncapturable, then commits the new size. This detects the EGL-resize starvation exposed during persistent-policy work. `TAROT_LOCK_FRAME_TRACE=1` is an opt-in trusted-locker diagnostic, capped at 200 stage/geometry/callback metadata lines per process; it does not log credentials or input coordinates. It does not disable process hardening.

## Read-only resource sampling

`tests/phone_resource_sample.py` collects bounded sequential `/proc` and sysfs observations on the spare phone. Stage it under `~/tarot-dev/` and run through the authenticated root terminal, for example:

```sh
python3 phone_resource_sample.py --session phosh \
  --output artifacts/resources-phosh-locked-off --label locked-panel-off \
  --seconds 300 --interval 5 --detach
```

Use `--session tarot` for the shell and its Rust helpers. The sampler excludes ordinary apps, audio daemons and system services from the named-process totals; compare the same scope in both sessions and account for other processes separately. It records summed PSS, descriptor counts, per-process PID/start identities and CPU ticks, temperature, battery, panel state and coarse network/radio state without recording app text or network names. DRM allocations are deduplicated by underlying device, driver and client ID, including aliases across processes; repeated sequential readings use the largest observed counter. GPU allocation totals are separate from PSS.

DRM deduplication removes repeated descriptors for the same client. Shared buffers may still contribute to multiple clients' allocation counters; their sum is not unique physical GPU memory.

It performs no input injection or policy changes and allows normal idle/suspend behavior. Specify the workload phase in the label and inspect all samples. A process-membership change or observed kernel sleep suppresses the single-core CPU percentage; phase transitions remain raw evidence. A completed collection does not certify frame deadlines, touch latency, energy consumption or an idle budget. The sampler's own process is excluded, so account for sampling overhead when interpreting results. Process identity reads the effective UID from `status`, independently of procfs file ownership; the kernel documents those fields in its [procfs interface](https://www.kernel.org/doc/html/v7.0/filesystems/proc.html). Three host accounting regressions cover PID reuse, suspend clocks, unusual process names and duplicate DRM descriptors. A six-sample native Phosh smoke run completes with verified sampler source and duplicate Panfrost descriptors observed. Its roughly ten-second CPU/memory readings are functional evidence only. Matched five-minute measurements remain pending.

`tests/phone_resource_tarot_phase.py` expects an installed diagnostic Tarot login already authenticated through greetd. It waits for the normal three-minute idle panel-off policy, independently checks DPMS/backlight and capture denial, runs a five-minute locked/off sample and ends only the verified compositor through a pidfd. It requires cgroup/socket cleanup and leaves Phrog ready; finish in the normal Tarot session after the full workload. No locker authentication is attempted in this resource-only phase. The phase controller now passes on revision 8 with normal panel off, five-minute collection and automatic cgroup/socket cleanup. An earlier attempt retained a fresh activity timestamp and stopped at its original deadline; preserve it separately. This pass measures initial protected startup, not memory after ordinary app use.

That startup-only measurement does not establish the footprint after using the shell. For that workload, the installed-session replay accepts `--cycles 0 --panel-off --observe-sensors --require-audio --resource-sample`: actual PAM and native Calculator rendering precede the normal panel-off interval and five-minute sample, followed by protected wake, fresh PAM and app/audio restoration. This mode backs up the original preferences and restores them only after the tested login cgroup empties. Preserve the executed replay source separately from earlier endurance sources. Resource and repeated-use/100-cycle gates retain their own scope.
