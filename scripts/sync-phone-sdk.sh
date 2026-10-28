#!/bin/sh
# Copies development files from an already-provisioned phone. Uses normal SSH authentication.
set -eu
tarot_repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tarot_sdk=${TAROT_SYSROOT:-"$tarot_repo/target/phone-sysroot"}
tarot_target=${1:-user@172.16.42.1}
set --
if [ -n "${TAROT_SSH_CONTROL:-}" ]; then
    set -- "$@" -S "$TAROT_SSH_CONTROL"
fi
if [ -n "${TAROT_SSH_KNOWN_HOSTS:-}" ]; then
    set -- "$@" -o "UserKnownHostsFile=$TAROT_SSH_KNOWN_HOSTS" -o StrictHostKeyChecking=yes
fi
set -- "$@" "$tarot_target"
mkdir -p "$tarot_sdk"
tarot_transfer=$(mktemp "$tarot_sdk/.transfer.XXXXXX")
trap 'rm -f "$tarot_transfer"' EXIT INT TERM
ssh "$@" 'cd / && tar -chf - lib/ld-musl-aarch64.so.1 usr/lib/libxkbcommon.so* usr/lib/libgcc_s.so* \
    usr/lib/libgbm.so* usr/lib/libinput.so* usr/lib/libseat.so* usr/lib/libudev.so* usr/lib/libdrm.so* \
    usr/lib/libwayland-client.so* usr/lib/libwayland-egl.so* usr/lib/libffi.so* \
    usr/lib/libpulse.so* usr/lib/pulseaudio/libpulsecommon-*.so* \
    lib/libmtdev.so* lib/libevdev.so* usr/include usr/lib/pkgconfig usr/share/pkgconfig' > "$tarot_transfer"
tar -C "$tarot_sdk" -xf "$tarot_transfer"
ssh "$@" 'set -eu
tarot_tmp=$(mktemp -d /tmp/tarot-sdk.XXXXXX)
trap '\''rm -rf "$tarot_tmp"'\'' EXIT INT TERM
apk fetch --no-progress --output "$tarot_tmp" musl-dev >&2
set -- "$tarot_tmp"/musl-dev-*.apk
[ "$#" = 1 ] && [ -f "$1" ]
cat "$1"' > "$tarot_sdk/musl-dev.apk"
# GNU tar is required to read all concatenated members of the signed APK archive.
tar --ignore-zeros --warning=no-unknown-keyword -C "$tarot_sdk" -xf "$tarot_sdk/musl-dev.apk"
ssh "$@" 'set -eu
 tarot_tmp=$(mktemp -d /tmp/tarot-audio-sdk.XXXXXX)
 trap '\''rm -rf "$tarot_tmp"'\'' EXIT INT TERM
 apk fetch --no-progress --output "$tarot_tmp" pulseaudio-dev >&2
 set -- "$tarot_tmp"/pulseaudio-dev-*.apk
 [ "$#" = 1 ] && [ -f "$1" ]
 cat "$1"' > "$tarot_sdk/pulseaudio-dev.apk"
tar --ignore-zeros --warning=no-unknown-keyword -C "$tarot_sdk" -xf "$tarot_sdk/pulseaudio-dev.apk"
ln -sfn libxkbcommon.so.0 "$tarot_sdk/usr/lib/libxkbcommon.so"
ln -sfn libgcc_s.so.1 "$tarot_sdk/usr/lib/libgcc_s.so"
echo "Prepared SDK: $tarot_sdk"
