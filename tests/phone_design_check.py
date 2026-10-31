"""Native check of the packet-adherent shell details in a diagnostic session.

Captures the device menu and the shell keyboard, and dismisses a real
libnotify notification by touch: the banner by a sideways swipe (it stays in
the dashboard), then its dashboard row by a sideways swipe. Runs as root
inside phone_installed_session_replay; notify-send runs as the session user.
"""
import json, os, pwd, subprocess, time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request


def replay(socket, screen, state, capture, output):
    result = {'passed': False, 'steps': []}

    def step(label, value=None):
        result['steps'].append({'label': label, 'value': value})

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.15)

    def swipe(y, x0=60.0, x1=320.0):
        screen.down(x0, y)
        for i in range(1, 9):
            screen.move(x0 + (x1 - x0) * i / 8, y)
        screen.up()
        time.sleep(.3)

    # Device menu: glyph rows, right-aligned values, no top-level chevrons.
    request(socket, 'home')
    state('design-home', lambda s: s['view'] == 'Home' and not s['system_menu_open'])
    tap(360 - 35, 16)
    state('design-device-menu', lambda s: s['system_menu_open'] and s['top_menu']['entry'] == 'Device')
    time.sleep(.5)
    capture('design-device-menu')
    request(socket, 'system_menu', open=False)
    state('design-device-menu-closed', lambda s: not s['system_menu_open'])
    step('device-menu-captured')

    # Shell keyboard through Just type: glyph Shift/Backspace, "?123", "space".
    tap(180, 60)
    state('design-keyboard', lambda s: s['keyboard_visible'] and s['shell_text_focus'])
    time.sleep(.5)
    capture('design-keyboard')
    request(socket, 'home')
    state('design-keyboard-closed', lambda s: not s['keyboard_visible'])
    step('keyboard-captured')

    # A real notification from the session user, dismissed by swipes only.
    tarot = [p for p in Path('/proc').iterdir() if p.name.isdigit() and p.stat().st_uid == 10000
             and (p / 'cmdline').read_bytes().split(b'\0')[0] == b'/usr/bin/tarot']
    assert len(tarot) == 1
    environ = dict(v.split('=', 1) for v in (tarot[0] / 'environ').read_bytes().decode().split('\0') if '=' in v)
    account = pwd.getpwnam('user')
    env = {k: environ[k] for k in ['DBUS_SESSION_BUS_ADDRESS', 'XDG_RUNTIME_DIR'] if k in environ}
    env.update(HOME=account.pw_dir, PATH='/usr/bin:/bin')
    settings = request(socket, 'status')['notifications']
    original = {'dnd': settings['dnd'], 'hide_previews': settings['hide_previews']}
    request(socket, 'notification_action', action={'Settings': {'dnd': False, 'hide_previews': False}})
    state('design-banners-allowed', lambda s: not s['notifications']['dnd'])
    before = len(request(socket, 'status')['notifications']['items'])
    subprocess.run(['notify-send', '--app-name', 'Design check', '--expire-time', '0',
                    'Design check', 'Swipe me aside'], env=env, user=10000, group=account.pw_gid,
                   extra_groups=os.getgrouplist('user', account.pw_gid), check=True, timeout=10)
    shown = state('design-banner', lambda s: s['notifications']['banner'] is not None
                  and len(s['notifications']['items']) == before + 1)
    time.sleep(.4)
    capture('design-banner')
    well = shown['notifications']['well']
    swiped_at = time.monotonic()
    swipe(well['y'] + well['height'] / 2)
    # Banners also hide on a timer; only a prompt disappearance is the swipe.
    hidden = await_value(lambda: request(socket, 'status'), lambda s: s['notifications']['banner'] is None
                         and len(s['notifications']['items']) == before + 1, seconds=2)
    result['banner_hidden_after_s'] = round(time.monotonic() - swiped_at, 2)
    step('banner-hidden-by-swipe', hidden['notifications']['surface'])
    request(socket, 'notifications', open=True)
    opened = state('design-dashboard', lambda s: s['notifications']['dashboard_open'])
    time.sleep(.5)
    capture('design-dashboard')
    well = opened['notifications']['well']
    # The newest row sits first, below the 48-unit header and the row margin.
    swipe(well['y'] + 4 + 48 + 4 + 4 + 24)
    gone = state('design-row-swiped', lambda s: len(s['notifications']['items']) == before)
    step('row-dismissed-by-swipe', len(gone['notifications']['items']))
    request(socket, 'notifications', open=False)
    state('design-dashboard-closed', lambda s: not s['notifications']['dashboard_open'])
    request(socket, 'notification_action', action={'Settings': original})
    state('design-settings-restored', lambda s: s['notifications']['dnd'] == original['dnd']
          and s['notifications']['hide_previews'] == original['hide_previews'])
    result['clock'] = request(socket, 'status')['clock']
    assert not result['clock'].startswith('0'), result['clock']
    result['passed'] = True
    (Path(output) / 'design-check.json').write_text(json.dumps(result, indent=2) + '\n')
    return result
