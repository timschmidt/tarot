# Keyboard activation continuity

Revision-32 checkpoint, October 28, 2026. Repeated activation of the same keyboard layout now retains its mapped surface, graphics and frame callback while clearing context-specific input. Host verification, signed native focused and broader replays, saved restart and independent cleanup audits pass. Revision 32 is the last accepted complete scoped native checkpoint. This does not close a complete development-plan milestone.

October 29 follow-up: revision 36 reproduces cancellation of a held fullscreen browser key. Server traces show two enable requests in one committed activation batch arriving about 7 ms after keyboard down; the helper resets the same surface context with the contact held and emits no text action at release. Later fresh keys work. This narrows the observed ordering without establishing the originating toolkit callback or a production fix. The revision-34 return-key failure also remains open. [Recorded failure and limits](recordings/r36/README.md).

## Failure and correction

Revision-50 follow-up: consecutive enables before one commit exposed a separate
pending-state bug in the compositor. Earlier surrounding text, sensitive hints
and terminal purpose could survive the final enable. The dispatcher now clears
the complete pending state at each enable; only later fields are forwarded.
Current/minimum real-protocol replays and rendered terminal/held-reset checks
pass. The installed phone passes terminal-to-normal layout resets, fresh touch
typing, actual PAM and normal-session restoration. This does not change the
keyboard's release-based submission or canceled-contact guard, and it does not
resolve the Firefox overlap. [Failure, source and native scope](recordings/text-enable-reset/README.md).

Two later unchanged-package sessions pass real GTK/Qt fields and the complete
21-return Firefox component. Their metadata contains no held-contact resets,
so the earlier overlap remains open. [Comparison and limits](recordings/text-enable-reset/README.md#unchanged-package-gtkqt-and-firefox-comparison).

Revision 31 destroys and recreates the native keyboard on every input-method activation. Re-enabling an unchanged field therefore temporarily removes its input surface and sends the application through unnecessary resize/configure work. Retained native browser failures include a fresh, focused field with an unchanged value after a key press, and no keyboard down/up delivery to either adjacent generation. A separate deterministic synthetic fixture reproduces the needless replacement: re-enabling the same normal-purpose field changes surface generation 1 to 2.

Revision 32 separates context reset from surface teardown. Repeated activation discards complete queued clicks, cancels egui's held pointer outside all keys, suppresses held touchscreen contacts until release, and resets shift/symbol/terminal selections and surrounding-text state. The new input-method batch supplies its content type and surrounding state. A fresh click can then commit with the new serial. The existing egui context, fonts, graphics and pending frame callback remain usable. [Implementation](../crates/tarot-keyboard/src/main.rs).

Deactivation and real layout-class changes still tear down the surface. The layout class includes terminal controls, numeric layout and secure presentation. Consequently genuine normal/terminal changes retain the compositor's scoped touch-ownership policy: app-owned contacts survive keyboard remapping, keyboard-owned contacts are canceled, and protection/view interruption remains global. [Earlier ownership and first-frame corrections](KEYBOARD_REACTIVATION.md).

The diagnostic helper adds a fixed `context-reset` stage to its existing text-free metadata schema. A context serial can advance without changing surface generation. Normal sessions continue to omit keyboard metadata. No text, coordinates or key identities are added to these records.

## Verification

A real-widget unit test covers both an already delivered held press and complete queued taps. Resetting the context produces no old action, retains the egui context/frame state, and accepts exactly one fresh click. All 163 workspace tests pass on current Rust and Rust 1.89; strict host/ARM and minimum-compiler ARM checks pass. [Unit regression](../crates/tarot-keyboard/src/context_tests.rs).

The extended private Wayland/EGL terminal replay re-enables a real field while a keyboard pointer press is held. It requires the same surface generation, a context reset, no teardown, no stale character and a fresh character afterward. Existing terminal shortcuts, purpose/hidden-state resets and landscape behavior also pass. Protected virtual-key dispatch and helper-crash cleanup pass independently. [Rendered replay](../tests/terminal_keyboard_replay.py).

Native acceptance uses the unchanged audited synthetic fixture for same-layout contexts and a separate purpose-cycling fixture for actual remaps. Both require app-contact/multitouch survival, canceled held keyboard input with no stale commit, fresh input, protected interruption and real PAM restoration. The same-layout regression fails deterministically on installed revision 31 before the correction and passes all five cases on revision 32 with identical fixture and replay bytes. All five actual-remap cases also pass with the explicit purpose-cycling fixture. The signed focused Firefox replay passes 21 card returns, queued taps, exact text, masked password, navigation/history, popup close and fullscreen Unicode entry through real PAM. Its 1,300 helper records match the text-free schema, including 70 context resets and no missing-placement warnings. Independent normal-session and hardware-rendering audits pass. Exact originals and sources remain in ignored local evidence. [Native replay](../tests/phone_keyboard_reactivation_replay.py).

## Browser target geometry

Independent observation also proves a replay defect: keyboard mapping or an application buffer commit can change geometry while target pixels are being analyzed. The replay previously accepted that stale capture. The target helper now requires the application's committed dimensions and presented pose to match the intended region, then rechecks geometry after pixel analysis. Changed captures are discarded and recaptured; repeated invalidation fails without supplying a tap target. Tests cover remapping, pending native size, fullscreen reservation, rotation and exhausted retries. [Target acquisition](../tests/browser_target.py) · [Regressions](../tests/test_browser_target.py).

This capture correction alone did not fix native text delivery: a subsequent revision-31 run still loses a first character in a focused field. The activation-continuity correction therefore has its own production and native gates. Typing speed, the existing 15-second exact-text deadline, expected strings, browser sandbox and PAM boundaries remain unchanged.

The broader run also passes all 13 native component results and saved restart. Both independent audits verify normal default Tarot, exact loaded payloads, diagnostics/metadata off, restored preferences/policies, absent fixture/input/alarm leftovers and hardware rendering. The broader helper trace contains 11,215 fixed-schema records, 107 context resets and no missing-placement warnings. Both controllers request zero suspend cycles. [Aggregate acceptance](recordings/r32/README.md). The next visual/interaction priority is the packet’s named Launcher pages. Current-binary suspend endurance, physical interaction/visual calibration, broader applications and unavailable carrier/SIM tests remain separate requirements.

## Revision 34 follow-up

The Launcher component passes native touch and restart, but the broader revision-34 run fails a browser text check at repeat 19, after 19 successful card returns and both five-case context/remap components. The observer stays fresh, the original field stays focused, and the required final character is missing. Text-free metadata shows an activation/context reset between keyboard touch down and touch up, with no text action and no surface-generation change. There are no missing-placement warnings. This matches the current cancellation boundary; the origin/timing of that activation remains to be investigated. Do not remove the held-input protection or substitute a typing delay/retry for an input correction. Original evidence and the unchanged exact-text gate are retained.

Normal Tarot, original preferences and policies, diagnostics off and Panfrost are independently verified afterward. Revision 32 remains the last accepted complete scoped native checkpoint; the new page component does not certify broader browser reliability. [Revision-34 scope and identities](recordings/r34/README.md).
