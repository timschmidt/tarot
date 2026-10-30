# Audio activity and system sleep

Signed revision 57 now runs as the phone's normal default session. Native
ordinary-user descriptor ownership, protected panel off/wake, actual PAM and
exact restoration pass. This is scoped M4 progress; the full plan remains active.

Tarot observes uncorked playback streams and recording streams on the normal
user's PulseAudio-compatible server. When either count is nonzero, its ordinary
user worker requests a login1 **sleep/block** inhibitor. It holds the returned
descriptor while that observation remains current, including while the session
is locked. One descriptor covers concurrent streams. Corking or closing the last
stream releases it; muted or zero-volume streams still count as active.

This policy does not inhibit automatic locking or display idle, and it does not
replace the separate sleep-delay descriptor that protects presentation before
suspend. Power-button screen-off and authentication retain their existing rules.
The Power & display page reports a held grant or a visible unavailable/error
state. Normal session startup enables the policy with `--sleep-coordination`;
nested diagnostics require an explicit private manager bus.
The status uses a fixed-height row so background audio/grant changes cannot move
the following Power controls under a finger. A rendered regression reproduces
the original movement and verifies the corrected layout.

The Rust audio wrapper reads only the typed corked flags and bounded counts,
with separate playback/recording callbacks. It subscribes to sink-input and
source-output changes and uses the existing coalesced inventory/reconnect path.
It collects no stream names, app identifiers, metadata or samples. A monitor
recording counts as recording activity even though monitor devices remain absent
from Sound's microphone chooser.
[Playback stream information](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__sink__input__info.html),
[recording stream information](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__source__output__info.html).

The inhibitor worker uses the existing bounded, cancellable Unix-bus transport,
now shared with native app menus through `tarot::local_bus`. It checks the
manager's unique owner and UID: root on the actual device, the ordinary test user
on a private nested bus. Calls target that unique owner, and owner/broker loss
retires the connection. A grant for an older audio observation is dropped. Every
method failure retires the connection before retry, including timeout, so a late
descriptor cannot remain queued as an unobserved sleep lock. Setup and calls are
bounded; shutdown cancels the owned socket before joining its worker. Descriptors
are close-on-exec and close on shutdown, session exit, manager loss or audio
observation loss. Healthy operation does not poll.
[login1 descriptor contract](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html).

If audio observation is unavailable, Tarot releases its audio block and reports
the service failure; it cannot promise uninterrupted media in that condition.
An uncorked but starved stream can keep the phone awake. Apps must cork or close
their inactive streams. Direct ALSA audio, independent audio servers, automatic
call policy and non-audio work inhibitors remain separate integration work.
MPRIS media controls and metadata are not implemented by this component.

Private acceptance uses owned broker, audio server, null devices, zero-PCM
playback and a monitor recording whose samples are discarded without inspection
or storage. It never selects physical audio or invokes Suspend. It checks stream
corking/loss, one combined grant, late and rejected replies, timeout, manager and
broker replacement, audio server recovery, automatic lock/redaction and pending
logout cleanup. Rust 1.98.1 and minimum 1.89.0 each pass 275 workspace tests, with
nine explicitly ignored; strict host/ARM lint, audio/feedback matrices, protected
sleep-delay and shared-transport regressions pass. A recurring synthetic URI
helper failure reports Linux ETXTBSY. A controlled inherited-writer check
reproduces it; publishing that fixture from an owned child eliminates the test
process's writer lease. Forty complete process suites pass afterward. URI
runtime behavior is unchanged.

On the phone, null-device playback and monitor capture produce one actual
login1 sleep/block descriptor owned by Tarot's UID/PID. Its close-on-exec flag,
combined-stream retention and last-stream/client-exit release are independently
verified. Automatic locking and panel off still occur, with driver DPMS and
backlight-power readback. A neutral wake precedes the full locker-presentation
proof and actual PAM; original preferences, modes/controls, feedback settings
and policies are restored. Independent normal/default Tarot57, loaded binaries,
Panfrost, services and cleanup pass. All 645 collected members and 630 frozen
inputs verify. [Evidence and retained failures](recordings/audio-inhibit/README.md).

No physical playback/capture, acoustic, Bluetooth or call acceptance is added.
The test does not invoke Suspend: kernel enforcement/reliability with other
inhibitors absent, broader work/call policy and endurance remain required.
