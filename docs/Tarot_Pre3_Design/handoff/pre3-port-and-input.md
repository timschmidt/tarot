# Tarot / HP Pre 3 integration handoff

**Scenario:** Assume Nura/postmarketOS has been ported to the Pre 3, per the user's request. This document does **not** claim that the required display, input, modem, graphics, or suspend drivers are verified.

## Core profile

- Native LCD 480 × 800. Design coordinate space 360 × 600, output scale 4/3; verify fractional compositor scaling support before production.
- Status height 24 units; no on-screen gesture strip in normal operation.
- Hardware gesture sensor directly below display is privileged input owned by the shell; its Linux evdev node and reports require discovery.
- Physical QWERTY slider; Option, Shift, Sym, Enter and key repeat require input mapping tests. Slider-open state does not change LCD dimensions or imply rotation.
- Quick Launch has five total positions, four editable and a fixed Launcher slot.
- Full portrait display used by application excluding status and transient dashboard; software IME only when needed.
- No default Waydroid support claim; 512 MB memory requires profiling and binder/GPU validation.

## Input recognizer contract

1. Hardware-origin gesture-area events are distinct from touchscreen events. Do not misclassify app touches as navigation.
2. Recognize center tap, back swipe, upward navigation, upward-hold Quick Launch wave, optional advanced full-swipe.
3. Cancel recognizers on lost hardware, lock, incoming call, rotation, modal grab, client disappearance and second contacts.
4. Synthetic keyboard events must never target a protected lock screen unless originating from trusted physical keyboard routing.
5. Provide accessible soft navigation fallback if the gesture sensor driver is absent, but label it a compatibility deviation.

## Rendering and resource limits

- The Pre 3 has Adreno-class graphics (not PinePhone Pro Panfrost). Select verified DRM/KMS, GBM, GLES and buffer import paths.
- Favor direct compositing of client surfaces and budget shadows; do not hold full-screen texture copies for every historical card.
- Proposed 3 retained previews and 12 MiB preview cache are **starting budgets**, not measured targets.
- Run low-memory, high-thermal, app-crash, suspend and power-key sequences before claiming hardware readiness.

## App compatibility

GTK/Qt/Firefox/Thunderbird remain normal Wayland clients. Cooperative browser/mail bridges may supply semantic card metadata or promote a tab/message to its own native toplevel. Never rasterize app interiors as shell-owned screens. Any Android/Waydroid support is optional, subject to driver and capacity evidence.

## Legal provenance

All vector UI studies in this generated packet are newly constructed for Tarot and are provided as reference/source assets. The historical HP guide is linked, not copied. Lato is referred to as a system typography choice; this packet does not distribute font files. The user should assign the intended project license and preserve third-party notices when distributing integrated assets.
