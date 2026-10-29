# Revision 38: repeated Just Type targets and settings aliases

October 29, 2026. This builds on the initial [revision-37 Just Type implementation](../r37/README.md). It corrects a reproduced dashboard navigation defect and expands the real-input acceptance gates. It does not fix the retained native-browser failures or close the complete development plan. Raw logs, captures, archives, preference backups and QA source ledgers remain private in ignored artifacts.

## Correction

Revision 37 scrolls to a retained notification only once per query/record identity. After manually scrolling away, closing the dashboard and selecting that identical search result again, the correct diagnostic ID remains selected but the visible list does not return to it. The original actual-input regression retains its initial visible target, newer entries after real viewport drags and four failed reselection captures.

Each validated selection now advances a separate selection revision. The dashboard stores one UI token containing query revision, selection revision and notification ID/revision, and requests scrolling once for that token. Choosing the same result again therefore requests another scroll; ongoing manual scrolling remains free. Invalid/stale choices cannot advance it. The token uses one persistent egui ID rather than accumulating an ID for every query/record combination. Queries, selection state and tokens remain ephemeral.

Settings share one matcher between model and rendering. Common aliases include wifi/wlan, mute, rotate/brightness, dnd and charging, and open the existing corresponding controls. Backend availability and errors remain authoritative. Contacts/messages/documents, query-carrying creation actions, provider registration, broader matching/locales and accessibility/performance work remain unfinished. [Full Just Type contract](../../JUST_TYPE.md).

## Host verification

All **196 workspace tests** pass on current Rust and Rust 1.89, with seven opt-in cases ignored. Strict host nested-feature lint, AArch64 DRM lint, minimum-compiler AArch64 checking, workspace binary/example builds and 24 Python fixture checks pass. The alias test checks real model routing; the existing notification test also verifies that stale selection cannot advance the new revision.

The exact retained failing revision-37 real-drag/reselection replay passes on the corrected compositor. The expanded canonical replay also passes portrait scales 1 and 2 and landscape scale 2. It first finds the buried record in the dashboard, drags within that panel until newer entries are visible and the target is absent, closes it and selects the same query/notification identity again. A new selection revision and actual rendered target reappearance are both required. All final reselection captures recognize the actual record at 96% or higher confidence.

Dashboard OCR now excludes the dimmed matching search row behind the panel. The first portrait revision-37 visibility claims are explicitly retracted; fresh stronger runs on that unchanged version establish initial visibility while retaining the real repeat-selection failure. Four bounded observational captures allow scroll animation to settle without repeating input or lowering the 80% confidence gate. Host readiness also waits for a nonempty presentation list before accessing its first native window.

One initial current-compiler URI argument fixture invocation returns failure. Its unchanged focused follow-up, twelve bounded process-traced checks on the original test binary and the subsequent complete workspace suite pass. The original failure remains retained; its cause is unestablished. Successful real GIO/URI replays are independent behavior evidence, not a diagnosis of that transient fixture failure.

## Package and native validation

The clean build and signature/payload audits pass. The source snapshot contains 350 Rust files, nine Cargo inputs and 446 locked registry packages. All Rust/Cargo inputs match the frozen manifest. ARM/musl binaries, four unique command providers, root modes, runtime dependencies, policy/notices and absence of install scripts pass audit. Compositor, locker and keyboard payloads change from revision 37; session-launcher bytes are identical. One clean build is recorded; earlier reproducibility evidence remains historical. Final QA/documentation changes follow freezing and retain separate source identities.

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r38.apk` | `bdc96ba8285117936b6bf34b783ccda8d7be69092636654c156c7f0f780cd456` |
| Vendored source archive | `3f868b425e544d2238ea9e3f8ce64956997e8d27b7527aa9e3bb8c36ba153c7f` |
| `tarot` | `d0374d3e043626dbc60e765cf56602bb2adfadab596856ab9ab63d0ea84c4496` |
| `tarot-session` | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| `tarot-lock` | `d0a75971c010c4cd9b332bbe70a03cb95f1d6760b535bd420ec5d9d8e27c41f6` |
| `tarot-keyboard` | `3b9654b0b13bc8ff800dc83f4412e94ff6d3f37f18d858c6b510f4d9346b079e` |

The phone verifies the package signature and all 197 staged QA-source hashes before its first controller installs revision 38. Both five-case keyboard suites, native protocol/transients, GTK/Qt dialogs, terminal/clipboard/controls/fullscreen and protected virtual-key/helper-crash components pass. Initial dashboard-only visibility and actual manual scrolling pass, then the new same-record reselection assertion fails after the notification sender expires.

The selected green search band exposes a native OCR-preprocessing mismatch: the shared dialog helper's threshold merges that background and its dark text. Seven recognition attempts delay selection until the notification probe reaches its default 120-second lifetime; orphaning changes the retained record's revision. The assertion correctly rejects that changed identity. This is not an accepted native reproduction of a remaining product scroll defect. URI/rotation/PAM and later shell/restart cases are not accepted from that run.

The controller restores normal default Tarot and exact owner preferences/MIME settings. Independent audits verify exact loaded payloads, ordinary-user active VT7, hardware keys, diagnostics/metadata disabled, normal logging, unchanged policies/services, fixture/input/alarm cleanup and Panfrost. Its private archive preserves 197 hash-verified QA sources, SHA-256 `c871610cd4aba72caa5a3747c6555a12bafc9c49a29ce13f8b1fce5a2d36c033`.

Read-only reprocessing of those same phone captures with a dark-text threshold recognizes every required search-row word at 87–92% confidence, including the selected Archive row at 89.4%, using the phone's Tesseract 5.5.2. The native helper now preserves both original/upright images and a separate recognition raster. The 80% confidence threshold and semantic identity gates remain unchanged. Both owned URI-fixture children receive an explicit matching 240-second lifetime, rather than the notification sender expiring earlier than its Reader. Production deadlines are unchanged; the updated fixture passes another actual-input/GIO host replay.

A fresh controller on the unchanged installed APK verifies 201 staged source hashes and passes all **13 scoped shell components** plus saved restart: both five-case keyboard context/remap suites; protocol/transients and GTK/Qt dialogs; terminal/clipboard, controls and fullscreen; virtual-key protection/helper crash; Just Type; favorite organization; Quick Launch; rounded cards; three/five-card stack navigation; cooperative GTK editor Cancel/Save; and hardware keys. The retained browser component is excluded and its failure gate remains open.

Just Type passes existing-card focus, Sound selection, default notification actions, held-replacement cancellation, dashboard-only visibility and repeated selection of the same record, Unicode/provider/address GIO handoff, four orientations and return, and real PAM query preservation. The 162.4-second component completes within both fixture children's matched 240-second lifetime. The selected search row is recognized immediately on reselection, with no scrolling attempts; actual dashboard target visibility reaches 92% confidence. Favorite organization passes all 28 native holds and its orientation/landscape/PAM cases. Fresh startup retains saved favorites and contains no active search or saved query.

The controller restores normal default Tarot and the owner's exact current preferences/MIME defaults. Independent normal-session and Panfrost audits pass again, including loaded hashes, ordinary-user active VT7, hardware keys, diagnostics/metadata off, normal logging, unchanged policies/services and fixture/input/alarm cleanup. The complete private archive has 201 locally hash-verified QA sources, SHA-256 `bca636cdea013ec4e83b021de261f65efe6212d3d12c5fe71786d7d7cd64bd47`. Winning native captures are visually reviewed; raw evidence remains private.

After normal restoration, the owner qualitatively confirms that search entry, typing, result selection and returning feel usable on the phone. This does not establish calibrated gesture timing, text enlargement or historical visual conformance.

The native replay requires dashboard-only OCR and the same actual drag/reselection gate, alongside settings/content/URI paths, all orientations and real PAM query preservation. Its legacy favorite helper now respects the Just Type menu, restores finite fixture queries through actual oriented keys after leaving search, and uses the new field/compact-row geometry. These changes preserve original gesture/cancellation and exact-value gates; they do not mutate search text through diagnostics.

## Open gates

Full native acceptance remains **false**. Revision 32 remains the last accepted complete scoped native checkpoint. Retained revision-34 return-key and revision-36 fullscreen failures require a separate native-input correction. Kernel USB suspend failure, current-binary endurance, physical/visual/accessibility calibration, remaining shell/service/application work, carrier access without an active SIM and all unfinished M0–M6 gates stay open.
