# Revision 37: Just Type

October 29, 2026. This checkpoint applies the packet's Just Type sheet to real shell state. It does not close M1, the broader native-browser reliability gate or the complete development plan. Raw device logs, captures, archives, preference backups and executed-source ledgers remain in ignored private artifacts.

## Behavior

Home and Card view expose Just type; attached-keyboard input there or in Launcher starts a query. The activity retains its origin, uses the shared shell keyboard and leaves named Launcher pages and favorite identities independent. Clear retains focus, keyboard Hide retains the query, and closing returns to the origin. The query is runtime state rather than saved history.

Grouped results use installed applications, existing system controls and retained notifications. Selecting an existing application's result focuses its card. Settings open the corresponding control page. Notification matches respect hidden-preview policy; usable default actions retain exact ID/revision, while other entries open and scroll to the exact dashboard record. A stale held row cannot select a replacement result.

An explicit HTTP(S) address exposes Open website. Selecting DuckDuckGo encodes the entire query as one URL parameter. Enter accepts local results but cannot implicitly submit an unselected external provider. A bounded Rust worker uses standard `xdg-mime` and `gio` helpers, direct arguments and the selected session environment; helper output and submitted queries are not logged. A matching existing handler card can regain focus only while the originating search remains current and unprotected. Successful handoff does not establish page loading. [Contract and remaining sources](../../JUST_TYPE.md).

The light Nocturne sheet includes grouped bands, three-column app results and virtualized application rows. Compact landscape omits the first band and uses 56-unit result rows. Dashboard controls also use a compact layout that leaves the selected record readable above the keyboard. These are implemented adaptations, not measured historical conformance.

Keyboard metadata is formatted into one buffer before writing to the shared child log, reducing fragment interleaving. This is neither a lossless event transport nor a change to native text-input activation or held-contact cancellation. The revision-34 return-key and revision-36 fullscreen browser failures remain open. [Retained input diagnosis](../r36/README.md).

## Host verification

All **195 workspace tests** pass on current Rust and Rust 1.89, with seven opt-in cases ignored. New core tests cover local matching, explicit provider selection, origin/nonpersistence, stale/modal choices, notification privacy/revision and URL bounds/encoding. Real-widget regressions cover retained settings queries and same-coordinate held-result replacement. URI tests verify literal argument handling, rejection of non-web schemes and owned-child deadline/reaping.

Strict host nested-feature lint, AArch64 DRM lint, minimum-compiler AArch64 checking, workspace binary/example builds and 24 Python fixture checks pass. The dependency set remains 446 registry packages; `url` 2.5.8 was already locked and gains a direct core dependency edge. All 350 frozen Rust files and nine Cargo inputs match the packaged sources.

Private-display replays pass portrait scales 1 and 2 and landscape scale 2. They use owned XDG/MIME defaults, a real native client, a real notification service/sender and actual GIO invocation to a finite local URI fixture without web requests. Input covers Unicode and URL delimiter preservation, app/settings/content routes, explicit provider/address handoff, existing-card focus and nonpersisted queries. Separate favorite-organization and named Launcher-page regressions pass. A later stronger test exposes the repeated-target defect below.

An initial host attempt stops before UI because Xvfb is absent from PATH. Another landscape replay has an incorrect observer key; its corrected run completes, but visual review exposes insufficient compact-dashboard content space. The compact layout and actual frame bounds are corrected. A subsequent audit retracts the first portrait dashboard visibility results: OCR recognizes the dimmed search result behind the panel. The gate now excludes that region and retains its 80% confidence threshold. Fresh replays pass all three geometries with the actual dashboard target visible in the second observational capture, at 96% or higher confidence. The first animated frames remain retained; no selection/input is repeated to satisfy this observation gate.

## Installed-session work

The signed package is installed. The first native run passes both five-case keyboard context/remap suites, protocol/transient and GTK/Qt dialogs, terminal/clipboard/controls/fullscreen and protected virtual-key/helper-crash checks. Its Just Type case focuses an existing Reader card, then stops when a fixed settings tap selects the installed Disks application: its description also matches “volume,” placing it before Sound. Later content/URI/rotation/PAM and shell/restart cases are not accepted from that run.

The controller restores normal default Tarot and exact current owner preferences/MIME defaults. Independent audits verify loaded package identities, active ordinary-user VT7, hardware keys, diagnostics/metadata disabled, normal logging, unchanged policies/services, fixture/input/alarm cleanup and a live Panfrost descriptor. The private first-run archive preserves 182 hash-verified sources, SHA-256 `5652a650f35d78ac49f2e312b7a32ae8677aa1c3f3648258f728d841c6441e83`.

A second replay on the unchanged APK locates the actual rendered settings/content rows and scrolls within the results viewport when needed. It keeps the 80% OCR confidence gate, native touch input and semantic result gates. Upright OCR coordinates are transformed back through the actual output orientation/scale. It does not hide matching installed applications or repeat a failed selection. Existing-card focus, Sound selection, default notification actions, stale-held-record cancellation, explicit Unicode/provider/address GIO handoff, all four orientations and real PAM query preservation pass. However, its dashboard visibility result has the same dimmed-background false positive and is retracted pending the stronger native gate.

That run stops at the following favorite-hold capture: the legacy helper opens the active Just Type menu and chooses Clear search, assuming it is the former Search applications entry. The capture correctly shows an empty query rather than a held app tile. The revised helper enters search only when needed, restores its finite fixture query through actual oriented keys after leaving search, and uses the new field/compact-row geometry. It retains original favorite/value/cancellation gates. Remaining shell components and saved restart are not accepted from either revision-37 run.

The second controller and independent normal-session/Panfrost audits again verify exact owner preferences/MIME restoration, loaded payloads and cleanup. Its private archive preserves 186 hash-verified sources, SHA-256 `f6be90626d52ea796e94530b2d4a39d9ad390ddffe301bdec41e1a74d920b205`. A third controller is staged and its 190 source hashes verified, but it is never started: the following product defect is reproduced before its password prompt.

## Repeated target defect

A separate private-display regression first finds the actual dashboard record, drags within its real content viewport until newer entries are visible and the target is absent, closes the dashboard and selects the identical retained search result again. The diagnostic target identity is correct, but four dashboard-only captures fail to find the record. The UI's previous once-per-query/record scroll token suppresses the repeated request. This is a product failure, distinct from the earlier observer/menu mistakes.

Initial probe attempts include a blocked pre-UI local socket, drags outside the content viewport, a transient empty presentation list and an unsuccessful wheel-scroll precondition. Those attempts remain separate from the successful real-drag reproduction. The host readiness predicate now waits for a nonempty presentation list. Revision 38 introduces a fresh selection revision and one bounded UI token, with the same real-drag/reselection sequence added to host and native acceptance.

## Package identities

The signature, unique command providers, ARM/musl payloads, root modes, dependencies, policy/notices and absence of install scripts pass audit. All four Rust payloads differ from revision 36. This package has one clean build; earlier reproducibility results remain historical. Final QA/documentation changes follow source freezing and retain their own source hashes.

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r37.apk` | `f150ba6ff1b90ed7b9e8440ab1f33ac5dfb4ac2bba3f54f59cb9ed7de6d66186` |
| Vendored source archive | `e13dcebea4220555f18d96260e49c8b94bb8e7feb2feefba2f3cbd1e9befd6a8` |
| `tarot` | `437664aa494dff499411629e929f4eb11ca7891eefa82604db11e366d51b58e0` |
| `tarot-session` | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| `tarot-lock` | `1ffaa2c23d8be428cd6f091daeb7a5cef2d68d7b395c4d5c8d862887f35fd44a` |
| `tarot-keyboard` | `af96036c6a606eeb110e6f5aef413562dfe137e42658b19b3329bc3e00663cba` |

## Open gates

Full native acceptance remains **false**. Revision 32 remains the last accepted complete scoped native checkpoint. Current controllers request zero suspend cycles; current-binary endurance and the retained kernel USB suspend failure remain independent work.

Contacts/messages/documents, query-carrying creation actions, optional provider registration, common settings aliases, broader matching/locales, live catalog refresh, semantic accessibility and performance/physical/visual calibration remain open. Menu/notification composition, service/application work, carrier access without an active SIM and all unfinished M0–M6 requirements retain their scope.
