# Recording inputs and audio routes

The Rust libpulse adapter extends Sound settings with recording-input level,
mute, default input for new recordings, and the routes reported by each input
and output. A playback-monitor source is excluded from the recording-input list.
Tarot opens no recording or playback stream to provide these controls. Input
mute controls the sound server; it is not a hardware microphone kill switch.

Inputs and outputs share one monotonically allocated identity namespace, with
separate backend direction/index keys. Routes have opaque identities tied to
their parent device. Removal retires device and route identities even when a
remove/re-add is coalesced into one inventory update. A bounded set retains
removal notifications, and inventory queries retry a bounded number of times
when removals arrive between replies. A reconnect retires every old identity.

A control is accepted only for a currently observed device in the correct
direction. An explicitly unavailable or missing route is rejected; unknown
availability allows explicit selection, as required for devices without jack
presence detection. The UI preserves its scroll position across service updates while retiring held
choices on observed generation changes. Scroll-area identity stays stable;
interactive groups carry both audio and feedback generations.
The worker refreshes inventory before dispatch. Volume, mute and route writes
use the server's device index; default selection uses the API's device name.
Every completion requires observed readback after acknowledgement. Uncertain
outcomes are surfaced and never automatically replayed after service recovery.

Input volume writes preserve channel balance, accept 0–100%, and retain mute.
Default changes affect new streams; applications can retain existing routes.
Route/profile policy and actual acoustic behavior remain the platform's concern.
This input/route layer does not move individual application streams, record
samples, provide microphone meters, or implement call routing. Explicit card
selection is now a separate [audio-mode extension](AUDIO_MODES.md).

The adapter uses the pinned libpulse-sys 1.23.0 binding. It copies callback-owned
strings and port arrays while they are valid, bounds device/port inventories,
uses separate typed sink/source callbacks, and preserves the existing threaded
mainloop/operation cancellation ownership rules. Diagnostics include opaque IDs,
levels, mute/default and route state, without backend names or descriptions.
[Upstream introspection contract](https://www.freedesktop.org/software/pulseaudio/doxygen/introspect.html),
[source information](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__source__info.html),
[route availability](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__sink__port__info.html).

Rust 1.98.1 and minimum 1.89.0 each pass 267 workspace tests, with nine
explicitly ignored. Strict host and AArch64-musl/DRM lint pass. Both frozen
binaries pass the real private-server input/output matrix and the feedback
profile/notification/recovery matrix. Audio fixtures use synthetic sources/sinks,
a private broker and X display, and open no recording or playback stream.

Signed revision 55 passes native level/mute controls, selected-input-route
reselection, output-route adapter readback, a synthetic-input default round trip,
stale-input rejection and actual PAM. Exact original input/output controls,
preferences and policies are restored. An independent audit verifies normal
default Tarot55, current loaded programs, Panfrost and cleanup. All 653 collected
members and 623 frozen inputs verify. No recording or playback stream was opened.
[Evidence and retained failures](recordings/audio-input/README.md).

The phone currently exposes one physical input and one route per input/output.
Acoustic quality, real capture, multiple physical routes/jack hotplug, per-app
routing, PipeWire runtime and call policy remain open. Revision 56 adds card
selection and prevents menu explanation-label selection during touch drags;
broader phone touch/accessibility work remains. No full milestone or release-readiness claim is
made. Physical Bluetooth testing remains deferred at the owner's request.
