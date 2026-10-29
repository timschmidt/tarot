# Revision 28 favorite organization and queued native keyboard input

The signed revision-28 package passes the favorite/shell **component** replay and saved restart. Full native/browser acceptance remains open after repeated password-field DOM mismatches. Revision 26 remains the last accepted complete native replay. [Exact source/package/QA identities and original archives](evidence.json) accompany the [interaction contract](../../FAVORITE_DRAG.md).

The component run uses native uinput/libinput, Panfrost/KMS and actual PAM. All 28 favorite holds pass: insertion/reordering, full/fixed rejection, explicit removal preserving the running Calculator card, second-contact/protection/rotation cancellation, four orientations and return, and a filtered source above the landscape keyboard. Existing Quick Launch, rounded-card, stack-matrix, real editor Cancel/Save and hardware-key checks pass, followed by saved restart. The [exact collected controller](native-component-controller.py) explicitly records `full_native_acceptance = false`; it is historical QA source tied to the audited device/staging layout.

These are unmodified captures from the passing component run. Landscape frames retain the raw 720×1440 panel orientation on disk:

- [Insert preview](favorite-add-beta-preview.png) shows the held source and proposed placement before preference bytes change.
- [Dock reorder preview](favorite-dock-reorder-preview.png) shows the same identity moving within the tray.
- [Remove a running app’s shortcut](favorite-remove-running-shortcut-target.png) retains the native Calculator card.
- [Landscape filtered search](favorite-landscape-filtered-search.png) leaves a compact result above the keyboard; [drag preview](favorite-landscape-keyboard-source-preview.png) hides that keyboard and reveals the tray.
- [Return after PAM](favorite-after-pam.png) confirms the interrupted drag left favorites unchanged.

A separate real-widget/native regression proves revision 27 collapsed queued `caf` taps to `f`. Revision 28 takes one completed tap per egui frame and preserves subsequent contacts, page changes and one-shot modifiers. The native deliberately queued burst passes on revision 28. All 161 ordinary tests pass on current Rust and Rust 1.89, strict host/ARM checks and minimum-compiler ARM checking pass, and six relevant private-display suites pass on the final host binaries.

Earlier revision-28 attempts separately pass the browser/protocol/toolkit/terminal/virtual-key cases. They are **separate evidence on identical installed binary hashes**, not sections of the passing component run. Other attempts expose stale HTTP observations, stale toolkit bounds, incorrect search-caret placement and rotation dismissing old shell search focus. Corrected QA preserves exact-value gates and actual input timing. Seven phone observer checks pass; the optional Node queue check is skipped there, while all eight pass on the host.

The [retained password discrepancy](browser-observer-discrepancy.png) shows six masked characters while the last received report says four. A later independent heartbeat sees an unexpected DOM value with zero reported transport failures. The image alone does not establish the final DOM value. Full browser/IME acceptance remains open; transport hardening and the queued-tap fix do not establish that all input failures share one cause.

Independent final [session](normal-session-audit.py) and [GPU](gpu-audit.py) audits pass: normal default Tarot on UID 10000/VT7, exact loaded hashes, hardware keys, diagnostics disabled, original preference bytes/owner/group/mode and policies, absent fixture processes/input/RTC alarm, and a live Panfrost descriptor. Both component sessions request zero suspend cycles. The evidence reconciles all 335 frozen Rust files and 411 build inputs, 36 repository QA/support files, the byte-identical installed replay, and 15 separately collected support sources. Final QA/documentation follows source freezing; the complete current Git tree is not asserted byte-identical to the source archive.

Original failed outputs and executed sources remain under ignored `artifacts/m1/favorite-drag-r27/` and `artifacts/m1/favorite-drag-r28/`; public hashes preserve their identities. No complete M0–M6 milestone, current-binary endurance, full visual conformance or new physical-finger acceptance is claimed.

## Browser observer follow-up

Six later focused trials use these same installed binaries. [Exact archive/source identities and scoped results](browser-observer-evidence.json) preserve every passing and failing result under ignored `artifacts/m2/browser-ime-r28/`. The [unmodified failure frame](browser-after-pam-observer-stale.png) visibly contains `tarotxz`, although that trial's latest HTTP report still says `tarotx` and is 17.3 seconds old. A stale value cannot establish that the native key was lost.

| Trial | Observer / diagnostic scope | Result |
| --- | --- | --- |
| 1 | HTTP; protocol metadata | Post-PAM exact-state timeout despite browser commit receipt; no causal serial conclusion |
| 2 | HTTP; protocol and focus metadata | Complete focused browser case passes |
| 3 | HTTP; protocol, focus and filtered IME metadata | Post-PAM timeout; final received DOM snapshot predates unlock |
| 4 | HTTP with three-second deadlines; same metadata | Three counted delivery failures; expected text visible while report remains stale |
| 5 | Persistent WebSocket; same metadata | Complete focused browser case passes; 116 heartbeats, maximum reported reconnect counter 1, maximum delay 1118.8 ms |
| 6 | Default persistent WebSocket; protocol/IME logging disabled | Overview-return input fails with fresh unchanged text, report age 366.4 ms, document focus true and zero transport failures |

The [exact final focused controller](native-browser-focused-controller.py) explicitly records full native acceptance as false. Independent [normal-session](browser-normal-session-audit.py) and [GPU](browser-gpu-audit.py) audits pass after each trial: normal default Tarot, UID 10000/VT7, exact loaded payloads, hardware keys, diagnostics disabled, original preferences/policies, absent fixtures/input/RTC alarm and Panfrost. No package is reinstalled; all 335 frozen Rust files remain unchanged. The final collected installed-session replay and nine QA/support files match the repository byte-for-byte. These historical scripts reference their original audited staging paths.

Sixteen host observer checks pass; the phone passes thirteen and skips three optional Node producer checks. The WebSocket is the default QA observer, with bounded HTTP retained for comparison. Text-free diagnostics add focus, composition and numeric selection metadata; strict filters discard raw protocol/IME strings and keys. Input timing, 15-second exact-value checks, PAM and sandbox boundaries remain intact. The fresh failure requires investigating native keyboard touch/frame/activation delivery before another complete native replay. These focused results do not close broader browser coverage, current-binary endurance or the complete development plan.
