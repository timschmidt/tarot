# Android apps through Waydroid

This page records the [M6A](DEVELOPMENT_PLAN.md#m6a-android-applications-as-cards-through-waydroid)
investigation. The plan allows its first two steps (platform audit and window
model) to start before M6; no Tarot code integrates Android yet.

## Platform audit, October 31, 2026

On the development PinePhone Pro, running postmarketOS 26.06 with kernel 7.0.0:

| Requirement | Finding |
| --- | --- |
| Packaging | `waydroid` 1.6.2 with `waydroid-openrc` from the distribution, pulling LXC 7.0.0, libgbinder 1.1.45, gbinder-python and iptables |
| Binder | `CONFIG_ANDROID_BINDER_IPC=y` with fixed `binder`, `hwbinder` and `vndbinder` devices; binderfs is not built, and Waydroid uses the fixed devices |
| Containers | Device cgroups, PSI, memfd, veth, bridge and NAT are available; loop and FUSE load as modules |
| Image | `waydroid init -s VANILLA` fetched LineageOS 20.0 (Android 13), system build 2026-09-27 (912 MB download, 1.99 GB image), and the MAINLINE vendor image 2026-10-22 (148 MB, 422 MB) from Waydroid's OTA channel on SourceForge. VANILLA carries no Google applications |
| Rendering | Hardware accelerated. SurfaceFlinger reports `Mali-T860 MC4 (Panfrost)`, OpenGL ES 3.1 with Mesa 26.0.8; buffers come from minigbm (`gbm_mesa`) on `/dev/dri/renderD128`, and `ro.hardware.vulkan` is `panfrost`. Tarot keeps `card1` (rockchip-drm) for the display |
| Boot | Android reported `sys.boot_completed` about 24 s after the session started |
| Memory | 303 MB in use before; 1,700 MB with Android and one app running (the container cgroup reports 2.8 GB including page cache); 333 MB after stopping it, on a 3.8 GB phone |
| Storage | 2.3 GB of images under `/var/lib/waydroid/images`; 88 GB were free |

Image license and updates: the images are LineageOS builds published by the
Waydroid project, mostly Apache 2.0 and GPL components; a full license
inventory of the image has not been made. They update through the OTA URLs in
`/var/lib/waydroid/waydroid.cfg`, and `waydroid upgrade` applies them. Choosing the
VANILLA type is how a Google-free image is selected. Tarot neither ships nor
modifies the image.

### Networking on postmarketOS

The container got no address at first: Android's DHCP requests went
unanswered. postmarketOS's `inet filter` table drops input and forwarding by
default, and nftables requires every base chain on a hook to accept a packet,
so Waydroid's own accepting `lxc` table is not enough. The fix is the
distribution's `waydroid-nftrules`, but installing it only places
`51_waydroid.nft` in `/usr/share/nftables.avail`. It must be linked into
`/etc/nftables.d`, like `50_sshd.nft`, and nftables reloaded.

A reload flushes the `lxc` tables that Waydroid's network script created, so
NAT stops until the container restarts. After restarting it, Android got
192.168.240.112 and reached the internet (about 14 ms to 1.1.1.1, with DNS).
Tarot's Settings integration must order these steps, or recover the container
after any firewall reload.

### Power behavior

With `suspend_action = freeze` (the default), Android asks the container
manager to suspend when it has nothing on screen, and the whole container is
frozen through `lxc-freeze`. While frozen, `waydroid app install` silently does
nothing and `waydroid prop` thaws it briefly. `waydroid` commands that talk to
the session need the session bus, so tools must run with Tarot's environment.

## Window model, first observations

`persist.waydroid.multi_windows` set to `true` gives each Android task its own
Wayland window instead of one full Android screen. F-Droid 1 (APK from
f-droid.org, signature verified against F-Droid's key
`37D2 C987 89D8 3119 4839 4E3E 41E7 044E 1DBA 2E89`) installed with
`waydroid app install` and launched with `waydroid app launch`:

- It opened as Android task 7. Its first-run permission prompt was the resumed
  activity in the same task, so the dialog stays with its owner.
- Android's display bounds changed from 720×1304 to 720×1208, consistent with
  Waydroid following the size Tarot configures for a card (360×604 logical at
  scale 2). Touch arrives as a `wayland_touch` device.
- Android's own launcher stayed an invisible home task.

Tarot's side has not been observed yet: the normal session has no control
socket, and the phone was locked during the launch. Card identity per task,
titles, icons, popups, fullscreen and keyboard reservation still need a
diagnostic run or the owner's eyes.

## Launch and catalog, first observations

Waydroid writes one desktop entry per Android app to
`~/.local/share/applications/waydroid.<package>.desktop`, for example:

```
Name=F-Droid
Exec=waydroid app launch org.fdroid.fdroid
Icon=/home/user/.local/share/waydroid/data/icons/org.fdroid.fdroid.png
Categories=X-WayDroid-App;
```

Tarot's launcher reads these like any other entry, from the next compositor
start. `waydroid app launch` starts the Android session itself when none is
running, which is the lazy start the plan asks for, but it needs the root
container manager. That service is enabled on the development phone for this
evaluation (`rc-update add waydroid-container default`); Android itself only
boots on the first launch. Undo with `rc-update del waydroid-container
default`, or remove the packages and `/var/lib/waydroid`.

## Next

1. Observe Android cards in Tarot: one card per task, titles and icons, close
   semantics, and that no Android launcher card appears.
2. Measure idle power with the container stopped, frozen and running.
3. Text input (Tarot keyboard or Android IME), Back mapping, lock and suspend
   with the container frozen.
