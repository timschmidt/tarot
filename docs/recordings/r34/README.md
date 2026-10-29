# Revision 34: named Launcher pages, component acceptance

The signed package is installed. The native Launcher component and its saved restart pass. The last accepted complete scoped native checkpoint remains [revision 32](../r32/README.md): the revision-34 broader run fails its browser text check during repeat 19. No complete milestone, complete current native suite, historical visual conformance or current-binary endurance is claimed.

## Implemented and verified

Launcher pages retain numeric page IDs, desktop-entry identities, a discovery inbox and independent favorite references. The top-left menu manages names/order/removal and opens search/favorite editing. Horizontal page swipes and vertical scrolling have separate ownership; icon holds support same-page placement and edge-dwell movement. Names use the real shell keyboard, including landscape controls above the keyboard. Version-4 preferences migrate old schemas lazily and preserve other settings.

All 179 workspace tests pass on current Rust and Rust 1.89. Strict host/AArch64 checks, minimum-compiler ARM checking and the actual private-display/GLES page and landscape favorite replays pass. The source archive contains 342 Rust files and 417 locked registry packages; compiled Rust/Cargo inputs match the frozen sources. Package and signature audits pass; PAM policy and installed font notices remain unchanged. No new runtime dependencies are added.

Native touch passes creation, same-page reorder, cross-page placement, immediate menu interruption, all four orientations and portrait return, vertical scroll ownership, landscape name editing, rename Cancel/Enter, page reorder/removal, non-reused identity, moving a real favorite's original catalog entry and focusing its existing native card. Cancel cases cover an outside edge preview, second contact, protected interruption with real PAM and rotation. The fresh login verifies the same saved page identities, order, name, selected page and favorite references. The controller and independent audits verify exact original preferences/ownership/mode, removed fixtures/input/alarm, normal default Tarot, diagnostics/metadata off, hardware keys and Panfrost. These are automated native touches; physical-finger calibration and panel comparison remain pending.

## Retained failure and correction

Revision 33 fails a drop that returns near its original press coordinates after moving across a page edge. The gesture mistakes final proximity for a stationary hold. The same sequence fails on the desktop and in a real egui regression. Revision 34 retains movement history and passes the byte-identical native page replay. Its only payload change from revision 33 is the compositor. A separate revision-33 controller cleanup race is retained; independent recovery restores the exact original state and normal Tarot, with audits passing. The controller now tolerates an exiting zombie before waiting for the greeter.

## Broader input failure remains open

The broader revision-34 attempt passes both five-case keyboard context/remap components and 19 successful browser card returns, then fails on repeat 19 before reaching the remaining components. The focused synthetic field remains fresh and focused with its original five-character value instead of the required six. There are no observer transport failures or missing-placement warnings. Text-free helper metadata shows a context reset between touch down and touch up, with no text action; surface generation stays unchanged. The cause/timing of that activation still needs investigation. The current held-input cancellation boundary is retained; the replay does not retry the lost character, change expected text or extend its exact-state deadline.

That failed attempt also restores normal Tarot and original state, with independent normal-session/Panfrost audits passing. A separate shell-component run passes all 12 recorded components and saved restart: context/remap, protocol/transients, GTK/Qt dialogs, terminal/clipboard/controls/fullscreen, protected virtual-key/helper-crash checks, favorites, Quick Launch, rounded cards, stacks, real editor close and hardware keys. Its controller explicitly retains `full_native_acceptance = false`; the browser result is not replaced by that narrower run. [Context contract](../../KEYBOARD_CONTEXT.md) · [Launcher contract](../../LAUNCHER_PAGES.md).

## Identities and evidence

| Artifact | SHA-256 |
| --- | --- |
| Signed revision-34 APK | `acbf72d4c6ae2e9a6a73bd9675ff7a3ee9f335713eb12cd34ff60cd3edd0c598` |
| Revision-34 source archive | `41029721881d2f8fefc7f1ba001904d5900699f90013fc8f58cb285baa43c7f3` |
| Compositor | `ad631d7392692c2968b49aac53efc18c93eb531dbafb390a14c4079e0264d308` |
| Session supervisor | `9be2b9ecdfdb9bd299e268e71fe7da5907c7217859d0881039f68374589b957e` |
| Trusted locker | `7a360daee4e94387dc3dd2628be79a7d4fc62f1e9c86ace930deb531686ac6b5` |
| Native keyboard | `52170202f2ed6e558a7763ee81109e9128aa6fa7b33d770aa5583128c8ce9555` |

Raw device/session evidence and executed QA snapshots remain private in ignored artifacts. Original revision-33 failure/recovery archive: `b5c0a9325196bd65d7ace789bd3bd6aed7392e6e096662fad4eb284008842cc3` (124 collected sources). Accepted revision-34 Launcher component archive: `9159462097543b7bf1d6c39adc945fefd52b312e8412a320b0f4271d614d9c50` (131 sources). Failed broader revision-34 archive: `21d42e6fe92d06893ee2f90253aafc86c00158af79373a6329870b3d26c24c22` (132 sources). Passing shell-component archive: `137161810a78d4d506bad486d2195542c7cb3bbd2530a602bdef464de6b0a666` (136 sources), including independent normal-session/Panfrost audits. All current controllers request zero kernel-suspend cycles. The existing kernel USB suspend failure, broader compatibility, native icons/Just Type/menu/notification composition, accessibility, visual/gesture calibration, carrier access and all remaining M0–M6 gates stay open.
