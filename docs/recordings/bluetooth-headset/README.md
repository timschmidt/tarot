# Bluetooth headset audio and buttons

From revision 66 the Sound page chooses outputs and inputs with radio lists,
and the greeter's leftover services no longer take the headset's HFP mode.

October 31, 2026. The owner's JLab Studio headset, held close to the phone,
plays A2DP stereo and HFP wideband voice in both directions. From signed
revision 65 its play and pause buttons control the media player Tarot shows.
The owner listened to and confirmed each audio result.

## Reconnecting a headset that forgot the phone

The phone still held its revision-52 pairing, but connecting failed with
`br-connection-key-missing`: the headset no longer had the link key, most
likely because it has since paired with other devices. With the headset in
pairing mode, the stale bond was removed and BlueZ paired, trusted and connected
it again (Just Works, no code). The earlier revision-52 `br-connection-page-timeout`
fits the distance and coexistence findings in the
[test-peer investigation](../bluetooth-peer/README.md). After a reboot the
trusted headset reconnected with one Connect.

## Audio

The phone's PulseAudio 17 has `module-bluez5-discover` and `module-bluetooth-policy`
loaded. On connect, PulseAudio created the headset card and made its A2DP sink the default output
(`module-switch-on-connect`).

| Profile | Codec and format | Check | Result |
| --- | --- | --- | --- |
| A2DP sink | SBC, 44.1 kHz stereo | Low tone left, higher tone right, then three beeps in both | Heard correctly, sides right |
| HFP head unit | mSBC, 16 kHz mono | Beep, 6 s recording from the headset microphone, double beep, playback | Beeps and playback heard; the recording is 5.9–6.0 s without gaps |

The first HFP recording, made in silence, holds only a steady low hum at about
150 RMS, which is the microphone's noise floor. The second, with the owner
speaking, peaks at 19220 of 32767 and shows clear syllables. Its energy sits
almost entirely between 100 and 300 Hz, with 1–4 kHz some 40–50 dB lower, so
it sounds muffled. The owner describes the headset's microphone as very low
quality, and mSBC itself carries up to 8 kHz, so this is attributed to the
headset rather than the phone. A comparison with CVSD or another headset was
not made.

The schematic wires the module's PCM voice pins to the rt5640 codec's second
interface, with an unfitted link to the modem. Voice audio nonetheless arrived
over the host link (HCI), the path PulseAudio uses, so no PCM routing change
was needed.

## Buttons

The headset's buttons appear as the input device `JLab Studio (AVRCP)`:

| Button | Key sent |
| --- | --- |
| Play/pause | `KEY_PLAYCD` (200) and `KEY_PAUSECD` (201), alternating |
| Long press, volume down / up | `KEY_PREVIOUSSONG` (165) / `KEY_NEXTSONG` (163) |
| Volume up/down | Nothing; the headset changes its own volume and the phone's sink stays at 100% |

Revision 64 ignored these keys. Revision 65 routes the media keys (play, pause,
play/pause, next, previous) to the player the dashboard shows, using MPRIS's own
`Play` and `Pause` for the explicit keys, so the headset can't toggle a player
into the opposite state. Only actions the player offers are sent, one at a time.
The keys work while locked or dark and never wake the display. They no longer
reach the focused client.

On signed r65, with Decibels playing a generated audible track to the headset:

| Time | Headset key | Decibels |
| --- | --- | --- |
| 1.9 s | | Playing (registered on Tarot's bus) |
| 11.4 s | Pause | Paused |
| 14.3 s | Play | Playing |
| 34.1 s | Pause | Paused |
| 37.1 s | Play | Playing |

The owner confirmed that the sound stopped and started. A fourth run, with the
committed tool, checked the locked case. Decibels registered while the phone was
awake, then the owner locked it with the power button, and the display stayed
off (`bl_power` 4) for every press:

| Time | Headset key | Decibels |
| --- | --- | --- |
| 15.56 s | Pause | Paused at 15.60 s |
| 18.73 s | Play | Playing at 18.79 s |
| 26.98 s | Next | Unchanged; Decibels offers no next track for one file |

Decibels exports MPRIS
only once its window is active, so two earlier runs with the screen off
registered no player; those key presses reached Tarot with nothing to control.

Next and previous were then checked with mpv and its MPRIS plugin playing a
three-track playlist (low, middle and high patterns), without a window:

| Time | Headset key | mpv |
| --- | --- | --- |
| 0.7 s | | Playing "Track 1 low" |
| 18.2 s | Next | "Track 2 middle" |
| 26.0 s | Next | "Track 3 high" |
| 30.0 s | Previous | "Track 2 middle" |
| 37.4 s | Pause | Paused |

## Sound page (revision 66)

With signed r65, the owner reported two problems on the Sound page:

- **Choosing an output was confusing.** Each device had its own "Use for new
  playback" button, which turned into a "Default output" label once chosen, so
  the page rearranged itself with every choice. Revision 66 puts two radio
  lists at the top, **Play sound on** and **Record from**, so the chosen device
  is visible in one place and choosing another moves nothing. Device modes are
  radio buttons too; choosing the current device or mode does nothing.
- **The headset's HFP mode was greyed out.** Tarot showed PulseAudio's state
  correctly: PulseAudio itself reported HFP unavailable. Its log showed that
  the headset's hands-free connection never reached it ("still has disconnected
  profiles: handsfree_head_unit"). A second PulseAudio, owned by the `greetd`
  user, had it. The greeter (Phrog) starts its own PulseAudio, feedbackd,
  keyring, session bus and other services. They daemonize, and nothing ended
  them after login. feedbackd stays connected to the greeter's PulseAudio, so
  it never exits on idle. That PulseAudio registered BlueZ's hands-free profile
  first at boot, so the headset's hands-free link, and with it the microphone,
  went to a process owned by the greeter account. HFP had worked earlier only
  because, in that boot, the greeter's PulseAudio did not hold the profile.

  Stopping the greeter's PulseAudio and letting the session's own PulseAudio
  register made HFP available at once. On the way, reloading the session's
  Bluetooth module crashed PulseAudio 17 (`object_manager_init()` assertion),
  and an autospawned replacement took over; that diagnostic step is not part
  of the fix. Revision 66, with the owner's agreement, installs
  `/etc/elogind/logind.conf.d/40-tarot-greeter-cleanup.conf`
  (`KillUserProcesses=yes`, `KillOnlyUsers=greetd`): when the greeter's session
  ends at login, elogind ends its remaining processes. No other user is
  affected. elogind reads it at startup.

After installing r66 and rebooting, the owner logged in through Phrog. The
greeter's session cgroup existed and held no processes, the session's
PulseAudio was the only one, and HFP was available as soon as the headset
connected. On the Sound page the owner then switched the default output
between speaker and headset 13 times, each read back from PulseAudio within a
second, and switched the headset A2DP → HFP → A2DP. In HFP, PulseAudio made the
headset's speaker and microphone the defaults; back in A2DP, the phone's
microphone returned. Volume steps (100 → 60 → 100%) were checked on r65. The
owner found the new layout fine.

## Tools

- `tests/phone_media_keys_check.py` runs on the phone in the normal session.
  It plays a generated track in Decibels on Tarot's bus and logs input keys
  next to the player's status. The first three runs used an earlier copy that
  read the device through a buffered file, which delayed some key lines by one
  event; the player status lines were unaffected. The committed version, used
  for the locked run, reads unbuffered and adds the kernel's event time.
  `--player mpv` plays the three-track playlist.
- `tests/bluetooth_reachability.py` and `tests/phone_bluetooth_visible.py`
  check reachability, as before.

## Evidence

Kept in `artifacts/m4/media-keys/`: signed `tarot-0.1.0-r65.apk` (SHA-256
`e77f6214ac8fa2f714b668299b3fdc0015a96832e39e1c13902d24ccb5463063`, compositor
`30f20c0e36b37e5da93e7db151ae5e305721346dfd9c5cba9b4a7af7d154d071`, matching
the running process), the package audit, the re-pair log, both HFP recordings,
the r64 button capture, the four r65 key runs and the mpv run. Signed
`tarot-0.1.0-r66.apk` (SHA-256
`7b9a4049672fb4124f55553c2fbc4478dd90427d9bb20e67aacc8530444c2c28`, compositor
`a4a5715407efb5c2da0164b204feed3474f073244069c40d7677fcb6dcfea03d`) with its
audit, the Sound page logs for r65 and r66 and PulseAudio's HFP debug log are
alongside. A first r66 build without the elogind drop-in was never installed. Source archive
`d67a7358a95210f798f8f341cee506e67a75a6d6796908cd3e9a64f3731339ac`.

## Limits

- Bluetooth audio during calls is untested: the phone has no SIM.
- Mute on the headset output from the Sound page was not separately exercised.
- `tests/phone_audio_input_replay.py` now taps the device name in **Record
  from**; it has not been rerun natively since.
- With direct boot there is no greeter at startup, so the cleanup only matters
  after a logout or with the login screen selected; direct boot was not rebooted
  with r66.
- These were native owner-assisted checks, not frozen QA controller runs.
