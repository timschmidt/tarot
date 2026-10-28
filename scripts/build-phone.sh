#!/bin/sh
set -eu
tarot_repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tarot_sdk=${TAROT_SYSROOT:-"$tarot_repo/target/phone-sysroot"}
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="$tarot_sdk"
export PKG_CONFIG_LIBDIR="$tarot_sdk/usr/lib/pkgconfig:$tarot_sdk/usr/share/pkgconfig"
export CC_aarch64_unknown_linux_musl=${CC_aarch64_unknown_linux_musl:-"$tarot_repo/scripts/aarch64-cc.sh"}
cd "$tarot_repo"
exec cargo build --target aarch64-unknown-linux-musl --workspace --features drm --bins "$@"
