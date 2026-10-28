# PinePhone Pro development baseline

Initial connection: October 26, 2026 (Detroit; October 27 UTC on the phone).

## Verified initial installation

| Item | Observed value |
| --- | --- |
| Device tree model | Pine64 PinePhone Pro |
| Architecture | AArch64 |
| OS | postmarketOS v24.12, before its Nura rename |
| Kernel | 6.11.11, `linux-pine64-pinephonepro-6.11.11-r1` |
| Device package | `device-pine64-pinephonepro-4-r4` |
| Desktop | Phosh 0.45.0 with Phoc 0.45.0 |
| Graphics packages | Mesa 24.2.8 |
| Service manager | OpenRC, with elogind and tinydm integration |
| Root storage | `/dev/mmcblk2p2`, ext4, approximately 115 GiB |
| Boot storage | `/dev/mmcblk2p1`, approximately 243 MiB |
| Memory | Approximately 3.8 GiB RAM; 1.9 GiB zram swap |
| Display mode | 720 by 1440 |
| GPU nodes | Panfrost GPU at `/dev/dri/card0`; display controller at `/dev/dri/card1`; render node `/dev/dri/renderD128` |
| USB networking | Host `172.16.42.2`, phone `172.16.42.1` |
| Development account | `user`, UID 10000; wheel, input, video, audio, feedbackd groups |
| Connectivity | Wi-Fi has a working default route; APK repositories are reachable |
| Power sample | Battery 90%; USB power online; the battery reported discharging at that instant |
| Services observed | NetworkManager, ModemManager, Phosh/Phoc, SSH |

These observations do not establish call, camera, audio, or suspend reliability. Those require the tests in the development plan. No SIM or carrier baseline has been established.

## Development access

The owner enabled SSH and supplied the account password. Credentials are not stored in this repository. An authenticated administrative SSH terminal is used for package operations. Password-free root access was rejected by automatic approval review and was not configured.

The owner explicitly approved release/build/runtime package installation, keeping SSH enabled at boot, and upgrading/rebooting the spare phone. SSH was added to the OpenRC default runlevel so development access can survive normal reboots. The first added tools are `postmarketos-release-upgrade` and `tmux`.

## Verified final release

The supported sequence 24.12 → 25.06 → 25.12 → 26.06 completed, with a verified reboot at each transition. Final observations on October 27 UTC:

| Item | Observed value |
| --- | --- |
| OS release file | `postmarketOS v26.06` (historical package/OS identifiers remain upstream) |
| Kernel | 7.0.0 |
| Mesa | 26.1.6-r0 |
| Phoc | 0.55.1-r0 |
| Phosh | 0.55.0-r2, after repairing the package retained at 0.51.0 by the distribution upgrade |
| libc | musl 1.2.6-r3 |
| SPI bootloader evidence | Read-only first-MiB sample contains `Tow-Boot TPL 2023.07` and `Tow-Boot SPL 2023.07` |
| Modem firmware | Quectel EG25, revision `EG25GGBR07A08M2G`; ModemManager reports `sim-missing` |
| ALSA playback | PinePhonePro card, `ff880000.i2s-rt5640-aif1`; actual speaker/microphone/call audio unverified |
| Service manager | OpenRC/elogind; greetd with Phrog replaced the original tinydm session entry |
| SSH | Started automatically after reboot |
| Tarot renderer | `Mali-T860 MC4 (Panfrost)`, OpenGL ES 3.1 Mesa 26.1.6 |

An additional `apk upgrade phosh` completed the matching Phosh/libphosh installation. Restarting greetd restored the normal Phosh shell, verified through its process and an actual 720×1440 output capture. A subsequent complete APK upgrade simulation reported no remaining changes. This repaired stock session remains the recovery desktop.

Runtime/build test packages include GTK3 demo, grim, wtype, libinput/GBM/libseat/eudev development files, and pkgconf. The isolated desktop cross-build SDK was resynchronized to these runtime libraries and the official matching musl development archive. Tarot's optimized executable runs unprivileged from `~/tarot-dev/bin`.

Tarot's home, a real GTK3 client, and its transformed overview were captured from its composed framebuffer on Panfrost. A preliminary sample with one GTK card showed Tarot RSS 78,024 KiB and five threads; this single nested-session sample is not a performance budget result. Parent-output inspection first showed the stock lock screen. After normal password authentication and waking Phoc's display, a 720×1440 panel capture showed Tarot and its native GTK client. This nested experiment preceded the direct touch test below. No active SIM is available, as confirmed by the owner.

The subsequent direct DRM session rendered the same scene through the display controller. The owner confirmed native-app taps, bottom-strip overview and card activation. Logs recorded physical touchscreen contacts and changing GTK selection. A VT switch paused/resumed the session and touch continued. A sample after interactive GTK use showed Tarot RSS 97,032 KiB, high-water mark 104,068 KiB and four threads. The successful guarded run recorded compositor/harness exit codes zero, restored greetd/Phrog and removed temporary seatd access. These initial checks do not establish production recovery or suspend reliability.

A stock `rtcwake -m mem -s 10` experiment returned zero and preserved USB/SSH and Wi-Fi routes. Kernel logs showed s2idle entry at monotonic 1854.497 and exit at 1855.214 seconds: it woke early, after about 0.7 seconds, rather than sustaining the intended ten seconds. The available modes are `[s2idle] deep`; deep was not selected or tested. Wi-Fi reported channel-setting warnings after resume. Record this as a stock-platform observation, not a successful ten-second suspend certification. Battery was 84% after this work while attached to the host USB port.

Tow-Boot documents holding volume-up through the second startup vibration for USB mass-storage recovery (blue LED), or volume-down for SD boot (aqua LED). These cold-recovery modes have not been exercised in this session. No SPI/raw-storage write has been performed. Do not apply edge's newer bootloader migration instructions to this verified stable installation without a separate recovery-backed migration. [Tow-Boot device instructions](https://tow-boot.org/devices/pine64-pinephonePro.html)

## Policy authority recovery

The October 27 Wi-Fi experiment exposed a runtime/package mismatch after the earlier provisioning work. Installed `polkit-elogind` was 127-r2, and its executable linked libelogind, but the running authority still referenced a deleted older executable. Libelogind identified Tarot's temporary UID-10000/seat0/VT8 session as local and active. Direct authorization nevertheless failed because the authority tried to read a missing ConsoleKit database. NetworkManager returned authentication-required/denied permissions. The first network replay stopped at its permission precondition before touching the UI or changing a connection.

Restarting the authority loaded the installed elogind backend. All three required Wi-Fi permissions then reported allowed, and the unprivileged native touch replay passed scan, disconnect, activation of the existing profile and Wi-Fi off/on. USB stayed connected and the exact original Wi-Fi profile returned without administrative recovery. Authorization rules and saved network secrets were not changed or read. ModemManager was stopped as a dependency of the policy restart and was restored.

D-Bus activation raced subsequent OpenRC starts, leaving working authority/modem processes outside their normal supervision. This was not accepted as completed service cleanup. After verifying all replay/helper/compositor/harness exits and restoring the stock greeter, the owner-authorized reboot completed. Post-reboot inspection verified a changed boot ID, both daemons parented by their OpenRC supervisors, and started status for polkit, modemmanager, networkmanager, SSH and greetd. The original Wi-Fi profile, enabled radio, USB connection, requested/actual brightness 15 and VT7 were retained. No development processes, temporary sockets, replay touchscreen or VT8 session remained. No init-service definitions or authorization policies were edited.

The native compositor/keyboard hashes are recorded in [implementation status](IMPLEMENTATION_STATUS.md#wi-fi-system-controls); ignored `artifacts/m4/network/` retains separate pre/post-reboot cleanup records and replay evidence. The phone reported 39% and discharging on external power during the run; that is not charging-transition evidence. Routine records omit network names, profile UUIDs and addresses, while private screenshots may reveal nearby SSIDs. The final reboot proves this recovery cycle, not sustained suspend/wake or cold-storage/bootloader recovery.

## Wi-Fi credential experiments

The later connection-setup build created a separate volatile private user profile for the phone's existing personal network. Its unprivileged UI activated that profile and restored the exact original connection; the volatile profile disappeared after disconnection. A root QA fixture then created a distinct memory-only profile without a password. The real NetworkManager daemon called Tarot's registered secret agent, the masked form supplied the credential, and that profile reached Connected. Restoring the original and deleting the QA profile left USB and the GTK card intact. The passing replay used no administrative connection recovery.

The root-only QA script read the spare device's original credential into memory to make the RF test reproducible, supplied it through private development control, and submitted through a real touch on keyboard Enter. A real touch letter/delete was checked separately. No password was put in an evidence file, process argument or routine status/log message. This is not full touch transcription or native remembered-password persistence across a shell restart. Private screenshots can show network names and masked length. No authorization, package or service configuration was changed during these credential tests.

The first profile used NOT_SAVED secret flags and requested a password again before activation. The corrected volatile profile keeps a system-owned secret in NetworkManager's memory and is removed on disconnect. Separate failed QA runs exposed nmcli permission syntax and retained menu-page assumptions; the fixture now validates the syntax offline and resets its starting page. Final build/replay/cleanup evidence is recorded under ignored `artifacts/m4/network-credentials/`, with hashes in [implementation status](IMPLEMENTATION_STATUS.md#connection-setup-and-secret-agent). Battery was approximately 36% and discharging on external power during the experiment; sustained power and charging remain separate observations.

## Initial rendering measurements

An optimized direct-DRM build on the final release was driven through ten app/overview round trips. The sampled counter delta was 32 rendered frames and 260,602 microseconds of CPU scene preparation/GLES submission, averaging **8.14 ms**. Startup-inclusive maximum CPU cost was **41.85 ms**. The counter excludes screenshot readback and precedes scanout queueing; it is neither GPU execution time nor touch-to-photon latency. This is an initial sample, not a sustained frame-rate result.

In a separate settled two-second interval, render/presentation counters remained at 12 and process CPU ticks remained 44 user / 23 system (`CLK_TCK=100`). This demonstrates no observed redraw loop in that particular idle scene, not a long-running power guarantee.

Panfrost fdinfo for DRM client 128 reported 173,084 KiB total memory, 33,708 KiB resident, 8,304 KiB shared and 11,776 KiB purgeable. Two descriptors reported the same client ID and must not be added together. Another client reported zero allocation. Idle fragment/vertex-tiler frequency was 200 MHz against a reported 600 MHz maximum. Driver allocation counters are not interchangeable with compositor RSS or total system GPU use.

## Remembered Wi-Fi and renderer follow-up

A later native replay entered the complete personal-network password through shared-keyboard touches and verified a new private remembered profile's disk credential and root-owned mode-0600 keyfile. A second compositor session reconnected without entry. Original Wi-Fi/radio/USB and brightness 15 were restored, and the test profile/keyfile/UUID ledger removed. NetworkManager stayed running and no reboot occurred. See [revision-specific evidence](IMPLEMENTATION_STATUS.md#remembered-profiles-printable-keyboard-and-resize-recovery).

The same session series exposed Smithay 0.7 SHM damage exceeding a resized client buffer (1432 versus 952 pixels high). The exact upstream clipping backport passes a reduced desktop case and the actual phone keyboard/launcher and hidden-buffer lifecycle replays without GL errors. Cross compilation now supplies Clang with the phone musl sysroot to C probes; both GBM feature probes succeed. Final inspection again found greetd on VT7, normal policy/modem supervision and no test processes/sockets/input devices.

Battery readings fell from about 35% to 32–33% during these experiments while USB supply remained present and UPower reported discharging. Charging capability under the current USB data setup remains unresolved; long endurance/suspend tests need a suitable power setup.

## Playback audio follow-up

The active user endpoint reports PulseAudio 17.0, with an RT5640 internal speaker sink and microphone source, and HiFi (Mic, Speaker) as the card profile. Original speaker values were 57672 on each stereo channel (about 88%), unmuted. Tarot's UID-10000 phone replay passed real touch volume/mute/default controls, independent sound-server readback, a temporary null output and exact restoration. It played/recorded no sound. Earpiece, headphones, microphone, actual acoustic playback and call audio are still unverified.

A temporary ordinary-user subscription client kept the development audio service available while the stock greeter/session was stopped; Tarot itself does not start a daemon. The subscriber and null-sink module were removed after testing. The SDK adds runtime/common libraries plus a fetched development archive without phone package installation. The final session again restored stock VT7, services, original network/radio/USB, brightness 15 and the exact original audio state. Battery was 29% and still reported discharging with USB present. [Revision-specific audio evidence](IMPLEMENTATION_STATUS.md#playback-audio-and-modal-recovery)

## Session-lock protocol follow-up

The optional synthetic lock client passed direct DRM tests with a UID-10000 compositor on the existing 26.06/Panfrost installation. The full-panel opaque frame was acknowledged only after KMS completion. Real uinput key/touch sequences checked held input on entry and exit, fresh locker-only input, clipboard/capture denial, destroyed-role fallback, invalid unlock, owner crash and trusted replacement. VT release/reacquisition produced a new protected presentation; zero extra flips occurred in a subsequent half-second idle sample. App lifecycle and normal keyboard-helper restart retained the locker's held touch. This is protocol evidence, with no authentication UI implemented.

The pre-lock notification and post-unlock power/typing regressions also passed. Root cleanup inspection found stock greetd active on VT7, normal supervised policy/modem daemons, original Wi-Fi/radio/USB retained, brightness 15, no test processes/sockets/uinput devices and no graphics errors. No packages, boot configuration or authentication policy were changed in this experiment. Battery was 27%, still discharging with USB supply present. [Revision-specific evidence and hashes](IMPLEMENTATION_STATUS.md#session-lock-protocol-foundation)

## Upgrade and recovery notes

The selected stable target was v26.06, resolved from the [installation page](https://nura.eco/install/). Each supported transition completed with a verified reboot. This was an in-place package upgrade, so no new OS image was flashed.

The upgrade retained OpenRC, which is the first tested Tarot configuration. Upstream requires reinstallation to migrate from OpenRC to systemd; an in-place package upgrade does not perform that migration. [25.06 release and migration instructions](https://postmarketos.org/blog/2025/06/22/v25.06-release/)

The updates ran in a root-owned tmux session named `tarot-upgrade`. Distribution-tool logs and repository backups remain under `/var/lib/postmarketos-release-upgrade/`; technical recovery backups are under `/var/lib/tarot-provision/`. Tow-Boot identification came from a read-only sample. No SPI or raw storage flashing has been performed.

## Authenticated locker follow-up

The dedicated `tarot-lock` PAM service is installed root:root 0644 under `/etc/pam.d/`, including the existing Nura `base-auth` and `base-account` stacks. Linux-PAM is 1.7.1-r2, and its setgid-shadow Unix helper permits ordinary UID-10000 reauthentication. The graphics client/worker have no set-ID installation. No PAM development package, password/database change, greeter-policy change or new boot service was needed.

A real Rust/egui lock client rendered on Mali-T860/Panfrost and passed wrong physical-key entry, correct touch entry with Cancel, then complete touch re-entry/authentication and normal capture recovery. Killing the client during an actual PAM attempt killed its private worker and retained opaque compositor protection; a fresh trusted client recovered. The final auth harness, compositor and local-session helper returned zero. Its openvt returned the recognized VT-deallocation code 8; stock-session cleanup and explicit VT8 deallocation initially remained busy; clearing kernel console selection allowed deallocation to return 0. A separate native notification/power/keyboard regression passed; its openvt reported the already-recognized deallocation code 8 with all substantive client/helper/harness results zero, followed by successful VT8 reuse in the authentication run.

Root cleanup again found stock greetd on VT7, supervised policy/modem daemons, original Wi-Fi/radio/USB, brightness 15, no test clients/workers/sockets/replay devices and no graphics errors. Battery was 24%, still discharging with USB supply present. Own-client QA captures show masked entry only. The clock follows the phone's current local time configuration; timezone/locale settings and faithful final styling remain future work. [Exact builds, failures and limits](IMPLEMENTATION_STATUS.md#authenticated-rustegui-locker)

## Protected startup and repeated user locking

The current compositor reserves protection before DRM initialization when a locker is configured. A native replay verified a protected first KMS frame and zero ordinary presentations before authentication, automatic fresh-client recovery after an in-flight PAM crash, the real menu Lock tap and a second complete touch authentication with the same deck/view restored. The real locker/keyboard binaries and PAM service were unchanged. The compositor build is recorded in the [current status](IMPLEMENTATION_STATUS.md#protected-startup-reusable-locking-and-recovery).

Cleanup again returned to stock greetd/VT7, original Wi-Fi/radio/USB and brightness 15, normal policy/modem supervision and no test clients/workers/sockets/input devices or graphics errors. Client/local-session/harness exits were zero; openvt retained its recognized deallocation code 8 with raw evidence preserved. Read-only sysfs inspection found `vcs7` present and `vcs8` absent, confirming the test console was deallocated. Battery was 23% and still discharging with USB supply present. The unresolved power setup limits sustained device work, while host development continues.


The subsequent bounded protected-recovery run passes actual KMS recovery presentation, fresh touch/physical Enter retry, held/canceled/multitouch/device-removal isolation, fresh real PAM authentication and original app/clipboard restoration. Ordinary power/modal/text-keyboard behavior passes afterward. The compositor is `b9107fe081b085a8563e45898338a07c6a01cd5ce340c6339e977907c00f13f7`; the actual locker/keyboard/helper and dedicated PAM policy are unchanged. This run adds no packages or persistent device configuration. All client/helper/launcher/harness exits are zero, stock greetd/VT7 is restored, VT8 is deallocated, original services/network/radio/brightness are intact, and no test processes/sockets/input devices or GL errors remain. Battery is 22%, still limiting sustained device work. [Recovery evidence](IMPLEMENTATION_STATUS.md#protected-recovery-after-exhausted-retries)


The opt-in sleep-coordination checkpoint adds no packages, daemon policy or persistent configuration. A real UID-10000 elogind sleep-delay inhibitor remains listed while VT8 is paused after an authoritative prepare signal, disappears after the new protected KMS completion, and is reacquired for the next cycle on a signaled resume. Resume waits for its own new frame. The script uses the daemon's privileged signal-only helper, not Suspend; this does not count toward the plan's stock/sustained kernel sleep gates. Fresh PAM authentication restores the same GTK card/view and capture. Final stock VT7, absent VT8, original services/network/radio/brightness, no development processes/input devices/sockets/inhibitors and zero graphics errors are independently checked. Battery is 21%. [Exact current build/evidence](IMPLEMENTATION_STATUS.md#sleep-preparation-and-resume-presentation)


The idle/panel checkpoint adds opt-in six/ten-second QA timers and a surface idle-inhibit protocol. Native replay verifies a visible app prevents both deadlines, an opaque child and an inactive VT remove the effect, and uncover/return restore it. Four real panel-off cycles use matching protected KMS tickets before clearing the output. Independent DPMS/backlight-power readings are Off/4 and On/0 on wake; requested brightness stays 15. Held release, fresh touch/Enter wake, device removal and sleep-preparation interaction preserve protection, with real PAM and the original app/view/clipboard restored. Final stock VT7/services/network/radio/brightness, no temporary clients/PAM workers/sockets/input devices/inhibitors and zero compositor/helper/harness exits/graphics errors are independently checked. The final openvt launcher retains its recognized raw deallocation code 8, with absent VT8 allocation verified separately. [Final build and exact scope](IMPLEMENTATION_STATUS.md#idle-locking-and-panel-power)

Read-only discovery also finds `freeze mem disk`, default `[s2idle] deep`, rk808 RTC wakealarm support exposed in sysfs and `/usr/sbin/rtcwake`. This is capability inventory, not a programmed alarm or demonstrated suspend/wake. Battery remains at 20% and discharging; endurance work still depends on the unresolved power setup.

## Actual stock suspend and RTC wake

October 27 follow-up on boot `2bbeedc6-5745-43df-b577-68a2ea59ca0a`: normal greetd IPC/PAM authentication launched actual user Phoc/Phosh (UID 10000) on VT7 without changing login policy. A full-resolution capture after a fresh uinput Shift shows the stock lock view. Earlier harness cleanup restored greetd/Phrog's login screen; references to restoring the stock session did not establish an authenticated user desktop. The separate login evidence now makes that distinction explicit.

RTC0 is the rk808 RTC, wake-enabled and within one second of UTC wall time. Installed util-linux `rtcwake` is 2.42.3. With no existing alarm, an awake `-u -d /dev/rtc0 -m on -s 6` alarm wait took 6.43 seconds and left kernel suspend counters unchanged. A subsequent owned 15-second RTC alarm plus normal login1 `Suspend(false)` completed **14.33 seconds of actual suspended time**, measured independently as boottime elapsed minus monotonic elapsed. Kernel success changed 0→1, every failure counter stayed zero, and the same Phoc/Phosh PID/start identities and boot survived. Kernel logs show deep entry/exit; `/sys/power/mem_sleep` changed from `[s2idle] deep` to `s2idle [deep]` through normal elogind handling. The tests did not directly write the mode or kernel suspend request. The owned alarm was cleared afterward.

This does not erase the earlier direct-rtcwake s2idle early-wake observation. It demonstrates the normal elogind/deep/RTC path on the current baseline, not every sleep mode or wake source. Post-resume manual touchscreen/sensor/power-key behavior, carrier wake and sustained energy comparison remain separate gates. Initial Tarot deep suspend also preserves its app and clipboard after fresh PAM authentication. [Exact evidence, interrupted regression and follow-up](IMPLEMENTATION_STATUS.md#actual-kernel-suspend-and-rtc-wake)


The subsequent Tarot regression now passes twenty consecutive normal elogind/RTC deep-suspend cycles: kernel success 35→55, no failures, 9.010–10.253 seconds actually suspended per cycle. Independent delay-FD observation verifies protected KMS before each sleep; all resume barriers and next-cycle inhibitors stay in the same manager epoch. Fresh uinput/libinput PAM, original app/view/clipboard, expected app pixels, acknowledged native key/touch and original network restoration pass each time. App/compositor/helper/openvt/harness exits are zero. The first cleanup observation preceded Phrog startup; settled independent inspection verifies stock greetd/Phrog on VT7, absent VT8/test resources/alarms/inhibitors, original services/network/brightness and unchanged PAM policy. Battery is 17%; this sample is not an energy/endurance comparison. Earlier partial attempts remain recorded separately. [Exact outcomes and scope](IMPLEMENTATION_STATUS.md#actual-kernel-suspend-and-rtc-wake)

Final restoration separately authenticates the user through normal greetd PAM and verifies actual UID-10000 Phoc/Phosh on VT7. Fresh uinput Shift is followed by an inspected 720×1440 green stock lock-view capture; the authenticated-session cleanup passes with no development resources, unchanged PAM policy and original services/network/brightness. This verifies the final displayed recovery state, without claiming manual sensor or stock unlock testing.


## Installed rotation/session follow-up

Revision `tarot-0.1.0-r5` runs through the actual Phrog picker as UID 10000 on VT7 with diagnostics disabled by default. Its separate diagnostic QA session passes four-orientation KMS/touch/keyboard checks, real sensor directions, ten normal login/exit cycles and twenty consecutive RTC deep-suspend cycles (10.124–10.661 seconds). Kernel success advances 111→131, with failures unchanged at zero. A normal 180-second idle test also passes actual DPMS/backlight off/on, successful SensorProxy release/reclaim replies and fresh PAM/app/audio restoration. These are functional results, not sustained latency/power measurements.

Phosh is restored through its visible picker, with a reviewed settled launcher capture and independent empty-cgroup/resource, network, brightness 15, service and policy checks. The initial startup capture and failed identity audit remain separate from that settled pass. The phone was recharged before this work; the final audit reports 43% battery. The existing 500 mA USB data supply can still be insufficient for net charging under load. No SIM, bootloader or storage changes were made. [Detailed outcomes](IMPLEMENTATION_STATUS.md#rotation-and-orientation-checkpoint)


## Persistent-policy installed follow-up

Revision `tarot-0.1.0-r8` is now installed with verified payload hashes and unchanged greetd/PAM policies. Actual touch controls, settings migration/restart, saved panel-off timing, protected landscape startup and touch-keyboard/PAM authentication in every orientation pass. Fresh ten-login and twenty-cycle actual RTC deep-suspend runs pass on these binaries: kernel successes 131→151, failures zero, 10.089–10.659 seconds asleep each cycle. Forced-crash teardown and independent authenticated Phosh recovery pass; fourteen old login cgroups are empty, temporary resources/alarms are absent, and network/services/brightness 15 are restored. The reviewed final capture shows the normal Phosh lock view. Battery is 35%. These checks do not establish physical power/volume-key behavior or sustained energy budgets. [Exact evidence](IMPLEMENTATION_STATUS.md#persistent-display-policy-checkpoint)
