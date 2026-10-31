# Bluetooth headset audio and buttons

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

The owner confirmed that the sound stopped and started. Decibels exports MPRIS
only once its window is active, so two earlier runs with the screen off
registered no player; those key presses reached Tarot with nothing to control.

## Tools

- `tests/phone_media_keys_check.py` runs on the phone in the normal session.
  It plays a generated track in Decibels on Tarot's bus and logs input keys
  next to the player's status. The runs above used an earlier copy that read
  the device through a buffered file, which delayed some key lines by one
  event; the player status lines were unaffected. The committed version reads
  unbuffered and adds the kernel's event time.
- `tests/bluetooth_reachability.py` and `tests/phone_bluetooth_visible.py`
  check reachability, as before.

## Evidence

Kept in `artifacts/m4/media-keys/`: signed `tarot-0.1.0-r65.apk` (SHA-256
`e77f6214ac8fa2f714b668299b3fdc0015a96832e39e1c13902d24ccb5463063`, compositor
`30f20c0e36b37e5da93e7db151ae5e305721346dfd9c5cba9b4a7af7d154d071`, matching
the running process), the package audit, the re-pair log, both HFP recordings,
the r64 button capture and the three r65 key runs. Source archive
`d67a7358a95210f798f8f341cee506e67a75a6d6796908cd3e9a64f3731339ac`.

## Limits

- Next and previous were sent by the headset but not checked against a player:
  Decibels with one file offers neither.
- A press while locked was not checked on the phone.
- Tarot's Sound page with the headset card, Bluetooth audio for calls, and
  switching between A2DP and HFP from Tarot remain untested. The profile switch
  above used `pactl`.
- This was a native owner-assisted check, not a frozen QA controller run.
