# Revision 18 native transient checkpoint

These are unmodified actual PinePhone Pro/Panfrost/KMS captures of external diagnostic applications, using GTK 3.24.52 and Qt/PySide 6.11.1. The text is synthetic fixture data. They show the separate native dialog after real touch-keyboard entry and before opening its child popup. [Exact package, binary, fixture and image identities](evidence.json).

- [GTK dialog](gtk-dialog.png): natural-size transient within its parent card; toolkit modality filters the parent's close before its delete callback.
- [Qt dialog](qt-dialog.png): native text, popup and response actions work; visual review exposes an incorrect full-app initial size that still needs correction.

Actual native protocol tests pass first-buffer lifecycle, nested modal focus, reparent/orphan semantics, four-orientation touch/key coordinates and app/overview pixels, plus a transient arriving during real PAM protection. Both toolkit fixtures pass exact touch-keyboard text, child-popup ownership/selection, card retention, overview/return, Cancel/Apply and cooperative close. Qt sends the xdg-dialog modal hint; GTK3 uses toolkit-side modality. These are diagnostic toolkit cases, not a certification of all GTK/Qt applications.

The initial protocol pass and a fresh final toolkit/regression pass use the same audited revision-18 payload. Two failed toolkit attempts remain in ignored local artifacts: an invalid expectation of a toolkit close callback, and a capture taken at 48% launch opacity. The corrected runner checks retained content and waits for full opacity/dock dismissal. No product behavior or OCR-confidence threshold was relaxed to pass these cases.

Quick Launch, rounded corners, three/five-member stacks, real GTK editor Cancel/Save, actual PAM, hardware panel/volume controls and saved restart pass. Final independent audit confirms ordinary UID 10000/VT7, normal default Tarot, hardware keys, diagnostics disabled, exact installed payloads, preserved settings/policies, and no test process/input/RTC alarm. These sessions request zero kernel-suspend cycles. Native popup overflow, Qt natural sizing, broader toolkit/clipboard/accessibility cases and the complete development plan remain open.
