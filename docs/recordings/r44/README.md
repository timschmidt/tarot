# Revision 44 favorite handoff correction

Revision 43's [retained native run](../r43/README.md) passes ten components, then fails a favorite drop using an obsolete tray coordinate. A separate focused reproduction reveals a product issue: with a retained notification in landscape, a favorite hold hides its keyboard, restores the rail and cancels itself. Both paths need correction before accepting the notification composition.

## Resulting behavior

Compact landscape Just Type now defers banners and the rail through keyboard hiding and drag preparation. The notification button/count remains available. Leaving search restores the ordinary notification well; an explicit dashboard still opens. Entering search or focusing a shell text field dismisses the dashboard before new input begins. Exposed native application input retains its non-modal behavior.

Actual panel, work-area, source/revision and context changes keep their cancellation rules. The fix stabilizes this intentional handoff rather than allowing held actions across a replacement context. Tests cover both retained rail and banner cases, successful favorite placement after keyboard hiding, explicit dashboard interruption and search/field/keyboard/Clear navigation. Phone favorite drop/removal coordinates now account for the actual well height.

## Host evidence

All 233 ordinary workspace tests pass on current Rust and Rust 1.89, with strict host/AArch64 DRM lint and minimum-compiler AArch64 checking. The new real-input favorite replay owns its private display, bus, sender, catalog and settings. With the same QA source, the frozen revision-43 host build fails at the keyboard-visible source hold even after tray coordinates are corrected. The revision-44 host build passes hold/place/reorder/remove, full/fixed/outside cancellation, filtered keyboard source, absence of native click leakage and saved restart. Recognition confidence, timing and deadlines remain unchanged.

The replay source is `c40627497895cfeafc9872b7d1247f35ece1cbfdd86f3bfe2674b32ba4052da3`. The compared compositor binaries are `bc17190a3f2bcfb3ccff08fc293876ea4fa7b27804a8acdcba69b592d000ad73` before and `f42ad009049332f572f3b5054e558aa567a2e5a6df4bde6d8c7bbf8c1c93c543` after. This is software-rendered host evidence, not native device acceptance.

Portrait/landscape Just Type passes local results, explicit URI handoff and repeated dashboard targeting. The real-client notification replay passes 40 requests at each of scales 1 and 2, actual popup/input alignment, trusted keyboard commits, four orientations and return, fullscreen, protected arrival and exact restoration. The signed package is built; native component results and remaining failures are recorded below. Host protection uses the managed protocol probe rather than actual PAM/KMS.

The phone passed an independent normal revision-43 audit before installing the signed revision-44 candidate for acceptance. Native favorite behavior and the remaining shell components/pages/restart now pass in the scopes recorded below; independent restoration audits pass after each retained run. Physical/visual calibration, enlarged text, verified notification artwork, durable history/media, native menu exports, the retained browser activation overlap, kernel USB suspend failure and the unfinished development plan remain open. No complete milestone or current-binary endurance is claimed.

## Signed package and native run

The source archive is `a8b341625b12ff020255d706b93f10fbdd3b33ee08b37db9eb084269f524d861` (446 locked registry packages, all 364 Rust/Cargo inputs verified). The signed APK is `3c26b4cfa5632852957d735aa0cf7b0fb158d5390f7909316ab5251cf40034b6`. Only the compositor changes from revision 43; the other installed program bytes are unchanged.

| Program | SHA-256 |
| --- | --- |
| tarot | `21b7feac6ae51409f71c989c8ea11a3e624070e090d2c41dca44cb0ef5587207` |
| tarot-session | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| tarot-lock | `600740557b0809514829003ff5774b1eb5c920096972ae3f5f944facfb158441` |
| tarot-keyboard | `f13e48bbc0ac0662de7865b29a6f5005c7d7a1d256aa19728b40e192b94f726f` |

Local provider/ELF/notice/PAM/install-script audits pass. The phone verifies the trusted signature, original installed revision-43 payloads and the unchanged native protocol fixture before the transaction. The new controller freezes 546 actual source/audit inputs and requests zero suspend cycles. The first run fails at Firefox. Subsequent unchanged-package runs isolate shell acceptance; each original failure remains separate below.

## First native run: browser failure

Both keyboard context components pass, followed by 21 Firefox card returns, actual PAM, masked password, history and popup checks. The run fails its original fullscreen `café` gate. The final field metadata reports a three-character value with a mismatch state; the observer records 207 heartbeats and zero transport failures.

During the first fullscreen key, two Firefox Enable requests arrive before one Commit/activation. The helper resets 19 ms after key down, with that contact held; release arrives 55 ms after down with zero actions. Surface generation remains 39 while serial changes from 227 to 228. The browser-mapped-to-failure interval has 1,083 fixed-schema metadata records, 52 context resets and one held reset. The browser fixture/replay bytes are identical to the failed revision-36 fixture. This reproduces the retained activation mechanism; it does not establish DOM-field identity or permit forwarding an old held action to a replacement context.

The controller restores normal default revision 44. Independent checks verify actual payloads, policies, preference/MIME restoration, ordinary-user VT7, hardware keys, diagnostics disabled, cleanup and Panfrost. Raw audio/brightness controls were not sampled because menus were not reached; the post-failure audit explicitly leaves that gate unverified and records its own source inside the evidence archive.

The archive is `9383a03541d844c31fa0cd598fb9c67064ea940ba744725530565f2d2550fbbd` (70,127,940 bytes, 1,004 safe members). All 546 frozen source/audit hashes, actual controller/public runner, all 364 compiler inputs, metadata schema and independent normal/GPU evidence verify locally. Controller validation remains false. The notification/favorite/remaining shell cases and pages/restart are not reached in this run.

## Focused native run: favorite passes, Calendar selection fails

The next controller verifies the unchanged package without reinstalling it and freezes 551 source/audit inputs. Ten components pass: keyboard context, keyboard reactivation, native transients, notification well, GTK/Qt dialogs, terminal/clipboard/fullscreen, virtual keyboard/lock, menus, Just Type and favorite dragging. The favorite case includes the corrected portrait drop, landscape keyboard hiding, rotation and actual PAM.

Quick Launch then fails `quick-calendar-added`: its obsolete catalog-index coordinate is inside the tray, with Calendar clipped below the editor viewport. No favorite is added. Later cards/stacks/editor/hardware cases and pages/restart are not reached. Normal default Tarot, exact payloads/policies/preferences/MIME/raw brightness and per-channel audio restoration, cleanup and Panfrost pass independent audits. The verified archive is `43bfd6f86d20bee06b516c0451ac66cad1c75e5e9ecdc01a16d09b63ad80441a` (51,830,793 bytes); all 551 sources and 364 compiler inputs verify. Controller acceptance remains false.

## Native DND-off notification coverage and retained scroll failure

The next QA helper chooses actual visible application text at the existing recognition confidence, before one selection with the original exact favorite-ID gate. Its first native run freezes 556 inputs and passes DND-off notifications and Just Type, but its full-height scan flings Calendar from below the viewport to above it. It then exhausts its bounded search without selecting Calendar. Original captures prove the skipped row; this run does not establish Quick Launch acceptance.

The notification component passes all 40 original wire-fixture inputs through actual ordinary-user Panfrost/KMS, touchscreen/keyboard routing and real PAM. The helper temporarily disables DND while preserving preview policy, observes actual banner, dashboard, rail and hidden states, and restores the original notification policy. Oversized native client/popup input, fullscreen, four orientations and return, trusted keyboard commits and protected arrival/restoration pass. Earlier DND-on runs exercised rail arrival; their `well-banner` filenames alone were not native banner evidence. Physical banner/action usability and reference calibration remain separate gates.

Independent normal/GPU/restoration audits pass, including raw controls. The verified archive is `22b2ef3d595b27dffb52b7f82cad374b459a699d84f30aa063049e5ac80d3a39` (16,334,795 bytes, 772 safe members), with all 556 frozen sources and 364 compiler inputs verified. Controller acceptance remains false; later components/pages/restart remain unreached.

## Visible Calendar selection passes; landscape corner sample fails

The next unchanged-package run uses overlapping application scans and stops scroll momentum before recognition. Reopening the editor returns its favorite controls to the top. Quick Launch passes its complete wave/favorite/failure/calendar/saved-settings component. Calendar is recognized at logical `(53.5, 445.75)` inside the editor; the obsolete target was `y=565`. Exact favorite-ID gates, recognition confidence and component deadlines are preserved.

Notifications and Just Type pass again. Rounded cards pass portrait pixels/input, then fail the landscape top-corner pixel assertion. The smaller preview has radius 3.129 units: the old fixed one-unit inset now samples **inside** the rounded shape. In the retained capture, the actual sampled pixel center has signed distance −0.201 units and blue client pixels. A radius/8 inset samples outside at +0.506 units and yields the clipped background. The renderer bytes are unchanged; this is a geometry-dependent QA sample defect. The original failed run remains failed, and a same-capture analysis alone does not accept all orientations.

Independent exact normal/raw-control/preference/MIME/policy cleanup and Panfrost audits pass. Its verified archive is `e8334dbc4a8578e91831ca95defee78257ef57b69a4155162aa02a1c3f19cb77` (15,462,914 bytes, 759 safe members). All 561 frozen sources and 364 compiler inputs verify. Stack/GTK/hardware/pages/restart remain unreached in this run.

## Final scoped run passes

The fresh run on the unchanged signed package passes all seven enabled components: DND-off notifications, Just Type, Quick Launch, rounded cards, the three/five-card stack matrix, GTK close/unsaved changes and hardware keys. Radius-relative corner samples pass in portrait, both landscape directions, upside down and return to portrait, with the original clipped-corner, inner-client, lower-card, shadow and real-touch gates. Calendar selection passes at the recognized visible control. No rendering or other production bytes change after the signed revision-44 build.

A separate authenticated session passes full named Launcher pages, and another restart verifies the exact saved page configuration and favorite identity, with no stale Just Type query. Normal default Tarot is restored. Independent audits verify actual loaded program hashes, ordinary-user VT7, unchanged PAM/login/sleep policies, exact original preferences/MIME/brightness/per-channel audio/mute, diagnostic/metadata disablement, hardware keys, fixture/socket cleanup and Panfrost ownership.

The sealed archive is `d789ca36bef828fea0ddccde25818a3bb14e273925e5bba6a42e9644c35e3711` (44,748,956 bytes, 1,093 safe members). All 566 source/audit hashes, controller/public runner, 364 source-archive compiler inputs and independent normal/GPU evidence verify locally. The source freeze changes only the rounded-card helper from the preceding ledger. The diagnostic stream contains 580 fixed-schema keyboard metadata records and zero held context resets; browser input is excluded from this run. Native banner and landscape keyboard captures were visually inspected, without making a physical-panel or historical-fidelity claim.

Together with the earlier ten-component prefix on these same installed binaries, all fifteen non-browser shell components now have passing revision-44 evidence. This is coverage across separate runs, not a single combined fifteen-component pass. Original failed runs remain failed. The first revision-44 browser failure, physical/reference and enlarged-text review, native menu exports, current-binary endurance and unfinished M0–M6 gates remain open. No suspend cycles are requested by these controllers.
