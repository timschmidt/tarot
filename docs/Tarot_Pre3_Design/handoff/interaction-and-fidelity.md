# Tarot Pre 3 / interaction and fidelity audit

Source baseline: HP Pre 3 with webOS 2.2, not Pre 2. Historical gestures, keyboard-first Just Type, stacks and bottom notifications drive this packet. The screenshots are newly drawn concept studies, **not** screenshots from a measured historical firmware build. Numerical geometry and animation values are proposals.

## Behavior and acceptance

| State | Expected behavior | Evidence required |
| --- | --- | --- |
| Focused card | Same client content scales into card | source-to-target screenshot pixel continuity |
| Stacks | Hold to stack, reorder, pull out, close one | full actual device touch replay |
| Quick Launch | Five positions, transient wave, Launcher fixed | gesture sensor and wave timing capture |
| Launcher | Named pages and 3 columns; no persistent dock | reference screenshot and tap mapping |
| Just Type | Physical keyboard begins actions from Card view/Launcher | physical slider key events + search |
| Status | App menu left, connection menu right, centered time | actual service states and menus |
| Notifications | Banner and persistent bottom dashboard | native client, popup and keyboard tests |
| Lock | Protected pre-rendered scene, PAM or local auth | no input/capture disclosure under protection |
| Incoming call | Real modem state plus urgent trusted controls | hardware carrier and audio routing gates |

## Do not silently substitute

- Pre 2 keyboard/gesture layout for Pre 3 hardware.
- A permanent software keyboard or navigation strip.
- Fx0 Home-key or Gold default theme.
- Fake 9:41 in runtime; it is illustrative only.
- Application interiors that a normal Wayland client doesn't own.
- Nura availability for proven driver support.
