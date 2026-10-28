"""Native hardware-key slice for the installed-session controller.

Uses temporary uinput devices, real DRM/backlight readback and the existing
actual-PAM callback. Does not claim physical GPIO button or acoustic coverage.
"""
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess
import time

from lock_replay import any_request
from notification_replay import await_value
from phone_lock_replay import Keyboard


def replay(socket, pid, screen, guard, tap, state, authenticate, capture, before, out):
    initial = state('hardware-enabled', lambda s: s.get('hardware_keys') is True
                    and s['audio']['available'] and s['display_settings']['panel_power_available'])
    assert initial['output']['orientation'] == 'normal'
    account = pwd.getpwuid(10000)
    groups = os.getgrouplist(account.pw_name, account.pw_gid)
    env = dict(os.environ, HOME=account.pw_dir, XDG_RUNTIME_DIR='/run/user/10000')
    for name in ['WAYLAND_SOCKET', 'DISPLAY', 'PULSE_SERVER']:
        env.pop(name, None)

    def pactl(*args):
        return subprocess.check_output(['pactl', *map(str, args)], env=env, text=True,
                                       user=10000, group=account.pw_gid, extra_groups=groups, timeout=8)

    original_default = pactl('get-default-sink').strip()

    def sink():
        return next(o for o in json.loads(pactl('-f', 'json', 'list', 'sinks')) if o['name'] == original_default)

    def percent():
        return round(max(v['value'] for v in sink()['volume'].values())*100/65536)

    original = sink()
    values, muted = [v['value'] for v in original['volume'].values()], original['mute']
    panels = [p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip() == 'connected']
    assert len(panels) == 1
    backlight = Path('/sys/class/backlight/backlight/bl_power')
    records = {'passed': False, 'physical_buttons_tested': False, 'acoustic_output_tested': False,
               'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'original_audio': {'default_sha256': hashlib.sha256(original_default.encode()).hexdigest(),
                                  'values': values, 'muted': muted}}
    keyboard = None

    def panel(off):
        value = await_value(lambda: {'dpms': (panels[0]/'dpms').read_text().strip(),
                                    'backlight_power': int(backlight.read_text())},
                            lambda p: p == {'dpms': 'Off' if off else 'On', 'backlight_power': 4 if off else 0}, seconds=8)
        return value

    def off(label):
        # The reserved opaque KMS frame can power off before the trusted
        # client's lock request arrives. Keep that actual off-time proof even
        # when the later owner begins a new generation on the disabled panel.
        value = state(label, lambda s: s['lock']['phase'] != 'Unlocked'
                      and s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase'] == 'Off'
                      and (proof := s['lock']['idle'].get('off_presentation')) is not None
                      and before['lock']['generation'] < proof['generation'] <= s['lock']['generation']
                      and proof['off_count'] == s['lock']['idle']['off_count']
                      and proof['output_revision'] <= s['lock']['output']['presented_revision'])
        records[label+'-kms-proof'] = value['lock']['idle']['off_presentation']
        assert set(value) == {'ok', 'pid', 'lock'}
        records[label+'-driver'] = panel(True)
        return value

    def wake(label):
        keyboard.tap(116)
        value = state(label, lambda s: s['lock']['phase'] == 'Locked'
                      and s['lock']['last_presented_surface'] is True
                      and s['lock']['last_presented_generation'] == s['lock']['generation']
                      and not s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase'] == 'Awake')
        records[label+'-driver'] = panel(False)
        return value

    def same_app(label):
        restored = authenticate(label)
        assert restored['cards'] == before['cards'] and restored['view'] == before['view']
        return restored

    try:
        keyboard = Keyboard(guard)
        pactl('set-sink-volume', original_default, '40%')
        pactl('set-sink-mute', original_default, '0')
        state('hardware-volume-baseline', lambda s: s['audio']['pending'] is None
              and any(o['default'] and o['percent'] == 40 and not o['muted'] for o in s['audio']['outputs']))
        keyboard.tap(115)
        state('hardware-volume-up', lambda s: s['audio']['pending'] is None
              and any(o['default'] and o['percent'] == 45 for o in s['audio']['outputs']))
        assert percent() == 45
        capture('hardware-volume-feedback')
        keyboard.tap(114)
        state('hardware-volume-down', lambda s: s['audio']['pending'] is None
              and any(o['default'] and o['percent'] == 40 for o in s['audio']['outputs']))
        assert percent() == 40
        keyboard.tap(113)
        state('hardware-mute', lambda s: s['audio']['pending'] is None
              and any(o['default'] and o['muted'] for o in s['audio']['outputs']))
        assert sink()['mute']
        keyboard.tap(113)
        state('hardware-unmute', lambda s: s['audio']['pending'] is None
              and any(o['default'] and not o['muted'] for o in s['audio']['outputs']))
        # Old contacts and held ordinary keys cannot undo an explicit power-off.
        screen.down(180, 350)
        keyboard.key(16, True)
        keyboard.key(116, True)
        first = off('hardware-power-off-held')
        for x in [185, 190, 195]:
            screen.move(x, 350)
        keyboard.key(16, True)
        time.sleep(.6)
        held = off('hardware-old-input-remains-off')
        assert held['lock']['idle']['off_count'] == first['lock']['idle']['off_count']
        screen.up()
        keyboard.key(16, False)
        keyboard.key(116, False)
        keyboard.tap(114)
        await_value(percent, lambda p: p == 35)
        off('hardware-volume-does-not-wake')
        assert any_request(socket, 'capture', path=str(out/'hardware-denied.png')) == {'ok': False, 'error': 'Session is locked'}
        assert not (out/'hardware-denied.png').exists()
        wake('hardware-power-wake-protected')
        same_app('hardware-power-fresh-pam')
        # Pause a real rejected PAM worker before completion. Focus loss must
        # kill/reap it, retain the retry budget and require fresh authentication.
        keyboard.tap(116)
        off('hardware-before-pam-cancel-off')
        protected = wake('hardware-before-pam-cancel-wake')
        locker = protected['lock']['locker']['pid']
        tap(180, 608)
        keyboard.tap(16)  # A synthetic wrong character, never a real credential.
        keyboard.tap(28)

        def workers():
            children = Path(f'/proc/{locker}/task/{locker}/children')
            found = []
            for child in children.read_text().split():
                try:
                    cmd = Path('/proc', child, 'cmdline').read_bytes().split(b'\0')
                    if cmd[1:2] == [b'--pam-worker']:
                        found.append(int(child))
                except FileNotFoundError:
                    pass
            return found

        worker = await_value(workers, lambda ps: len(ps) == 1, seconds=3)[0]
        fd = os.pidfd_open(worker)
        try:
            assert Path(f'/proc/{worker}/exe').resolve() == Path('/usr/bin/tarot-lock')
            assert any(line.split()[1] == '10000' for line in Path(f'/proc/{worker}/status').read_text().splitlines() if line.startswith('Uid:'))
            signal.pidfd_send_signal(fd, signal.SIGSTOP)
            await_value(lambda: Path(f'/proc/{worker}/stat').read_text().rsplit(')', 1)[1].split()[0], lambda s: s == 'T')
            keyboard.tap(116)
            off('hardware-canceled-pam-panel-off')
            await_value(lambda: Path(f'/proc/{worker}').exists(), lambda alive: not alive, seconds=4)
        finally:
            os.close(fd)
        records['real_paused_pam_worker_killed_and_reaped'] = True
        canceled = wake('hardware-canceled-pam-wake')
        assert canceled['lock']['locker']['pid'] == locker
        time.sleep(2.2)  # First canceled submission retains the normal 2s delay.
        assert state('hardware-no-old-auth-completion')['lock']['phase'] == 'Locked'
        same_app('hardware-canceled-pam-fresh-authentication')
        records.update(passed=True, native_drm_and_backlight=True, held_power_single_off=True,
                       old_held_input_cannot_wake=True, volume_while_off_does_not_wake=True,
                       protected_capture_denied=True, fresh_actual_pam_after_power=True,
                       same_native_app_restored=True, input_source='Temporary libinput/uinput keyboard and touchscreen')
    finally:
        if keyboard is not None:
            keyboard.close()
        pactl('set-sink-volume', original_default, *values)
        pactl('set-sink-mute', original_default, int(muted))
        actual = sink()
        records['exact_original_audio_restored'] = (pactl('get-default-sink').strip() == original_default
            and [v['value'] for v in actual['volume'].values()] == values and actual['mute'] == muted)
        assert records['exact_original_audio_restored']
        (out/'hardware-validation.json').write_text(json.dumps(records, indent=2)+'\n')
    return records
