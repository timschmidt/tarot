# Tarot development plan

Planning baseline: October 26, 2026. Tarot will recreate the classic WebOS/LuneOS phone experience in Rust using egui, as a complete Linux phone shell for Nura on PinePhone Pro. Build a working compositor and validate the real phone early, then develop the interaction design, system integration, and phone applications in stages.

Implementation is underway; [implementation status](IMPLEMENTATION_STATUS.md) records current evidence. Milestone completion below means demonstrated behavior with recorded evidence, not merely merged code or a convincing mockup. The complete product scope remains the objective.

October 28 design update: the owner supplied the original [Tarot design packet](Tarot_Design_Packet/README.txt) and requested that it be committed, consumed and applied. **Nocturne, based on the Pre 2-era webOS 2.x phone interaction model, is now the visual authority.** Its proposed tokens supersede the earlier scaffold values. The exact historical firmware build is still unpinned. [Application map and ordered remaining work](DESIGN_APPLICATION.md) connect all 25 studies to implementation and acceptance evidence.

Revision-32 update: unchanged-layout keyboard activation retains its mapped surface while clearing stale context input. The complete scoped native suite and saved restart now pass, including ten keyboard cases, 21 Firefox card returns, native protocol/GTK/Qt/terminal/clipboard, favorite/card/stack/editor/hardware checks and independent cleanup. Geometry-aware capture validation prevents stale target selection. No complete M0–M6 milestone is closed. Next, implement the packet’s named Launcher pages, native icon resolution, Just Type and menu/notification composition while retaining the separate device-endurance and broader compatibility gates. [Current scoped acceptance](recordings/r32/README.md) · [Keyboard contract](KEYBOARD_CONTEXT.md).

Revision-34 update: named Launcher pages now pass their native touch/PAM/rotation/name-management/favorite-link/restart component. The source and installed package include the original packet's page interaction. Physical calibration, native icons, Just Type, menu/notification composition and all remaining milestones retain their existing scope. The broader revision-34 run fails a browser character check on repeat 19; current input reliability and endurance are not certified by the page component. [Current evidence and limits](recordings/r34/README.md).

Revision-35 update: native application PNG/SVG artwork now loads on a bounded Rust worker, preserving original Nocturne role art. The native icon, orientation, favorite, real-PAM and saved-restart component passes, with independent normal-session/Panfrost audits. XPM/imported SVG images, live catalog/theme refresh, physical/visual review and performance budgets remain open. The browser activation failure and all unfinished M0–M6 gates retain their scope. [Revision-35 results and limits](recordings/r35/README.md).

Revision-36 update: numeric server-side traces now show browser enable/activation arriving during a held fullscreen key, whose context reset cancels delivery. The native replay passes 21 returns first; browser reliability remains open. A separate QA correction defers accumulated checkpoints during held contacts: the unchanged revision-35 APK then passes all 13 components after an earlier checkpoint crossed the card edge-paging deadline. Preserve both failures, the stale-input guard and original exact-value gates. A separate run on the same revision-36 package passes 12 shell components and saved restart. Normal-session/policy/preference/Panfrost restoration audits pass after both runs. No remaining milestone or current-binary endurance gate closes. [Current evidence and limits](recordings/r36/README.md).

Revision-37 update: Just Type now uses actual applications, settings and retained notifications, with explicit HTTP(S)/provider handoff through standard Linux helpers. Host rendered portrait/landscape and local URI/content paths pass, alongside 195 current/minimum-Rust workspace tests. The signed package is installed; native attempts retain settings-coordinate and legacy favorite-menu QA failures. A stronger dashboard-only visibility gate replaces a dimmed-background false positive. A separate real-drag/reselection regression exposes a repeat-target scroll defect, now being corrected in revision 38 alongside common settings aliases. Contacts/messages/documents, creation actions, provider registration and accessibility/performance review remain open. Existing browser/endurance failures and all unfinished milestones retain their scope. [Contract](JUST_TYPE.md) · [Current evidence and limits](recordings/r37/README.md).

Revision-38 update: selecting the same retained notification now issues a fresh dashboard scroll, with the original failing real-drag/reselection replay and expanded portrait/landscape gates passing. Common settings aliases route through one shared matcher. All 196 workspace tests pass on current/minimum Rust, with strict host/ARM checks. The signed package passes all 13 scoped native shell components plus saved restart, including Just Type local/URI paths, reselection, orientation and actual PAM; independent restoration/Panfrost audits and qualitative owner usability pass. The retained browser component remains excluded, and no full milestone or current-binary endurance gate closes. [Current scope](recordings/r38/README.md). The next [app/device menu implementation plan](MENUS_PLAN.md) separates shell controls from validated client exports and records the protocol, input, privacy and real-device acceptance work.

Revision-42 update: correct touch departure, ordered gesture cancellation and scroll ownership resolve the retained landscape result failure. All 222 current/minimum-Rust tests, strict host/ARM checks and thirteen host reports pass. One native controller passes all fourteen scoped shell components, full Launcher pages and saved restart; exact restoration/normal/Panfrost audits pass. The owner confirms repeated landscape scrolling and Sound selection by finger. The first signed candidate is withheld after stronger release tests expose dropped intents. Original failures, source/hash comparisons and remaining browser/endurance/accessibility/export gates stay recorded. No full milestone is closed. [Current evidence](recordings/r42/README.md).

Unchanged revision-42 follow-up: focused Firefox and combined Firefox/fourteen-shell-component runs pass their original native gates, with 21 browser returns each. Full Launcher pages, saved restart and independent exact restoration/normal/Panfrost audits pass in the combined run. These are comparison results; the retained intermittent browser activation failure and current-binary endurance remain open. Next delivery is refined into [shared notification geometry/routing and the well/rail/dashboard](DESIGN_APPLICATION.md#next-notification-composition-sequence) and [stable native-menu process ownership](MENUS_PLAN.md#stable-process-ownership-precheck), with explicit native acceptance requirements before capability claims.

Revision-43 development: the shared notification well/source rail and exposed foreground now have real client/popup/keyboard/rotation/protection component evidence on host and phone. The broader native run passes ten components, then fails the first favorite drop at an obsolete QA tray coordinate; independent restoration/normal/Panfrost checks pass. A focused additional reproduction finds cancellation of a landscape favorite hold when hiding its keyboard restores the rail. Revision-44 development corrects both paths. Its first native run repeats the held-activation browser failure after 21 returns, with normal-session/preference/policy/Panfrost restoration verified; a separate unchanged-package run passes ten components including the native favorite correction before an obsolete Calendar coordinate fails. A subsequent run passes actual DND-off notification coverage, then its QA fling skips Calendar. Preserve both failures and require passing remaining shell/pages/restart acceptance with overlapping visible-control scans, while retaining the browser failure. Preserve original failures, exact package/source identities and the separate browser/endurance/accessibility/export gates. [Contract](NOTIFICATIONS.md) · [Current evidence](recordings/r43/README.md).

Revision-41 update: keyboard retirement now removes client focus before clearing held seat/forwarded state, then consumes physical release tails. Both exact revision-40 failures pass unchanged; 219 current/minimum-Rust tests, strict host/ARM checks and twelve host replay reports pass. Both native runs pass seven components including both held-key directions, then fail rotated search scrolling. Paced test input does not resolve the native gate. Independent exact restoration/normal/Panfrost audits pass; remaining native components/pages/restart were not reached. Search drag/wheel ownership, exported commands, calibration, enlarged text, browser/endurance and unfinished milestones remain open. [Current evidence](recordings/r41/README.md). The separate [GIO experiment](../experiments/menu-export/README.md) establishes host binding feasibility and uncertain action outcome after timeout; ownership binding, bounds, real app exports and phone integration remain required.

Revision-40 update: menu opening/closing now waits for a matching presented retirement frame before restoring input or committing a deferred choice. Vector glyphs, separated brightness painting, fixed empty-detail alignment, pinned long-heading navigation and landscape credential scrolling have host evidence. All 219 current/minimum-Rust workspace tests and strict host/ARM checks pass; one native controller passes fourteen shell components, Launcher pages and saved restart with independent exact restoration/Panfrost audits. A separate host regression exposes native-to-menu held-key bookkeeping and fails in both poses at the original zero-held-key gate. Preserve and correct it before accepting menu keyboard ownership. Timing/physical calibration, enlarged text, exported commands, browser/endurance and all unfinished milestones remain open. [Evidence and failure](recordings/r40/README.md).

## Confirmed direction

| Topic | Owner decision |
| --- | --- |
| Product scope | Full phone shell/compositor with native Linux application support |
| Rust boundary | Tarot-owned application code in Rust; standard Linux services, native libraries, and drivers are allowed |
| UI direction | Original Nocturne packet; Pre 2-era webOS 2.x behavior, faithful where practical and explicitly adapted for Linux/PinePhone Pro |
| Platform | Nura on PinePhone Pro; use Nura terminology in project documentation |
| Hardware | A spare development phone is available; the owner authorizes development control and updating its OS |
| Device baseline | The connected phone was upgraded from 24.12 through supported releases to stable 26.06; retain the verified baseline and recovery procedure |
| Present task | Implement the development plan and verify its gates on the available phone |
| Project license | MIT OR Apache-2.0, selected by the owner October 27, 2026 |
| SIM testing | No active SIM is available currently; carrier call/SMS gates require later access |
| Default phone session | Tarot; leave routine development/testing in Tarot rather than restoring Phosh (owner instruction October 27) |

Working assumptions are one primary developer, a Linux desktop development host, portrait-first interaction with landscape support, and an initial English keyboard/layout test set. These assumptions guide sequencing, not a delivery-date commitment. Exact language coverage and staffing can be decided without delaying the technical experiments.

## Product scope and completion levels

| Level | Included | Evidence required |
| --- | --- | --- |
| Interaction prototype | Home, cards, launcher, notifications, gestures, motion, and simulated platform state | Deterministic replay and visual review; clearly identified as a prototype |
| Native compositor preview | Real Wayland apps rendered as cards in a nested desktop session | Window lifecycle, input, text-entry experiment, and multi-toolkit compatibility tests |
| Device shell alpha | Bootable/selectable Nura session, touch, keyboard, cards/stacks, launcher/search, notifications, system controls, authenticated lock, and basic power management | Installed package on the phone; recovery, lock, and device tests pass |
| Phone alpha | Rust phone/messages/contacts apps plus validated modem and call-audio integration | Calls and SMS tested with a named carrier/modem configuration, including wake behavior |
| Tested beta | Stable session, documented application compatibility, accessibility checks, controlled resource usage, and repeatable packaging/update/recovery | A published test matrix and measured performance/reliability results |

The initial release does not include a kernel, modem firmware, graphics driver, browser engine, or replacement for every system daemon. Legacy Mojo/Enyo/WebOS application compatibility, Luna service-bus compatibility, Android apps, cloud account aggregation, MMS, a software store, broad device support, and desktop convergence are separate future projects. Existing Linux browser, terminal, media, and camera applications can run within Tarot as compatibility allows.

Keep a WebOS feature recognizable even when its first implementation is small. For example, start universal search with apps and settings; start stacks after individual cards; start phone support with one active call and ordinary SMS. Do not replace essential interaction behavior with a permanently decorative mock.

## Architectural approach

The proposed foundation is a Smithay Wayland compositor, an integrated egui shell, and an EGL/OpenGL ES renderer. Native applications remain client surfaces; Tarot controls their placement, transforms, lifecycle, and focus. A pure state model drives navigation and gestures, while asynchronous adapters connect to Linux services. A separate trusted locker enforces authenticated session locking together with compositor policy.

This follows the division between egui's UI integration and Smithay's compositor mechanisms. OpenGL ES is the practical first target for the phone's Mali-T860; Mesa does not list PanVK support for this GPU. These findings make the approach plausible but do not replace the integration experiment. [egui integration](https://docs.rs/egui/latest/egui/#integrating-with-egui), [Smithay](https://smithay.github.io/smithay/smithay/), [Panfrost](https://docs.mesa3d.org/drivers/panfrost.html)

See [architecture](ARCHITECTURE.md) for component boundaries, rendering, input, protocols, services, packaging, and the decision register. See [interaction design](INTERACTION_DESIGN.md) for the proposed behavior. [Research](RESEARCH.md) records the sources and uncertainties.

Revision-39 update: initial distinct app/device menus now have native touch/PAM/service/client-lifetime component evidence within a 14-component shell run. A separate focused page/restart run passes after an attached-chevron OCR correction; the first failed controller remains retained. Independent normal-session/restoration/Panfrost audits pass after both. All 209 current/minimum-compiler tests and strict host/ARM checks pass. Menu motion, glyph/rail contrast, enlarged/physical calibration, real exported app actions and all unfinished M0–M6 gates remain open. [Contract](MENUS.md) · [Native evidence and limits](recordings/r39/README.md) · [Export experiment](MENUS_PLAN.md#next-native-export-experiment).

## Milestone sequence

The critical path is **M0 feasibility → M1 interaction model → M2 native app management → M3 device session → M4 shell alpha → M5 phone alpha → M6 beta**. Real hardware testing begins in M0, and all later phases retain the nested desktop development path. Design, service mocks, and packaging preparation can progress independently when their dependencies permit.

| Milestone | Main outcome | Rough focused effort |
| --- | --- | --- |
| M0 | Reproducible phone baseline and proven rendering path | 2–4 person-weeks |
| M1 | Testable state model and faithful interaction prototype | 2–3 person-weeks |
| M2 | Native Linux apps work as cards | 3–5 person-weeks |
| M3 | Tarot runs as a recoverable device session | 3–5 person-weeks |
| M4 | Usable shell alpha with services, keyboard, and locking | 4–8 person-weeks |
| M5 | Phone, messages, and contacts integration | 6–12 person-weeks |
| M6 | Performance, accessibility, compatibility, and reliability | 4–8 person-weeks |

The aggregate planning envelope is roughly **24–45 person-weeks**, with low confidence before M0. It excludes waiting for upstream driver/carrier fixes and the deferred projects above. Calendar time depends on actual availability and experience; re-estimate after M0 and M3 instead of treating these ranges as promises.

## M0 Establish feasibility and the device baseline

**Purpose:** retire the highest architectural and hardware risks before investing in a large UI.

1. Identify the spare phone, its storage/boot arrangement, OS, kernel, modem firmware, and available USB/SSH/serial access. The owner requires no existing user data to be saved. Record technical recovery information and establish a recovery path before replacing its installation.
2. Resolve the newest supported Nura release at execution time. Default to its latest stable PinePhone Pro image and updates; use edge for a demonstrated requirement. Record image URL/checksum, package versions, service manager, boot chain, recovery media, and setup steps in a future device-baseline record.
3. Use the installation instructions matching that release. In particular, distinguish the documented edge U-Boot/systemd-boot transition from older Tow-Boot instructions. The bootloader name does not itself select the userspace service manager. [Nura installation](https://nura.eco/install/), [device boot transition](https://nura.eco/edge/2026/08/30/pinephonepro-u-boot/)
4. Establish a stock-shell baseline for graphics acceleration, touch, orientation, Wi-Fi, audio, charging, display sleep, suspend/resume, modem registration, and available call/SMS behavior. Record missing carrier/SIM access explicitly. This separates existing platform limitations from Tarot regressions.
5. Build a minimal nested Smithay compositor with one real xdg-shell client and egui chrome sharing the GLES path. Test alpha, clipping, output scale, resize, texture lifetime, and shell/client damage independently.
6. Run the same rendering experiment on the PinePhone Pro. Confirm the actual GPU renderer rather than a software fallback. Exercise a minimal direct DRM/KMS backend through the chosen seat setup, including touch and session release/reacquisition.
7. Resolve compatible Smithay, egui, glow, compiler, and system-library versions. Prefer releases; if an essential upstream fix requires a Git dependency, pin its revision and state why. Confirm AArch64/musl build and runtime linkage.
8. Record initial frame times, process memory, GPU allocation observations, idle activity, and one short suspend/resume run. Document the selected renderer integration and any rejected alternative with its concrete failure.

**Exit criteria:** a real client and egui shell content render together on desktop and the phone; touch reaches the intended target; hardware acceleration is confirmed; a minimal direct-device session works; a repeatable build/install/recovery procedure exists; D05 and D06 have evidence supporting acceptance or a specific replacement design. An egui-only fullscreen demo does not meet this gate.

If integration fails, compare the alternatives in [the architecture decision register](ARCHITECTURE.md#decision-register) before expanding product scope. If a platform fault exists in the stock image too, record it as a platform dependency instead of hiding it behind a UI workaround.

## M1 Build the interaction model and visual prototype

**Dependency:** enough M0 evidence to commit to the rendering boundary.

- Establish the initial workspace and pure state model, using the same command/event interfaces for mocks and real integration.
- Build the reference board and design tokens. Implement home, status area, dock, launcher, app search, card overview, notification previews, and provisional lock presentation.
- Implement explicit gesture ownership, finger-following transitions, cancellation, card selection, and close-request states. Keep stack identity in the model even before its complete UI ships.
- Add reusable touch widgets with semantic names, visible focus, large targets, font scaling, and reduced motion. Avoid a custom drawing-only control that cannot be activated through accessibility or keyboard input.
- Add a deterministic clock and recorded touch sequences. Keep the preview runnable with mock services and varied phone dimensions; an optional eframe harness must use the same UI/model as the compositor.

**Exit criteria:** launch → overview → select → dismiss → launcher works through touch replay and desktop controls; canceled gestures return to coherent states; no duplicate command is emitted when egui evaluates a view more than once; representative screens receive visual comparison against the reference board. Lock UI is explicitly a prototype until M4 security tests pass.

## M2 Manage real application windows

**Dependency:** M0 renderer and M1 model.

- Implement the native application lifecycle: launching, initial surface mapping, loading placeholder, focus, configure/ack, usable geometry, title/icon updates, close requests, client exit, and unresponsive states.
- Handle subsurfaces, popups, transient dialogs, and multiple windows per application. Preserve parent/child relationships during overview and return to focus.
- Compose real surface trees as cards with accurate clipping and input coordinates. Add manual stacks/reordering after basic card management is stable.
- Support the protocol set identified for M2 in the architecture. Test shared-memory and DMA-BUF clients, clipboard, touch cancellation, output transforms, and scale changes.
- Discover and launch desktop entries correctly; do not pass their `Exec` value through a shell. Show failure or missing-executable state instead of leaving an eternal loading card.
- Run a text-input/input-method experiment with a trusted compatible keyboard and real GTK/Qt fields. Test preedit/commit, focus changes, and keyboard geometry while switching cards. Include a terminal and a browser test where available.

**Exit criteria:** at least a GTK app, Qt app, terminal, and egui app are launchable in the nested session; two unrelated clients and one multiwindow app work together; dialogs remain actionable; dismissal follows app close semantics; switching does not relaunch apps; text reaches the intended field; the compositor survives a client crash. The application matrix records actual versions and exceptions.

Transient checkpoint, October 28: [native family ownership and dialog policy](TRANSIENT_WINDOWS.md) now pass nested Wayland lifecycle, modal/nonmodal focus, late parenting, orphan/remap and scale-1/2 pixel/input checks in all orientations, including managed protocol-lock isolation. Revision 18 now passes packaging, touchscreen/KMS, native keyboard, child-popup and separate GTK3/Qt6 widget acceptance, with real PAM and independent default-session cleanup. Revision 19 then corrects the observed premature full-app configure and remap ordering: native configure-history tests and actual Qt 280×240 response sizes pass. Revision 20 corrects popup clipping at the family-card boundary; actual overflow pixels/input pass in all orientations and real GTK3/Qt6 long menus select items outside conservative dialog bounds. The first Qt timing failure and corrected stable-geometry replay remain separate. Revision 21 adds explicit configure replies, committed root fullscreen/maximization, natural transient rejection and root minimization. All five native protocol cases, 40 input requests and real GTK3/Qt6 mode controls pass with fresh PAM and independent normal-session cleanup. Revision 22 reproduces fullscreen keyboard obstruction on revision 21 and corrects it with an owning-family viewport reservation. Real GTK3/Qt6 bottom fields remain above the keyboard, retain exact text through hide/reopen, overview and actual PAM, and restore full size. The five protocol cases, shell regressions and saved restart pass again. [Policy and remaining cases](FULLSCREEN_TEXT_INPUT.md) retain broader IME/window constraints alongside toolkit, terminal, clipboard and accessibility gates. The revision-17 in-window GTK close result remains a separate scope; M2 exit criteria remain open.

Terminal/clipboard follow-up on the unchanged revision-22 package: real foot/ash touch typing, virtual Backspace/Enter and exact UTF-8 command output pass. An independent GTK client exchanges Unicode/newline clipboard text with foot, including real PAM restoration and a fresh saved-settings restart. GTK → foot uses physical Ctrl+Shift+V; touch-only modifiers/paste, primary selection, broader formats and browser/accessibility coverage remain open. [Evidence and scope](TERMINAL_CLIPBOARD.md). This advances the M2 compatibility matrix without closing its full gate.

Protection checkpoint, revision 23: a real protocol test reproduces trusted virtual-key/modifier delivery to the synthetic locker through an already-bound connection. Per-request protected dispatch now blocks it and clears retained modifiers; held-Control and raw post-unlock delivery pass on host and the installed phone binary in a nested Wayland session. The outer session independently retains physical KMS/real PAM, fullscreen-field, terminal/clipboard, native app/card/hardware, restart and cleanup checks. [Evidence and limits](VIRTUAL_KEYBOARD_PROTECTION.md). The revision-25 checkpoint below addresses touch terminal controls; broader M2 and plan gates stay open.

Terminal controls checkpoint, revision 25: terminal-purpose fields use the taller touch keyboard with one-shot Ctrl/Alt, navigation, Tab/Escape and Paste. Real foot/ash editing, history, foreground Ctrl+C, exact Alt/Escape PTY bytes, completion, helper replacement, Unicode/newline clipboard exchange and fullscreen/PAM checks pass. The uninstalled revision-24 candidate exposes stuck Control after helper death; actual source ownership and physical-state cleanup correct it before phone installation. A fresh same-package visual replay requires keyboard pixels after fullscreen launch/reopen/PAM, retaining the first capture mismatch. Host/minimum/ARM, native app/shell, saved restart and normal/Panfrost audits pass. [Evidence and remaining cases](TERMINAL_CONTROLS.md). Broader bindings/layouts/clipboard, browser/accessibility, current-binary endurance and complete M2/M0–M6 gates remain open.

Browser checkpoint, revision 26: standard `wp_viewporter` support corrects a reproduced real Firefox scale/input mismatch. A separate parent-only subsurface position regression exposes a pinned Smithay defect; the attributed adaptation of unmerged proposal #2141 and delayed-parent test address it. Scale-1/2 viewport pixels/input, protected commits and malformed-client isolation pass. Real phone Firefox text/Backspace, overview/PAM, password, history, popup and fullscreen Unicode entry pass. All six native protocol cases/47 input requests, GTK/Qt, terminal/clipboard, shell/editor/hardware, saved restart and independent normal/Panfrost audits pass. [Evidence and remaining scope](BROWSER_COMPATIBILITY.md). Media, permission agents/portals, accessibility, wider applications and all remaining M0–M6 gates remain open; both sessions request zero suspend cycles.

## M3 Deliver a native PinePhone Pro session

**Dependency:** M2, building on the M0 device experiment.

- Turn the experimental backend into a selectable Nura Wayland session with correct seat ownership, DRM/KMS/GBM/EGL initialization, libinput, hardware keys, and output teardown.
- Map touch and display rotation consistently, including a gesture or keyboard active during an orientation change. Add rotation lock and capability-driven sensor handling.
- Add frame scheduling, output damage, idle display blanking, bounded thumbnail storage, and basic suspend/resume restoration. Begin comparison with the stock-shell baseline.
- Package the session through the distribution build environment. Record native dependencies and architecture-specific build settings; install alongside a known working session.
- Provide useful diagnostics, crash logs without sensitive content, a recovery session, and a documented SSH/TTY route. Keep routine shell execution unprivileged.

Actual sleep checkpoint, October 27: an awake RTC check and authenticated stock Phosh baseline now pass before Tarot tests. Normal elogind suspend sustains 14.33 seconds of stock deep sleep; Tarot's initial cycle sustains 14.21 seconds with independently observed protected KMS before kernel sleep, protected resume/re-arm, fresh actual PAM and the original app/clipboard restored. A first twenty-cycle attempt stops on cycle five after four fully validated cycles: its eight-second alarm leaves 2.772 seconds actually suspended, below the three-second gate. Native logs also expose a transient inhibitor re-arm race, now fixed and covered by a faithful private-manager regression. A second attempt has ten fully validated cycles before an unacknowledged app contact after authentication; its snapshot does not yet show an ordinary presented frame. The replay now waits for ordinary KMS/capture and acknowledged down before release. A third attempt validates sixteen cycles before its synthetic app reaches the unextended 180-active-second fixture deadline. The corrected replay gives the app the same bounded lifetime as the harness. Its fresh fifteen-second-alarm run now passes **twenty consecutive actual RTC-woken deep-suspend cycles**, each with 9.01–10.25 seconds suspended, protected KMS/resume/re-arm in the same manager epoch, fresh PAM, original app/clipboard/pixels and acknowledged native input/network restoration. Kernel success changes 35→55 with no failure increase; all app/compositor/helper/launcher/harness exits are zero and settled stock cleanup passes. Earlier incomplete attempts remain separate and do not accumulate toward this sample. At that temporary-harness checkpoint, manual sensor/power-key checks, output rotation and production session packaging remained required; later installed/rotation results are recorded below. [Exact evidence and limitations](IMPLEMENTATION_STATUS.md#actual-kernel-suspend-and-rtc-wake)

Installed-session checkpoint, October 27: the source-built APK and Rust supervisor run the ordinary user through actual greetd/PAM/elogind on VT7. Revision 4 passes ten normal login/exit cycles and a fresh twenty-cycle RTC deep-suspend run with protected resume, actual PAM, app/audio continuity and forced-crash cgroup/socket cleanup. Independent authenticated Phosh restoration and resource/policy checks pass. Earlier incomplete attempts remain separate. Actual visible-picker selection and authentication also pass on revision 5, launching its installed default entry with diagnostics disabled. [Package procedure](PACKAGING.md) [Evidence and failures](IMPLEMENTATION_STATUS.md#installed-authenticated-session-checkpoint)

Rotation checkpoint, October 27: revision 5 is built, audited and installed. Shared geometry, held-contact cancellation, keyboard/locker resizing and SensorProxy handling pass host tests; native touch coordinates and framebuffer pixels agree in all four orientations, with native keyboard typing in each pose. Real sensor observations cover both landscape directions, and the owner confirms the UI stays upright. The physical replay did not instrument a final portrait return after both directions. Revision 5 now separately passes ten complete login/exit cycles and twenty consecutive actual RTC deep-suspend cycles (10.124–10.661 seconds each), with protected resume, PAM, app/audio and forced-crash cleanup. A separate native panel-off test now observes successful SensorProxy release/reclaim replies, actual DPMS/backlight off/on, protected wake and fresh PAM/app/audio restoration. Rotation-lock persistence and broader wake/key behavior remain open. [Evidence](IMPLEMENTATION_STATUS.md#rotation-and-orientation-checkpoint)

Revision 5's final recovery audit also passes: authenticated Phosh is visually reviewed after startup, all recorded old cgroups and temporary resources are clear, and original network/brightness/services/policies are retained. Exact package, binary and executed QA identities remain separate from later QA-only changes.

Next M3 work, in dependency order:

Revision 8 checkpoint: actual timeout dropdowns, saved panel-off timing, protected landscape restart and touch/PAM in all four orientations pass. Fresh ten-login and twenty-suspend runs also pass on these exact binaries (10.089–10.659 seconds asleep per cycle, kernel successes 131→151, no failures), followed by forced-crash cleanup and independently checked authenticated Phosh restoration. Original settings are restored. Broader M3 gates remain open.

The retained revision 8 → 5 → 8 local package round trip now also passes both actual PAM/app/audio/exit checks with exact payload restoration and unchanged policies/settings. An after-app five-minute panel-off sample records 0.217% of one CPU core, about 103.1 MiB summed Tarot PSS and 106 descriptors throughout; active rendering, energy and repeated-use growth remain unverified. Tarot is now the persistent default session and is left running normally at the owner's request. A matched stock control is deferred; the earlier stock sample used different power conditions. Sensor claim timeouts observed before a phone reboot remain a reliability investigation, despite successful claims and native panel/wake checks afterward.

1. Extend wake/endurance coverage on the current binaries, retaining the separate synthetic-input, physical-button, RTC and carrier-dependent results. Revision 9 now passes native power/volume and protected panel tests, and the owner confirms the physical buttons; its kernel-suspend endurance is not yet established.
2. Complete stock-versus-Tarot frame/idle/resource measurements, broader update/schema-rollback/recovery checks and the sensor reliability investigation. The retained-package round trip is now verified separately. Reconcile every M0–M3 exit criterion with named evidence before declaring a milestone complete.

Hardware-key checkpoint: the revision 8 read-only snapshot identifies separate `gpio-keys` and `adc-keys` devices and explicit elogind `HandlePowerKey=ignore`; revision 9 preserves that policy. Session-owned short power now locks/toggles the panel through fresh protected presentation. Held power acts once; five-point volume steps have bounded repeat, release/removal cancellation and default-output/epoch guards. Native tests verify stale held-input suppression, DPMS/backlight, off-screen volume, paused real-PAM-worker cancellation and fresh authenticated app restoration. The owner confirms physical power and volume behavior in normal Tarot. Kernel suspend remains separate from panel off; broader physical wake, long-power UI and current-binary endurance remain open. [Evidence and scope](IMPLEMENTATION_STATUS.md#hardware-buttons)

**Exit criteria:** ten launch/exit cycles and twenty consecutive suspend/resume cycles complete without a Tarot-attributable display/input failure; nested and device backends share the same shell logic; portrait and landscape touch coordinates agree with the screen; a failed session can be recovered through the documented route. Until authenticated locking ships, this remains a development session.

## M4 Complete a useful shell alpha

**Dependency:** M2 compatibility and M3 device session.

- Add real NetworkManager, UPower, audio, BlueZ, sensor, and feedback adapters, with reconnects and visible unavailable/pending/error states. Implement required secret/pairing/Polkit agent flows rather than only successful-path toggles.
- Implement the standard notification server with banners, dashboard, replacement, actions, expiry, do-not-disturb, and privacy controls. Native app notifications must work without a Tarot-specific SDK.
- Complete launcher favorites, app/setting search, stack interactions, Settings, and preference persistence. Add `.desktop` visibility rules, icon resolution, default app/URI integration, and first-launch failures to the compatibility matrix.
- Ship a working trusted on-screen keyboard path. After protocol validation, build the initial Rust/egui keyboard layout if it can meet the quality gate; otherwise label the compatible external keyboard as a temporary alpha dependency with a tracked replacement task. Cover shell search, external app fields, password entry, and a physical keyboard.
- Implement the trusted locker and compositor lock enforcement, platform authentication, startup/restart policy, notification redaction, and input/capture restrictions. Test actual failures, not only the unlock animation.
- Coordinate idle, locking, display off, audio/call inhibitors, suspend, and resume with the chosen session stack. Keep the initial service-manager configuration explicit in packaging.

Implementation checkpoint, October 27: the initial standard notification service, banners/dashboard, replacement/actions/expiry, DND and banner-preview settings now have native phone evidence, including installed libnotify delivery and input isolation. Notification settings persist through versioned atomic TOML saves; notification-bus restart/name-release recovery has isolated integration evidence. This does not complete M4: broader service adapters/reconnection, startup readiness, durable history/general preferences, authenticated lock redaction and the remaining platform controls are outstanding. [Current notification evidence](IMPLEMENTATION_STATUS.md#native-notification-service)

The first power adapter/status menu also has native battery comparison and touch-controlled brightness/readback evidence, plus isolated authorization/failure/recovery tests. This covers initial battery/display controls; the broader power gate still requires production idle/inhibitor policy, actual kernel suspend/wake and sustained device tests, despite the subsequent authenticated-lock and idle/panel experiments below. The temporary local-session harness is not the final authenticated session package.

The initial NetworkManager adapter now has real phone touch evidence for scanning, saved-network activation, disconnect and Wi-Fi radio off/on, with the original profile restored and USB retained. New visible open/personal network setup, a masked shared-keyboard form, temporary/remembered profiles and a restricted secret agent are implemented. The phone also activated a separate volatile profile and answered a real missing-password request, removing both test profiles afterward. Isolated tests cover confirmation, timeout, denied/auth-required actions, credential validation, agent caller/cancellation/loss behavior and stale identities. A remembered personal password now has complete touch-entry, private disk verification and reconnection evidence in a new native compositor session. The fresh-install shell-alpha criterion still needs broader evidence. NetworkManager remained running; daemon/reboot persistence, hidden/enterprise setup and Polkit authorization remain unfinished or unverified. The final session must obtain ordinary local-session permissions through its authenticated greeter/PAM integration. [Network evidence and limits](IMPLEMENTATION_STATUS.md#wi-fi-system-controls)

Playback volume, mute and default output now have an initial libpulse implementation with actual phone touches and independent server readback. Private real-server tests cover balance, zero/same-value completion, hotplug, delay/timeout, daemon replacement and pending shutdown. New-window mapping preserves modal panels, and explicit layer ordering fixes scrim dismissal/reopen. The installed session starts the ordinary user audio service and retains available playback output across its revision-specific repeated login/suspend tests. Revision 9 adds native hardware volume/mute routing and physical volume-button confirmation. Acoustic speaker/microphone/headset behavior, profiles/ports, per-app routing, PipeWire runtime checks and Bluetooth/call policy remain outstanding. [Audio evidence](IMPLEMENTATION_STATUS.md#playback-audio-and-modal-recovery)

The single-output compositor lock boundary now has nested and native presentation/input/clipboard/capture/crash evidence, including a negative test for invalid-unlock fallthrough in pinned Smithay. The initial opt-in synthetic fixture was followed by a trusted Rust/egui client with real same-user PAM authentication and touch/physical entry on the phone. Protected compositor startup, normal menu locking and bounded trusted-client replacement now have host and phone evidence. Protected recovery after retry exhaustion now has native KMS/input evidence: canceled/removed contacts cannot retry, fresh Enter/touch can, and real PAM remains necessary before the same app/clipboard returns. An opt-in login1 delay-inhibitor/presentation boundary now has host FD/service-failure and native paused-VT/KMS evidence, using real elogind signals without kernel suspend. Opt-in idle locking, surface inhibition and actual panel off/wake now have nested protocol and native KMS/driver/input/PAM evidence, including occlusion and inactive-seat handling. Next implement persistent idle preferences/power keys, broader application/audio/call inhibitors, actual suspend/wake reliability, and authenticated package installation/session-failure recovery; persistent authentication-failure and permanent configuration-failure policy remain outstanding. Keep production lock enablement behind those actual device gates. [Evidence and limits](IMPLEMENTATION_STATUS.md#session-lock-protocol-foundation)

**Exit criteria:** from a fresh installation, use touch to connect to Wi-Fi, launch a native app, enter text, switch and stack apps, receive and act on a notification, change audio/brightness, lock, suspend, resume, and authenticate. Repeat with service disconnects and rejected authorization. Killing the locker leaves the session protected; compositor failure returns to a protected startup path. No test requires root access for normal shell use.

Call this a **shell alpha**, with a documented list of unsupported apps/features and remaining platform faults. It is not yet a tested replacement for every phone function.

## M5 Add phone capabilities

**Dependency:** M4 and a recorded stock-platform modem/carrier baseline.

- Implement Rust Phone, Messages, and Contacts applications as separate processes using shared egui components and typed service interfaces.
- Phone: SIM/PIN and registration state, dial, incoming call, answer/reject/hang up, mute, speaker/earpiece routing, DTMF, call history, and proximity behavior. Begin with one active call.
- Messages: send/receive ordinary SMS, multipart handling, pending/failure states, deduplication, Unicode cases, and persistence across UI and service restarts. Record modem delivery-report capabilities without promising unsupported status guarantees.
- Contacts: local storage, name/number search, basic import/export, duplicate handling, and integration with dialer/message actions. Choose storage and migrations explicitly.
- Validate the existing modem power and call-audio stack, including PipeWire/WirePlumber or compatible control paths and device UCM policy. Modem call state alone does not establish two-way audio.
- Integrate incoming calls and messages with lock privacy, inhibitors, wake, vibration, ringtone, missed events, and notification history. Handle loss of modem/service, unavailable SIM, airplane mode, and network loss without blocking the shell.

**Exit criteria:** on the documented SIM/carrier/firmware configuration, demonstrate outgoing/incoming calls with two-way audio, required route changes and DTMF, SMS in both directions, persistence, and recovery after service reconnect. Exercise at least twenty incoming-call attempts while awake and twenty from suspended states, report every failure, and separate platform failures from Tarot failures. This sample is a regression gate, not a statistical guarantee of carrier reliability.

Emergency dialing policy, modem support, lock-screen access, and carrier constraints need a separate verification record; do not place live emergency calls as routine tests. MMS, multi-call handling, Bluetooth call audio expansion, and account synchronization enter the backlog only after the basic phone path is reliable.

## M6 Validate a repeatable beta

**Dependency:** shell alpha and the selected phone-alpha scope.

- Profile animation, idle work, CPU wakeups, memory, texture churn, launch latency, and power. Optimize measured bottlenecks without weakening input or lock correctness.
- Complete keyboard fidelity and any planned replacement of the temporary keyboard dependency. Expand text/layout coverage based on explicit language priorities.
- Validate shell accessibility through AT-SPI and a real assistive client, including card navigation, notification actions, settings, and authentication. Track limitations in third-party apps separately.
- Expand app compatibility deliberately: XWayland where needed, browser popups, drag/drop, app-specific minimum sizes, and Flatpak with a documented portal/backend strategy. Merely running one Flatpak window is not complete permission or portal integration.
- Implement permission-controlled screenshots/screen sharing only when lock and trust rules are in place. Defer multi-output convergence if it would weaken the single-phone session.
- Complete upgrade/configuration migration, rollback, crash supervision, diagnostic export, resource limits, dependency review, license inventory, and a reproducible release package.
- Run a 24-hour mixed-use soak, repeated app open/close cycles, service restarts, and at least 100 suspend/resume cycles on a recorded baseline. Include UI interaction immediately after resume.

**Exit criteria:** performance and reliability gates below are met or consciously revised with measurements; no unexplained growing resource use; no open lock/input isolation failure; install/update/recovery is repeatable; known limitations and tested configurations accompany the release. Hardware or carrier blockers remain visible and can limit the release claim to shell beta.

## Quality targets and measurement

These are proposed engineering budgets, not measured results. Capture an initial report in M0, refine after M3, and retain a fixed workload for comparisons. Record build type, device temperature, power source, brightness, radios, package versions, and application workload with every device measurement.

| Dimension | Initial target | Measurement |
| --- | --- | --- |
| Animation | At a verified 60 Hz mode, p95 frame processing within 16.7 ms and fewer than 1% missed presentation deadlines in a 60-second card interaction | Presentation/frame timestamps; distinguish CPU preparation, GPU completion, and display deadlines |
| Response | p95 touch-to-visible response below 80 ms for navigation | Input/presentation traces plus camera-based validation when available; submission time alone is insufficient |
| Idle work | Settled shell averages under 1% of one CPU core; no continuous animation/redraw timer | Five-minute idle trace with fixed clock/status behavior and no active clients |
| Shell memory | Initial budget of 200 MiB summed PSS across Tarot shell/helper processes, excluding ordinary apps | Process accounting after repeated use; report GPU allocations separately and bound preview caches initially to 96 MiB |
| Resource lifetime | No monotonic memory, file-descriptor, or GPU-resource growth across 100 repeated launch/close cycles | Compare steady states after settling, not only peak allocations |
| Power | Target no more than 10% additional energy versus the recorded stock-shell workload | Repeated matched runs with equivalent display/radio/app state; report measurement error and suspend behavior separately |
| Recovery | All planned M3 and M6 suspend cycles pass without Tarot-attributable input/display failure | Device scripts and manual post-wake checks; preserve logs from each failure |
| Lock | No normal app frame, input, or capture escapes in the tested lock lifecycle | Instrumented adversarial clients, locker/compositor failure tests, wake tests, and output changes where supported |

Do not convert these into battery-life promises. Ambient conditions, firmware, carrier registration, and upstream drivers can dominate power and reliability. Adjust a budget only with a recorded reason and evidence of its user impact.

## Test strategy

| Layer | Tests that matter | Environment |
| --- | --- | --- |
| Domain model | Navigation, stack membership, focus, close/launch states, notification lifecycle, lock precedence, and stale-event rejection | Fast Rust tests with deterministic time |
| Gestures | Recorded touch sequences, cancellation, multiple contacts, thresholds, rotation, interrupted motion, and exactly one committed action | Synthetic input through the real recognizer |
| UI semantics | Accessible names/actions, keyboard focus, text scale, reduced motion, and meaningful state changes | egui semantic harness; selected reviewed image comparisons |
| Renderer | Blending, clipping, scaling, surface trees, damage, resource lifetime, and context recovery | Actual GLES compositor captures on desktop and phone |
| Protocol integration | Configure/ack, popups, input ownership, text-input, clipboard, activation, buffer release, and crashing clients | Nested compositor plus small protocol clients and real GTK/Qt/egui apps |
| Services | Unavailable services, rejected permissions, delayed/out-of-order responses, restart, burst events, and credential prompts | Mock adapters followed by isolated D-Bus integration tests |
| Device | Touch, keys, orientation, audio routes, brightness, charging, suspend, modem, and thermals | Spare PinePhone Pro with recorded Nura baseline |
| Packaging | Clean install, dependencies, supported service manager, startup, upgrade/migration, and recovery | Matching Nura build/runtime environment |

Use `egui_kittest` for semantic UI tests where it fits. Its wgpu snapshot support is not a substitute for the production GLES path. [egui_kittest documentation](https://github.com/emilk/egui/tree/main/crates/egui_kittest)

Once code exists, CI should run formatting, Clippy for the supported feature configurations, domain/UI tests, and a build of the nested backend. Add AArch64/Nura package builds and protocol integration as those paths land. Run relevant tests on each change; reserve expensive soak and device suites for affected milestones or release candidates. Do not make an indiscriminate all-features build the production configuration.

## Main risks and responses

| Risk | Earliest evidence | Response |
| --- | --- | --- |
| Smithay/egui/glow mismatch or GL state corruption | M0 shared-renderer experiment | Pin compatible versions; compare owned offscreen shell layer versus custom render element before expanding UI |
| Hardware passes desktop tests but fails on Mali or DRM | M0 real-device run, then M3 | Confirm renderer/driver, test direct backend early, preserve stock baseline |
| Gestures interfere with app input | M1 replay and M2 toolkit tests | Reserve gesture region and explicit ownership; cancel correctly instead of delivering contradictory sequences |
| Keyboard looks complete but cannot enter text in apps | M2 external text experiment | Test protocols, composition, focus, and toolkit compatibility before styling a complete keyboard |
| Shell or locker failure reveals protected data | M4 failure tests | Compositor-enforced lock, protected restart, restricted privileged interfaces, and no security claim before tests pass |
| Smooth animations consume phone battery at idle | M0/M3 timing and power reports | Demand-driven frames, cached shell geometry, bounded effects, and background-client policy |
| Calls work but audio or wake does not | Stock baseline in M0 and M5 scenario tests | Treat modem, audio, suspend, and carrier support as distinct subsystems; preserve exact test configuration |
| Latest OS changes break provisioning or packaging | Release/edge review at M0 and upgrades | Versioned baseline, correct boot instructions, matching sysroot, and a tested recovery path |
| UI scope expands into an entire app ecosystem | Milestone scope review | Reuse native Linux apps; stage phone features and keep deferred projects separate |
| Asset/source licensing is unclear | M1 reference/asset inventory | Original implementation/artwork by default; project code uses the selected MIT OR Apache-2.0 license, with upstream licenses and deliberate reuse recorded separately |
| Custom-painted controls are inaccessible | M1 semantic tests and M6 AT-SPI tests | Semantic widgets and alternate controls from the start; verify the custom host integration |

## Initial implementation backlog

These are proposed work items, not completed tasks. Each should become a reviewable change with its result recorded before the next dependent item.

| ID | Work item | Depends on | Done when |
| --- | --- | --- | --- |
| T001 | Record device access, identity, OS, and recovery method | Access to spare phone | Baseline inventory and recovery instructions exist |
| T002 | Provision/update latest supported Nura and capture stock behavior | T001 | Boot, access, package manifest, and hardware baseline are repeatable |
| T003 | Choose candidate compiler/crates/features and build a minimal nested compositor | None | A real xdg client maps and receives input |
| T004 | Add egui GLES shell rendering and isolated damage handling | T003 | Shell and client draw correctly through resize, scale, and repeated texture changes |
| T005 | Deploy the experiment and test direct device input/rendering | T002, T004 | Accelerated rendering and touch work on the phone; recovery remains available |
| T006 | Record M0 measurements and accept/revise architecture decisions | T005 | Dependency/rendering baseline and next estimates are justified |
| T007 | Select visual references and define design tokens | Confirmed UI direction | Eight core screen states and intentional adaptation policy are recorded |
| T008 | Extract the pure shell model and command/event boundaries | T006 | Navigation and lifecycle tests run without graphics or services |
| T009 | Implement gesture arbitration and deterministic motion | T008 | Recorded input covers commitment, cancellation, and interruption |
| T010 | Build home, dock, cards, launcher, and mock notification UI | T007, T009 | The complete prototype navigation sequence is usable |
| T011 | Map real toplevels, popups, dialogs, and focus to cards | T010 | Multiwindow lifecycle and close semantics pass |
| T012 | Implement desktop-entry discovery/launching | T011 | Selected native apps launch with correct metadata and failure reporting |
| T013 | Prove external keyboard/text-input interoperability | T011 | Composed text reaches GTK/Qt fields and shell search correctly |
| T014 | Implement stacks and run the first app compatibility matrix | T011, T012, T013 | M2 behavior is recorded across toolkits |

The first concrete engineering deliverable is **T003 through T005: one real native app and egui shell content composited on the actual PinePhone Pro**, with T001/T002 preparing its platform.

## Open decisions and upkeep

The core product direction, project license, development-device authorization and initial OS/service-manager baseline are settled. Keep the remaining decisions tied to the work they affect:

- Retain the verified USB/SSH access and Nura 26.06/OpenRC/elogind baseline in the device record. Reassess release/channel and boot-chain changes at the next OS upgrade; choose edge only for a demonstrated requirement.
- Project code license is now MIT OR Apache-2.0. Complete the dependency and asset source/license inventory before publishing a release or accepting external contributions.
- Available SIM/carrier and test peer, before modem baseline and M5 acceptance; no assumption of universal carrier compatibility.
- Visual reference emphasis, during T007; choose a coherent classic phone baseline while preserving the owner's broader WebOS/LuneOS intent.
- Development availability, language priorities, and app priorities, as scheduling and M4/M5 scope become concrete.

Keep this plan in the repository. After each milestone, update the decision register, measured budgets, tested package/app versions, known failures, and next work items. A changed architecture should replace obsolete instructions rather than leave two competing recipes. Capture device reports and reviewed screenshots/recordings alongside their commit and environment, using a small artifact set rather than checking in large uncurated logs.
