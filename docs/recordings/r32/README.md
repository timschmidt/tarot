# Revision 32 activation-continuity checkpoint

October 28, 2026. Signed native focused and broader replays, saved restart and independent cleanup audits pass. Revision 32 is the last accepted complete scoped native checkpoint. No complete development-plan milestone is closed. Raw device/session evidence and exact executed-source archives remain in ignored local artifacts; this public report contains aggregate results only.

The native keyboard retains its surface across repeated activation of an unchanged layout while canceling stale held/queued input. Real layout changes still remap and retain scoped touch cancellation. [Implementation contract and regressions](../../KEYBOARD_CONTEXT.md).

| Check | Result |
| --- | --- |
| Current/minimum Rust workspace tests | 163 pass on each compiler |
| Strict host/ARM and minimum-compiler ARM checks | Pass |
| Rendered terminal context reset and existing controls | Pass |
| Protected virtual-key and helper-crash suites | Pass |
| Browser target/observer checks | 19 host checks pass; phone runs 19 with three optional Node skips |
| Native same-layout contexts | Five cases pass; identical fixture/replay bytes fail before the fix |
| Native actual layout remapping | Five cases pass with explicit normal/terminal purpose cycling |
| Focused Firefox | 21 card returns, exact input, real PAM, password/history/popup and fullscreen Unicode pass |
| Focused normal-session and hardware-rendering cleanup | Pass |
| Broader native suite and saved restart | All 13 component results and restart pass |
| Broader normal-session/hardware-rendering cleanup | Pass |

The focused helper's 1,300 records match the fixed text-free schema, including 70 context resets and no missing-placement warnings. Normal sessions omit metadata. Both compiler and package source records reconcile all 338 frozen Rust files. Only the keyboard-helper payload changes from revision 31; compositor, launcher, locker and PAM policy remain unchanged. Registry dependencies remain the same 417 pinned packages.

The original revision-31 failure, observation-only geometry run, geometry-guard-only failure and deterministic same-context failure remain intact locally. The geometry guard prevents accepting captures invalidated by mapping or a pending client resize; its correction alone did not fix lost native input. Typing speed, the 15-second exact-text deadline, expected strings and authentication boundaries are unchanged.

No current-binary suspend endurance, manual physical interaction/visual calibration, carrier/SIM, broad application or full M0–M6 acceptance is claimed. Earlier checkpoints retain their original binary-specific scope.

The broader suite includes 47 native protocol input requests, GTK/Qt, terminal/clipboard/controls/fullscreen, nested virtual-key protection/crash, 28 favorite holds across orientations, Quick Launch, rounded cards, stack matrix, real editor Cancel/Save and hardware keys. It recaptures one invalidated browser target. Its 11,215 helper records include 107 context resets and no missing-placement warnings. Both focused and broader controllers request zero suspend cycles. The phone ends in normal default Tarot with metadata/diagnostics disabled, exact payloads and original preferences/policies restored. The next visual/interaction priority is named Launcher pages from the original packet.
