# Tarot

Tarot is a Rust phone shell in development for Nura on the PinePhone Pro, recreating the visual character and interactions of classic WebOS and LuneOS with egui.

The intended result is a complete Wayland session that runs native Linux applications: cards and stacks, a launcher and quick-launch dock, gestures, notifications, system controls, and eventually Rust phone applications. Tarot-owned application code will be Rust; standard Linux services, drivers, and native libraries are part of the platform.

The original [Nocturne design packet](docs/Tarot_Design_Packet/README.txt) now guides the shell’s appearance and Pre 2-era interaction model. Its wallpaper, original icons, Lato typography and shared geometry run in the installed phone session. [Design coverage and remaining work](docs/DESIGN_APPLICATION.md) distinguish implemented foundations from the full design.

## Project status

Implementation is underway. The workspace contains a Smithay compositor with shared egui/GLES rendering, a platform-independent window/gesture model, a nested desktop backend, and an experimental direct DRM/libseat/libinput backend. The optimized AArch64 build runs on the phone's Panfrost GPU with a real GTK client. Basic touchscreen navigation and VT release/reacquisition have been exercised on hardware.

M0 remains in progress. The phone has been upgraded through supported releases to 26.06; build reproduction, recovery, and the remaining hardware baseline are being recorded. Most planned phone-shell features are still outstanding. See [implementation status](docs/IMPLEMENTATION_STATUS.md) for evidence and limitations.

Tarot is now the default login session on the development phone, with the normal installed entry running and diagnostics disabled. The retained revision 8 → 5 → 8 package round trip passes real PAM, native app/audio and cleanup checks on both revisions.

The installed revision-40 development package adds menu opening/closing motion, vector service/selection glyphs, a separate brightness rail and bounded landscape layouts. All 219 workspace tests pass on current/minimum Rust, plus strict host/ARM checks. One controller passes fourteen scoped native shell components, Launcher pages and saved restart; independent restoration/normal-session/Panfrost audits pass. An additional reverse-direction held-key host regression fails and is the next required correction. [Current evidence and limits](docs/recordings/r40/README.md) · [Menu contract](docs/MENUS.md). It retains distinct app/device menus, [Just Type](docs/JUST_TYPE.md), native PNG/SVG artwork and named pages. Exported app commands, physical calibration and the retained browser failures remain open; revision 32 remains the last accepted complete scoped native checkpoint including the browser. The complete development-plan goal remains unfinished.

Revision 26 introduced [native Firefox acceptance and standard surface viewports](docs/BROWSER_COMPATIBILITY.md), including corrected rendering/input, text/password entry, history, popup windows and fullscreen Unicode entry above the keyboard. It retains native transient families, popup overflow and [explicit maximize/fullscreen negotiation](docs/WINDOW_STATES.md). [Fullscreen text fields](docs/FULLSCREEN_TEXT_INPUT.md) now remain above the trusted keyboard: real GTK/Qt bottom fields resize, accept exact text and survive keyboard hide/reopen, overview and actual PAM unlock. All six native protocol cases (47 input requests) and existing shell regressions pass, with saved restart and independent cleanup. Revision 23 also [blocks virtual keyboard input during protection](docs/VIRTUAL_KEYBOARD_PROTECTION.md), closing a reproduced path from the trusted helper to the locker. [Historical revision-26 evidence and identities](docs/recordings/r26/README.md) retain the exact scope. Broader IME/application coverage and the remaining plan gates stay open.

Revision 14 adds the live local Calendar icon date and retained KMS power-off evidence, with native touch/PAM, favorite persistence, protected rotation and power/volume checks. Visual conformance remains open; isolated captures include pixel checks for status visibility. The phone remains in normal Tarot with diagnostics disabled.

Revision 9 introduced [hardware power and volume buttons](docs/BUILDING.md#hardware-buttons), with native protected panel off/wake and real PAM-worker cancellation tests. The owner confirms the physical buttons and volume indicator work. Revision 8's longer login/suspend results remain scoped to that earlier package.

Real foot/ash [terminal and clipboard acceptance](docs/TERMINAL_CLIPBOARD.md) now covers native touch typing, Backspace/Enter, exact UTF-8 output and Unicode/newline exchange with GTK through real PAM restoration. Revision 25 adds [touch terminal controls](docs/TERMINAL_CONTROLS.md): one-shot Ctrl/Alt, navigation, Escape/Tab and Paste, with actual shell editing, foreground interruption, exact PTY bytes, helper recovery and fullscreen/PAM checks. A reproduced stuck-Control helper-death bug is corrected before installation. Broader bindings, layouts and clipboard coverage remain open.

## Build and run

On Linux with Rust, a C linker, libxkbcommon development files, and Wayland/EGL/GLES runtimes for the native keyboard:

```sh
cargo test --workspace
cargo run --bin tarot
cargo run --bin tarot -- -- gtk3-demo
```

F1 opens card overview; F2 opens the installed-app launcher. The bottom strip also provides overview access. Client arguments follow `--`. The launcher now discovers desktop entries, searches their metadata and launches them with structured arguments. GTK3, GTK4, Qt/Kalk and foot have rendered together on the phone; the [compatibility record](docs/APPLICATION_COMPATIBILITY.md) distinguishes proven behavior from pending checks.

The interaction prototype now follows a strip drag into overview, browses cards horizontally and requests close with an upward card flick. Canceled drags settle back without opening the card; launcher scrolling stays within the launcher. Motion uses elapsed time, with `--reduced-motion` available. A real phone input replay covers these paths; faithful visual styling and the remaining shell features are still in development. See the [reference board](docs/REFERENCE_BOARD.md).

In overview, hold a card to arrange it, drag onto another card to stack it, or use the separate-card rail to reorder/unstack it. Edge holds page through the deck. Each member remains its own native window; its menu also supplies move/stack/separate actions. These interactions have a [phone replay](docs/BUILDING.md#native-stack-interaction-replay), including cancellation and closing one member while retaining its sibling.

A separate Rust/egui [native keyboard](docs/BUILDING.md#native-egui-keyboard) now types into real GTK4 fields on the phone through the native text bridge. The same key view supports touch search in the launcher and masked Wi-Fi password entry. Letter and two symbol pages cover all 95 printable ASCII characters, with native touch replay coverage. Native typing uses Panfrost/GLES, reserves app space above the gesture strip and preserves app focus. The strip's Keyboard button reopens a hidden keyboard, and bounded process supervision recovers from a helper crash. These paths have real phone replay evidence; broader toolkit input, layouts and visual refinement remain in development.

An ordinary [egui application fixture](docs/BUILDING.md#ordinary-egui-application-fixture) also runs as independent native cards, with phone tests for text/password entry, two windows, cooperative close and app-crash isolation. Its pinned egui host includes a direct-commit adapter. Multiwindow testing exposed and fixed hidden-window frame starvation; hidden clients now receive bounded callbacks while windows sharing a visible client remain responsive.

The development wrappers now enable a Rust [standard notification service](docs/BUILDING.md#native-notifications) on their private session bus. Native notifications appear as bottom banners and remain in a touch dashboard opened through Alerts. Replacement, actions, expiry, do-not-disturb, preview hiding and input isolation have real phone replay evidence. Notification settings use a versioned TOML file with atomic saves. The service retries unavailable buses/names and retires old actions after a bus disconnect. History, authenticated lock privacy and notification styling remain in development.

The top-right status area now opens a [power and display menu](docs/BUILDING.md#power-and-display-controls). With `--system-services`, battery/charging data comes from UPower and brightness changes go through the active local login session. Real phone taps changed brightness with driver readback, and the menu preserved native text focus on dismissal. Opt-in idle/panel paths and twenty consecutive actual RTC-woken kernel suspend cycles now have native evidence; broader wake/reliability coverage, power policy and a full Settings application remain in development.

The menu's [Audio page](docs/BUILDING.md#audio-controls) now controls playback volume, mute and the default output for new playback through the existing sound server. Native phone taps passed independent server readback, temporary-output selection/removal, exact restoration and same-field typing. Private real-server tests cover hotplug, delayed replies, timeout, service replacement and pending shutdown. Hardware volume keys now have native readback and owner confirmation. Microphone, headset/profile routing, call audio and acoustic validation remain in development.

Its [Wi-Fi page](docs/BUILDING.md#wi-fi-controls) now scans, connects saved or new visible open/personal networks, disconnects Wi-Fi and changes the Wi-Fi radio through NetworkManager. A masked form uses the shared private keyboard, with remembered or temporary profiles; a restricted secret agent answers missing-password requests for user-initiated connections. Native phone tests created a temporary connection, answered a real password request, restored the original network and kept USB available. A remembered personal profile also passed complete touch password entry, private on-disk credential verification and reconnection without entry in a new compositor session. NetworkManager was not restarted. Pending operations require observed service state before reporting completion. Hidden networks, enterprise setup, authorization prompts and broader Settings remain in development.

An opt-in [Rust/egui locker](docs/BUILDING.md#authenticated-locker-experiment) now provides a clock/unlock view and private touch/physical password entry. An unprivileged bounded worker verifies the current account through Linux-PAM. Real phone tests cover wrong-password rejection, Cancel, authenticated touch unlock and trusted crash recovery. The compositor separately protects rendering, input, clipboard and captures. With a locker configured, startup is protected before the first compositor frame, the system menu supplies a reusable Lock action, and bounded automatic recovery replaces a failed locker. Native tests cover startup, crash recovery and a second touch lock/authentication cycle. A separate protected recovery view now retries an exhausted locker, with native touch/keyboard and PAM handoff evidence. Opt-in sleep coordination holds a real elogind inhibitor until protected KMS completion and requires a new protected frame and next-cycle inhibitor before unlock. Native evidence includes twenty consecutive actual RTC-woken kernel suspend cycles with fresh PAM and app/input restoration, in addition to signal-only tests. Opt-in idle locking, visible-surface inhibition and real panel off/wake have native input/PAM evidence. Broader wake/endurance coverage, authentication policy and release packaging remain in development; the authenticated installed session preview is described below.

The inspected NVIDIA desktop driver produces an empty framebuffer with the current integration. For the verified Mesa software path on this host:

```sh
TAROT_SOFTWARE_RENDERER=1 ./scripts/run-nested.sh -- gtk3-demo
```

A source-built [installed session preview](docs/PACKAGING.md) now runs unprivileged through the actual Phrog picker and greetd/PAM/elogind alongside Phosh. Revision 5 passes ten normal login/exit cycles and twenty consecutive actual RTC deep-suspend cycles, with protected resume, fresh PAM, app/audio continuity and forced-crash cgroup/socket cleanup. Independent Phosh restoration and resource/policy checks pass; earlier incomplete attempts remain separate. Broader reliability and release packaging remain under development.

Shared output rotation and the SensorProxy adapter now have native four-orientation KMS/pixel/touch/keyboard evidence, plus real sensor directions and owner-confirmed upright behavior. Native panel-off tests observe accepted sensor release/reclaim and protected wake/authentication. Installed revision 8 also passes persistent rotation/idle settings, actual timeout dropdowns, saved panel-off timing, protected landscape restart and touch/PAM in all four orientations. Fresh ten-login and twenty-suspend runs plus independent Phosh recovery also pass on these exact binaries. Broader hardware keys/wake sources and sustained performance/power measurements remain open. See the [current plan](docs/DEVELOPMENT_PLAN.md#m3-deliver-a-native-pinephone-pro-session) and [evidence](docs/IMPLEMENTATION_STATUS.md#persistent-display-policy-checkpoint).

See [building and diagnostics](docs/BUILDING.md) for cross-compilation, captures, and device access.

## Development documentation

- [Development plan](docs/DEVELOPMENT_PLAN.md): scope, milestones, acceptance criteria, testing, risks, and the initial backlog.
- [Session packaging](docs/PACKAGING.md): source APK builds, authenticated startup, diagnostics and recovery.
- [Architecture](docs/ARCHITECTURE.md): component boundaries, rendering, protocols, services, packaging, and proposed decisions.
- [Interaction design](docs/INTERACTION_DESIGN.md): WebOS fidelity, card behavior, gesture ownership, visual design, and accessibility.
- [Research](docs/RESEARCH.md): dated findings, primary sources, dependency candidates, and remaining technical unknowns.
- [Device baseline](docs/DEVICE_BASELINE.md): observed hardware, OS transitions, and provisioning changes.

Start with the development plan. Its milestones define the intended complete system; the status record distinguishes demonstrated behavior from remaining work. Tarot-owned code is licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Adapted upstream scaffolding retains its [source attribution](third-party/README.md).
