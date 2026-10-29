# Applying the original Tarot design packet

Accepted October 28, 2026 at the owner’s request. The packet and existing implementation were first committed together in `5ee7671`. The originals under [Tarot_Design_Packet](Tarot_Design_Packet/README.txt) remain the design source. The redundant overview at `docs/Tarot_Visual_Overview.png` was removed; the canonical overview remains inside the packet.

## Authority and interpretation

Use **Nocturne**, with the **Pre 2-era webOS 2.x phone interaction model**. The 31-page PDF, all 25 screen studies, token file, motion notes, engineering handoff, fidelity audit and source manifest have been consumed. The packet is a researched proposal. Its author explicitly did not inspect the existing compositor or phone. It does not certify implemented behavior, measured historical dimensions or a captured firmware build. Keep those evidence boundaries intact.

The precise reference device/firmware build remains an acquisition task. Freeze model, full build identifier, locale, time format, patches, wallpaper, capture provenance and checksums before scoring historical conformance. Pre 3 and LuneOS material can clarify behavior; they cannot silently replace Pre 2 artwork or gestures. The packet’s fictional contacts, messages, signal levels and clock time are examples, not data to show as actual device state.

The current Nura/OpenRC/elogind and PulseAudio baseline is independently observed. The handoff’s WirePlumber recommendation is a future audio-policy evaluation, not authorization to assume PipeWire is installed or to replace a working audio stack while applying artwork. Back, app menus and styled application interiors require client cooperation. Ordinary GTK/Qt/Wayland clients keep their own buffers and controls.

## Implemented foundation

Installed revision 17 applies the packet to running Rust/GLES surfaces rather than displaying its screen mockups:

- Compile shared geometry, motion and gesture constants from an exact snapshot of `tokens/tarot.tokens.json`. The 360 × 720 portrait baseline has a 32-unit status bar, 36-unit exclusively owned gesture strip and 360 × 652 app area. The resting five-position tray occupies `[3,611,354,69]`.
- Preserve app aspect ratio at the proposed 0.66 card scale and 139-unit portrait anchor; constrain the same geometry safely in landscape and above keyboards. Shared native corner masks, depth shadows and rounded input now pass on Panfrost. Shared fan poses and inactive scale now apply to live surfaces; native two-client rotation/touch acceptance passes.
- Load the original Nocturne wallpaper, supplied role icons, Lato regular/bold/italic and reserved Instrument Serif identity font. The shared egui view, native keyboard and trusted locker use the font setup. Application buffers remain independent.
- Render the supplied translucent tray material as a continuous textured mesh, including beneath the transient wave. Keep four editable favorite identities plus fixed Launcher. Ordinary Launcher has no persistent tray and reclaims its vertical space. Favorite editing may show it.
- Use an actual centered local clock and local Calendar icon day, updated together at minute boundaries by the compositor event loop. The derived Calendar background omits the packet’s illustrative date; unknown dates remain blank. Do not substitute the mockup’s 9:41. Unknown service state remains unknown rather than displaying fictional signal or battery readings.
- Seed maximize/minimize/launch/dismiss transitions at 300/350/400/300 ms. Use 10-unit tap slop, 14-unit axis lock, a 420 ms stationary arrangement hold and a 140 ms wave dwell after 24 units of qualifying upward travel. These are proposed calibration values, not historical measurements.
- Preserve established input ownership, cooperative close, authentication and protected-resume boundaries. A second contact, changed view/deck, modal interruption or device transition cancels owned gestures. A design change must not turn an unlock gesture into authentication success.

The update also fixes diagnostic preference isolation: native DRM diagnostic sessions now load the real account’s saved settings. Nested fixtures remain isolated unless given an explicit configuration file. Revision 10’s native Quick Launch replay exposed this bug before reaching the gesture tests; its failed evidence is preserved.

## State-by-state application map

“Foundation” means some live behavior or material is present; it does not mean the screen is visually accepted. Earlier phone evidence stays attached to its exact installed binary. New layout and timing changes need fresh evidence.

| Packet study | Current live implementation | Remaining application / acceptance |
| --- | --- | --- |
| 01 Card view / Nocturne | Wallpaper, shared scale/anchor, live native cards and tray; rounded mask/shadow passes desktop GLES and native Panfrost | Title treatment and continuity comparison |
| 02 Card stack | Real member identity, grouping, extraction and independent close; native-verified fan angles, shared corner/input mask and shadows | Native fan reorder/extraction, lift, gesture calibration and reference comparison |
| 03 Launcher | Native revision-34 component passes named persistent pages/menu/placement; revision-35 PNG/SVG native artwork passes rotation/PAM/favorite/restart checks while retaining role art, three columns, two-line labels and no ordinary tray | XPM/imported-image SVG and live refresh, physical gesture calibration, text enlargement and reference comparison; broader browser input gate remains open |
| 04 Email | Ordinary native mail clients may run in cards | Cooperative Rust app styling/navigation; do not repaint a foreign client’s content |
| 05 Banner | Standard service, replacement/actions/expiry and bottom overlay | Packet bottom well: coherently shrink foreground rather than cover it; compact/accessible heights and arrival transitions |
| 06 Dashboard | Retained live notification actions and settings | Non-modal bottom expansion, 56-unit rows, grouping, media controls, durable history and privacy audit |
| 07 Wave Launcher | Real preview, selection, one release action, cancel, current-app focus and native-verified hold-and-drag favorite organization | Exact curve/lift/label treatment and physical timing calibration |
| 08 Device menu | Actual battery, brightness, rotation, idle, audio and Wi-Fi controls | Packet anchor/material/row rhythm; Bluetooth, authorization prompts and complete submenu treatment |
| 09 App menu | Native card lifecycle/arrangement actions available | Distinct top-left menu and cooperative app actions; unsupported app commands must be absent |
| 10 Just Type | Application search and private shell keyboard | Card-view entry, settings/content/actions/provider result types and explicit external query submission |
| 11 Lock | Real protected compositor boundary and PAM locker | Packet lock composition, drag-to-authentication affordance, counts-only privacy and notifications |
| 12 PIN/authentication | Private masked touch/physical password entry through PAM | Explicit credential policy and packet layout; do not assume the account uses a numeric PIN |
| 13 Incoming call | Not implemented | ModemManager state machine, service failure, urgent presentation, routes and authorized carrier/device tests |
| 14 Active call | Not implemented | Rust call UI, route readiness independent of connection, proximity/inhibitors, hang-up/error handling |
| 15 Settings | Functional shell control pages and versioned atomic preferences | Reusable dimensional light chrome, full Settings app, accessible rows and asynchronous state variants |
| 16 Empty Card view | Live original wallpaper, status and tray without promotional copy | Packet search affordance and final empty-state spacing |
| 17 Heritage | Original assets retained in packet | Theme selection after Nocturne acceptance; identical geometry and semantic action colors |
| 18 Vellum | Original assets retained in packet | Same theme gate; contrast and readability on actual panel |
| 19 Messaging | Not implemented | Rust conversations/editor and ModemManager SMS; contacts, storage, failure/retry and privacy |
| 20 Boot | Secure existing session startup | Original identity mark/wordmark and bounded truthful startup/readiness feedback |
| 21 Calendar | Native app compatibility; illustrative supplied icon | Live local icon date verified on the phone; cooperative Rust calendar and data integration remain |
| 22 Notification rail | Existing Alerts entry opens dashboard | Retained application icon rail above strip, overflow, keyboard/rotation behavior; remove provisional Alerts text only when replacement works |
| 23 Launch pending | Pending identity/feedback and duplicate launch suppression | Packet pending card, real timeout/reconciliation and focus semantics |
| 24 Launch failed | Recoverable message preserves original app/input | Packet failure composition and retry; truthful application-specific explanation |
| 25 Locked incoming call | Not implemented | Urgent trusted call surface without exposing app data or bypassing lock; suspend/wake/carrier tests |

Revision 32 now passes the complete scoped native suite and saved restart with stable keyboard activation, including the existing favorite/card/stack/editor tests. [Current acceptance](recordings/r32/README.md) keeps this runtime foundation separate from design conformance.

## Ordered remaining work

1. The foundation now passes Rust/MSRV/AArch64 checks, actual nested GLES input replays and clean installed-package touch/PAM/keyboard/protected-rotation tests. Keep original attempts and exact sources. A subsequent pixel audit retracts the suspected missing status text: the actual native clock/status regions are identical across captured states. Pixel visibility/comparison checks now guard the host replay. Leave the phone in normal Tarot.
2. Rounded native clipping, shadow and corner selection now pass on the phone. Fan/input transforms and three/five-card browsing now pass on the phone. Actual-frame recordings include launch, typing, minimize, browse, maximize and cooperative dismiss/refusal in a real GTK editor. Compare motion and gestures against the reference; capture overhead prevents frame-pacing conclusions.
3. Continue everyday shell composition: named Launcher pages and native PNG/SVG icon loading now have native component evidence. Complete remaining icon formats/refresh, Just Type, distinct app/device menus and the notification well/rail/dashboard. Establish keyboard-visible and landscape contracts before polish. Reuse light app chrome only for shell-owned or cooperating applications.
4. Complete the phone/trust paths: locker presentation, privacy policy, authorization agents/portals, Bluetooth, ModemManager phone/SMS and audio routes. No active SIM is available; carrier gates remain explicitly pending.
5. Run the packet’s A–L capture suite, motion/gesture calibration, text enlargement, reduced-motion and real-panel review. Only then apply Heritage/Vellum and score cross-theme conformance.

These tasks refine M1–M6 in the [development plan](DEVELOPMENT_PLAN.md); they do not replace platform reliability gates. The latest endurance attempt failed in the kernel’s `fe800000.usb` suspend path before a successful sleep. Preserve that failure and investigate it independently; revision 8’s earlier successful cycles do not certify current binaries.

## Assets and deviations

[Runtime artwork](../crates/tarot-ui/assets/README.md) records derivation and licensing. `scripts/design-assets.py` copies the original wallpaper/fonts/token snapshot and rasterizes editable SVG assets using the supplied fonts. Runtime loading/rendering uses Rust PNG/resvg/egui/GLES; Python, CairoSVG and fontconfig are development tools. Native SVG loading uses bundled Lato without system font discovery. Font OFL and introduced icon-renderer notices are retained in source and installed packages. Historical reference fragments and complete mockup screens are never runtime assets.

Current deviations include immediate reduced-motion settling rather than the proposed 100 ms fade, three-preview fans with logical member-browse steps still awaiting motion calibration, incomplete menu/dashboard/locker styling, fallback artwork for unresolved native icons, and existing text affordances in the gesture strip. CairoSVG filter/shadow matching needs visual review against the packet’s PNGs. These are tracked gaps, not accepted historical substitutions.

Verification outcomes and exact package identity belong in [implementation status](IMPLEMENTATION_STATUS.md). Appearance, behavior and platform adaptation must receive separate acceptance decisions. No complete milestone or historical visual conformance is claimed by this foundation.
