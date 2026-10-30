#!/usr/bin/env python3
"""Save settings by touch, then verify them in a separate bounded phone session."""
import argparse
import json
import os
from pathlib import Path
import time
import tomllib
from notification_replay import Fixture, await_value, session_environment
from phone_gesture_replay import Touchscreen, request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', choices=('save', 'restore'), required=True)
    parser.add_argument('--socket', type=Path, default=Path('/run/user/10000/tarot-drm.sock'))
    parser.add_argument('--config', type=Path, default=Path('/home/user/tarot-dev/artifacts/config/settings.toml'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    owner = args.socket.stat()
    os.chown(args.output, owner.st_uid, owner.st_gid)
    initial = await_value(lambda: request(args.socket, 'status'), lambda s: s['notifications']['available'])
    records = {'initial': initial}
    screen = fixture = None

    def status(label, condition=lambda s: True):
        value = await_value(lambda: request(args.socket, 'status'), condition)
        assert value['pid'] == initial['pid']
        records[label] = value
        return value

    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty8'
        assert request(args.socket, 'status')['pid'] == initial['pid']

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.15)

    def saved(label, expected):
        value = status(label, lambda s: s['notifications']['dnd'] == expected and
                       s['notifications']['hide_previews'] == expected and s['notifications']['settings_saved'])
        assert value['notifications']['settings_error'] is None
        preferences = tomllib.loads(args.config.read_text())
        assert preferences['version'] == 5
        assert preferences['notifications'] == {'dnd': expected, 'hide_previews': expected}
        assert preferences['feedback'] == {'notifications': True}
        assert args.config.stat().st_mode & 0o777 == 0o600
        assert args.config.parent.stat().st_mode & 0o777 == 0o700
        assert args.config.stat().st_uid == owner.st_uid
        return value

    def capture(name):
        request(args.socket, 'capture', path=str(args.output / (name + '.png')))
        status('capture-' + name, lambda s: not s['capture_pending'] and not s['capture_error'])

    try:
        expected = args.phase == 'restore'
        assert initial['notifications']['dnd'] == expected and initial['notifications']['hide_previews'] == expected
        if args.phase == 'restore':
            saved('restored-before-input', True)
        screen = Touchscreen(guard)
        tap(40, 710)
        status('dashboard-open', lambda s: s['notifications']['dashboard_open'])
        time.sleep(.3)
        if args.phase == 'save':
            tap(50, 300)
            status('dnd-on', lambda s: s['notifications']['dnd'])
            tap(50, 352)
            saved('saved-by-touch', True)
            capture('preferences-saved')
        else:
            capture('preferences-restored')
        tap(40, 710)
        status('dashboard-closed', lambda s: not s['notifications']['dashboard_open'])
        fixture = Fixture(Path('/home/user/tarot-dev/bin/notification-probe'), args.output / 'sender',
                          session_environment(initial['pid']), owner)
        identifier = fixture.notify(summary='Suppressed by persistent DND', timeout=0)
        received = status('suppressed-by-saved-setting', lambda s: any(n['id'] == identifier for n in s['notifications']['items']))
        assert received['notifications']['banner'] is None and received['view'] == initial['view']
        if args.phase == 'restore':
            tap(40, 710)
            status('dashboard-reopened', lambda s: s['notifications']['dashboard_open'])
            time.sleep(.25)
            tap(50, 300)
            status('dnd-off', lambda s: not s['notifications']['dnd'])
            tap(50, 352)
            saved('defaults-restored-by-touch', False)
            capture('defaults-saved')
            tap(40, 710)
            status('finished', lambda s: not s['notifications']['dashboard_open'])
        print('PASS:', args.phase, 'settings changed by real touch, private TOML checked, DND delivery verified')
    finally:
        if screen is not None:
            screen.close()
        if fixture is not None:
            fixture.close()
        request(args.socket, 'notification_action', action='Clear')
        (args.output / 'status.json').write_text(json.dumps(records, indent=2) + '\n')


if __name__ == '__main__':
    main()
