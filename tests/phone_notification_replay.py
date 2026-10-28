#!/usr/bin/env python3
"""Real notification banner/dashboard taps and input isolation on the spare phone."""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from notification_replay import Fixture, await_value, session_environment
from phone_gesture_replay import Touchscreen, request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket', type=Path, default=Path('/run/user/10000/tarot-drm.sock'))
    parser.add_argument('--probe', type=Path, default=Path('/home/user/tarot-dev/bin/notification-probe'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    owner = args.socket.stat()
    os.chown(args.output, owner.st_uid, owner.st_gid)
    account = pwd.getpwuid(owner.st_uid)
    initial = await_value(lambda: request(args.socket, 'status'), lambda s: s['notifications']['available'] and len(s['cards']) == 1)
    environment = session_environment(initial['pid'])
    fixture = Fixture(args.probe, args.output / 'sender', environment, owner)
    records = {}
    screen = client = None
    client_log = None

    def state(label):
        value = request(args.socket, 'status')
        records[label] = value
        assert value['pid'] == initial['pid']
        return value

    def wait(label, condition):
        return await_value(lambda: state(label), condition)

    def metadata(identifier):
        return next((n for n in request(args.socket, 'status')['notifications']['items'] if n['id'] == identifier), None)

    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty8'
        assert request(args.socket, 'status')['pid'] == initial['pid']

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.15)

    def capture(name):
        request(args.socket, 'capture', path=str(args.output / (name + '.png')))
        wait('capture-' + name, lambda s: not s['capture_pending'] and not s['capture_error'])

    def dashboard(opened):
        tap(40, 710)
        value = wait('dashboard-' + str(opened), lambda s: s['notifications']['dashboard_open'] == opened)
        time.sleep(.25)
        return value

    try:
        screen = Touchscreen(guard)
        # Exercise an installed Linux client with no Tarot code or SDK.
        subprocess.run(['notify-send', '--app-name', 'libnotify fixture', '--expire-time', '0',
                        'Tarot libnotify', 'Ordinary Linux notification client'],
                       env=dict(environment, HOME=account.pw_dir), cwd=account.pw_dir,
                       user=owner.st_uid, group=owner.st_gid,
                       extra_groups=os.getgrouplist(account.pw_name, owner.st_gid),
                       timeout=5, check=True)
        external = wait('libnotify', lambda s: s['notifications']['banner'] is not None and
                        len(s['notifications']['items']) == 1 and s['notifications']['items'][0]['orphaned'])
        assert external['notifications']['items'][0]['action_count'] == 0
        capture('libnotify-banner')
        tap(320, 635)  # Hide the banner while retaining its dashboard entry.
        wait('banner-hidden', lambda s: s['notifications']['banner'] is None)
        assert len(state('retained-after-hide')['notifications']['items']) == 1
        dashboard(True)
        tap(130, 405)
        wait('libnotify-cleared', lambda s: not s['notifications']['items'])
        dashboard(False)
        identifier = fixture.notify(summary='Touch notification', body='A native app sent this.', actions=['reply', 'Reply'], resident=True, timeout=0)
        wait('banner', lambda s: s['notifications']['banner'] is not None)
        assert state('arrival-preserves-view')['view'] == initial['view']
        capture('native-banner')
        # With no default action, tapping the banner opens its dashboard.
        tap(150, 635)
        wait('banner-opened-dashboard', lambda s: s['notifications']['dashboard_open'])
        time.sleep(.3)
        capture('dashboard-action')
        tap(140, 540)
        fixture.signal('action', identifier, key='reply')
        assert metadata(identifier)['live']
        tap(50, 300)
        wait('dnd-on', lambda s: s['notifications']['dnd'])
        tap(50, 352)
        wait('privacy-on', lambda s: s['notifications']['hide_previews'])
        tap(130, 405)
        fixture.signal('closed', identifier, reason=2)
        wait('cleared-by-touch', lambda s: not s['notifications']['items'])
        # Taps in the empty panel do not reach the app under the scrim.
        tap(180, 540)
        assert state('panel-still-open')['notifications']['dashboard_open']
        tap(50, 300)
        wait('dnd-off', lambda s: not s['notifications']['dnd'])
        tap(50, 352)
        wait('privacy-off', lambda s: not s['notifications']['hide_previews'])
        dashboard(False)
        identifier = fixture.notify(summary='Default action', actions=['default', 'Open'], timeout=0)
        wait('default-banner', lambda s: s['notifications']['banner'] is not None)
        time.sleep(.15)
        tap(150, 635)
        fixture.signal('action', identifier, key='default')
        fixture.signal('closed', identifier, reason=2)
        assert not state('default-keeps-view')['notifications']['dashboard_open']
        request(args.socket, 'notification_action', action='Clear')
        wait('reset', lambda s: not s['notifications']['items'])

        # An arrival cannot cancel or commit an already held card arrangement.
        tap(180, 710)
        wait('overview', lambda s: s['view'] == 'Overview')
        time.sleep(.3)
        before = [(c['id'], c['stack']) for c in state('before-hold')['cards']]
        screen.down(180, 300)
        held = wait('arranging', lambda s: s['presentation']['arrange'] is not None)
        identifier = fixture.notify(summary='Arrived during a card hold', timeout=0)
        arrived = wait('arriving-while-held', lambda s: s['notifications']['banner'] is not None)
        assert arrived['presentation']['arrange']['id'] == held['presentation']['arrange']['id']
        assert [(c['id'], c['stack']) for c in arrived['cards']] == before
        capture('arrival-during-hold')
        screen.up()
        wait('hold-finished', lambda s: s['presentation']['arrange'] is None)
        # A hold on the banner must not start arranging an underlying card.
        screen.down(150, 635)
        time.sleep(.65)
        assert state('banner-held')['presentation']['arrange'] is None
        screen.up()
        wait('overview-dashboard', lambda s: s['notifications']['dashboard_open'])
        assert state('overview-preserved')['view'] == 'Overview'
        dashboard(False)
        request(args.socket, 'notification_action', action='Clear')
        wait('cleared-before-text', lambda s: not s['notifications']['items'])

        # A real text-input client confirms focus is retained on arrival, removed
        # for the modal dashboard, and restored to the same field on dismissal.
        report, commands = args.output / 'text-events.jsonl', args.output / 'text-commands.jsonl'
        commands.write_text('')
        os.chown(commands, owner.st_uid, owner.st_gid)
        text_env = dict(environment, WAYLAND_DISPLAY=initial['wayland_display'], XDG_RUNTIME_DIR=str(args.socket.parent), HOME=account.pw_dir)
        text_env.pop('WAYLAND_SOCKET', None)
        text_env.pop('DISPLAY', None)
        client_log = (args.output / 'text-client.log').open('w')
        client = subprocess.Popen(['/home/user/tarot-dev/bin/text-probe', '--role', 'app', '--report', str(report), '--commands', str(commands)],
                                  env=text_env, stdin=subprocess.DEVNULL, stdout=client_log, stderr=client_log,
                                  cwd=account.pw_dir, user=owner.st_uid, group=owner.st_gid,
                                  extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
        opened = wait('text-opened', lambda s: s['keyboard_visible'] and len(s['cards']) == 2)
        text_view = opened['view']
        time.sleep(.4)

        def events():
            return [json.loads(line) for line in report.read_bytes().split(b'\n')[:-1] if line] if report.exists() else []

        def text(expected):
            await_value(events, lambda es: any(e['event'] == 'applied' and e['text'] == expected for e in es))

        fixture.notify(summary='Typing continues below this banner', timeout=0)
        arrived = wait('arrival-with-keyboard', lambda s: s['notifications']['banner'] is not None)
        assert arrived['keyboard_visible'] and arrived['view'] == text_view
        tap(21, 504)
        text('hello q')
        capture('banner-above-native-keyboard')
        dashboard(True)
        assert not state('dashboard-hides-keyboard')['keyboard_visible']
        start = len(events())
        tap(21, 504)  # This is dashboard content while the keyboard is absent.
        time.sleep(.15)
        assert not any(e['event'] == 'applied' for e in events()[start:])
        dashboard(False)
        wait('field-restored', lambda s: s['keyboard_visible'] and s['view'] == text_view)
        time.sleep(.3)
        tap(21, 504)
        text('hello qq')
        capture('same-field-after-dashboard')
        print('PASS: real notification taps/actions/settings, banner ownership, held-card arrival, native text focus and modal isolation')
    finally:
        if screen is not None:
            screen.close()
        if client is not None and client.poll() is None:
            client.terminate()
            try:
                client.wait(timeout=3)
            except subprocess.TimeoutExpired:
                client.kill()
                client.wait()
        if client_log is not None:
            client_log.close()
        fixture.close()
        request(args.socket, 'notifications', open=False)
        request(args.socket, 'notification_action', action='Clear')
        request(args.socket, 'notification_action', action={'Settings': {'dnd': False, 'hide_previews': False}})
        (args.output / 'status.json').write_text(json.dumps(records, indent=2) + '\n')


if __name__ == '__main__':
    main()
