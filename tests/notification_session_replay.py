#!/usr/bin/env python3
"""Isolated real notification protocol, bus recovery and settings regressions.

Build tarot and --example notification-probe first. Requires a desktop Wayland
session, Mesa's software EGL vendor and dbus-daemon/gdbus. Each case gets its own
bus, owner-only control directory and settings file, outside desktop services.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib
from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def run_case(args, case):
    output = args.output / case
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    config = output / 'settings.toml'
    environment = dict(os.environ, LIBGL_ALWAYS_SOFTWARE='1',
                       __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    bus = holder = compositor = fixture = None
    bus_directory = tempfile.TemporaryDirectory(prefix='tarot-test-bus-')
    control = Path(bus_directory.name) / 'control.sock'
    address = 'unix:path=' + str(Path(bus_directory.name) / 'bus')
    environment['DBUS_SESSION_BUS_ADDRESS'] = address
    logs = []
    result = {'case': case}

    def log(name):
        stream = (output / name).open('w')
        logs.append(stream)
        return stream

    def start_bus(name):
        process = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--address', address, '--print-address=1'],
                                   stdout=subprocess.PIPE, stderr=log(name), text=True)
        assert process.stdout.readline().strip().startswith(address), 'Private bus failed'
        return process

    def launch(name):
        process = subprocess.Popen([str(args.tarot), '--publish-session-environment', '--notifications',
                                    '--config-file', str(config), '--control-socket', str(control), '--exit-after-ms', '120000'],
                                   env=environment, stdout=log(name), stderr=subprocess.STDOUT)
        await_value(lambda: control.exists() or process.poll() is not None, bool)
        assert process.poll() is None, (process.returncode, output)
        return process

    def status(condition=lambda s: s['notifications']['available']):
        return await_value(lambda: request(control, 'status'), condition, seconds=8)

    def settings(dnd, privacy, saved=True):
        request(control, 'notification_action', action={'Settings': {'dnd': dnd, 'hide_previews': privacy}})
        return status(lambda s: s['notifications']['dnd'] == dnd and s['notifications']['hide_previews'] == privacy
                      and s['notifications']['settings_saved'] == saved and
                      (not s['notifications']['settings_error'] if saved else bool(s['notifications']['settings_error'])))

    def stop():
        request(control, 'exit')
        code = compositor.wait(timeout=5)
        assert code == 0
        return code

    def name_owner():
        return subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus',
                                        '--object-path', '/org/freedesktop/DBus', '--method',
                                        'org.freedesktop.DBus.GetNameOwner', 'org.freedesktop.Notifications'],
                                       env=environment, timeout=4, text=True).strip()

    try:
        if case != 'start-offline':
            bus = start_bus('bus.log')
        if case == 'collision':
            report, commands = output / 'holder.jsonl', output / 'holder-commands.jsonl'
            commands.write_text('')
            holder = subprocess.Popen([str(args.probe), '--hold-name', '--report', str(report), '--commands', str(commands)],
                                      env=environment, stdout=log('holder.log'), stderr=subprocess.STDOUT)
            await_value(lambda: report.read_text() if report.exists() else '', lambda v: 'owning-name' in v)
            result['owner_before'] = name_owner()
        elif case == 'invalid-settings':
            config.write_text('version = 99\n[notifications]\ndnd = true\n')
        compositor = launch('session.log')
        if case == 'protocol':
            replay = subprocess.run(['python3', str(Path(__file__).with_name('notification_replay.py')),
                                     '--socket', str(control), '--probe', str(args.probe), '--output', str(output / 'replay')],
                                    env=environment, timeout=45)
            result['replay_exit'] = replay.returncode
            assert replay.returncode == 0
        elif case == 'collision':
            result['unavailable'] = status(lambda s: not s['notifications']['available'] and 'claiming' in (s['notifications']['error'] or ''))
            result['owner_after'] = name_owner()
            time.sleep(.8)  # Retries must still leave the existing owner intact.
            assert name_owner() == result['owner_after'] == result['owner_before'] and holder.poll() is None
            request(control, 'notifications', open=True)
            assert request(control, 'status')['notifications']['dashboard_open']
            commands.write_text('{"command":"quit"}\n')
            assert holder.wait(timeout=4) == 0
            result['recovered_after_release'] = status()
            fixture = Fixture(args.probe, output / 'after-release', environment)
            identifier = fixture.notify(summary='Name became available', timeout=0)
            status(lambda s: any(n['id'] == identifier for n in s['notifications']['items']))
        elif case == 'start-offline':
            result['unavailable_at_start'] = status(lambda s: not s['notifications']['available'] and bool(s['notifications']['error']))
            result['offline_settings'] = settings(True, True)
            bus = start_bus('late-bus.log')
            result['recovered'] = status()
            assert result['recovered']['pid'] == result['unavailable_at_start']['pid']
            fixture = Fixture(args.probe, output / 'late-client', environment)
            identifier = fixture.notify(summary='Late bus with restored policy', timeout=0)
            received = status(lambda s: any(n['id'] == identifier for n in s['notifications']['items']))
            assert received['notifications']['banner'] is None
        elif case == 'offline-exit':
            status()
            bus.terminate()
            result['bus_exit'] = bus.wait(timeout=4)
            result['unavailable'] = status(lambda s: not s['notifications']['available'] and bool(s['notifications']['error']))
            result['offline_settings'] = settings(True, True)
            assert not result['offline_settings']['notifications']['available']
            request(control, 'notifications', open=True)
            assert request(control, 'status')['notifications']['dashboard_open']
        elif case == 'bus-recovery':
            before = status()
            fixture = Fixture(args.probe, output / 'before-loss', environment)
            identifier = fixture.notify(summary='Old bus owner', actions=['reply', 'Reply'], resident=True, timeout=0)
            old = status(lambda s: bool(s['notifications']['items']))['notifications']['items'][0]
            bus.terminate()
            result['bus_exit'] = bus.wait(timeout=4)
            result['unavailable'] = status(lambda s: not s['notifications']['available'] and bool(s['notifications']['error']))
            retired = next(n for n in result['unavailable']['notifications']['items'] if n['id'] == identifier)
            assert not retired['live'] and retired['orphaned'] and retired['action_count'] == 0
            assert result['unavailable']['notifications']['banner'] is None
            fixture.close()
            fixture = None
            result['offline_settings'] = settings(True, True)
            assert not result['offline_settings']['notifications']['available']
            bus = start_bus('restarted-bus.log')
            result['recovered'] = status()
            assert result['recovered']['pid'] == before['pid']
            fixture = Fixture(args.probe, output / 'after-recovery', environment)
            new_id = fixture.notify(replaces_id=identifier, summary='New bus owner', actions=['reply', 'Reply'], resident=True, timeout=0)
            assert new_id > identifier
            request(control, 'notification_action', action={'Invoke': {'id': identifier, 'revision': old['revision'], 'key': 'reply'}})
            time.sleep(.15)
            assert not any(e['event'] == 'action' for e in fixture.events())
            new = next(n for n in status(lambda s: any(n['id'] == new_id for n in s['notifications']['items']))['notifications']['items'] if n['id'] == new_id)
            request(control, 'notification_action', action={'Invoke': {'id': new_id, 'revision': new['revision'], 'key': 'reply'}})
            fixture.signal('action', new_id, key='reply')
            assert settings(False, False)['notifications']['available']
        elif case == 'settings':
            status()
            assert not config.exists()
            result['saved'] = settings(True, True)
            assert tomllib.loads(config.read_text()) == {'version': 6, 'notifications': {'dnd': True, 'hide_previews': True}, 'display': {'rotation_locked': False, 'locked_orientation': 'normal', 'idle': {'lock_after_seconds': 120, 'screen_off_enabled': True, 'off_after_seconds': 180}, 'sleep': {'automatic': False, 'after_off_seconds': 60, 'on_external_power': False}}, 'feedback': {'notifications': True}}
            assert config.stat().st_mode & 0o777 == 0o600
            result['first_compositor_exit'] = stop()
            assert not control.exists()
            compositor = launch('restarted-session.log')
            result['restored'] = status()
            assert result['restored']['notifications']['dnd'] and result['restored']['notifications']['hide_previews']
            fixture = Fixture(args.probe, output / 'restored-client', environment)
            identifier = fixture.notify(summary='Suppressed by restored DND', timeout=0)
            restored = status(lambda s: any(n['id'] == identifier for n in s['notifications']['items']))
            assert restored['notifications']['banner'] is None
        elif case == 'invalid-settings':
            original = config.read_bytes()
            result['load_error'] = status()
            assert result['load_error']['notifications']['settings_error']
            assert not result['load_error']['notifications']['dnd']
            result['session_only_change'] = settings(True, True, saved=False)
            assert config.read_bytes() == original
        elif case == 'save-retry':
            status()
            config.mkdir()  # A real filesystem failure, reversible without permissions changes.
            result['write_error'] = settings(True, True, saved=False)
            assert result['write_error']['notifications']['available'] and config.is_dir()
            request(control, 'notifications', open=True)
            request(control, 'capture', path=str(output / 'settings-error.png'))
            status(lambda s: not s['capture_pending'] and not s['capture_error'])
            config.rmdir()
            result['write_recovered'] = settings(False, True)
            assert tomllib.loads(config.read_text())['notifications'] == {'dnd': False, 'hide_previews': True}
        if fixture is not None:
            fixture.close()
            fixture = None
        started = time.monotonic()
        result['compositor_exit'] = stop()
        result['shutdown_seconds'] = time.monotonic() - started
        result['passed'] = True
        print('PASS:', case, output, flush=True)
    finally:
        if fixture is not None:
            fixture.close()
        for process in (compositor, holder, bus):
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        for stream in logs:
            stream.close()
        bus_directory.cleanup()
        (output / 'validation.json').write_text(json.dumps(result, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tarot', type=Path, default=Path('target/debug/tarot'))
    parser.add_argument('--probe', type=Path, default=Path('target/debug/examples/notification-probe'))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    args.tarot, args.probe = args.tarot.resolve(), args.probe.resolve()
    args.output = args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix='tarot-notification-sessions-'))
    args.output.mkdir(parents=True, exist_ok=True)
    for case in ('protocol', 'collision', 'bus-recovery', 'settings', 'invalid-settings', 'save-retry', 'offline-exit', 'start-offline'):
        run_case(args, case)


if __name__ == '__main__':
    main()
