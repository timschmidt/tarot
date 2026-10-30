# Audio device modes and menu touch behavior

Sound settings now observe audio cards and their modern PulseAudio-compatible
profiles as an expandable “Audio device modes” section. Each mode has the
server's description, availability, active state and declared input/output
counts. The selector remains available when Off removes the card's audio
endpoints. The distribution can retain a dummy output when no physical output
remains; Tarot displays that observed inventory rather than changing server
policy to force an empty global list.
Mode selection uses the existing ordinary-user audio worker, with no stream,
call, privileged helper or automatic mode switch.

Cards and profiles have opaque identities in the audio adapter's existing global
namespace. A card removal retires its modes and child devices, including a
coalesced same-name/index re-add before child removal signals arrive. An exporter
reconnect retires every identity. Unavailable, missing or wrong-parent modes are
rejected, and the worker refreshes inventory before dispatch. Writes use the
card index and backend-owned profile name, never UI text. Acknowledgement waits
for observed active-profile readback before clearing the reservation.

The adapter copies the typed modern card/profile callback data during its valid
lifetime, bounds cards/profiles and names, and uses the modern availability
boolean. It does not reinterpret that boolean as port availability or guess
selectable modes from obsolete profile information. A missing modern list is
shown as unavailable. Card changes use the existing subscription/reconnect path.
[Card API](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__card__info.html),
[profile availability](https://www.freedesktop.org/software/pulseaudio/doxygen/structpa__card__profile__info2.html).

The phone's UCM configuration exposes default and call-oriented modes as well
as Off. These names describe configuration, not proof of call or acoustic
support. Tarot's call policy, Bluetooth profiles, per-app routing and acoustic
quality remain separate work. The distribution's restore module owns mode
persistence; Tarot stores no duplicate backend profile preference.
[PulseAudio 17 restore implementation](https://raw.githubusercontent.com/pulseaudio/pulseaudio/v17.0/src/modules/module-card-restore.c)
flushes pending saves during graceful teardown. Restart acceptance must use a
fresh ordinary-user server and independently observe its restored mode; it
must not inject a saved mode before measuring persistence.

Shell menu labels no longer select explanatory text during a finger drag.
The change is local to the shell menu's UI scope. Editable fields retain text
selection and copy; native application selection is handled by those clients.
Scroll state remains stable across service updates, while held mode choices
retire with the audio/feedback generations.

Current/minimum Rust each pass 273 workspace tests, with nine explicitly ignored;
strict host/ARM lint and both private audio/feedback matrices pass. Rendered checks
reproduce the old label-selection behavior, verify its removal and preserve
editable-field copy. Callback, policy and identity tests cover active/disabled
modes, invalid bounds, Off without endpoints and card/child replacement. Native
revision 56 passes rendered Speaker/Earpiece/Off switching, physical-endpoint
recovery, unavailable/stale rejection, bounded server pause/timeout/reconnect,
live server replacement, saved mode across a fresh server/login and actual PAM.
The dummy output exposed an incorrect initial QA empty-list assumption; that
failed check and its independent restoration audit remain retained. The accepted
retry uses the unchanged installed package.

The exact original card modes, input/output controls, preferences, feedback
settings and policies are restored. An independent audit verifies normal default
Tarot56, diagnostics off, exact loaded binaries, Panfrost, normal user audio
service ownership and cleanup. All 671 native archive members and 626 frozen
inputs verify. No playback/recording stream, acoustic test, call, Bluetooth peer
test or suspend cycle is part of this acceptance. Broader automatic policy,
per-app routing, PipeWire runtime, physical hotplug, accessibility and endurance
remain open. [Evidence, retained failure and reproduction](recordings/audio-modes/README.md).
