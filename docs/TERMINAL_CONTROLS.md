# Touch terminal controls

Installed and native-validated revision 25, October 28, 2026. Tarot remains the default session, with original preferences restored, hardware keys enabled and diagnostics disabled. This extends the [real terminal/clipboard case](TERMINAL_CLIPBOARD.md) and retains the [protected virtual-key boundary](VIRTUAL_KEYBOARD_PROTECTION.md).

## Interaction and routing

An app declaring the text-input terminal purpose gets a 288-unit native keyboard in portrait. The extra row preserves the ordinary letter/symbol key positions and 44-unit terminal control targets. Other text and private-entry layouts remain 240 units. Constrained landscape fits the existing half-output panel bound; its controls can be smaller.

The first row contains Ctrl, Alt, Tab, Escape, Up, Down and More. More opens Home, End, Left, Right and Paste. Ctrl and Alt are visibly selected one-shot modifiers. Selecting them sends no server modifier until an actual key is chosen; a complete press/release chord is followed by a zero-modifier request. Navigation and Enter/Backspace can use those modifiers too. Paste sends the terminal's standard Ctrl+Shift+V shortcut; the helper does not read clipboard data itself.

Unmodified text retains the text-input commit path, including Unicode. Modified printable ASCII maps to the same US keymap published by the helper, with modifier masks derived from that actual keymap. Unsupported modified Unicode emits no partial shortcut and displays “ASCII shortcuts only.” Hide, deactivation and terminal/private purpose changes clear pending modifiers and the tools page. Purpose changes reconfigure the layer even without a new activation. Protected dispatch continues to deny already-bound virtual keys/modifiers.

## Current host evidence

All 145 ordinary tests pass on current and minimum compilers, with strict host/ARM checks. New UI tests exercise actual modifier/page taps, reset, preserved letter positions and constrained Hide reachability. The shortcut test checks all 95 printable ASCII strokes against libxkbcommon's real US keymap, including Control semantics.

The real rendered keyboard replay observes actual Wayland key/modifier events for Ctrl, Alt, Ctrl+Shift, combined modifiers, Tab/Escape/navigation and Paste. Plain text remains on the IME path. Hide/reopen clears selections; live terminal/password/normal purpose changes resize/reset correctly. A landscape Tab reaches the native client. Existing keyboard/card/stack/editor/lock checks pass. One card run initially fails before compositor startup because its Xvfb display-number pipe is closed before the terminating newline; the complete handshake correction passes on the same compiled compositor, with the failed run retained.

## Helper lifetime

Revision 24 was built and signed but never installed. A new crash replay found that a trusted virtual keyboard could leave Control active in an ordinary client after SIGKILL. Merely restoring the physical keymap was insufficient: Smithay deduplicates identical keymap text, so the reset was skipped when both keyboards used US layout.

The correction tracks the actual virtual-keyboard protocol object and focus epoch separately from the keymap hash. Source changes send the corresponding modifier state even when the map text is identical. Destroying the active helper restores actual physical modifiers and releases its held virtual keys only for the original focus epoch, without releasing a physical hold of the same key. Inactive objects cannot replace another source's modifiers. The crash replay holds a real protocol key plus Control, kills only the verified helper, observes release and reset without another key, and verifies managed replacement. The protected-input regression remains required.

## Device acceptance

The installed package passes touch-only real foot/ash cursor editing, Home/End, history, Ctrl+A/E, Ctrl+C against an actual foreground `/bin/sleep` child, exact raw PTY Alt/Escape bytes and Tab completion. A verified SIGKILL of the actual native keyboard triggers automatic replacement, restores terminal geometry and clears pending Ctrl/tools before fresh Tab completion. GTK ↔ foot Unicode/newline clipboard exchange uses the new touch Paste key, with no physical-keyboard fixture. Pending Ctrl/tools selections clear through actual PAM, followed by fresh completion/typing.

A separate fullscreen foot case reserves the 288-unit panel, negotiates 360×396, restores 720 when hidden and retains the same shell through real overview/card activation and fresh PAM. Existing GTK3/Qt6 fullscreen fields, all five protocol cases/40 requests, shell/card/stack/editor/hardware checks, saved restart and independent normal-session/Panfrost cleanup also pass.

The first fullscreen post-command capture omits the keyboard despite model/functional success. That image and its executed QA remain immutable. A fresh no-reinstall replay waits for the post-command viewport and checks five actual key-background pixels at launch, after reopening and after PAM/command completion. All three captures pass on their first attempts. No product change occurs between these controllers. [Curated captures and exact identities](recordings/r25/README.md) retain the distinction.

All 332 packaged Rust files match the frozen source; 27 collected QA/support files match the final repository. The final replay function matches the device-executed visual replay, with final QA improvements and documentation recorded separately from the frozen package. The signed clean build passes after moving disposable compilation output off an exhausted host filesystem and making completion logging independent of a host wrapper. Both native controllers complete cleanup and normal-session restoration. Every replay requests zero suspend cycles; no new physical-finger result is claimed.

Other terminal bindings/applications, clipboard formats/source lifetime, international shortcuts/layouts, broader purpose/focus/landscape constraints, accessibility, current-binary endurance and all remaining M0–M6 gates remain open.

## Replaying the cases

The host rendered case is `python3 tests/terminal_keyboard_replay.py --xvfb /path/to/Xvfb --output /new/private/output`. The helper-death case runs `tests/virtual_keyboard_crash_replay.py` through `tests/with_xvfb.py`; it accepts explicit `--compositor`, `--text-probe` and `--parent-wayland` paths for installed-binary nested evidence. Build the two probe examples separately: root examples are excluded from the release source archive, and their binaries/source hashes are audited independently.

For native application acceptance, stage `phone_terminal_controls_replay.py`, `phone_terminal_clipboard_replay.py`, `phone_fullscreen_terminal_replay.py` and their existing support modules alongside the installed-session runner. Use the [real terminal/clipboard fixtures](TERMINAL_CLIPBOARD.md), adding `test.tarot.TerminalFullscreen.desktop` with foot's `--fullscreen` flag and a separate private `terminal-fullscreen` directory. `--terminal-clipboard --cycles 0` includes the real controls, helper replacement, clipboard and fullscreen case; `--fullscreen-terminal --cycles 0` isolates the pixel-checked fullscreen case. Keep output paths fresh and backup/restore original settings and temporary desktop entries. The native protocol wrapper additionally runs both the lock boundary and held-key/helper-death cases against audited revision-25 probes inside the real ordinary-user login. Their synthetic protocol evidence remains separate from physical KMS and real PAM.
