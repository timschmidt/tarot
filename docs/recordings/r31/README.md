# Revision 31 keyboard component checkpoint

October 28, 2026. This report contains aggregate development results. Original device/session diagnostics, screenshots, controllers, source ledgers and exact execution evidence remain in ignored local artifacts. This is a verified component checkpoint with a retained broader-run failure. Revision 26 remains the last accepted complete native checkpoint; no M0–M6 milestone closes here.

## Corrected behavior

Keyboard layer teardown now cancels only a keyboard-owned implicit touch grab. Ordinary app contacts survive remapping; protection/view interruption retains global cancellation. A second correction supplies committed-tree coordinates for a newly mapped visible keyboard before its first render, restricted to its registered private subtree. App/card transforms still require presented geometry. [Contract](../../KEYBOARD_REACTIVATION.md) · [Touch ownership](../../../src/keyboard.rs) · [Coordinate conversion](../../../src/focus.rs).

The same synthetic fixture fails on revision 29 and passes all five ownership cases on revision 30. Revision 30's subsequent browser replay passes 21 card returns but loses the first fullscreen character; a missing-placement warning and touch-up without touch-down identify that separate gap. The original failures are preserved locally.

The renderer-free real Wayland regression maps SHM root/subsurface buffers through configure/ack/commit. Before the correction, all expected keyboard conversions are absent. Afterward, root, child and held-outside coordinates match, while a same-client sibling, hidden keyboard and protected input are rejected. [Regression source](../../../src/focus_keyboard_tests.rs).

## Validation

| Check | Result |
| --- | --- |
| Workspace tests, current Rust and Rust 1.89 | 162 pass on each compiler |
| Strict host and ARM checks; minimum-compiler ARM check | Pass |
| Private terminal, protected virtual-key and helper-crash suites | Pass |
| Clean signed ARM package build | Pass |
| Native touch ownership cases | All five pass |
| Focused Firefox replay | Pass, including 21 card-return cycles |
| Subsequent broader native replay | Fails reopening Firefox's keyboard after its first card return |
| Independent cleanup audits after both runs | Pass |

The focused Firefox replay covers queued taps, exact text, actual PAM, masked password, navigation/history, popup input/cooperative close and fullscreen Unicode entry above the keyboard. Input comes through the native keyboard, without WebDriver. The observer reports zero transport failures; 1,171 helper records match the text-free schema and no missing-placement warning occurs. [Synthetic ownership replay](../../../tests/phone_keyboard_reactivation_replay.py) · [Browser replay](../../../tests/phone_browser_replay.py).

In the broader run, the field briefly activates/enables/remaps, then Firefox blurs it and deactivates the keyboard. DOM observations are fresh and no placement warning occurs. The remaining activation/resize sequence has no established cause. The failure is retained rather than replaced by a passing retry. The full protocol/toolkit/terminal/favorite/card/editor/hardware suite and saved restart still require a complete passing run on the current package.

Both controllers restore normal default Tarot. Independent audits verify exact loaded payloads, ordinary-user operation, diagnostics/keyboard metadata off, hardware keys, original settings and policies, no fixture/input/alarm leftovers and hardware rendering. Only the compositor payload changes from revision 30; the launcher, locker, keyboard helper and PAM policy remain unchanged. All 337 frozen Rust files and Cargo manifests match the installed build inputs. Final QA/publication documentation follows source freezing; registry dependencies remain the same 417 pinned packages.

Both runs request zero suspend cycles. No new physical-finger, visual-conformance, current-binary endurance, carrier/SIM, media/permissions/portal or broad application acceptance is claimed. Earlier suspend and favorite results retain their original binary-specific scope.

The next gate is diagnosing and correcting the remaining card-return activation failure with correlated touch/configure/focus evidence, followed by the complete native replay and saved restart. Keep exact text deadlines, input timing, original failures and independent cleanup audits.
