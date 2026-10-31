"""Capture real phone screens for project documentation.

Launches installed applications as cards, posts one desktop notification
through the standard service, and captures ordinary (unprotected) views.
Nothing is changed beyond opening and closing these cards.
"""
import json
import pwd
import subprocess
import time
from pathlib import Path
from phone_gesture_replay import request

PREFERRED = ['org.gnome.clocks.desktop', 'org.gnome.Weather.desktop', 'org.gnome.TextEditor.desktop',
             'foot.desktop', 'org.kde.kalk.desktop']


def replay(socket, state, capture, before, directory):
    r = dict(passed=False, captures=[], launched=[])
    ids = {a['id'] for a in request(socket, 'applications')['applications']}
    r['available'] = [i for i in PREFERRED if i in ids]
    for app in r['available'][:2]:
        count = len(state('showcase-before-' + app, lambda s: True)['cards'])
        request(socket, 'launch', id=app)
        state('showcase-launched-' + app, lambda s: len(s['cards']) > count, seconds=40)
        r['launched'].append(app)
        time.sleep(2)

    def shot(name, condition):
        state('showcase-' + name + '-ready', condition, seconds=20)
        time.sleep(1.2)
        capture('showcase-' + name)
        r['captures'].append(name)

    account = pwd.getpwnam('user')
    environ = dict(f.decode().split('=', 1) for f in Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in f)
    env = dict(PATH='/usr/bin:/bin', HOME=account.pw_dir, XDG_RUNTIME_DIR='/run/user/10000',
               DBUS_SESSION_BUS_ADDRESS=environ['DBUS_SESSION_BUS_ADDRESS'])
    # --wait keeps the sender connected, so the notification stays actionable.
    sender = subprocess.Popen(['notify-send', '--wait', '--app-name=Tarot', 'Welcome to Tarot',
                               'Cards, gestures and notifications on the PinePhone Pro.'],
                              env=env, user=account.pw_uid, group=account.pw_gid,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(2)
    request(socket, 'overview')
    shot('overview', lambda s: s['view'] == 'Overview')
    request(socket, 'home')
    shot('home', lambda s: s['view'] == 'Home')
    request(socket, 'launcher')
    shot('launcher', lambda s: s['view'] == 'Launcher')
    request(socket, 'home')
    request(socket, 'notifications', open=True)
    shot('notifications', lambda s: s['notifications']['dashboard_open'])
    request(socket, 'notifications', open=False)
    sender.terminate()
    sender.wait(timeout=10)
    request(socket, 'system_menu', open=True)
    request(socket, 'system_page', page='Power')
    shot('power', lambda s: s['system_menu_open'] and s['system_page'] == 'Power')
    request(socket, 'system_menu', open=False)
    # Close only the cards this replay opened.
    original = {card['id'] for card in before['cards']}
    for card in state('showcase-close', lambda s: True)['cards']:
        if card['id'] not in original:
            request(socket, 'close', id=card['id'])
    state('showcase-closed', lambda s: {c['id'] for c in s['cards']} == original, seconds=30)
    request(socket, 'home')
    r['passed'] = True
    (directory/'showcase-result.json').write_text(json.dumps(r, indent=2)+'\n')
    return r
