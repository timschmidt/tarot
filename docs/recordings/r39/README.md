# Revision 39: app and device menus

October 29, 2026. The installed development package adds shared shell menus following the original Nocturne packet. Fourteen scoped shell components pass in the first native run; a separate focused Launcher-page and saved-restart run passes on the unchanged binaries. Browser and complete milestone acceptance remain open. Raw captures, logs, preference backups, source ledgers and archives remain private in ignored artifacts. [Interaction contract](../../MENUS.md) · [Research and remaining delivery](../../MENUS_PLAN.md).

The left title opens an activity menu and the right status area opens the device root. A root switches or dismisses without stacking independent overlays. The bounded panels share dark gradient material, rounded borders, shadows, wrapping touch rows and scroll areas. Launcher page commands now use a submenu hierarchy; Just Type retains Clear and Close. Ordinary native apps expose explicitly labeled card controls with cooperative close. Device controls use observed date/battery/network/audio state and the existing asynchronous services, including a release-committed brightness slider.

Menu choices carry typed targets and revision identities. Owner replacement, stale held input and service changes cannot reuse an old row or brightness drag. Dismissal restores eligible input only for its retained owner. Device controls remain usable after an underlying app exits without refocusing its replacement on dismissal. Protection retires menu state. Arrow selection/Enter applies to menu rows; detail widgets retain egui keyboard handling, and Tab transfers focus to the actual widget.

## Host checks

All **209 workspace tests** pass on current Rust and Rust 1.89. Strict host and AArch64 DRM lint, minimum-compiler AArch64 checking, workspace binary/example builds and all 24 Python fixture tests pass. New tests cover client/menu lifetimes, page identities and persistence, secret cancellation, explicit keyboard selection, stale brightness generations, passive drawing, actual held-row and slider release, outside dismissal, owned search focus, and Tab/Enter on a detail-page Back button.

Rendered menu replays pass portrait scales 1 and 2 and landscape scale 2, using private X/Wayland sessions and actual pointer/keyboard input. Accepting and refusing native clients establish cooperative close, focus retirement/return and client death. A visible Back-button focus outline is required before its actual Enter activation. The complete Launcher page replay passes creation, rename/reorder/removal, exact settings and saved restart through the new hierarchy. Just Type replays pass in portrait and landscape with isolated GIO and notification fixtures.

A first package candidate was preserved and never installed: source review found that menu Enter/arrow interception also covered detail-page widgets. The corrected candidate restricts row handling to the roots and preserves standard widget activation. The first landscape replay also exposed a premature presentation-list read; the correction retains the original fully presented-frame gate. A fixed Tab count and an assumed green focus fill were incorrect replay assumptions; retained captures show egui's actual light outline. Final checks observe that outline above the Back label before pressing Enter.

The recurring URI argument fixture fails once in an early suite, then the original test binary passes twelve process-traced runs and all final suites pass. Its cause remains unestablished. Initial sandbox socket failures are retained separately; unchanged tests pass with private socket access.

## Package identities

The corrected clean build, signature and payload audits pass. All 352 Rust files and nine Cargo inputs match the frozen archive with 446 locked registry packages. The session-launcher payload is unchanged from revision 38; compositor, locker and keyboard payloads change. Policies, notices, four unique command providers, ARM/musl ownership/modes and absence of install scripts pass audit. Subsequent QA and reporting changes retain separate source identities.

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r39.apk` | `de7cd1a06892a07936511c80f104bbcd1be005ea0e5a4baf33331771c6fc560b` |
| Vendored source archive | `081928c383605465f692249af4e3ee31a20560d4b56bd378989d0c0b841ec6c3` |
| `tarot` | `8ef3bb355ccff24613c83d3867f4a4dbe2db152bd149f61c5015d38316301b60` |
| `tarot-session` | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| `tarot-lock` | `e1872f0d0f6a837749be211809103124c941478a6f6b0d32a90feebeea823984` |
| `tarot-keyboard` | `1c44171548b5b23bc62acf27da815c43fdd7cc3a305c644e961999646187c346` |

## Native acceptance

The phone verifies the signature, package hash and all 207 staged source hashes/syntax before installation. The first native run passes **14 scoped shell components**: keyboard context and reactivation, native protocol/transients, GTK/Qt dialogs, terminal/clipboard/controls/fullscreen, protected virtual-key/helper-crash handling, menus, Just Type, favorite drag, Quick Launch, rounded cards, stack matrix, real-editor Cancel/Save and hardware controls. The menu component uses native uinput/libinput touch in all four orientations, actual PAM, sysfs/libpulse readback, and accepting/refusing clients with verified ordinary-user/session ownership. It restores the exact original raw brightness and channel volumes/mute.

That controller then fails its separate page phase at Back recognition after removal; saved restart is not reached. The actual capture shows the control clearly. Original recognition joins the chevron and label as `«Back`, while the helper requires a separate `Back` token. A header-only lookup accepts explicit attached-chevron spellings while retaining the 80% confidence threshold. The exact original capture passes at 88.33% on the host and 88.81% on the phone. No product or package bytes change.

A fresh **focused page and saved-restart controller** verifies all 211 staged source hashes/syntax and the same installed payloads. It preserves the original temporary catalog and requires the full page scenario again: real name entry/Enter, landscape controls, horizontal/vertical ownership, four orientations and return, edge placement without premature persistence, outside/second-contact/rotation/protection cancellation, stable IDs after reorder/removal, a replacement page and moved-favorite activation. The previously failing Back lookup passes at 88.81%. The subsequent authenticated restart retains exact page names/order/identities/placement and the Calculator favorite, with no persisted query. This is separate evidence from the original 14-component run, not a claim that its failed controller passed.

Both controllers restore normal default Tarot. Independent audits verify exact loaded binaries, UID 10000/active local VT7, hardware keys, diagnostics/keyboard metadata disabled, normal logging, unchanged policies/services, exact current preference/MIME bytes and metadata, raw brightness/channel volumes/mute, absent fixture/input/alarm leftovers and a live Panfrost render-device descriptor. All recorded sessions request zero suspend cycles. Representative actual menu, page and landscape-name captures are visually reviewed; calibrated physical/visual approval remains open.

The sealed first-run archive is SHA-256 `a2c8a39b5fcf6eb0c7727fa1372d36ab7c8ee0e73cf90eacd5c876ddd52ca15f` (74,911,969 bytes, 207 locally hash-verified sources). The focused passing archive is `69157d6e6102dfbb86885148fb863e779973fe2cf377fd2b9760f193c777ff2e` (18,143,564 bytes, 211 locally hash-verified sources). Both are private; safe extraction and controller/replay source identities are checked locally.

## Remaining scope

Menu descent/closing motion, brightness-rail contrast/service glyphs, enlarged text and physical packet-G calibration, exported GTK/KDE commands and a cooperating Rust app spike remain open. Broader credential/layout/accessibility coverage remains required. Notification composition, retained browser input failures, device endurance and every unfinished M0–M6 criterion retain their scope. Revision 32 remains the last accepted complete scoped native checkpoint including the browser.
