# Applying the original Tarot design packet

Accepted October 28, 2026 at the owner’s request. The packet and existing implementation were first committed together in `5ee7671`. The originals under [Tarot_Design_Packet](Tarot_Design_Packet/README.txt) remain the design source. The redundant overview at `docs/Tarot_Visual_Overview.png` was removed; the canonical overview remains inside the packet.

## Authority and interpretation

Use **Nocturne**, with the **Pre 2-era webOS 2.x phone interaction model**. The 31-page PDF, all 25 screen studies, token file, motion notes, engineering handoff, fidelity audit and source manifest have been consumed. The packet is a researched proposal. Its author explicitly did not inspect the existing compositor or phone. It does not certify implemented behavior, measured historical dimensions or a captured firmware build. Keep those evidence boundaries intact.

The precise reference device/firmware build remains an acquisition task. Freeze model, full build identifier, locale, time format, patches, wallpaper, capture provenance and checksums before scoring historical conformance. Pre 3 and LuneOS material can clarify behavior; they cannot silently replace Pre 2 artwork or gestures. The packet’s fictional contacts, messages, signal levels and clock time are examples, not data to show as actual device state.

The current Nura/OpenRC/elogind and PulseAudio baseline is independently observed. The handoff’s WirePlumber recommendation is a future audio-policy evaluation, not authorization to assume PipeWire is installed or to replace a working audio stack while applying artwork. Back, app menus and styled application interiors require client cooperation. Ordinary GTK/Qt/Wayland clients keep their own buffers and controls.

## Implemented foundation

Installed revision 17 applies the packet to running Rust/GLES surfaces rather than displaying its screen mockups:

- Compile shared geometry, motion and gesture constants from an exact snapshot of `tokens/tarot.tokens.json`. The 360 × 720 portrait baseline has a 32-unit status bar, 36-unit exclusively owned gesture strip and 360 × 652 app area. The resting five-position tray occupies `[3,611,354,69]`.
- Preserve app aspect ratio. The packet proposes a 0.66 card scale at a 139-unit portrait anchor; since October 31 cards carry no close or arrange buttons (close is the upward flick; Open, Arrange and Close are in the System Menu), so card view uses 0.73 from y 92, and a stack's position joins the title below the card; constrain the same geometry safely in landscape and above keyboards. Shared native corner masks, depth shadows and rounded input now pass on Panfrost. Shared fan poses and inactive scale now apply to live surfaces; native two-client rotation/touch acceptance passes.
- Load the original Nocturne wallpaper, supplied role icons, Lato regular/bold/italic and reserved Instrument Serif identity font. The shared egui view, native keyboard and trusted locker use the font setup. Application buffers remain independent.
- Render the supplied translucent tray material as a continuous textured mesh, including beneath the transient wave. Keep four editable favorite identities; the fifth position shows the Launcher unless the owner puts an application there (October 31, at the owner's request). The Launcher stays reachable by swiping up on the gesture strip and from the System Menu. Ordinary Launcher has no persistent tray and reclaims its vertical space. Favorite editing may show it.
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
| 05 Banner | Revision-44 shared reserved well and accessible 48-unit banner; native DND-off client/input/keyboard/rotation/PAM component passes | Physical banner/action usability, enlarged text, packet-H/reference comparison and timing calibration |
| 06 Dashboard | Reserved non-modal bottom expansion, 56-unit collapsed rows, real actions/settings and source groups; native component passes | Verified artwork, media integration, durable history, enlarged text and physical/reference/privacy review |
| 07 Wave Launcher | Real preview, selection, one release action, cancel, current-app focus and native-verified hold-and-drag favorite organization | Exact curve/lift/label treatment and physical timing calibration |
| 08 Device menu | Revision-42 anchored root/details, proposed presented-frame motion, vector glyphs and separated rail pass scoped native touch/rotation/PAM, both held-key directions and exact brightness/volume restoration | Enlarged text and physical/timing packet-G calibration; Bluetooth and remaining permissions/submenu work |
| 09 App menu | Revision-42 distinct activity/card controls and page hierarchy, vector selection mark/pinned headings and proposed motion; native accepting/refusing close, owner death, both held-key directions and saved page restart pass | Physical/timing comparison, enlarged text, actual exported actions and cooperating Rust-app spike |
| 10 Just Type | Revision-38 Card/Home/attached-keyboard entry, actual application/settings/retained-notification results, aliases and explicit HTTP(S)/provider actions; rendered and native reselection/URI/orientation/PAM/restart gates pass, with qualitative owner usability | Contacts/messages/documents, creation actions, provider configuration, matching/locales, keyboard/accessibility/performance and reference comparison |
| 11 Lock | Real protected compositor boundary and PAM locker | Packet lock composition, drag-to-authentication affordance, counts-only privacy and notifications |
| 12 PIN/authentication | Private masked touch/physical password entry through PAM | Explicit credential policy and packet layout; do not assume the account uses a numeric PIN |
| 13 Incoming call | Not implemented | ModemManager state machine, service failure, urgent presentation, routes and authorized carrier/device tests |
| 14 Active call | Not implemented | Rust call UI, route readiness independent of connection, proximity/inhibitors, hang-up/error handling |
| 15 Settings | Functional shell control pages and versioned atomic preferences | Reusable dimensional light chrome, full Settings app, accessible rows and asynchronous state variants |
| 16 Empty Card view | Live original wallpaper, status, tray and Just type affordance | Final empty-state spacing and reference comparison |
| 17 Heritage | Original assets retained in packet | Theme selection after Nocturne acceptance; identical geometry and semantic action colors |
| 18 Vellum | Original assets retained in packet | Same theme gate; contrast and readability on actual panel |
| 19 Messaging | Not implemented | Rust conversations/editor and ModemManager SMS; contacts, storage, failure/retry and privacy |
| 20 Boot | Secure existing session startup | Original identity mark/wordmark and bounded truthful startup/readiness feedback |
| 21 Calendar | Native app compatibility; illustrative supplied icon | Live local icon date verified on the phone; cooperative Rust calendar and data integration remain |
| 22 Notification rail | Reserved source rail, bounded overflow and notification button/count; revision-44 native notification component passes | Broader compatibility, verified artwork, text enlargement and physical/reference calibration |
| 23 Launch pending | Pending identity/feedback and duplicate launch suppression | Packet pending card, real timeout/reconciliation and focus semantics |
| 24 Launch failed | Recoverable message preserves original app/input | Packet failure composition and retry; truthful application-specific explanation |
| 25 Locked incoming call | Not implemented | Urgent trusted call surface without exposing app data or bypassing lock; suspend/wake/carrier tests |

Revision 32 established the last accepted complete scoped native checkpoint, including saved restart and favorite/card/stack/editor checks. Later revision-34 return-key and revision-36 fullscreen input failures leave current browser reliability open. Revision-35 follow-ups also expose and correct large QA checkpoints prolonging held card-edge gestures, with an unchanged-runtime 13-component pass afterward. Keep this runtime evidence separate from design conformance. [Current status and limits](IMPLEMENTATION_STATUS.md) · [Input ordering diagnosis](recordings/r36/README.md).

## Ordered remaining work

1. The foundation now passes Rust/MSRV/AArch64 checks, actual nested GLES input replays and clean installed-package touch/PAM/keyboard/protected-rotation tests. Keep original attempts and exact sources. A subsequent pixel audit retracts the suspected missing status text: the actual native clock/status regions are identical across captured states. Pixel visibility/comparison checks now guard the host replay. Leave the phone in normal Tarot.
2. Rounded native clipping, shadow and corner selection now pass on the phone. Fan/input transforms and three/five-card browsing now pass on the phone. Actual-frame recordings include launch, typing, minimize, browse, maximize and cooperative dismiss/refusal in a real GTK editor. Compare motion and gestures against the reference; capture overhead prevents frame-pacing conclusions.
3. Continue everyday shell composition: named Launcher pages, native PNG/SVG icon loading and initial Just Type now have native component evidence. Complete remaining search sources/actions, icon formats/refresh, [distinct app/device menus](MENUS_PLAN.md) and the notification well/rail/dashboard. Establish keyboard-visible and landscape contracts before polish. Reuse light app chrome only for shell-owned or cooperating applications.
4. Complete the phone/trust paths: locker presentation, privacy policy, authorization agents/portals, Bluetooth, ModemManager phone/SMS and audio routes. No active SIM is available; carrier gates remain explicitly pending.
5. Run the packet’s A–L capture suite, motion/gesture calibration, text enlargement, reduced-motion and real-panel review. Only then apply Heritage/Vellum and score cross-theme conformance.

These tasks refine M1–M6 in the [development plan](DEVELOPMENT_PLAN.md); they do not replace platform reliability gates. The latest endurance attempt failed in the kernel’s `fe800000.usb` suspend path before a successful sleep. Preserve that failure and investigate it independently; revision 8’s earlier successful cycles do not certify current binaries.

### Next notification composition sequence

The installed revision-42 dashboard dims and intercepts the foreground, and banners overlay it. Revision-43 development now implements the shared well, accessible banner, source rail and exposed foreground described in [the notification contract](NOTIFICATIONS.md). Host and revision-44 native checks include real client/popup input, keyboard, four orientations and actual PAM/KMS protection. Fifteen non-browser shell components have passing current-package evidence across separate runs; browser and physical/reference acceptance remain open. Studies [05](Tarot_Design_Packet/screens/png/05-banner.png), [06](Tarot_Design_Packet/screens/png/06-dashboard.png) and [22](Tarot_Design_Packet/screens/png/22-notification-rail.png) instead propose a bottom well, usable reduced foreground and retained icon rail. Implement that composition separately from native app-menu adapters:

1. Define one geometry/reservation model for the well, dashboard, rail, keyboard and gesture strip. Arrival, replacement and dismissal must not accumulate insets. Establish portrait/landscape, enlarged-text and keyboard-visible variants before replacing the existing Alerts affordance. Reduced motion must reach the same usable layout.
2. Prove real client content and touch/popup coordinates remain aligned as available space changes. Compare client reconfiguration with the existing presentation transforms, including a client whose minimum size exceeds the remaining area. Choose the bounded fallback from that experiment. Interrupt animation from its presented pose; restoring a prior height must not restore a stale client or input context.
3. Render bounded rail groups and overflow from actual retained notifications, with available verified app artwork or a neutral fallback. Display grouping must not merge action ownership. Keep image decoding and filesystem access off the render loop. Show no fictional app, message, media or service state from the studies.
4. Expand the dashboard upward while leaving the exposed foreground interactive. Notification contacts, underlying app contacts and gesture-strip contacts need distinct owners; closing the dashboard must not deliver its release to the app. Preserve ID/revision action guards, replacement/expiry/disconnection behavior, DND and privacy. Keep earlier history visibly distinct from actionable live items. Media controls require a real service adapter.
5. Exercise simultaneous arrival, replacement while pressed, overflow/scrolling, expiry, app death, menus, search, keyboard remap, real PAM and protection during motion. Require foreground interaction, no unintended app action on dismissal, exact restoration and native four-orientation evidence. Then capture packet-H before physical/visual acceptance; this does not certify historical fidelity or device endurance.

Implement the geometry across [core notification hit testing](../crates/tarot-core/src/notifications.rs), [native work areas](../src/window_state.rs), [shell focus/routing](../src/state.rs) and [notification rendering](../crates/tarot-ui/src/notifications.rs) together. Changing only the scrim would leave modal input/focus rules behind. The public notification snapshot now has a bounded opaque source identity for rail grouping, with neutral fallback artwork. Verified application artwork remains open; keep display grouping separate from service action authorization.

## Design audit, October 31

At the owner's request, rendered host captures of every shell surface were compared with the packet's studies, tokens and control states. The shell surfaces (status bar, tray, launcher, top-level menus, Just Type results) were close; everything one level deeper used egui's default styling. Applied:

- **Controls.** egui buttons paint `weak_bg_fill`, so the menu's intended fills never appeared. The theme now gives every button the packet's dark control material, the pressed teal (49,83,77) and a light edge for keyboard focus. Checkboxes became the packet's ON/OFF switch (`tarot-ui/src/controls.rs`). Service pages have normal spacing and lost their redundant second back buttons.
- **Glyphs.** Back, close, chevron, Bluetooth and search glyphs from `glyphs/` replace text characters, with accessible names. The status bar draws Wi-Fi and battery glyphs from observed state and nothing for unknown state. The volume and brightness steppers keep "−" and "+", which the phone audio replay locates by shape.
- **Notifications.** Dashboard rows use packet 06's gradient and 15-unit secondary text; the header is padded; media is an ordinary row with frameless transport glyphs; banners show the source icon with left-aligned text.
- **Keyboard.** Packet 10's light panel, pale character keys and dark Shift, Backspace and Enter, including on the lock screen.
- **Just type.** An outlined pill with the bundled Lato Italic (previously a slanted regular face) and a magnifier. Just Type's text colours are the token values (muted text 4.9:1 on the paper).
- **Palette.** The Nocturne theme and type sizes are compiled from the token file with the fixed palette; new styling uses them. Errors on dark surfaces use the control states' (229,129,109), since the token's destructive red is 2:1 on dark rows.
- **Cards.** Card view has no per-card buttons; cards grow to 0.73 from y 92 (see above).

Open, needing the owner's decision: per-row and banner dismiss buttons (the packet shows none), keyboard key labels and height (packet: glyphs, "?123", "Go", 220 units), device-menu typography (labels ~20 vs ~17–18, right-aligned values, chevrons on root rows), the 24-hour zero-padded clock, and differing fallback icons in the launcher and tray. Remaining hard-coded colours outside the touched surfaces still need moving onto the token constants.



[Runtime artwork](../crates/tarot-ui/assets/README.md) records derivation and licensing. `scripts/design-assets.py` copies the original wallpaper/fonts/token snapshot and rasterizes editable SVG assets using the supplied fonts. Runtime loading/rendering uses Rust PNG/resvg/egui/GLES; Python, CairoSVG and fontconfig are development tools. Native SVG loading uses bundled Lato without system font discovery. Font OFL and introduced icon-renderer notices are retained in source and installed packages. Historical reference fragments and complete mockup screens are never runtime assets.

Current deviations include immediate reduced-motion settling rather than the proposed 100 ms fade, three-preview fans with logical member-browse steps still awaiting motion calibration, incomplete menu/dashboard/locker styling, fallback artwork for unresolved native icons, and existing text affordances in the gesture strip. CairoSVG filter/shadow matching needs visual review against the packet’s PNGs. These are tracked gaps, not accepted historical substitutions.

Verification outcomes and exact package identity belong in [implementation status](IMPLEMENTATION_STATUS.md). Appearance, behavior and platform adaptation must receive separate acceptance decisions. No complete milestone or historical visual conformance is claimed by this foundation.

Revision 41 corrects the retained menu-entry held-key failure on the host and phone. Both native runs stop at a separate rotated Just Type scrolling failure after seven passing components; paced QA input does not resolve it. Exact restoration/normal/Panfrost audits pass. Search drag ownership and wheel polarity remain open alongside visual/timing calibration. [Evidence and identities](recordings/r41/README.md).

Revision 42 corrects repeated touch scrolling, dense result drag ownership and wheel direction while preserving completed favorite/page actions and cancellation. The signed first candidate is withheld after stronger touch-release tests find dropped intents. The corrected build passes all fourteen scoped native shell components, full Launcher pages and restart, independent restoration/Panfrost audits and the owner’s landscape scrolling/selection check. Visual/timing/accessibility calibration and unfinished milestones remain open. [Evidence and identities](recordings/r42/README.md).
