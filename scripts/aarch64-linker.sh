#!/bin/sh
set -eu
tarot_repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tarot_sysroot=${TAROT_SYSROOT:-"$tarot_repo/target/phone-sysroot"}
if [ ! -f "$tarot_sysroot/usr/lib/libxkbcommon.so.0" ]; then
    echo "Missing phone sysroot at $tarot_sysroot; see docs/BUILDING.md." >&2
    exit 1
fi
tarot_rust_target=${TAROT_RUST_TARGET:-aarch64-unknown-linux-musl}
tarot_rust_libdir=$(rustc --print target-libdir --target "$tarot_rust_target")
if [ -n "${TAROT_GCC_RUNTIME:-}" ]; then
    [ -f "$TAROT_GCC_RUNTIME/crtbeginS.o" ] && [ -f "$TAROT_GCC_RUNTIME/libgcc.a" ] || {
        echo "Missing target GCC runtime objects." >&2; exit 1;
    }
    set -- -B"$TAROT_GCC_RUNTIME" -L"$TAROT_GCC_RUNTIME" "$@"
fi
exec clang --target=aarch64-unknown-linux-musl --sysroot="$tarot_sysroot" \
    -B"$tarot_rust_libdir/self-contained" -fuse-ld=lld "$@"
