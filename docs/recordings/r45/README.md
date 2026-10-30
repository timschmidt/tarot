# Revision 45: installed session-bus process credentials

October 29, 2026, Detroit. Signed revision 45 and `tarot-session-bus-1.16.2-r0`
are installed on the PinePhone Pro. The normal Tarot login now selects the
packaged session daemon through `dbus-run-session`. Its real `ProcessFD`
capability passes on both authenticated diagnostic and normal session buses.
The distribution system bus and shared client library remain unchanged.

## Change and package boundaries

The [separate package](../../../packaging/nura/tarot-session-bus/README.md) builds
official D-Bus 1.16.2, with one Meson header-detection patch. It reads
`SO_PEERPIDFD` from the target Linux UAPI when libc omits it. It preserves the
OpenRC target's normal daemon features, enables X11 and modular-test builds,
and explicitly disables systemd integration. Its internal libdbus is static;
only Expat and musl are dynamic dependencies. Upstream licenses are installed
with the executable. This is a standard Linux dependency with its own license,
separate from Tarot's Rust code.

Tarot's launcher verifies the installed daemon and selects
`/usr/libexec/tarot-dbus-daemon`. The package adds that dependency. Only the
**session executable** changes relative to revision 44; compositor, keyboard
and locker bytes match exactly. The installed README also updates from its
older snapshot. All other installed payload files, including PAM and licenses,
match. The new bus package installs only its daemon and license notices; it
has no service/configuration/install scripts.

| Artifact | SHA-256 |
| --- | --- |
| Tarot source snapshot | `d08db3daeb6bc7987ff29d37d42fc050edb3c61c92598b112d16c7f5960de12c` |
| Tarot 0.1.0-r45 APK | `f7d6cadb6dee41a9d00ea77851de05ef4e84fb8032e0519a937593e62528cefa` |
| Session-bus 1.16.2-r0 APK | `371056233837255128c79a482e5064ab772414d5d34ab78f05f132ba86ed3ad7` |
| Packaged session daemon | `8088fbaffa066981c65ba23dd0786754cc53e2a6d884eb3b1692b1b99f5ac47f` |
| Tarot session launcher | `3a65cceebd2652f3fceef6ad23c47bc3779d3def381d74f0f28e086d81a74014` |
| Unchanged compositor | `21b7feac6ae51409f71c989c8ea11a3e624070e090d2c41dca44cb0ef5587207` |

Signature and payload audits verify AArch64 ELF identities, root ownership,
ordinary executable permissions, notices and dependencies. The Tarot source
snapshot has 446 locked registry dependencies; 357 frozen Rust/Cargo inputs
match the audited source. Transaction simulations permit only the new bus
package followed by the Tarot-only update. No global key is installed.

## Executed checks

Before installation, the packaged daemon passes four ordinary-user phone cases:
current/minimum-Rust ownership and current/minimum-Rust GIO regressions, plus
an independent activation/environment/credential/legacy-PID probe. Normal
Tarot 44's exact processes and settings remain unchanged during those tests.

After installation, both a diagnostic and a normal greetd/PAM login use the
actual packaged daemon in their own login cgroup. Each passes all four unchanged
experiment binaries, giving eight installed-bus cases. The ownership scenarios
retain actual callback counts and arguments, wrong-client rejection, lifecycle
changes and stale dispatch rejection. Each bus also passes the
[activation probe](../../../tests/session_bus_activation_probe.py): a temporary
service starts as UID 10000, inherits the normal desktop environment, returns
valid process descriptors for itself and its caller, and agrees with the legacy
PID API. The helper exits and its service entry is removed. No normal activation
environment variable is changed.

The installed-session replay passes protected KMS startup, all four protected
orientations, actual touch/PAM unlock, a catalog-launched GTK native app in the
same login session, audio readiness, clean exit to Phrog, empty old login cgroup
and diagnostic-socket removal. A fresh normal Tarot login follows. **Zero kernel
suspend cycles** are requested.

An independent audit verifies current loaded hashes, ordinary-user operation,
local active Wayland login, default Tarot selection, hardware keys enabled,
diagnostics/keyboard metadata disabled, running hardware services, Panfrost DRM
ownership, no replay input devices/alarm/service helper, and the unchanged system
bus process/start identity and stock daemon/library/configuration hashes. Exact
PAM/greeter/elogind policy and user preference/MIME snapshots match before/after.

Host validation passes four session-launcher tests, strict launcher Clippy,
formatting and a Rust 1.89 launcher build. The clean signed production APK builds
all workspace DRM binaries against the target SDK. The earlier 233-test UI
checkpoint remains revision 44's evidence; it is not rerun or relabeled here.
Upstream D-Bus target regression binaries are built, but cross-building skips
execution of its full target suite.

## Retained evidence and limits

The packaged-daemon pre-install archive is
`de4397a589b0260df19b55959b8748787ac9467d7ce89944611b7b7e343963f8`.
The installed native archive is
`60dd2b691e32856e89da6d06b2c81128e193d0fb7efc2d12e69220c656854e75`
(13,118,194 bytes, 161 safe members, 125 staged source/input files). Local
verification checks every collected hash, exact APKs, test sources, reports,
loaded binaries, baseline equality and the independent final audit. Raw
captures, packages, logs, sources containing device details and archives stay
private.

Initial build failures remain retained: abuild's metadata-only sysroot overrides
the first compiler invocation; the SDK then exposes elogind compatibility
metadata that would enable systemd integration. The final driver uses the
verified SDK, and the recipe explicitly selects the OpenRC behavior. A host DRM
check also stops for missing host libseat development metadata; scoped launcher
checks pass without that unrelated feature, and the real DRM APK build passes.
The initial payload audit additionally finds the expected installed README
update; its explicit source comparison passes. None of those failed attempts is
counted as a passing check.

The [stock missing-credential failure and private before/after comparison](../menu-ownership/README.md)
remain valid historical evidence. This delivery resolves the normal-session
credential prerequisite. It does **not** implement compositor export binding,
bounded menu decoding, real mapped GTK/KDE menus, a cooperating Rust/egui
producer or user-facing exported app actions. No global-menu capability is
advertised. Revision 44's Firefox activation failure, broader compatibility,
physical/accessibility/reference review, current-binary endurance and all
unfinished M0–M6 gates remain open.
