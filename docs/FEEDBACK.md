# Feedback and sound profiles

October 30, 2026. The development compositor now connects Sound settings and
notification lifecycle to the standard session feedbackd service through Rust
and zbus. It observes the daemon's Full, Quiet and Silent profiles and can change
them explicitly. The daemon owns their GSettings persistence and device themes.
Signed revision 54 is installed and independently verified in the normal default
Tarot session, with diagnostics off. [Acceptance and limits](recordings/feedback/README.md).

## Behavior

Sound settings retain the existing playback controls and add profile selection,
a persistent “Feedback for new notifications” switch and a vibration test.
Profile changes remain pending until the property is actually observed; errors
and unsupported profiles are visible. Re-selecting an already active profile
still retires its pending indicator. Just Type recognizes vibration, ringer,
haptic, feedback and silent as Sound aliases.

New notifications use a static event and an opaque notification identity. Their
text, app-provided sound files and important-policy overrides never enter the
feedback request. Low urgency and explicit `none` feedback hints suppress output;
`suppress-sound` caps it at Quiet. Do Not Disturb and the switch revoke pending
work and cancel tracked events. Closing, replacing, expiring or orphaning a
notification also retires its corresponding event. Disabling and re-enabling
feedback does not replay old alerts.

Requests use feedbackd's `-1` single-play mode. Positive timeouts repeat a theme
event, so they are unsuitable for ordinary alert pulses. A separate client
watchdog ends a test after two seconds and notification feedback after six seconds
if the completion signal is missing. The vibration test always caps its event
at Quiet and obeys the global profile. The daemon's no-feedback result is shown
as unavailable; a method reply alone never proves felt vibration or sound.

The settings switch uses Tarot preference schema 5. Versions 1–4 migrate lazily
without rewriting a file on startup; mixed old/new schemas and future versions
are rejected without overwriting the original. The existing single writer merges
notification, display, favorites, launcher and feedback changes.

## Service ownership and cancellation

The adapter starts only with system services and Tarot's explicitly private
notification/session environment. It activates `org.sigxcpu.Feedback` on that
session bus and requires the exporter to belong to the graphical user. Calls
target a particular unique owner. Owner loss, rapid release/reacquire and broker
replacement retire the old epoch and queued work. Recovery never reapplies an
old profile command or replays an old alert.

One ordered message reader consumes two exact broker subscriptions, with a
receive-position barrier before accepting replies. Completion signals carry the
owner generation so a delayed signal cannot retire a reused ID after service
replacement. Message sizes, queues and tracked events are bounded. Event batches
share one cancellation timeout; a stalled peer cannot multiply it by the number
of notifications. Session shutdown first allows cancellation, then closes the
connection and joins the reader/worker. Locking cancels an explicit test; normal
notification feedback may operate while locked without exposing its content.

## Evidence and native gate

Private rendered runs exercise actual profile controls, readback/timeout,
same-value selection, unavailable/unknown results, notification replacement,
suppression, Do Not Disturb, delayed replies, locking, sender loss, daemon/whole
broker recovery, rapid same-owner changes with reused IDs, 64-event cancellation
and bounded shutdown. The retained private-03 failure exposed the same-profile
pending indicator; private-04 through private-06 pass the corrected path.
Private-06 additionally verifies single-play framing and content-free lifecycle
metadata. Existing notification protocol/recovery/persistence, launcher controls
and display preference regressions pass. These fixtures have no hardware output.
Rust 1.98.1 and minimum 1.89.0 each pass all 261 workspace tests, with nine
explicitly ignored. Strict host and AArch64-musl/DRM lint pass. Both frozen
delivery binaries pass the rendered matrix, including 64-event cancellation
under one three-second deadline and unresponsive-peer shutdown near 1.07 seconds.
Local evidence is under ignored `artifacts/m4/feedback/`.

The installed phone has feedbackd 0.8.9-r2, its udev rules, device themes and the
GPIO vibrator force-feedback device. The standard user belongs to the expected
input/feedbackd groups. Its system and PinePhone Pro themes contain only vibration
for a Quiet-capped button event and no Silent button event. The exact 0.8.9 source
confirms single-play semantics, global/per-app/event caps, profile persistence
without starting new feedback, and client-disconnect cancellation.
[Release source](https://sources.phosh.mobi/releases/feedbackd/feedbackd-0.8.9.tar.xz),
[public API](https://honk.sigxcpu.org/projects/feedbackd/doc/iface.GdbusFeedback.html).

Native profile controls, standard daemon credentials/device access, explicit
Silent-mode unavailability, two Quiet-capped single plays, daemon recovery,
protected command rejection and actual PAM pass. A private raw-syscall tracing
instance records successful force-feedback uploads and 24-byte input-event
writes on feedbackd's verified GPIO-vibrator descriptor. Silent produces neither.
Raw events contain numeric arguments without copying user buffers. The instance
is removed and global tracing configuration remains unchanged.

The original Tarot preferences and logical feedback GSettings values, including
user override presence, are restored. An independent audit verifies the fresh
normal login, current executable hashes, default Tarot, Panfrost and cleanup.
All 673 native archive members and 621 frozen inputs verify. This establishes
kernel acceptance of device requests; felt vibration, acoustic quality,
notification-specific physical output and power/performance review remain open.
M4 and the full plan remain unfinished. Physical Bluetooth testing remains
deferred at the owner's request.
