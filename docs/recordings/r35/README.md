# Revision 35: native application artwork

October 29, 2026. This is a component checkpoint toward the complete Tarot development plan. No complete milestone or current-binary endurance is accepted here. Raw captures, device/session logs, preference backups and source ledgers remain in ignored local artifacts; this report contains aggregate results.

## Behavior

Launcher, Quick Launch and a lifted application icon now use installed PNG/SVG artwork when the original Nocturne role mapping does not apply. Theme lookup runs on a dedicated Rust worker with inheritance, size/scale selection, hicolor and absolute-path support. A bounded session cache retains completed/missing results and releases evicted GPU copies. File access and native decoding stay outside UI evaluation. Late artwork preserves active drag identity, and ignored requests cannot close menus or change focus.

The renderer is pinned to resvg/usvg 0.48.1 with Rust text shaping and bundled Lato. SVG image imports, SVGZ, system font discovery and optional raster-image decoders are disabled. Introduced dependency notices are installed. XPM/imported-image SVG, live theme/catalog refresh and runtime scale changes remain open. The explicit limits are resource guards, not whole-process memory or hostile-document guarantees. [Contract and limits](../../LAUNCHER_PAGES.md#native-artwork-foundation).

## Verification

- All **185 workspace tests** pass on current Rust and Rust 1.89; the same seven opt-in cases remain ignored. Strict host linting, AArch64 DRM linting, minimum-compiler ARM checking, 24 Python fixture tests and workspace binary/example builds pass. The host lint uses the nested feature set; DRM checks use the phone SDK.
- Decoder regressions verify aspect/alpha behavior, malformed/oversized inputs and image-import rejection. Private rendered replays verify inherited/hicolor/absolute SVG, scale-specific PNG, misses/import rejection, lifted image and saved Quick Launch pixels at scales 1 and 2. The final scale-2 replay, named-page/menu/keyboard/restart replay and landscape favorite/keyboard/restart replay pass.
- One final-binary scale-2 replay times out on a compositor status request during a held icon while release compilation is active. Its earlier pixel checks pass. The unchanged replay passes after the builder stops. The original failure is retained; its cause is unproven, and the later pass does not erase it.
- The installed phone icon component and saved restart pass. Native uinput/libinput touch verifies PNG, SVG and the installed themed Calculator icon; portrait, left, inverted and right orientations plus portrait return; lifted artwork and persisted Quick Launch placement; fresh real PAM; and the original native Calculator card afterward. Native cache observations contain five ready rasters, two cached misses, zero pending requests and 269,120 raster bytes. Three color checks produce 1,624 / 3,136 / 1,624 logical pixel matches in each orientation.
- The controller restores the owner's exact current settings bytes, ownership and mode, removes all temporary desktop entries and returns to normal default Tarot. Independent audits verify ordinary-user VT7 operation, exact loaded payloads, diagnostics/keyboard metadata off, hardware keys, original policies, default greeter choice, services, absence of test inputs/alarms and Panfrost ownership.
- A separate broader revision-35 shell run passes all 12 recorded components and saved restart: both five-case keyboard context/remap components; native protocol/transient and GTK/Qt dialogs; terminal/clipboard with touch controls and fullscreen; virtual-key protection/helper crash; favorite organization; Quick Launch; rounded cards; stack matrix; GTK cooperative close; and hardware keys. Its controller also restores the exact owner settings and normal Tarot; independent normal-session/Panfrost audits pass. Browser checks are deliberately excluded after the retained revision-34 failure, and full native acceptance remains false.

These are synthesized touchscreen and rendered-pixel checks. Physical icon review, representative visual comparison, accessibility and matched resource/frame-pacing measurements remain open. The owner separately confirmed revision-34 page/control usability; that statement does not claim revision-35 icon review.

The private icon component archive contains 143 hash-verified QA sources (SHA-256 `2bd154971463240a71adf8d596bd4017fb36ff17c552c521f3b0106db7050414`). The separate shell archive contains 148 (SHA-256 `ce246c2a1d503a9a21b5af6ef0cf76cc1011dce22c15eeb8ba5ae7c55b205e65`). Both preserve their exact controllers, source ledgers, run evidence and independent post-run audits.

## Package identities

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r35.apk` | `6190c19e48cd9c8b1691af6741ee9356f7d31ee366fb8895e4db7f073fe8fdfd` |
| Vendored source archive | `e1f81fab4f082f3d7c15368fe9bed2fa554f69ea7badeb50061a59f057d8f9e0` |
| `tarot` | `6d293ce5cf8eee9a0952f9249acccd8357dd8cc702e189b5437d3718d28023b3` |
| `tarot-session` | `60d430bd4a3826cc0b62780946d94dbe61a7ea8f64dd0bfea225a8f40f24484a` |
| `tarot-lock` | `ec4ef0d14be253d066fa1dacffa143f8e760ce95a61de453657d6a6ac79d2ddb` |
| `tarot-keyboard` | `1d71161fe2bc84f0cade0ff5beb59f7991d0eb30724810001d08bd68b1a24a05` |

All four payloads differ from revision 34. All 347 frozen Rust files and Cargo inputs match the package sources; the source contains 446 locked registry packages. The four unique revision-35 command providers, AArch64/musl binaries, root modes, policy/notices and absent install scripts pass audit. Signing verification uses the existing private QA key directory; global package trust and session policies are unchanged. The package was built once from a clean source tree; revision-9 reproducibility remains historical evidence.

## Retained gates

### Subsequent browser and recorder investigation

Three additional runs retain the exact revision-35 installed payloads and the original browser gesture, first-key and 15-second exact-value checks:

| Run | Result | Private archive SHA-256 / verified QA sources |
| --- | --- | --- |
| Browser first, protocol and filtered IME metadata | All 21 returns, later text/password/history/popup/fullscreen/PAM checks and saved restart pass | `82023cbaeb0c1a5cf5d36fd129da0557626a1055716406c3a704407350625298` / 153 |
| Both keyboard components before browser, protocol metadata only | Browser and ten components pass; stack matrix fails at the second join | `9973f21ff3ec7b78153e7cfac56c051e4358ed89b7abec11eb3065c25983addd` / 157 |
| Same combined replay, deferred held-contact checkpoints | All 13 components, 21 browser returns and saved restart pass | `6b5fef6723d3d46fced8f0dd091e7c8c87c1873399f8e12b19a88100ebee14c5` / 162 |

The second run exposes recorder interference with intentional card edge paging. Its accumulated observation report is 4,906,437 bytes: serialization takes 296 ms and writing another 19 ms while the synthetic finger remains at the left edge. The next observation begins 423 ms after the target preview, crossing the gesture's 350-ms edge dwell. The deck pans and release joins the card to Calculator instead of the previewed test stack. The failed result remains retained.

The third run changes only the wrapper's checkpoint scheduling: observations stay in memory while its owned touchscreen has active contacts, then the next released/final checkpoint writes them. The same target-to-next-observation interval is 105 ms, and all three probe cards join their intended stack while Calculator stays separate. The runtime, card interaction policy, browser replay, input intervals and acceptance expectations are unchanged. Small timing-journal writes, observer work and scheduling still affect execution; this change removes the measured large checkpoint cost, not all observer effects.

All three controllers restore the owner's exact current settings and normal default Tarot. Independent normal-session/Panfrost audits pass. Collection excludes only the isolated Firefox profile's verified lock symlink; it does not follow that link. Every archived QA source is hash-verified.

Passing follow-ups do **not** explain or resolve revision 34's held-key cancellation. In the first passing sample, no keyboard context reset overlaps a held contact; last-reset-to-down margins for the 21 return keys are 149–227 ms. The combined runs retain the preceding ten keyboard context/remap cases. None establishes production reliability or current-binary endurance, and full native acceptance remains false.

Full native acceptance remains **false**. Revision 32 remains the last accepted complete scoped native checkpoint. Revision 34's broader run loses a browser character on repeat 19 after 19 successful returns; a same-generation context reset cancels that held key. This revision does not change the activation policy or weaken its stale-input guard. [Original failure and limits](../r34/README.md).

Current controllers request zero kernel suspend cycles. The kernel USB suspend failure, broader compatibility, current-binary endurance, remaining M0–M6 requirements and carrier tests without an active SIM remain open. Continue native input diagnosis, Just Type, menu/notification composition, visual/accessibility comparison and device/service/application work under the full development plan.
