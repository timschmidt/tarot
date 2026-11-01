# Waydroid patch series: plan

Status: planning, started November 1, 2026. Nothing here is built or tested yet.

Tarot runs Android apps through Waydroid in multi-window mode: one Tarot card
per Android task (see [WAYDROID.md](WAYDROID.md)). Several problems that remain
are inside Waydroid's Android image or its Wayland composer, not in Tarot. This
plan collects them into one patch series. Tarot carries the series in a fork,
and each patch is offered upstream.

## Principles

- **Behind a property, off by default.** Each change is gated by a
  `persist.waydroid.*` property. Desktop Waydroid users see no difference, and
  each patch can be proposed upstream on its own.
- **Smallest change first.** If a runtime setting or a change on the Tarot side
  solves a problem, use that instead of rebuilding the image.
- **Verified on the phone.** Every patch is accepted only after
  `tests/phone_waydroid_replay.py` passes on the PinePhone, extended with that
  patch's own checks, and the normal session is restored.

## Problems to solve

| # | Problem today | Where it lives | Proposed change |
| --- | --- | --- | --- |
| 1 | Apps open small, in a floating window in the middle of their card | `lineage-sdk` patch, `PlatformService.launchApp` (plain `startActivity`, no launch options) | Launch fullscreen when `persist.waydroid.launch_maximized` is set |
| 2 | An app does not follow its card's size when the card is resized | Composer ([`wayland-hwc.cpp`](https://github.com/waydroid/android_hardware_waydroid/blob/HEAD/hwcomposer/wayland-hwc.cpp)); the task interface has no resize call | Add `resizeTask(id, w, h)` to the platform service, and call it when the window's size change is acknowledged |
| 3 | Android's title bar (back, minimise, maximise, close) duplicates Tarot's own controls | `frameworks/base`, the title bar drawn on floating windows | Likely solved by #1, since fullscreen tasks have no title bar. If not, skip drawing it when `persist.waydroid.hide_caption` is set |
| 4a | Android's keyboard opens as a separate card ("InputMethod"), away from the field being typed into | Composer ([`multi-window.cpp`](https://github.com/waydroid/android_hardware_waydroid/blob/HEAD/hwcomposer/modes/multi-window.cpp)) | Make the keyboard window a child of the focused task's window, so it stays inside that app's card |
| 4b | Tarot's own keyboard cannot follow Android's text focus; it is opened by hand and limited to the US layout | The composer supports no Wayland text-input protocol | Add the standard text-input protocol to the composer, plus a small Waydroid input method in Android that commits the text it receives |
| 5 | Cards show no Android app icon | Composer and platform service | Send each app's icon over the Wayland toplevel-icon protocol; Tarot must support that protocol too |
| 6 | Android's wakelocks (without audio) do not keep the phone awake | Waydroid's container manager (untested) | Measure first; if needed, map Android wakelocks to a sleep block on the phone |

Already working without patches: Back (the strip swipe sends `KEY_BACK`),
closing a card ends its Android task, audio holds Tarot's sleep block, and
sleeping with Android open.

## Stages

**W0: Try runtime settings first: done, negative (November 1, 2026).**
Android 13's `am start --windowingMode 1` asks for a fullscreen task. Two phone
runs on revision 85 used a new probe in `tests/phone_waydroid_replay.py`
(`waydroid_windowing`); both restored the session and passed their audits:

| Attempt | Android's task afterwards |
| --- | --- |
| Start on F-Droid's existing task | Same task #7, `mode=freeform`; title bar shown |
| Start with `NEW_TASK | CLEAR_TASK` | Android reused task #7, `mode=freeform` |
| Close the card (task removed), then start | New task #8, still `mode=freeform` |

Android ignores the launch option: in multi-window mode the task display area
forces freeform for every task. So #1 and #3 need a change inside the image,
either to the display area's windowing mode or to how Waydroid launches tasks.
Tarot's card stayed one "F-Droid" card at 360×652 throughout.

**W1: Build infrastructure.**
- Fork `waydroid/android_vendor_waydroid` (which carries the `lineage-sdk` and
  `frameworks/base` patches) and `waydroid/android_hardware_waydroid` (the
  composer). Change the host `waydroid` tool only if a property needs host
  support.
- Rebuild the stock LineageOS 20 Waydroid image unchanged, following Waydroid's
  build guide, and confirm it boots on the phone and passes the existing replay
  before adding any patch.
- This development host cannot do the build: its disk is full, and a LineageOS
  build needs about 300 GB of disk and plenty of RAM. A build machine is needed.
- Install on the phone as custom images (Waydroid's `waydroid-extra/images`
  directory, or our own update channel through `waydroid init` with system and
  vendor channel URLs), packaged for postmarketOS the same way Tarot itself is.

**W2: Launch fullscreen and follow the card's size (#1, #2, and with luck #3).**
Add phone checks: the window fills its card on first launch of a newly
installed app; resizing the card makes Android resize the task and does not
restart the activity; no title bar appears.

**W3: Hide the title bar (#3)**, only if W2 leaves it showing.

**W4: Keyboard (#4a, then #4b).** For #4a, the check is that Android's keyboard
appears inside the app's card with the field still visible. #4b is the largest
item: the composer side of the text-input protocol, an input method in Android,
and a link between them through the platform service. Its check is that
tapping an Android field shows Tarot's keyboard on its own, text and
backspace reach the field, and the keyboard hides when focus leaves.

**W5: Icons and wakelocks (#5, #6).**

## Owner decisions (November 1, 2026)

1. **Build machine:** this development host, with the tree on its external
   1.9 TB btrfs drive (`waydroid-build/` there: `src/` for the source tree,
   `bin/` for the `repo` launcher, `dl/` for scripts and logs). The host has 16
   cores and 125 GB of RAM.
2. **Forks private until the series is stable;** offer patches upstream after
   that.
3. **Waydroid's test keys,** as its official images use, and **images shipped as
   a postmarketOS package,** packaged like Tarot itself.

The phone runs Waydroid's official `waydroid_arm64` VANILLA image (LineageOS 20
system, MAINLINE vendor), which corresponds to the build target
`lineage_waydroid_arm64-userdebug`.

## Next step

W1 is under way: the LineageOS 20 tree with Waydroid's manifests (from Waydroid's
build guide; the manifest script was read before it ran, and the `repo`
launcher's signature was checked against its maintainers' key) is syncing.
Next, build the unmodified image, ideally in an Ubuntu container to match
LineageOS's supported build environment, and boot it on the phone.
