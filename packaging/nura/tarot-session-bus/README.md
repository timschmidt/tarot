# Tarot session bus

This separate package builds D-Bus 1.16.2 for Tarot's ordinary user session on
the Nura 26.06 / Alpine 3.24 AArch64 OpenRC baseline. It installs one executable,
`/usr/libexec/tarot-dbus-daemon`, plus upstream license notices. The distribution
`dbus` package continues to supply the standard session configuration, activation
services, tools, system daemon and shared client library.

`tarot-session` verifies the executable's ownership and permissions, then selects
it with the standard `dbus-run-session --dbus-daemon=…` option. The existing
display-session supervisor still owns its lifetime. No system-bus configuration,
service script, PAM policy, greeter policy or autologin setting is installed by
this dependency.

## Source and feature selection

The [recipe](APKBUILD) pins the official source archive and local patch with
SHA-512 checksums. D-Bus retains its own **AFL-2.1 OR GPL-2.0-or-later** license;
Tarot's MIT/Apache licensing does not replace it. All upstream `LICENSES/*.txt`
notices and `COPYING` accompany the executable.

Upstream's [Unix credential implementation](https://dbus.freedesktop.org/doc/api/html/dbus-sysdeps-unix_8c_source.html)
compiles its peer-process descriptor support only when `SO_PEERPIDFD` is defined.
The verified target's libc socket header omits that constant, although its kernel
supports the operation. The [Meson patch](0001-linux-process-fd.patch) checks the
target libc header first and otherwise reads the target `asm/socket.h` definition.
It does not guess the architecture's numeric value or add unrelated kernel socket
definitions to C source. See the [Linux UAPI](https://raw.githubusercontent.com/torvalds/linux/v7.0/include/uapi/asm-generic/socket.h)
and the [credential specification](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-get-connection-credentials).

The daemon keeps normal session authentication, activation, Unix FD passing,
inotify/epoll, usage checks and statistics. X11 support and modular regression
test builds are enabled. Systemd integration is explicitly disabled for this
OpenRC target, so an SDK's elogind compatibility metadata cannot accidentally
enable it. The baseline has no SELinux/AppArmor/audit development dependencies;
their automatic checks remain visible in the configure report. Documentation
generators are disabled. Review those choices for a different distribution.

Internal libdbus is linked statically into this executable. The package neither
installs a replacement shared libdbus nor relies on the distribution library to
contain the header fix. The runtime still requires a kernel that supplies the
credential; a package version alone is not a capability check. Menu ownership
must reject unavailable or invalid credentials.

## Build and verification

On an ordinary native Alpine AArch64 builder, copy this directory to an aport and
run the standard `abuild -r` workflow. `check()` runs the upstream Meson suite.
The existing [Tarot cross builder](../../../docs/PACKAGING.md) can also supply
`TAROT_SYSROOT` and `TAROT_GCC_RUNTIME`. The recipe adds a Meson SDK overlay and
uses [the clang driver](cross-cc.sh); the SDK must include signature-verified
target Expat, GLib, X11/SM/ICE development dependencies, Linux/libc headers and
GCC runtime objects. Keep that SDK separate from the ordinary Rust phone SDK.
Cross builds skip execution of the target test suite; building its binaries does
not establish that its tests passed.

Before installing, verify the APK signature, source and patch checksums, ELF
architecture, payload ownership/modes, licenses and automatic dependencies.
Require the transaction simulation to contain only this new package. Exercise
the packaged executable as the ordinary phone user first:

```sh
dbus-run-session --dbus-daemon=/absolute/path/to/packaged-daemon -- /absolute/path/to/menu-ownership
```

Repeat with the independent experiment's current/minimum-Rust binaries and GIO
regressions. After launcher installation, repeat on the actual session bus,
without wrapping the commands in another `dbus-run-session`. The bounded
[activation probe](../../../tests/session_bus_activation_probe.py) checks a
temporary `org.tarot.SessionBusProbe` service, inherited `XDG_CURRENT_DESKTOP`,
stable descriptors for both processes, and the legacy PID method. Its caller
must create and remove the temporary service entry and verify helper exit.

Retain normal-login/PAM, app/audio, session teardown, stock system-bus identity,
policy and settings checks alongside the package evidence. Native menu rendering
and actual mapped application exports remain separate acceptance requirements.
