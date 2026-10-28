# Tarot interaction design

Design direction confirmed October 26, 2026: retain WebOS's thoughtful visual and behavioral design while implementing it in Rust on modern Linux foundations. Fidelity includes the relationship between gestures, animation, application state, and screen layout. Modernization should solve a concrete hardware, accessibility, or application-compatibility problem.

This is the proposed interaction contract. Numerical sizes and timings are starting values for device evaluation, not measurements of historical WebOS.

The owner’s October 28 [original packet](Tarot_Design_Packet/README.txt) selects **Nocturne and Pre 2-era webOS 2.x**. Its 31-page specification, tokens, motion notes and engineering handoff supersede conflicting preliminary values below. [Design application](DESIGN_APPLICATION.md) records implementation coverage and deliberate deviations. No exact historical firmware capture has been obtained.

The [reference board](REFERENCE_BOARD.md) records the reviewed phone-guide pages, LuneOS behavior sources, initial tokens and remaining visual evidence. Its acceptance gaps remain open; the current shell presentation is preliminary.

## Reference and visual language

Use the classic phone experience as the starting point: a compact status area, recognizable app cards, a bottom dock, a launcher, restrained transient banners, and a persistent notification dashboard. Include stacks from later WebOS/LuneOS behavior without trying to reproduce every release's differences simultaneously. LuneOS's [cardshell](https://github.com/webOS-ports/luna-next-cardshell/tree/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml) supplies an inspectable reference; its [group list](https://github.com/webOS-ports/luna-next-cardshell/blob/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml/CardView/CardGroupListView.qml) demonstrates stack interactions.

Before visual implementation, assemble a small reference board of attributable source images or recordings covering home, a focused app, overview, stacks, launcher, notifications, system menu, and lock screen. Select a consistent phone-era baseline from those observations. Record each meaningful departure and its reason. The selected era is Pre 2/webOS 2.x; capture and pin one exact firmware build before claiming historical conformance.

Define tokens for typography, color, panel shading, corner radius, separators, shadows, icon sizes, spacing, and motion. Use original icons and artwork with a recorded license. Preserve recognizable depth, hierarchy, and screen composition; avoid importing arbitrary modern UI conventions merely because they are current defaults. Express most shell artwork as egui geometry or reusable vector assets; raster assets are optional where they materially improve fidelity.

Start at 360 by 720 logical units for a scale-2 portrait phone view. The accepted packet proposes a 32-unit status area, 36-unit gesture strip and 69-unit tray when visible. Ordinary Launcher has no persistent tray; favorite editing and the transient wave may reveal it. Interactive controls should generally have at least a 48 by 48 unit target, even when their artwork is smaller. Validate physical reach and readability on the phone. Use portrait as the primary layout, with a fully defined landscape reflow rather than a rotated screenshot.

## Navigation and cards

| Surface | Expected behavior |
| --- | --- |
| Home | Wallpaper, status, dock, and search/launcher access; a deliberate empty state when no apps are running |
| Focused app | One app occupies the usable area; shell reservations and keyboard changes produce correct client geometry |
| Card overview | Horizontally arranged live or cached app previews, with an obvious selected card and contextual app labels |
| Stack | Related cards remain independently selectable and closable; the group can be inspected and rearranged |
| Launcher | Browse installed apps, move favorites to the dock, and search; failed launches return a useful state |
| Search | Initially apps and settings; later contacts and local actions with explicit result types |

One ordinary top-level window corresponds to one card. Popups and transient dialogs stay visually attached to their parent, and an unsaved-document prompt must remain reachable. Do not equate a process with a card or merge all windows merely because they share an app ID.

In overview, horizontal movement browses cards and vertical flicks request dismissal. A completed dismissal sends the application's normal close request. While the app is deciding, represent the card as closing; an application that remains open or asks for confirmation must reappear with its dialog accessible. Offer explicit force-close only for a demonstrably unresponsive window, identifying the scope of the action. Never silently kill an entire multiprocess application because one card was flicked.

Start with individual cards. Add manual stacking and unstacking using a deliberate hold-and-drag interaction. Keep stable card identity throughout reordering, and preserve the stack when one member closes. Only add automatic grouping after observing parentage and launch behavior in real apps. Returning from overview restores the selected app's state and focus rather than relaunching it.

The initial implementation now arms arrangement after a stationary 420 ms hold. The card follows the finger translucently; dropping onto another member joins/reorders within that stack. Holding at a display edge pages the deck after 350 ms and then every 500 ms, without changing native focus. A separate-card rail below the previews supplies an explicit before/after-stack drop target for reordering and extraction. The model changes only at release. A second contact, changed deck/view or interrupted sequence cancels the preview. The card menu offers Move left/right, Stack with a neighbor and Separate from stack with 48-unit rows; while open, that menu owns input over underlying cards.

This is a functional prototype. Stack members expose 48 logical units horizontally and use small vertical steps; the selected member stays centered and supplies the stack's title and controls. The rail is a deliberate initial adaptation to distinguish joining from separating on a small touchscreen. Historical rotation, lift scale, mature stack motion and visual comparison still need refinement. No automatic grouping or durable stack restoration is claimed.

## Gesture mapping

The PinePhone Pro adaptation begins with a visible software gesture region at the bottom. Its area is excluded from the normal app input region. This makes system gestures discoverable and gives them a clear owner, reflecting the dedicated gesture concept in [LuneOS's gesture component](https://github.com/webOS-ports/luna-next-cardshell/blob/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml/WindowManager/LunaGestureArea.qml).

| Gesture and context | Proposed result | Cancellation or alternative |
| --- | --- | --- |
| Short upward swipe from the strip while an app is focused | Shrink the app into card overview | Return smoothly to the app if the gesture does not commit |
| Upward swipe from the strip on home or settled overview | Open launcher | Visible launcher control and desktop shortcut |
| Upward drag and hold from the strip | Reveal quick launch; slide to a favorite and release to activate | Release outside a target to cancel; dock taps remain available |
| Horizontal drag in overview | Browse cards/stacks | No app receives the touch sequence |
| Upward flick on an overview card | Request that card's close | Below the distance/velocity threshold, return it to position |
| Hold and drag an overview card | Reorder or form/extract a stack | Explicit drop targets; cancellation restores the original order |
| Tap an overview card | Activate it | App input begins on a subsequent touch sequence |
| Tap the status/system area | Open system controls | Tap outside or use Back/Escape to dismiss |
| Tap a notification banner or dashboard item | Invoke its valid action or focus its app | Locked sessions follow authentication and privacy rules |
| Right-to-left gesture within the strip | Back in Tarot apps or a documented compatible app integration | Unsupported apps retain their own Back control; do not guess by injecting Escape universally |

Resolve gestures from origin, direction, displacement, velocity, elapsed time, and current mode. The packet proposes 10 units of tap slop, 14 units for axis lock, 420 ms for card arrangement, and a separate 140 ms wave dwell after at least 24 units of upward travel. Tune using one-handed use and slow/fast gestures on hardware; do not inherit QML thresholds as if they were device-independent constants.

The short upward gesture and upward hold need a shared recognizer: progress may preview overview, but quick-launch activation must be resolved before a final navigation command is emitted. A second contact, rotation, power transition, app disappearance, or modal interruption must either complete a defined transition or cancel it cleanly. Never leave a stuck drag, locked orientation, or half-delivered touch sequence.

The revision 10 development implementation now resolves a 140 ms dwell after qualifying upward strip travel into a five-position wave. Four positions contain ordered favorites; Apps remains fixed at the right. Releasing outside a valid installed favorite or Apps cancels. The wave preserves native geometry and keyboard reservation, and an existing canonical desktop/app identity focuses its current card instead of launching another instance. Short gestures retain their existing navigation. A touch editor supplies explicit add/remove/Left/Right alternatives with 48-unit controls and atomic persistence; [hold-and-drag favorite organization](FAVORITE_DRAG.md) now has a passing revision-28 native component replay, with insertion/reordering, explicit shortcut removal and cancellation. Physical timing and visual conformance remain open. Initial roles use available installed Phone, Contacts, Mail and Calendar entries. Empty positions open the editor on a normal tap and do not commit from the wave. The supplied original role icons and tray material now replace the preliminary artwork; unknown native icons still need theme resolution and the wave needs further trajectory calibration. [Reference behavior, Palm guide pp. 23–26](https://support.bell.ca/_web/guides/User-Guides/Mobile/PalmOne/Palm-EN/palm_pre_userguide_en%28en%29.pdf)

Apps own ordinary gestures beginning inside their content. For the first release, the shell does not intercept their scrolling, drawing, or horizontal navigation. Fullscreen apps need a deliberate edge-reveal policy before the visible strip can be hidden. Desktop equivalents should cover overview, launcher, Back, selection, and cancellation without taking common app shortcuts indiscriminately.

## Motion and feedback

Animation follows the finger during a drag. On release, it resolves to an outcome using distance and velocity, with a damped settle or short easing curve. Use the packet’s proposed 300 ms maximize, 350 ms minimize, 400 ms launch and 300 ms dismiss durations, then tune through recordings; elapsed time drives progress, never a fixed amount per frame.

Keep the app spatially connected as it becomes a card, moves into a stack, returns to focus, or closes. Show pressed, selected, disabled, pending, and error states for controls. Provide optional subtle haptics for meaningful commitments, not every motion sample. Under reduced-motion settings, preserve navigation and selection with short fades or direct state changes.

## Notifications and system controls

Present ordinary notifications as brief banners that do not steal focus, with retained items available through a dashboard. Group related entries without hiding distinct actions. Separate low battery, connectivity, and incoming-call urgency. Do-not-disturb and lock-screen content settings apply before rendering the notification.

Map presentation onto the standard notification service, including replacement, timeout, action, and close semantics. A locally retained history entry is not automatically a still-live remote action after its sender exits. [Desktop notification protocol](https://specifications.freedesktop.org/notification/latest/protocol.html)

The initial native implementation displays a banner for five seconds, above the native keyboard when present. Hiding it keeps the dashboard entry. Default ordinary notifications remain live until acknowledged; explicit positive lifetimes expire, timeout zero remains live, and critical notifications do not auto-expire. Closed ordinary entries remain read-only history until dismissed or trimmed; transient entries are removed on closure. The newest critical banner takes priority over ordinary banners. Do-not-disturb suppresses every banner, including critical banners, while retaining entries. These are initial Tarot policies, subject to later phone/call urgency design.

Alerts opens the modal dashboard without navigating away from the current app or overview. It supplies actions, dismissal, clear-all and touch settings. A banner tap invokes the sender's default action when available, otherwise opens the dashboard. An arrival preserves held card arrangement and text focus. Dashboard opening suspends native text input; dismissal restores the same field. Preview hiding redacts banner summaries only: the explicitly opened dashboard still shows content. It is not lock-screen protection. The two notification settings now persist; failed saves show an error while retaining session behavior. They remain usable during a bus outage. On bus replacement, old entries become read-only history and cannot act on a new sender. Durable history, grouping, icons, sounds/haptics, accessible semantics, urgency refinement and historical styling remain pending.

System controls cover network, audio, brightness, Bluetooth, airplane mode, rotation lock, power, and settings. Show transitions and failures from real service state. A radio request can fail or be blocked by a hardware switch; the UI must not display a false successful toggle. Retain a classic app-menu position where meaningful, but arbitrary Linux clients do not automatically export a WebOS-style global app menu.

The initial top-right status menu now shows real battery level, charge state, external power and display brightness. A coarse level takes precedence over a numerical estimate when supplied. The brightness buttons show confirmed driver state, with separate pending/error feedback; driver quantization can make a requested 60% display as 61%. An active local login session authorizes changes. The modal menu hides the keyboard and restores the same field after dismissal. Its styling and limited controls are preliminary; clocks, other radios, audio, broader settings, idle/display-off and authenticated lock behavior remain separate work.

The system menu's Wi-Fi button opens a bounded, scrollable network page with a return button, software radio control, scan, disconnect and compatible saved-profile actions. Connected networks precede saved networks, then signal strength. Raw SSID/security identity stays separate from labels; a similarly named open network does not reuse a protected profile. Completion follows real scan/connection state, with pending, denied, credential-needed and timeout feedback. Authentication-required actions stay disabled until an authorization agent exists. Unsaved networks explain that new-network/password setup is pending. Hardware unavailability disables enable/scan/connect; turning Wi-Fi off never changes the global networking switch. System internet state may reflect another interface. The modal page blocks the underlying app, launcher search, shell keyboard and dock; closing it restores the selected app without navigation. Historical styling, connection setup prompts and accessible semantics remain unfinished.

Visible unsaved open/personal networks now open a connection form. Personal passwords remain masked, with the shared keyboard marked Private entry; its Enter key submits only a valid value. Remember this network distinguishes future reconnection from a profile removed after disconnect. Missing passwords for a user-requested saved connection open the same form through the system secret agent. Cancel, navigation, expiry and service loss dismiss entry rather than retaining a password for later. Hidden-network names, enterprise prompts, persistent agent-owned secret storage, clipboard/cursor editing, complete physical-keyboard/IME coverage and accessible field semantics remain pending. The earlier saved-profile-only checkpoint above is superseded for visible personal-network setup.

The Audio page presents observed playback outputs with independent volume and mute controls. Changing volume does not unmute; controls stay within normal volume and retain channel balance. Default selection is labelled for new playback because existing apps may retain routes. A newly mapped card cannot dismiss the status or notification panel or replace its underlying selection. Panels stay above their dismissal scrims after an outside-tap/reopen sequence. Native audio, power and notification replays exercise those transitions. Device profiles, microphone controls, headset/call behavior, physical audio keys, accessibility and visual refinement remain pending.

## Keyboard and first party apps

The phone must be usable without a physical keyboard. The keyboard reserves space, follows text focus, supports appropriate text/number/password modes, and disappears predictably. Shell search and external client fields must use the same user-facing keyboard behavior even though their integration differs. Physical keyboard shortcuts, selection, clipboard, and composition remain supported.

The implemented English key view covers all printable ASCII through letters and two symbol pages, retaining the existing accented/emoji keys. `#+=` opens the extended punctuation page; `123` returns to the first symbol page and `ABC` returns to letters. Letter Shift remains one-shot. Native phone touches and private-field pointer tests verify the full ASCII range. A remembered Wi-Fi password was completely touch-entered, saved and later reconnected without entry after replacing the compositor session; daemon/reboot persistence remains unverified.

Build Settings as the first substantive Tarot application. Phone, Messages, and Contacts follow as separate processes sharing visual components and service interfaces. Start with local contacts, basic SMS, and one active call. Preserve data and show explicit pending, failed, and disconnected states. Browser, terminal, media, and camera functionality can initially come from existing Linux apps; “Rust shell” does not require rewriting their engines.

Universal search begins locally. Network searches or actions that send a message or place a call require an explicit user action. Typing into the launcher must not silently send search text to a service.

## Locking and accessibility

Retain a recognizable clock and unlock interaction, followed by authentication where required. A swipe is an interaction affordance, not authentication. Hide sensitive previews by default; a call may expose narrowly defined answer/reject controls while preserving lock protection. Specify behavior for a failed authentication, dead locker, rotation, and resume before calling the lock screen complete.

Supply semantic names, roles, states, and actions for custom egui controls and card objects. Ensure keyboard navigation, visible focus, font scaling, contrast, reduced motion, and non-gesture alternatives. AccessKit has a Unix AT-SPI adapter, but Tarot must connect its custom integration and verify it with assistive technology. [AccessKit platform adapters](https://github.com/AccessKit/accesskit#platform-adapters)

Do not equate attractive screenshots with accessible or faithful interaction. Review the full sequence of launch, type, overview, stack, return, notify, close, lock, and wake on the physical device.

## Design acceptance evidence

For each milestone, preserve a small set of screenshots plus recordings of gesture progress and cancellation. Compare consistent states at the same logical dimensions against the reference board. Track intentional deviations in a table containing the original behavior, Tarot behavior, practical reason, and the milestone that introduced it.

The minimum interaction fixture set includes zero, one, and many cards; two stacks; a modal dialog; a stalled launch; a long app title; a notification arriving during a drag; keyboard focus changes; landscape; enlarged text; reduced motion; and locked incoming-call presentation. Automated gesture replay checks the state transitions, while device review checks reach, feel, and visual continuity.

## Lock implementation checkpoint

The compositor's experimental protection boundary now routes the full panel to a trusted lock surface or opaque fallback. Normal app gestures, F1/F2 navigation, launcher/search, notification actions and keyboard presentation are unavailable during protection. Held key/touch releases cannot cross entry or exit, and normal app lifecycle does not interrupt a locker contact. The current synthetic QA surface has no user-facing lock interaction or authentication.

The subsequent Rust/egui prototype below supplies the initial clock/lock presentation and private touch/physical credential entry, with platform authentication and pending/failure states. Classic WebOS visual and interaction references guide that screen; successful authentication must precede the protocol unlock request. Do not expose a normal Lock action until this client and crash/startup recovery pass device tests. Idle/display-off/suspend remain separate policy work. [Protocol evidence](IMPLEMENTATION_STATUS.md#session-lock-protocol-foundation)

## Authenticated locker prototype

The real Rust/egui client now displays the device-local clock/date and a lock puck, opened by tap or upward drag. Private entry occupies the upper panel and the shared keyboard the lower 240 logical units. Password characters render only as bullets; physical keys use the supplied xkb map. Unlock is permitted only after protected presentation and a successful PAM authentication/account check. Pending/retry states block new submission, failed attempts clear the field, and Cancel returns to the lock cover without unlocking. Notifications/app content remain fully hidden.

This initial layout and vector lock are functional development artwork. Historical wallpaper treatment, puck lift/drag motion, faithful type/material styling, clock/locale preferences, accessibility semantics, repeat/composition/multiple layouts and smaller/rotated allocations remain design work. At that checkpoint there was no normal shell Lock control; the reusable activation section below supersedes that gap. Idle/suspend activation remains unfinished. The UI's opt-in own-surface QA images do not grant normal applications screenshot access. [Phone evidence](IMPLEMENTATION_STATUS.md#authenticated-rustegui-locker)

## Reusable lock activation

When a trusted locker is configured, the Power & display menu supplies a Lock button. Activating it preserves the current app/deck, closes ordinary panels and immediately reserves protected input/rendering before the new lock client starts. Successful authentication returns to that view; every user lock gets fresh private entry. Configured session startup begins protected, and transient locker failures recover automatically without showing ordinary content. After the bounded retry budget is exhausted, an opaque dedicated view says the session remains locked and offers Try again (also Enter). A retry launches the same trusted locker and still requires authentication. It shows no application/notification content or credentials. Held/canceled/multitouch/device-loss contacts cannot retry; a fresh click or key is required. This functional recovery layout still needs final historical styling and accessibility work. Idle, display-off and suspend activation remain separate policy work.


## Sleep boundary

When opt-in sleep coordination receives authoritative preparation, it activates the existing protected lock path immediately and cancels held input. Returning from sleep never authenticates: the compositor must present a fresh protected frame before accepting an unlock, and the trusted client still requires normal PAM authentication. Service loss/denial keeps this development mode protected until coordination recovers. A polished user-visible unavailable/recovery flow, idle timeout preferences, power-key/display-off behavior, call/audio inhibitors and full wake interaction remain design/implementation work. Signal-only native evidence does not establish actual suspend/wake reliability.


## Idle and wake interaction

The development idle policy locks after a configured inactive interval and can power off the direct panel later. A visible native surface may inhibit automatic idle behavior; hiding, fully covering or unmapping it removes that effect. Releasing inhibition gives a fresh interval, rather than immediately locking after time spent watching the app. Explicit Lock and external sleep preparation retain their authority.

The first touch or key on an off panel wakes it without activating the lock puck, typing or authenticating. Held releases and removal of the waking input device cannot become a later click. A fresh protected frame must complete before input/unlock resumes; the user then opens private entry and authenticates normally. The requested brightness is preserved across panel off/on. Idle preferences, power-key behavior, dimming, accessibility, proximity/call handling and final historical motion/style remain work; explicit short CLI intervals serve device testing, not final UX defaults.
