# Revision 42: touch release and result scrolling

October 29, 2026. The corrected revision is installed in normal default Tarot. All fourteen scoped native shell components, full Launcher pages and saved restart pass. Independent normal-session/restoration/Panfrost audits pass, and the sealed evidence is verified locally. The complete development-plan goal remains unfinished.

Normal shell touch release now sends the button release followed by `PointerGone`. Previously the retained hover position made the next down jump back from the last release point; egui's immediately draggable scroll background applied that jump in reverse. Each landscape search drag returned to the same offset. The revision-41 captured frames show that reversal, and the isolated egui reproduction reaches only about 50.5 points after four drags. Correct touch departure accumulates 40/80/120/160 points. Mouse hover and native-client touch delivery retain their existing paths.

Favorite and page handlers process departure in event order, preserving a completed release before it. Interrupted input marks departure before synthetic cleanup releases, preventing cleanup from committing a gesture. The application tiles yield ordinary drag sensing to the parent result scroll area while the independent hold engine retains favorite organization. Shell wheel axes are negated for egui's content-movement convention; native client axes retain their convention.

## Host verification

All **222 workspace tests** pass on current Rust and Rust 1.89, with strict host/AArch64 DRM lint, minimum-compiler AArch64 checks and 24 Python fixture tests. Touch tests require constrained results to scroll with the keyboard visible, exact app selection on tap, one persisted favorite drop without launch, repeated drag accumulation, completed page selection/settling and cancellation without an edit. Page tests cover mouse/touch and reduced/normal motion. The existing cancellation and protected-input checks remain intact.

Twelve rendered commands produce **thirteen passing reports**: drag and normal wheel-down with one/three matching apps; portrait/landscape Just Type, favorites and menus; lock/clipboard/crash; recovery handoff and diagnostic-capture failure. The focused replay preserves real nested input, private displays/buses/catalogs, the unchanged 80% recognition threshold, current query/output identities and the exact Sound/Audio selection gate. The verified prior source fails dense dragging and normal wheel-down; exact executed Python source hashes match their new passing counterparts. The native KMS/PAM replay and owner finger review have separate evidence below.

## Preserved failures and setup corrections

The original revision-41 runs and focused diagnostics remain [recorded separately](../r41/README.md). The paced phone helper from the second failed revision-41 run is staged for revision 42 with the same hash, including its original attempts, recognition and selection gates. The menu helper is also unchanged.

The first signed revision-42 candidate is withheld after genuine release-plus-departure tests expose canceled favorite drops and page swipes. Initial tests used mouse-style releases and missed this integration. The identical stronger tests fail before ordered handling and all 54 isolated UI tests pass afterward. Their private evidence seal is `b86c0f9ded660368631463a7065d5231e6a42b72c661903be4ca04fb9abdb107` (49,125 bytes). The rejected signed APK is `320fbfc2559854e1f319ae1d6a4137bf57beeb2d304e6b7578fb2777eb7ced51`, with source `cdcceb7c5c3358b3dfb24325bffbe72ca63d798a8c15b14da6ef4e21cdbac40c`. It was never installed. The corrected second candidate has distinct source/package identities below.

An initial private test warm-up hid the keyboard; corrected tests require it before and after scrolling. Baseline isolation also retained two mistakes: integration-test compilation had replaced the debug executable before the first nominal old-binary copy, and rebuilding another checkout in the shared Cargo target left an old UI library in a later candidate build. Those reports do not count as current acceptance. All 364 prior compiler inputs match the frozen revision-41 manifest. Cleaning the local crates removes old paths and restores the exact initial candidate hash before fresh validation. The first provenance archive is `f61dde69d6a86aacb7115bebdf40cf8cf750eeabf0b504ff0258157013256e1f` (35,989,442 bytes). Raw logs/captures, binaries, backups and ledgers remain private in ignored artifacts.

## Corrected package and native gate

The corrected second source archive is `ba5b66c8f060cee04746e6aca400513df29c6699de1e0751d8161cf46b318f61`, with **355 Rust files, nine Cargo inputs and 446 vendored registry packages**. All 364 compiler inputs match the frozen manifest. The corrected signed package is `2c81826bba4104677c08602555908c4986f87f3d08c1ad45232c1518800f790b`. Payload/ABI/policy/notice audits and independent signature verification pass. The compositor, locker and keyboard change because they share input cleanup; the session launcher is byte-identical to revision 41.

| Payload | SHA-256 |
| --- | --- |
| `tarot` | `dc6e59945662a057899aac5b67f20a0ce95edb65127e9febbd7a6ca7af30aa5b` |
| `tarot-session` | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| `tarot-lock` | `572992bfe990ce8dcea2cd32e275c0c5c376796f78c5c746a92ffd7af9216dc1` |
| `tarot-keyboard` | `fa4dbca70a29932040e75fe1cf9d536d1c39b36b47e694dd0481714ed8b96bf0` |

The native controller requires all fourteen scoped shell components, both held-key directions/fresh balanced input, full Launcher pages and saved restart. All 255 staged source hashes, the actual runner's match to the public `run` syntax tree, trusted package signature, QA probe/source and prior installed payloads pass preflight. The builder is stopped. The controller passes all fourteen components plus full pages and saved restart. The native Just Type component passes normal, left, inverted, right and normal-return orientations, including the retained rotated Sound selection failure. Both held-key directions, fresh zero-held input, completed favorite organization, Quick Launch, card/stack actions, cooperative close and hardware keys pass.

Independent audits verify ordinary-user active VT7, exact loaded payloads/policies/services, exact current preference/MIME/raw brightness/per-channel audio restoration, temporary-entry/input/alarm cleanup, diagnostics/metadata disabled, hardware keys and Panfrost. The normal session is the persisted default; Phosh is absent. The private archive is `47e72e91de22bc429120ca3d0afc8a5e7660389db6430e6fc0eee8e343a3d2e1` (77,094,738 bytes, 1,057 safe members, all 255 source hashes verified). Actual controller/wrapper identities, the public runner syntax tree, unchanged paced search/menu helper hashes and all 364 compiler inputs match. The left-landscape result is visibly exposed after three drags and passes the original selection gate.

The owner also confirms that repeated up/down result scrolling in landscape and choosing Sound work with a physical finger. This is qualitative confirmation of that interaction; broader physical/timing/accessibility calibration remains open.

Browser return/fullscreen input, current-binary endurance, physical/timing calibration and all unfinished milestones remain open. These controllers request zero suspend cycles and exclude the retained browser failure; revision 32 remains the last accepted complete scoped native checkpoint including the browser.

## Unchanged-package browser follow-up

A fresh focused controller on the same package passes 21 card returns, queued taps, exact text and Backspace, actual PAM, password/history, popup input/cooperative close and fullscreen Unicode. Firefox ESR is `140.12.0-r0`, GTK3 is `3.24.52-r0` and mobile-config-firefox is `5.4.1-r0`. The browser fixture/replay and canonical public runner body remain unchanged. The original input timing, 15-second exact-text deadline and sandbox remain in force. Browser-side protocol/IME logging is disabled; fixed server/helper metadata is enabled.

The observer reports 159 heartbeats, zero transport failures and a maximum receipt-minus-browser timestamp of 783.6 ms. One changed-geometry target is discarded and recaptured. All 1,137 keyboard metadata records match their fixed schema; 54 context resets occur with zero contacts. This run does not reproduce the held-contact overlap from revision 36 and does not establish its correction. No browser finger test is claimed.

The controller restores the owner's current settings/MIME state and exact raw brightness/per-channel audio. Independent normal-default-session, ordinary-user/policy/payload/cleanup and Panfrost checks pass. No package reinstall occurs. The private archive is `41a465ae5bb9e28faf9428fc8018d0baafa24db9761537c75bb8d39a6551ff4f` (63,180,049 bytes, 692 safe members). All 260 staged source hashes, actual controller/wrapper identities, public runner syntax tree and all 364 frozen compiler inputs verify locally. The earlier 255 sources are unchanged. This is a focused comparison; the prior failures and broader acceptance gates remain open.

### Combined browser and shell comparison

A separate controller on the unchanged package passes the browser and all fourteen shell components in one session, followed by full Launcher pages and saved restart. Both keyboard context suites precede Firefox. Browser input still uses the exact fixture/replay bytes from the failed revision-36 run, with 21 returns, real PAM, queued taps, password/history/popup/fullscreen gates and browser-side protocol/IME logging disabled. Search/menu helpers, exact values, touch timing, confidence thresholds and original deadlines are unchanged.

The browser observer reports 192 heartbeats, zero transport failures and a maximum receipt-minus-browser timestamp of 3586.5 ms. All 11,429 keyboard metadata records match their schema and account for every emitted record. Restricting them to the browser's recorded mapped-to-closed interval yields 1,099 records and 53 context resets, none with held contacts. Thus this second pass also does not exercise the retained failure's overlap. These numbers do not establish frame pacing or a production activation fix.

Independent normal-session, exact preference/MIME/raw-control/policy/payload restoration and Panfrost audits pass. The package is not reinstalled; normal default Tarot has diagnostics disabled and hardware keys enabled. The archive is `fba0e7956e26620b67427f48fe0bdb0255d269764f45767bbad79cca6614bce9` (153,109,853 bytes, 1,479 safe members). All 265 staged source hashes, actual controller/wrapper and canonical public runner body, all 364 frozen compiler inputs, component results and persisted page settings verify locally. The previous 260 staged sources remain unchanged. Captures of fullscreen Unicode entry and linked page contents are inspected; raw evidence stays private. Zero suspend cycles and no browser finger test are requested. Full native acceptance remains false, and no complete milestone closes.
