#!/usr/bin/env python3
"""Real nested settings migration/restart, protected startup and sensor policy.

All services/configuration are private fixtures. The locker never authenticates;
native device/KMS/PAM and actual touch controls require separate verification.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib

from lock_replay import append, any_request, events
from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    runtime = tempfile.TemporaryDirectory(prefix='tarot-display-settings-')
    address = 'unix:path='+str(Path(runtime.name)/'bus')
    control = Path(runtime.name)/'control.sock'
    config = out/'settings.toml'
    env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, LIBGL_ALWAYS_SOFTWARE='1',
               __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    env.pop('WAYLAND_SOCKET', None)
    bus = fixture = proc = None
    logs = []
    record = {'sessions': []}
    folder = commands = report = None

    def log(name):
        stream = (out/name).open('w')
        logs.append(stream)
        return stream

    def state(label, condition=lambda s: True):
        value = await_value(lambda:request(control, 'status'), condition, seconds=12)
        record[label] = value
        return value

    def start(name, extra=()):
        nonlocal proc, folder, commands, report
        folder = out/name
        folder.mkdir(mode=0o700)
        commands, report = folder/'locker.commands', folder/'locker.jsonl'
        commands.write_text('')
        argv = [str(root/'target/debug/tarot'), '--config-file', str(config), '--idle-preferences',
                '--lock-client', str(root/'target/debug/examples/lock-probe'),
                '--system-services', '--platform-bus', address, '--notifications',
                '--publish-session-environment', '--control-socket', str(control), '--exit-after-ms', '90000', *extra]
        for arg in ['--role', 'locker', '--report', str(report), '--commands', str(commands)]:
            argv.append('--lock-client-arg='+arg)
        proc = subprocess.Popen(argv, env=env, stdout=log(name+'.log'), stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or proc.poll() is not None, bool)
        assert proc.poll() is None
        covered = state(name+'-startup', lambda s:s['lock']['phase']=='Awaiting' and s['lock']['first_presented_protected'] is True)
        assert set(covered)=={'ok','pid','lock'} and covered['lock']['unprotected_presentations']==0
        assert any_request(control,'idle_timeouts',settings={'lock_after_seconds':60,'off_after_seconds':120,'screen_off_enabled':True})=={'ok':False,'error':'Session is locked'}
        await_value(lambda:events(report), lambda es:any(e['event']=='ready' for e in es))
        append(commands,'lock')
        locked = state(name+'-locked', lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'] and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])
        assert locked['lock']['unprotected_presentations']==0
        append(commands,'unlock')
        return state(name+'-ordinary', lambda s:s['lock']['phase']=='Unlocked' and s['notifications']['available'])

    def stop():
        request(control,'exit')
        code = proc.wait(timeout=5)
        assert code==0 and not control.exists()
        record['sessions'].append({'name':folder.name,'exit':code})

    def saved(label, expected=True):
        return state(label, lambda s:s['display_settings']['settings_saved']==expected and
                     s['notifications']['settings_saved']==expected and
                     (not s['display_settings']['settings_error'] if expected else bool(s['display_settings']['settings_error'])))

    try:
        bus = subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address,'--print-address=1'],
                               stdout=subprocess.PIPE,stderr=log('bus.log'),text=True)
        assert bus.stdout.readline().strip().startswith('unix:')
        fixture = Fixture(root/'target/debug/examples/sensor-probe',out/'sensor',env)
        fixture.call('set',orientation='right-up')
        original = 'version = 1\n[notifications]\ndnd = true\nhide_previews = true\n'
        config.write_text(original)
        first = start('migration')
        assert first['notifications']['dnd'] and first['notifications']['hide_previews']
        assert config.read_text()==original
        state('live-sensor',lambda s:s['sensors']['claimed'] and s['output']['orientation']=='right-up')
        request(control,'rotate',orientation='left-up')
        state('manual-landscape',lambda s:s['output']['orientation']=='left-up' and s['output']['presented_revision']==s['output']['revision'])
        request(control,'rotation_lock',locked=True)
        state('rotation-frozen',lambda s:s['rotation_locked'] and not s['sensors']['claimed'])
        idle = {'lock_after_seconds':60,'off_after_seconds':120,'screen_off_enabled':True}
        # Interleave two real UI-command paths; each saved file must keep the
        # latest value of both sections rather than the last section written.
        for n in range(6):
            request(control,'notification_action',action={'Settings':{'dnd':bool(n%2),'hide_previews':True}})
            request(control,'idle_timeouts',settings={**idle,'lock_after_seconds':30+n})
        request(control,'idle_timeouts',settings=idle)
        request(control,'notification_action',action={'Settings':{'dnd':True,'hide_previews':False}})
        state('interleaved-final',lambda s:s['notifications']['dnd'] and not s['notifications']['hide_previews'] and s['display_settings']['idle']==idle)
        saved('both-sections-saved')
        data = tomllib.loads(config.read_text())
        assert data['version']==5 and data['notifications']=={'dnd':True,'hide_previews':False}
        assert data['feedback']=={'notifications':True}
        assert data['display']=={'rotation_locked':True,'locked_orientation':'left-up','idle':idle}
        assert config.stat().st_mode&0o777==0o600
        # Both malformed values and output-off-before-lock are rejected without
        # changing the last good disk file or the running policy.
        prior = config.read_bytes()
        for invalid in [dict(idle,lock_after_seconds=0),dict(idle,off_after_seconds=15)]:
            assert not any_request(control,'idle_timeouts',settings=invalid)['ok']
        assert config.read_bytes()==prior
        stop()
        second = start('restored')
        startup = record['restored-startup']['lock']
        assert startup['output']['orientation']=='left-up'
        assert startup['idle']['policy']['lock_after']=={'secs':60,'nanos':0}
        assert startup['idle']['policy']['off_after'] is None  # Nested backend never powers off the host.
        assert second['rotation_locked'] and second['display_settings']['idle']==idle and second['notifications']['dnd']
        state('restored-sensor-released',lambda s:s['sensors']['available'] and not s['sensors']['claimed'])
        request(control,'system_menu',open=True)
        request(control,'capture',path=str(out/'landscape-settings.png'))
        state('landscape-menu',lambda s:not s['capture_pending'] and not s['capture_error'])
        stop()
        overridden = start('explicit-overrides',['--orientation','bottom-up','--idle-lock-ms','90000'])
        assert overridden['output']['orientation']=='bottom-up' and overridden['rotation_locked']
        assert overridden['lock']['idle']['policy']['lock_after']=={'secs':90,'nanos':0}
        assert not overridden['display_settings']['idle_configurable']
        assert not any_request(control,'idle_timeouts',settings=idle)['ok']
        assert config.read_bytes()==prior
        stop()
        for name, content in [('newer','version = 99\n'),('invalid-idle','version = 2\n[display.idle]\nlock_after_seconds = 0\n')]:
            config.write_text(content)
            start(name)
            saved(name+'-preserved',False)
            request(control,'rotation_lock',locked=True)
            request(control,'idle_timeouts',settings=idle)
            request(control,'notification_action',action={'Settings':{'dnd':True,'hide_previews':False}})
            state(name+'-session-only',lambda s:s['rotation_locked'] and s['display_settings']['idle']==idle and s['notifications']['dnd'])
            saved(name+'-still-unsaved',False)
            assert config.read_text()==content
            stop()
        config.unlink()
        start('write-recovery')
        config.mkdir()
        request(control,'rotation_lock',locked=True)
        saved('write-failed',False)
        config.rmdir()
        request(control,'idle_timeouts',settings=idle)
        saved('write-retried')
        assert tomllib.loads(config.read_text())['display']['rotation_locked']
        stop()
        record['validation']={'passed':True,'version_one_migration':True,'interleaved_sections_preserved':True,
                              'locked_landscape_restored_before_first_frame':True,'protected_startup_every_session':True,
                              'sensor_claim_released_when_restored_locked':True,'explicit_overrides':True,
                              'invalid_files_and_values_preserved':True,'real_write_failure_recovery':True,
                              'native_kms_or_pam_tested':False,'touch_controls_tested':False}
        print('PASS: shared preferences migration, concurrent sections, protected restart, sensor policy and failures')
    finally:
        if proc is not None and proc.poll() is None:
            proc.terminate()
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
        if fixture is not None:fixture.close()
        if bus is not None and bus.poll() is None:bus.terminate();bus.wait(timeout=4)
        for stream in logs:stream.close()
        runtime.cleanup()
        (out/'status.json').write_text(json.dumps(record,indent=2)+'\n')


if __name__=='__main__':main()
