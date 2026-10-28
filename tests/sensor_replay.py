#!/usr/bin/env python3
"""Private SensorProxy integration: rotation, claims, failures and reconnects.

All providers live on an explicitly private session bus. This never starts a
real sensor service or validates a physical accelerometer, DRM or PAM.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    runtime = tempfile.TemporaryDirectory(prefix='tarot-sensors-')
    address = 'unix:path='+str(Path(runtime.name)/'bus')
    control = Path(runtime.name)/'control.sock'
    backlight = out/'backlight/panel'
    backlight.mkdir(parents=True)
    for name, value in [('max_brightness', 51), ('brightness', 15), ('actual_brightness', 15)]:
        (backlight/name).write_text(str(value))
    env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, LIBGL_ALWAYS_SOFTWARE='1', __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    bus = proc = fixture = sleep_fixture = None
    logs = []
    records = {}

    def log(name):
        stream = (out/name).open('w')
        logs.append(stream)
        return stream

    def start_bus(name):
        process = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--address', address, '--print-address=1'], stdout=subprocess.PIPE, stderr=log(name), text=True)
        assert process.stdout.readline().strip().startswith('unix:')
        return process

    def state(label, condition=lambda s: True, seconds=12):
        value = await_value(lambda: request(control, 'status'), condition, seconds=seconds)
        records[label] = value
        return value

    def oriented(label, orientation):
        return state(label, lambda s: s['sensors']['available'] and s['sensors']['claimed'] and s['sensors']['orientation'] == orientation and s['output']['orientation'] == orientation and s['output']['requested'] is None and s['output']['presented_revision'] == s['output']['revision'])

    def lock(value):
        request(control, 'rotation_lock', locked=value)

    try:
        bus = start_bus('bus-first.log')
        proc = subprocess.Popen([str(root/'target/debug/tarot'), '--system-services', '--platform-bus', address, '--backlight-root', str(backlight.parent), '--control-socket', str(control), '--exit-after-ms', '120000'], env=env, stdout=log('compositor.log'), stderr=subprocess.STDOUT)
        await_value(lambda: control.exists() or proc.poll() is not None, bool)
        assert proc.poll() is None
        state('initially-absent', lambda s: s['sensors']['enabled'] and not s['sensors']['available'])
        request(control, 'rotate', orientation='left-up')
        state('manual-without-sensor', lambda s: s['output']['orientation'] == 'left-up')
        fixture = Fixture(root/'target/debug/examples/sensor-probe', out/'fixture-first', env)
        first = oriented('provider-acquired', 'normal')
        epoch = first['sensors']['epoch']
        fixture.call('set', orientation='left-up')
        oriented('orientation-signal', 'left-up')
        fixture.call('set', orientation='undefined')
        unknown = state('undefined-retains-output', lambda s: s['sensors']['orientation'] is None)
        assert unknown['output']['orientation'] == 'left-up'
        lock(True)
        released = state('rotation-locked', lambda s: s['rotation_locked'] and not s['sensors']['claimed'])
        assert any(e['event'] == 'released' for e in fixture.events())
        fixture.call('set', orientation='right-up')
        time.sleep(.2)
        assert state('locked-ignores-sensor')['output']['orientation'] == 'left-up'
        lock(False)
        assert oriented('rotation-unlocked', 'right-up')['sensors']['epoch'] == epoch
        fixture.call('set', available=False)
        state('hardware-disappeared', lambda s: s['sensors']['available'] and not s['sensors']['has_accelerometer'] and not s['sensors']['claimed'])
        fixture.call('set', available=True, orientation='bottom-up')
        oriented('hardware-returned', 'bottom-up')
        lock(True)
        state('before-denial', lambda s: not s['sensors']['claimed'])
        fixture.call('set', deny=True)
        lock(False)
        denied = state('claim-denied', lambda s: not s['sensors']['available'] and 'denial' in (s['sensors']['error'] or '').lower())
        assert denied['output']['orientation'] == 'bottom-up'
        fixture.call('set', deny=False, orientation='normal')
        restored = oriented('denial-recovered', 'normal')
        assert restored['sensors']['epoch'] > epoch
        fixture.call('release')
        state('owner-lost', lambda s: not s['sensors']['available'])
        fixture.call('set', orientation='left-up')
        time.sleep(.2)
        assert state('old-owner-ignored')['output']['orientation'] == 'normal'
        fixture.call('acquire')
        oriented('owner-returned', 'left-up')
        lock(True)
        state('before-delay', lambda s: not s['sensors']['claimed'])
        fixture.call('set', delay=3000)
        lock(False)
        started = time.monotonic()
        state('responsive-during-call')
        assert time.monotonic()-started < 1
        state('claim-timeout', lambda s: not s['sensors']['available'], seconds=8)
        fixture.call('set', delay=0, orientation='right-up')
        oriented('timeout-recovered', 'right-up')
        fixture.close()
        fixture = None
        state('provider-exited', lambda s: not s['sensors']['available'])
        bus.terminate()
        bus.wait(timeout=4)
        bus = start_bus('bus-second.log')
        fixture = Fixture(root/'target/debug/examples/sensor-probe', out/'fixture-second', env)
        oriented('whole-bus-returned', 'normal')
        request(control, 'system_menu', open=True)
        request(control, 'capture', path=str(out/'rotation-menu.png'))
        state('menu', lambda s: not s['capture_pending'] and not s['capture_error'])
        started = time.monotonic()
        request(control, 'exit')
        code = proc.wait(timeout=4)
        elapsed = time.monotonic()-started
        assert code == 0 and elapsed < 3
        # A second session composes actual private login1 FD/signals with the
        # sensor provider. It never performs kernel suspend.
        sleep_fixture = Fixture(root/'target/debug/examples/sleep-probe', out/'sleep-fixture', env)
        locker_commands, locker_report = out/'locker.commands', out/'locker.jsonl'
        locker_commands.write_text('')
        command = [str(root/'target/debug/tarot'), '--system-services', '--platform-bus', address, '--sleep-coordination', '--lock-client', str(root/'target/debug/examples/lock-probe'), '--control-socket', str(control), '--exit-after-ms', '30000']
        for arg in ['--role', 'locker', '--report', str(locker_report), '--commands', str(locker_commands)]:
            command.append('--lock-client-arg='+arg)
        claims_before = sum(e['event']=='claimed' for e in fixture.events())
        proc = subprocess.Popen(command, env=env, stdout=log('protected-compositor.log'), stderr=subprocess.STDOUT)
        await_value(lambda: control.exists() or proc.poll() is not None, bool)
        assert proc.poll() is None
        state('protected-sensor-start', lambda s: s['lock']['phase']=='Awaiting' and s['lock']['sleep']['armed'])
        from lock_replay import append
        append(locker_commands, 'lock')
        state('protected-owner', lambda s: s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        await_value(fixture.events, lambda es: sum(e['event']=='claimed' for e in es)>claims_before)
        releases_before = sum(e['event']=='released' for e in fixture.events())
        sleep_fixture.call('prepare', start=True)
        prepared = state('sleep-preparing', lambda s: s['lock']['sleep']['gate']['preparing'] and not s['lock']['sleep']['armed'])
        await_value(fixture.events, lambda es: sum(e['event']=='released' for e in es)>releases_before)
        fixture.call('set', orientation='bottom-up')
        time.sleep(.25)
        assert state('sleep-does-not-rotate')['lock']['output']['orientation']==prepared['lock']['output']['orientation']
        sleep_fixture.call('prepare', start=False)
        resumed = state('protected-sensor-resumed', lambda s: s['lock']['sleep']['armed'] and s['lock']['output']['orientation']=='bottom-up' and s['lock']['output']['presented_revision']==s['lock']['output']['revision'] and s['lock']['last_presented_surface'] is True)
        assert resumed['lock']['unprotected_presentations']==0
        request(control, 'exit')
        assert proc.wait(timeout=4)==0
        records['sensor_claim_released_during_sleep_preparation'] = True
        records['validation'] = {'passed': True, 'private_sensor_bus': True, 'capability_and_orientation': True, 'rotation_lock_releases_claim': True, 'claim_denial_timeout_recovery': True, 'name_and_bus_replacement': True, 'compositor_exit': code, 'shutdown_seconds': elapsed, 'physical_sensor_tested': False, 'native_kms_tested': False}
        print('PASS: private SensorProxy rotation, lock/release, missing hardware, denial/timeout and service/bus replacement')
    finally:
        if proc and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        if sleep_fixture:
            sleep_fixture.close()
        if fixture:
            fixture.close()
        if bus and bus.poll() is None:
            bus.terminate()
            bus.wait(timeout=4)
        for stream in logs:
            stream.close()
        runtime.cleanup()
        (out/'status.json').write_text(json.dumps(records, indent=2)+'\n')


if __name__ == '__main__':
    main()
