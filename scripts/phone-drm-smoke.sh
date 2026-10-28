#!/bin/sh
# Run on the verified spare phone as root. Keeps the stock session recoverable.
set -eu
[ "$(id -u)" = 0 ] || { echo "Run through an authenticated administrator terminal." >&2; exit 1; }
case "$(tr -d '\000' < /proc/device-tree/model)" in
    *PinePhone\ Pro*) ;;
    *) echo "This experiment is restricted to the verified PinePhone Pro." >&2; exit 1 ;;
esac
tarot_bin=/home/user/tarot-dev/bin/tarot
tarot_logs=/home/user/tarot-dev/artifacts
tarot_seconds=${TAROT_DRM_SECONDS:-45}
tarot_reliability=${TAROT_SUSPEND_RELIABILITY:-0}
case "$tarot_reliability" in 0|1) ;; *) echo "TAROT_SUSPEND_RELIABILITY must be 0 or 1." >&2; exit 1 ;; esac
tarot_max_seconds=300
if [ "$tarot_reliability" = 1 ]; then tarot_max_seconds=900; fi
tarot_text_probe=${TAROT_TEXT_PROBE:-0}
case "$tarot_text_probe" in 0|1) ;; *) echo "TAROT_TEXT_PROBE must be 0 or 1." >&2; exit 1 ;; esac
tarot_lock_probe=${TAROT_LOCK_PROBE:-0}
case "$tarot_lock_probe" in 0|1) ;; *) echo "TAROT_LOCK_PROBE must be 0 or 1." >&2; exit 1 ;; esac
if [ "$tarot_lock_probe" = 1 ]; then [ -x /home/user/tarot-dev/bin/lock-probe ] || { echo "Stage lock-probe first." >&2; exit 1; }; fi
tarot_locker=${TAROT_LOCKER:-0}
tarot_idle=${TAROT_IDLE_PANEL:-0}
case "$tarot_idle" in 0|1) ;; *) echo "TAROT_IDLE_PANEL must be 0 or 1." >&2; exit 1 ;; esac
if [ "$tarot_idle" = 1 ]; then [ "$tarot_locker" = 1 ] || { echo "Idle panel test requires the authenticated locker experiment." >&2; exit 1; }; fi
tarot_sleep=${TAROT_SLEEP_COORDINATION:-0}
case "$tarot_sleep" in 0|1) ;; *) echo "TAROT_SLEEP_COORDINATION must be 0 or 1." >&2; exit 1 ;; esac
if [ "$tarot_sleep" = 1 ]; then [ "$tarot_locker" = 1 ] || { echo "Sleep coordination requires the authenticated locker experiment." >&2; exit 1; }; fi
case "$tarot_locker" in 0|1) ;; *) echo "TAROT_LOCKER must be 0 or 1." >&2; exit 1 ;; esac
[ "$tarot_lock_probe$tarot_locker" != 11 ] || { echo "Select either the synthetic lock probe or authenticated locker." >&2; exit 1; }
if [ "$tarot_locker" = 1 ]; then [ -x /home/user/tarot-dev/bin/tarot-lock ] || { echo "Stage tarot-lock first." >&2; exit 1; }; fi
tarot_lock_recovery=${TAROT_LOCK_RECOVERY:-0}
case "$tarot_lock_recovery" in 0|1) ;; *) echo "TAROT_LOCK_RECOVERY must be 0 or 1." >&2; exit 1 ;; esac
if [ "$tarot_lock_recovery" = 1 ]; then
    [ "$tarot_locker$tarot_lock_probe" = 00 ] || { echo "Select only one locker experiment." >&2; exit 1; }
    [ -x /home/user/tarot-dev/bin/recovery-client ] && [ -x /home/user/tarot-dev/bin/tarot-lock ] || { echo "Stage recovery-client and tarot-lock first." >&2; exit 1; }
fi
tarot_keyboard=${TAROT_KEYBOARD:-0}
tarot_local_session=${TAROT_LOCAL_SESSION:-0}
case "$tarot_local_session" in 0|1) ;; *) echo "TAROT_LOCAL_SESSION must be 0 or 1." >&2; exit 1 ;; esac
if [ "$tarot_local_session" = 1 ]; then [ -x /home/user/tarot-dev/bin/local-session ] || { echo "Stage local-session first." >&2; exit 1; }; fi
case "$tarot_keyboard" in 0|1) ;; *) echo "TAROT_KEYBOARD must be 0 or 1." >&2; exit 1 ;; esac
[ "$tarot_text_probe$tarot_keyboard" != 11 ] || { echo "Select either the text probe or the native keyboard." >&2; exit 1; }
if [ "$tarot_keyboard" = 1 ]; then [ -x /home/user/tarot-dev/bin/tarot-keyboard ] || { echo "Stage tarot-keyboard first." >&2; exit 1; }; fi
case "$tarot_seconds" in *[!0-9]*|'') echo "Invalid test duration." >&2; exit 1 ;; esac
[ "$tarot_seconds" -ge 10 ] && [ "$tarot_seconds" -le "$tarot_max_seconds" ] || { echo "Test duration must be 10–$tarot_max_seconds seconds." >&2; exit 1; }
if [ "$tarot_reliability" = 1 ]; then
    [ "$tarot_sleep$tarot_locker$tarot_local_session" = 111 ] || { echo "Suspend reliability mode requires sleep coordination, authenticated locking and a local session." >&2; exit 1; }
fi
"$tarot_bin" --help | grep -q -- --drm-device || { echo "Build Tarot with --features drm first." >&2; exit 1; }
for tarot_tool in seatd openvt chvt timeout su rc-service dbus-run-session; do command -v "$tarot_tool" >/dev/null; done
[ ! -S /run/seatd.sock ] || { echo "An existing seatd socket must be investigated first." >&2; exit 1; }
[ ! -e /run/user/10000/tarot-drm.sock ] || { echo "An existing Tarot control socket must be investigated first." >&2; exit 1; }
tarot_node=
for tarot_connector in /sys/class/drm/card*-DSI-*; do
    [ -r "$tarot_connector/status" ] || continue
    [ "$(cat "$tarot_connector/status")" = connected ] || continue
    [ -z "$tarot_node" ] || { echo "More than one connected DSI panel." >&2; exit 1; }
    tarot_name=${tarot_connector##*/}
    tarot_node=/dev/dri/${tarot_name%%-DSI-*}
done
[ -n "$tarot_node" ] || { echo "No connected DSI panel." >&2; exit 1; }
mkdir -p "$tarot_logs"
# openvt replaces its own stdio with the VT. The unprivileged child must
# therefore own its log and report its exit separately from the launcher.
: > "$tarot_logs/drm.log"
: > "$tarot_logs/drm-client.exit"
: > "$tarot_logs/drm-test.exit"
: > "$tarot_logs/drm-client.pid"
if [ "$tarot_local_session" = 1 ]; then : > "$tarot_logs/local-session.exit"; fi
chown user:user "$tarot_logs/drm.log" "$tarot_logs/drm-client.exit" "$tarot_logs/drm-client.pid"
if [ "$tarot_text_probe" = 1 ]; then
    [ -x /home/user/tarot-dev/bin/text-probe ] || { echo "Stage text-probe first." >&2; exit 1; }
    : > "$tarot_logs/text-method.commands"
    : > "$tarot_logs/text-method.jsonl"
    chown user:user "$tarot_logs/text-method.commands" "$tarot_logs/text-method.jsonl"
fi
if [ "$tarot_lock_probe" = 1 ]; then
    : > "$tarot_logs/lock-method.commands"
    : > "$tarot_logs/lock-method.jsonl"
    chown user:user "$tarot_logs/lock-method.commands" "$tarot_logs/lock-method.jsonl"
    chmod 600 "$tarot_logs/lock-method.commands" "$tarot_logs/lock-method.jsonl"
fi
if [ "$tarot_locker$tarot_lock_recovery" != 00 ]; then
    [ ! -e "$tarot_logs/locker-ui" ] || { echo "Archive the previous locker-ui captures first." >&2; exit 1; }
    mkdir -m 700 "$tarot_logs/locker-ui"
    chown user:user "$tarot_logs/locker-ui"
fi
tarot_session=$(mktemp /tmp/tarot-drm-session.XXXXXX)
cat > "$tarot_session" <<'TAROT_SESSION'
#!/bin/ash
echo $$ > /home/user/tarot-dev/artifacts/drm-client.pid
if [ "${TAROT_IDLE_PANEL:-0}" = 1 ]; then set -- --idle-lock-ms 6000 --idle-off-ms 10000 "$@"; fi
if [ "${TAROT_SLEEP_COORDINATION:-0}" = 1 ]; then set -- --sleep-coordination "$@"; fi
if [ "${TAROT_LOCK_PROBE:-0}" = 1 ]; then
    set -- --lock-probe-unlocked-start --lock-client /home/user/tarot-dev/bin/lock-probe \
        --lock-client-arg=--role --lock-client-arg=locker \
        --lock-client-arg=--report --lock-client-arg=/home/user/tarot-dev/artifacts/lock-method.jsonl \
        --lock-client-arg=--commands --lock-client-arg=/home/user/tarot-dev/artifacts/lock-method.commands \
        --verify-lock-blank --verify-lock-surface-rgba=336699ff "$@"
fi
if [ "${TAROT_LOCKER:-0}" = 1 ]; then
    set -- --lock-client /home/user/tarot-dev/bin/tarot-lock \
        --lock-client-arg=--capture-dir --lock-client-arg=/home/user/tarot-dev/artifacts/locker-ui \
        --verify-lock-blank "$@"
fi
if [ "${TAROT_LOCK_RECOVERY:-0}" = 1 ]; then
    set -- --lock-client /home/user/tarot-dev/bin/recovery-client \
        --lock-client-arg=--allow-file --lock-client-arg=/home/user/tarot-dev/artifacts/recovery-allow \
        --lock-client-arg=--program --lock-client-arg=/home/user/tarot-dev/bin/tarot-lock --lock-client-arg=-- \
        --lock-client-arg=--capture-dir --lock-client-arg=/home/user/tarot-dev/artifacts/locker-ui \
        --recovery-screenshot /home/user/tarot-dev/artifacts/recovery-own.png --verify-lock-blank "$@"
fi
if [ "${TAROT_KEYBOARD:-0}" = 1 ]; then
    set -- --input-method /home/user/tarot-dev/bin/tarot-keyboard "$@"
fi
if [ "${TAROT_TEXT_PROBE:-0}" = 1 ]; then
    set -- --input-method /home/user/tarot-dev/bin/text-probe \
        --input-method-arg=--role --input-method-arg=method \
        --input-method-arg=--report --input-method-arg=/home/user/tarot-dev/artifacts/text-method.jsonl \
        --input-method-arg=--commands --input-method-arg=/home/user/tarot-dev/artifacts/text-method.commands \
        --input-method-arg=--exit-after-seconds --input-method-arg=300 "$@"
fi
exec /home/user/tarot-dev/bin/tarot --system-services --config-file /home/user/tarot-dev/artifacts/config/settings.toml "$@"
TAROT_SESSION
chmod 755 "$tarot_session"
tarot_seat_pid=
tarot_seat_inode=
tarot_watchdog_pid=
tarot_restore() {
    trap - EXIT INT TERM
    # openvt's timeout can leave a blocked descendant alive. Kill only the
    # recorded unprivileged compositor executable, never a stock shell process.
    tarot_client_pid=$(cat "$tarot_logs/drm-client.pid")
    case "$tarot_client_pid" in
        *[!0-9]*|'') ;;
        *)
            tarot_exe=$(readlink "/proc/$tarot_client_pid/exe" 2>/dev/null || true)
            if { [ "$tarot_exe" = "$tarot_bin" ] || [ "$tarot_exe" = "$tarot_bin (deleted)" ]; } &&
                [ "$(stat -c %u "/proc/$tarot_client_pid" 2>/dev/null || true)" = "$(id -u user)" ]; then
                kill "$tarot_client_pid" 2>/dev/null || true
                sleep 1
                if [ "$(readlink "/proc/$tarot_client_pid/exe" 2>/dev/null || true)" = "$tarot_exe" ]; then
                    kill -KILL "$tarot_client_pid" 2>/dev/null || true
                fi
                # SIGKILL cannot run the compositor's socket destructor. Leave
                # any stale socket for explicit inspection before another run.
            fi
            ;;
    esac
    rm -f "$tarot_session"
    if [ -n "$tarot_seat_pid" ]; then kill "$tarot_seat_pid" 2>/dev/null || true; wait "$tarot_seat_pid" 2>/dev/null || true; fi
    if [ -n "$tarot_seat_inode" ] && [ "$(stat -c %i /run/seatd.sock 2>/dev/null || true)" = "$tarot_seat_inode" ]; then rm -f /run/seatd.sock; fi
    chvt 7 || true
    deallocvt 8 >> "$tarot_logs/drm-launcher.log" 2>&1 || true
    rc-service greetd start
    if [ -n "$tarot_watchdog_pid" ]; then kill "$tarot_watchdog_pid" 2>/dev/null || true; wait "$tarot_watchdog_pid" 2>/dev/null || true; fi
}
trap tarot_restore EXIT INT TERM
# This fallback survives a stalled foreground launcher. The compositor also has
# its own independent exit timer; SSH stays enabled throughout.
( sleep "$((tarot_seconds + 30))"; rc-service greetd start ) > "$tarot_logs/drm-watchdog.log" 2>&1 &
tarot_watchdog_pid=$!
rc-service greetd stop
seatd -u user -l info > "$tarot_logs/seatd.log" 2>&1 &
tarot_seat_pid=$!
for tarot_attempt in 1 2 3 4 5; do [ -S /run/seatd.sock ] && break; sleep 1; done
[ -S /run/seatd.sock ] || { echo "seatd failed to start." >&2; exit 1; }
tarot_seat_inode=$(stat -c %i /run/seatd.sock)
echo "Testing $tarot_node on VT8 for $tarot_seconds seconds; logs: $tarot_logs/drm.log"
tarot_launcher_status=0
if [ "$tarot_local_session" = 1 ]; then
    set -- /bin/ash -c '/home/user/tarot-dev/bin/local-session "$@" 2>/home/user/tarot-dev/artifacts/local-session.log; tarot_session_status=$?; echo "$tarot_session_status" > /home/user/tarot-dev/artifacts/local-session.exit; exit "$tarot_session_status"' tarot-local su
else
    set -- su
fi
timeout -k 5 "$((tarot_seconds + 15))" openvt -c 8 -s -w -- "$@" -s /bin/ash user -c \
    "env TAROT_IDLE_PANEL='$tarot_idle' TAROT_SLEEP_COORDINATION='$tarot_sleep' TAROT_LOCK_RECOVERY='$tarot_lock_recovery' TAROT_LOCKER='$tarot_locker' TAROT_LOCK_PROBE='$tarot_lock_probe' TAROT_KEYBOARD='$tarot_keyboard' TAROT_TEXT_PROBE='$tarot_text_probe' XDG_RUNTIME_DIR=/run/user/10000 LIBSEAT_BACKEND=seatd SEATD_SOCK=/run/seatd.sock RUST_LOG=tarot=debug,smithay=warn dbus-run-session -- '$tarot_session' --publish-session-environment --notifications --drm-device '$tarot_node' --scale 2 --exit-after-ms '$((tarot_seconds * 1000))' --control-socket /run/user/10000/tarot-drm.sock --screenshot '$tarot_logs/drm-app.png' --screenshot-after-ms 3000 -- gtk3-demo < /dev/null > '$tarot_logs/drm.log' 2>&1; tarot_client_status=\$?; echo \$tarot_client_status > '$tarot_logs/drm-client.exit'; exit \$tarot_client_status" \
    > "$tarot_logs/drm-launcher.log" 2>&1 || tarot_launcher_status=$?
echo "$tarot_launcher_status" > "$tarot_logs/drm-launcher.exit"
if [ "$tarot_local_session" = 1 ]; then
    tarot_session_status=$(cat "$tarot_logs/local-session.exit")
    [ "$tarot_session_status" = 0 ] || { echo "Local session helper failed: $tarot_session_status" >&2; exit 1; }
fi
tarot_client_status=$(cat "$tarot_logs/drm-client.exit")
case "$tarot_client_status" in *[!0-9]*|'') echo "Child did not report an exit status." >&2; exit 1 ;; esac
if [ "$tarot_launcher_status" -ne 0 ]; then
    if [ "$tarot_launcher_status" = 8 ] && grep -Fq "Couldn't deallocate console 8" "$tarot_logs/drm-launcher.log"; then
        # kbd openvt exits 8 if seatd has not released its VT descriptor yet.
        # Preserve that result separately; cleanup stops seatd before retrying.
        echo "openvt deferred VT deallocation; compositor status: $tarot_client_status"
    else
        exit "$tarot_launcher_status"
    fi
fi
exit "$tarot_client_status"
