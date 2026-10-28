# Revision 19 native configure and sizing checkpoint

These are unmodified actual PinePhone Pro/Panfrost/KMS captures of GTK 3.24.52 and Qt/PySide 6.11.1 diagnostic widgets. The text is synthetic. [Exact APK, binary, source, fixture, executed-QA and image identities](evidence.json).

- [Qt dialog](qt-dialog.png): the requested 280×240 size is preserved, with its parent visible behind it. Compare the [incorrect full-app dialog in revision 18](../r18/qt-dialog.png).
- [GTK dialog](gtk-dialog.png): the toolkit's natural layout remains actionable. GTK reports a 332×329 widget allocation at Cancel/Apply; that includes toolkit allocation details rather than specifying xdg window geometry.

The compositor now waits for the initial bufferless commit before sending its first focus/layout configure. This gives the client time to supply parent/size hints. An observed null-buffer unmap resets initial-configure state before family focus changes, and a fresh remapping commit starts the next handshake. This accounts for the pinned Smithay role hook running before Tarot's renderer buffer handler. A real configure-history regression fails against revision 18 and passes after the correction, including parented zero-size suggestions and remapping.

All four actual Wayland integration cases pass on the phone as audited UID-10000 clients in the real login cgroup. They include first-buffer lifecycle/callbacks, initial/remap configure history, nested parent/modal/orphan semantics, four-orientation real touch/key coordinates and app/overview pixels, plus a dialog arriving during real PAM protection. Actual toolkit Cancel and Apply check both exact text and native widget height; Qt reports 280×240 for both. Child popup ownership/selection, overview/return and final cooperative close still pass.

Quick Launch, rounded cards, three/five-member stack rendering/browsing, real GTK editor Save/Cancel, hardware controls/PAM and saved restart pass. Independent final audit confirms ordinary default Tarot on VT7, hardware keys enabled, diagnostics disabled, exact loaded payloads, preserved settings/policies, and no fixture process/input/RTC alarm. A read-only fdinfo observation confirms Panfrost. These sessions request zero kernel-suspend cycles.

This does not close M2 or the development plan. Popups extending beyond a fitted transient's own rectangle need an overflow regression and renderer correction. Pre-initial maximize/fullscreen requests and broader window-state policy also need dedicated coverage. Broader toolkit, clipboard, accessibility, hardware endurance and later milestones remain open.
