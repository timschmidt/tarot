# Notification composition

Tarot implements the notification service and shell presentation in Rust. Standard Linux clients use `org.freedesktop.Notifications`. The original design packet's [banner](Tarot_Design_Packet/screens/png/05-banner.png), [dashboard](Tarot_Design_Packet/screens/png/06-dashboard.png) and [rail](Tarot_Design_Packet/screens/png/22-notification-rail.png) studies guide the composition. These are contemporary design references, not measured historical specifications.

Revision 43 development replaces the full-content dashboard scrim with one reserved well. Native acceptance and physical/visual calibration remain separate gates; revision 43 passes its native notification component but stops at a later favorite test using an obsolete tray coordinate. Revision-44 development corrects that QA coordinate and the separately reproduced landscape drag handoff; its native favorite and notification components pass, with remaining shell acceptance open.

## Geometry and input

From top to bottom, the layout is status, foreground, notification well, keyboard when visible, and gesture strip. Each boundary is derived once from current state. Repeated arrival, replacement and dismissal cannot accumulate insets. Native clients receive the target work area once per transition. Painting and hit testing use the presented area. A client that retains an oversized buffer uses the existing centered fit transform, including inverse coordinates for input and popup placement.

| Surface | Reservation and behavior |
| --- | --- |
| Hidden | Zero height; the gesture-strip notification button remains available |
| Banner | Packet's accessible 48-unit variant; actual summary or privacy placeholder, default action when authorized, hide without deleting the retained item |
| Rail | 48 units; groups by opaque service-issued source, actual counts and bounded overflow |
| Dashboard | 48-unit header, up to three 56-unit collapsed rows and 8 units of allowance; bounded to 60% of available space and the minimum foreground |
| Scarce landscape space | Retain at least 64 foreground units for native apps; the dashboard uses a horizontally scrollable compact row |
| Shell text entry | Reserve at least 112 foreground units for the field plus a result/control row. Compact landscape Just Type defers the banner/rail through keyboard hiding and drag preparation, restoring them when search closes; the strip button/count persists and the dashboard can still open explicitly |

The keyboard stays adjacent to the gesture strip. The well sits above it. Rotation and keyboard geometry changes rebase the transition into valid bounds. Ordinary well changes animate a shared boundary over the packet's 300 ms maximize interval; interrupted motion begins at the presented pose. Reduced motion settles immediately to the same target geometry. This timing is an implementation choice awaiting device/reference calibration.

The exposed foreground accepts native app input, card gestures, launcher controls and Just Type. Entering search or focusing a shell text field dismisses the dashboard as an explicit activity change. Opening the dashboard retains the active native app's keyboard focus. A notification touch belongs to shell UI; its release cannot become an app press. Dashboard and work-area changes retire held input; ID/revision guards reject stale notification actions. Fresh gestures begun after a change can act. Device menus remain modal and retire app focus. Opening a card menu closes the dashboard; opening the dashboard retires an old shell popup. Explicit app activation/launch can dismiss the dashboard.

## Ownership and bounded state

The service assigns an opaque numeric source identity from the actual unique bus sender, independently of its claimed application name. Different senders claiming the same name remain distinct groups. Source identity is display metadata, never action authorization or verified application identity. Rail artwork currently uses a neutral vector fallback; verified native artwork remains future work.

Actions still require the current notification ID/revision and actual service ownership. Replacement changes widget identity, so a held old control cannot invoke a replacement. Expired entries remain visibly earlier records; disconnected senders lose their actions. Bus replacement retires live ownership before reconnecting. DND, preview privacy, bounded capacity, settings errors and atomic persisted preferences retain their service behavior.

The dashboard expands real bodies/actions on demand, scrolls retained entries, and has explicit settings and clear controls. Selection from Just Type scrolls the exact retained target into the actual dashboard viewport on every selection. Expansion state is bounded and pruned to retained ID/revision pairs. There are no fictional notifications or media controls from the studies.

Protected status remains redacted to `ok`, `pid` and `lock`; captures and ordinary input remain protected. Arrival during protection must not expose an unsynchronized foreground/well on the first visible frame after unlock.

## Verification and remaining work

A focused additional check exposes a held-favorite regression with a retained notification in landscape: hiding the shell keyboard changes the well from zero to 48 units, causing the active drag to cancel. The identical gesture without a retained notification commits. Revision-44 development defers the compact search rail through keyboard hiding and passes the focused reproduction, while still canceling on explicit dashboard interruption. Broader native acceptance remains pending; unrelated layout, panel and context changes retain cancellation.

The isolated protocol fixture uses a real Wayland client with a committed 160×1000 minimum/buffer, actual native pointer/key delivery, grabbed popup pixels/input, fullscreen, four orientations and return, text-input-v3 with the trusted Rust keyboard, source separation and protocol protection during motion. It checks boundary agreement on newly visible frames and exact restoration after clearing. Host protocol protection does not certify native KMS or PAM. Earlier native controllers enable DND, so their arrival captures exercise the rail; their “banner” filename labels alone are not banner acceptance. A subsequent unchanged revision-44 native case temporarily disables DND, observes banner/dashboard/rail/hidden states across all 40 inputs and restores the original policy. Actual KMS/PAM and exact restoration audits pass. Physical banner/action usability and reference review remain separate checks.

Native acceptance must run the same client through the installed ordinary-user session, real touchscreen/keyboard routing, Panfrost and actual PAM, followed by independent restoration audits. Shell search, menu cancellation, notification replacement/expiry/disconnection, overflow, settings, launch/restart and browser compatibility also remain regression gates. Earlier failures must be retained with their executed source and binary identities.

Remaining product work includes verified artwork, durable history, real media integration, text enlargement and accessibility review, interaction/animation calibration, packet-H captures and physical visual acceptance. No complete development-plan milestone or historical fidelity claim follows from this composition alone.
