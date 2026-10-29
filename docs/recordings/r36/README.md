# Revision 36: input ordering diagnosis and recorder timing

October 29, 2026. This checkpoint adds diagnostic visibility and removes measured recorder interference with held gestures. It does not fix the intermittent native browser failure or close a development-plan milestone. Raw logs, captures, device archives, preference backups and QA source ledgers stay in ignored private artifacts.

## Changes

Installed `--diagnostics` sessions enable fixed DEBUG targets for focused-client text-input enable/disable/commit receipt, applied activation, ordinary native touch down/up and card-drop targets. Receipt and activation are separate records; the active-object policy can reject a received request. Records contain static categories, numeric IDs/times and booleans. Native contact records are suppressed during protection. Neither request payloads, coordinates, key identities nor text are formatted. Normal sessions retain the exact `tarot=info,smithay=warn` filter and omit keyboard metadata.

The new Smithay metadata patch applies independently alongside the other three local patches to published 0.7.0; the resulting vendored Rust sources match exactly. It changes no activation/reset policy. The compositor additions also leave input and card commands unchanged. [Trace contract](../../BUILDING.md#native-browser-compatibility) · [Patch attribution](../../../third-party/README.md).

The installed replay now defers serialization of its accumulated JSON report while its owned touchscreen has active contacts. It keeps observations in memory for the next released-contact checkpoint and final failure report. This addresses the revision-35 card-test failure: 296 ms serialization plus 19 ms writing prolonged an edge hold past the intentional 350-ms paging dwell. On the same APK, deferral reduces the measured preview-to-next-observation interval from 423 to 105 ms and the original combined replay passes all 13 components. Native gesture timings, exact-text expectations and 15-second deadlines remain unchanged. Observer/capture/scheduler costs still exist. [Retained comparison](../r35/README.md#subsequent-browser-and-recorder-investigation).

## Verification and retained failure

All **185 workspace tests** pass on current Rust and Rust 1.89, with the same seven opt-in cases ignored. Strict host nested-feature lint, AArch64 DRM lint, minimum-compiler AArch64 checking, workspace binary/example builds and 24 Python fixture tests pass. All 347 frozen Rust files and their Cargo inputs match the packaged sources; 446 registry packages remain locked. The signature, command providers, ARM/musl ELF payloads, root modes, policies/notices and absent install scripts pass audit. Only `tarot` and `tarot-session` payloads change from revision 35; locker and keyboard bytes are identical. This package has one clean build; reproducibility evidence from revision 9 remains historical.

The first native run passes both five-case keyboard context/remap components, all 21 browser card returns and subsequent ordinary/password/PAM/history/popup checks, then fails fullscreen bottom-field exact input. Browser-side protocol/IME logging is disabled in this run; the server/helper traces are enabled. The browser observer has zero transport failures and fresh focused reports. Its last report before failure diagnosis is 472 ms old and retains an unrecognized three-codepoint value instead of the required four. Unknown content suppresses the failure capture.

The new trace establishes that the first fullscreen key reaches the helper before two browser enable requests arrive in one committed activation batch. Relative to the compositor's key down: enable receipt follows about 7 ms later; the helper resets context at about 9 ms with one held contact; release follows about 60 ms after down and emits no text action. Surface generation stays 39 while serial advances 228 → 229. Later fresh keys emit text actions. This is a concrete ordering reproduction of held-key cancellation, not proof of the toolkit callback that produced the enable requests or a production correction. The existing stale-input guard remains intact.

The first run records 254 text-input requests, 71 activation boundaries, 122 native downs and 119 native ups. All **566** added trace records match their fixed static/numeric schemas. Separately, 1,269 helper records parse with the existing schema; one earlier helper record interleaves another child's log output and is unparseable. The described failure window is intact, but the shared log is not a guaranteed lossless event transport. Card-drop tracing is not reached because the browser failure stops the combined replay first. Remaining shell components and saved restart are not accepted from this run.

The controller restores the owner's exact current preference bytes, ownership and mode, removes temporary entries and returns to normal default Tarot. Independent audits verify exact loaded revision-36 payloads, ordinary-user active VT7, unchanged policies/services, hardware keys, no diagnostic socket/helper metadata, the normal logging filter, no test inputs/alarms and a live Panfrost descriptor. The private failed-run archive has 169 hash-verified QA sources, SHA-256 `4cfd7f0f1e40b1ae64522babaeb97c729d55e876518c9015d1f354acf630e4a6`.

## Separate shell regression

A subsequent run on the unchanged installed revision-36 package excludes the failed browser component and passes all **12 shell components** plus saved restart: both five-case keyboard context/remap suites; native protocol/transient and GTK/Qt dialogs; terminal/clipboard, controls and fullscreen; virtual-key protection/helper crash; favorite organization; Quick Launch; rounded cards; stack matrix; cooperative GTK editor close; and hardware keys. The trace contains 1,049 text-input requests, 64 activation boundaries, 1,280 native downs, 1,276 native ups and five card drops. All 3,674 new records match their static/numeric schemas, and all five drop previews match their release targets. Separately, 10,108 helper records parse and four are interleaved/unparseable, retaining the shared-log limitation. This is a separate regression result, not a replacement browser pass.

The second controller again restores exact current owner settings and normal Tarot; independent normal-session/Panfrost audits pass with the normal logging filter and helper metadata disabled. Its private archive preserves 173 hash-verified QA sources, SHA-256 `45ddab8d036f9bf2bec9f9d9d3ae5c02d79d5d6cf587e27f34fe48415cb180b2`.

## Package identities

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r36.apk` | `221cf50208a6884dea1be3d3e88d6cee630d4be3aa0a4c18759a44717947c1d5` |
| Vendored source archive | `3ed9b87e1f3e848de515c5aa70ed94431c0a1fe02da6d44cb81cd57fd4f81078` |
| `tarot` | `f9b10888ee3679f2b7a92e6df97ff0029a5d4103fcc5d7b6497158876873b1e5` |
| `tarot-session` | `434939be79572cd4c82077378730d1ec00279f598fc89bf5d6be4ab457f862bc` |
| `tarot-lock` | `ec4ef0d14be253d066fa1dacffa143f8e760ce95a61de453657d6a6ac79d2ddb` |
| `tarot-keyboard` | `1d71161fe2bc84f0cade0ff5beb59f7991d0eb30724810001d08bd68b1a24a05` |

## Open gates

Full native acceptance remains **false**. Revision 32 remains the last accepted complete scoped native checkpoint. Revision 34's return-key failure and this revision's fullscreen failure remain retained; revision-35 passes do not erase them. Continue the activation-order investigation while preserving field-change cancellation, real native input and the original exact-value gates. [Browser research and limits](../../BROWSER_COMPATIBILITY.md#revision-35-activation-investigation).

These controllers request zero kernel suspend cycles. Current-binary endurance, kernel USB suspend failure, broader compatibility, physical/visual/accessibility review, icon format/refresh gaps, Just Type, menu/notification composition, service/application work, carrier access without an active SIM and the remaining M0–M6 gates stay open.
