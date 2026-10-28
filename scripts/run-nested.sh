#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

# Opt-in workaround for the NVIDIA rendering issue recorded in IMPLEMENTATION_STATUS.
if [ "${TAROT_SOFTWARE_RENDERER:-0}" = 1 ]; then
    tarot_mesa_vendor=/usr/share/glvnd/egl_vendor.d/50_mesa.json
    if [ ! -f "$tarot_mesa_vendor" ]; then
        echo "Mesa EGL vendor configuration not found: $tarot_mesa_vendor" >&2
        exit 1
    fi
    export __EGL_VENDOR_LIBRARY_FILENAMES="$tarot_mesa_vendor"
    export LIBGL_ALWAYS_SOFTWARE=1
fi

exec dbus-run-session -- cargo run --bin tarot -- --publish-session-environment --notifications "$@"
