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
