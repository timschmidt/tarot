#!/bin/sh
set -eu
: "${TAROT_SYSROOT:?verified AArch64 SDK required}"
: "${TAROT_GCC_RUNTIME:?target GCC runtime required}"
export CCACHE_DISABLE=1
for tarot_argument in "$@"; do
	case "$tarot_argument" in
		-c|-E|-S)
			exec clang --target=aarch64-alpine-linux-musl "$@" --sysroot="$TAROT_SYSROOT"
			;;
	esac
done
exec clang --target=aarch64-alpine-linux-musl "$@" --sysroot="$TAROT_SYSROOT" \
	-B"$TAROT_GCC_RUNTIME" -L"$TAROT_GCC_RUNTIME" -fuse-ld=lld
