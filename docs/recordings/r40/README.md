# Revision 40: menu motion and constrained layouts

October 29, 2026. The installed development package adds menu transitions, vector glyphs and bounded layouts. Its controller passes fourteen scoped native shell components, Launcher pages and saved restart. An additional reverse-direction held-key host regression fails and remains a required correction. Complete native and milestone acceptance remain open. Raw captures, logs, preferences, source ledgers and archives stay private in ignored artifacts. [Menu contract](../../MENUS.md) · [Remaining delivery and export research](../../MENUS_PLAN.md).

## Behavior

Opening and closing use a monotonic host clock, a proposed 160/120 ms cubic fade and 16 logical pixels of travel. Reduced motion uses the packet's 100 ms fade without translation. These are adaptations pending historical/physical calibration. Every production backend registers the clock; clockless model embedders retain synchronous commands. Root switches retain one scrim, while submenu page edits retain their surface.

A closing menu keeps input until its fully transparent frame is acknowledged: matching DRM page-flip completion on the phone or successful nested submission. Only then can a deferred leaf choice commit or native focus return. Reopening, owner death, protection, geometry changes and changed capabilities invalidate the old choice/frame. Lock protection starts immediately. Retiring shell widgets releases their egui key state and consumes their physical key tails after native focus returns.

Service and page-selection glyphs are vector paths. Separate brightness rail/fill/thumb painting retains the actual slider input, accessibility and release/service-generation guards. Empty detail text no longer shifts row labels upward. Long page/window titles stay inside a fixed header, Back remains visible while rows scroll, and landscape credentials scroll to Connect/Cancel above the keyboard. Reverse Tab and keypad Enter map to standard widgets.

## Host checks and retained failure

All **219 workspace tests** pass on current Rust and Rust 1.89. Strict host/AArch64 DRM lint, minimum-compiler AArch64 checking, workspace binaries/examples and 24 Python fixtures pass. The restricted-IPC failure is retained separately; unchanged tests pass with local socket access. An initial style-lint failure in the QA probe is corrected with a minimum-compiler-compatible array conversion.

Six rendered suites pass: menus in portrait scales 1 and 2 and landscape scale 2, complete Launcher page/restart persistence, and Just Type in portrait/landscape. Menu checks require actual Back focus and Enter, accepting/refusing clients, owner death, two held shell keys released after native focus returns, and a fresh balanced native pair. Probe reports contain event balance without keycodes or text. Actual widget tests cover long headings, pinned Back, credential scrolling, stale held rows/sliders, service replacement, reduced motion, presentation tokens and geometry cancellation. Long-heading/credential/reduced-motion cases are host evidence, not calibrated phone approval.

An additional unchanged-binary regression fails in portrait and landscape: Shift is pressed while the native client owns focus, the menu opens, Shift is released inside the overlay, and the original zero-held-key gate fails after a fresh native Tab pair. The probe reports one remembered key. The intercepted-release/forwarded-key bookkeeping path is the source-derived cause hypothesis; a corrective revision must pass this exact regression and both menu-crossing directions. No phone QA source or runtime changes during the native run.

The private failed host archives are SHA-256 `a33e4cd06c5aaca90927240f9fa3240fa864e4139cfcae2005223332369ff721` (1,759,944 bytes) and `0ec91f8e8ed315f829f465765de8860ab5c6e7b3525a6ad88f5133fee595d8d1` (2,261,070 bytes), with exact executed sources and original gates.

## Package identities

Clean build, signature and payload audits pass. All **355 Rust files and nine Cargo inputs** match the frozen archive, with 446 locked registry packages. Only the compositor payload changes from revision 39; session launcher, locker and keyboard are identical. Policy/notices, four unique command providers, ARM/musl ownership/modes and absence of install scripts pass audit. Later reporting and QA have separate identities.

| Artifact | SHA-256 |
| --- | --- |
| Signed `tarot-0.1.0-r40.apk` | `12ffc80f1d28937106ec2bad470408d891d02cb6f845c49e3891b167b30e124d` |
| Vendored source archive | `bfa801773a3784317e6660718a3cafe66630c00a04d9dd7dbda872f213c6697a` |
| `tarot` | `9b065f186ea567df171ffe23929fef198507c7180a1f412ecd4ff0a31d578e22` |
| `tarot-session` | `303e3d7ed3d0065a0b856599bcba34625a6a46ffd26ca46dfb866c7c2a98ad77` |
| `tarot-lock` | `e1872f0d0f6a837749be211809103124c941478a6f6b0d32a90feebeea823984` |
| `tarot-keyboard` | `1c44171548b5b23bc62acf27da815c43fdd7cc3a305c644e961999646187c346` |

The first phone preflight uses a relative public-key directory and reports an untrusted signature. Verification with the existing absolute directory passes on the same APK/key. The installation controller already uses the absolute path; no package or key change is required. Both preflight outcomes are retained.

## Native component acceptance

All **222 staged source hashes and Python syntax** verify before installation, alongside the APK signature/hash, separately audited ordinary-user probe/source and previous installed payloads. One controller passes all fourteen shell components: keyboard context/reactivation, native protocol/transients, GTK/Qt dialogs, terminal/clipboard/controls/fullscreen, protected virtual-key/helper-crash handling, menus, Just Type, favorite drag, Quick Launch, rounded cards, stack matrix, real-editor Cancel/Save and hardware controls.

Menu checks use real uinput/libinput touch in four orientations, actual PAM, sysfs/libpulse readback, accepting/refusing clients and owner death. Two held shell-key releases stay out of the refocused native client, followed by a fresh balanced pair. This phone case does not cover the newly failing reverse-direction host regression. Launcher pages then pass name entry/management, orientation and gesture cancellation, stable page identities/placement and a subsequent authenticated saved restart.

Independent audits verify normal default Tarot, exact loaded binaries, UID 10000/active local VT7, hardware keys, diagnostics/metadata off, normal logging, unchanged policies/services, exact current preference/MIME bytes and modes, exact raw brightness/channel volume/mute restoration, absent fixture/input/alarm leftovers and a live Panfrost descriptor. All sessions request zero suspend cycles. Actual menu rail/glyph and selected-page captures are reviewed; physical/timing calibration remains open.

The sealed native archive is SHA-256 `b54b08ed614e3a520eea90947a789d77c1281f6e16bbe7e2667c1de5f7613e3d` (76,906,261 bytes, 1,009 members). Safe extraction, all 222 source hashes and controller/executed-wrapper identities verify locally. The archive stays private.

## Remaining gates

Correct the retained reverse-direction held-key failure before accepting menu keyboard ownership. Enlarged text, packet-G physical/timing calibration, exported GTK/KDE commands and a cooperating Rust app remain open. Notification composition, retained browser input failures, current-binary endurance and every unfinished M0–M6 criterion retain their scope. Revision 32 remains the last accepted complete scoped native checkpoint including the browser.
