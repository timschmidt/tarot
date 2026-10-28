#!/bin/sh
# C feature probes must use the same target headers as the Rust link step.
set -eu
tarot_repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tarot_sysroot=${TAROT_SYSROOT:-"$tarot_repo/target/phone-sysroot"}
[ -f "$tarot_sysroot/usr/include/stdio.h" ] || { echo "Missing phone headers; see docs/BUILDING.md." >&2; exit 1; }
# Tiny feature probes should not need a writable host-wide ccache directory.
export CCACHE_DISABLE=1
exec clang --target=aarch64-unknown-linux-musl --sysroot="$tarot_sysroot" "$@"
