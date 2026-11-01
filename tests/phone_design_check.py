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

    def swipe(y, x0=60.0, x1=320.0, held=None, slow=False):
        screen.down(x0, y)
        if slow:
            time.sleep(.15)
        for i in range(1, 9):
            screen.move(x0 + (x1 - x0) * i / 8, y, delay=.06 if slow else .035)
            if i == 4 and held:
                capture(held)  # the finger rests halfway: does the row follow it?
        if slow:
            time.sleep(.3)
        screen.up()
        time.sleep(.3)

    def gone(predicate, seconds):
        try:
            return await_value(lambda: request(socket, 'status'), predicate, seconds=seconds)
        except AssertionError:
            return None

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
    # The banner's text row is the bottom 48 units of the well, which ends at
    # the gesture strip; the well's middle can sit on that boundary, where a
    # touch belongs to the strip.
    banner_y = well['y'] + well['height'] - 24
    def banner_hidden(s):
        return s['notifications']['banner'] is None and len(s['notifications']['items']) == before + 1
    swiped_at = time.monotonic()
    swipe(banner_y, held='design-banner-held')
    # Banners also hide on a timer; only a prompt disappearance is the swipe.
    hidden = gone(banner_hidden, 1.5)
    result['banner_fast_swipe'] = hidden is not None
    if hidden is None:
        # Diagnose timing: the banner timer restarts with a fresh notification.
        subprocess.run(['notify-send', '--app-name', 'Design check', '--expire-time', '0',
                        'Design check', 'Swipe me aside slowly'], env=env, user=10000, group=account.pw_gid,
                       extra_groups=os.getgrouplist('user', account.pw_gid), check=True, timeout=10)
        again = state('design-banner-again', lambda s: s['notifications']['banner'] is not None)
        before = len(again['notifications']['items']) - 1
        swiped_at = time.monotonic()
        swipe(banner_y, held='design-banner-held-slow', slow=True)
        hidden = gone(banner_hidden, 1.5)
        result['banner_slow_swipe'] = hidden is not None
    (Path(output) / 'design-check.json').write_text(json.dumps(result, indent=2) + '\n')
    assert hidden is not None, result
    result['banner_hidden_after_s'] = round(time.monotonic() - swiped_at, 2)
    step('banner-hidden-by-swipe', hidden['notifications']['surface'])
    request(socket, 'notifications', open=True)
    opened = state('design-dashboard', lambda s: s['notifications']['dashboard_open'])
    time.sleep(.5)
    capture('design-dashboard')
    # The well animates open; aim at its settled position, not the first
    # status that reports the dashboard open.
    well = request(socket, 'status')['notifications']['well']
    # The newest row sits first, below the 48-unit header and the row margin.
    swipe(well['y'] + 4 + 48 + 4 + 4 + 24)
    gone = state('design-row-swiped', lambda s: len(s['notifications']['items']) == before)
    step('row-dismissed-by-swipe', len(gone['notifications']['items']))
    request(socket, 'notifications', open=False)
    state('design-dashboard-closed', lambda s: not s['notifications']['dashboard_open'])
    request(socket, 'notification_action', action={'Settings': original})
    state('design-settings-restored', lambda s: s['notifications']['dnd'] == original['dnd']
          and s['notifications']['hide_previews'] == original['hide_previews'])
    # The launcher catalog follows desktop entries changed during the session.
    applications = Path(account.pw_dir) / '.local/share/applications'
    entry = applications / 'test.tarot.CatalogLive.desktop'
    assert not entry.exists()
    def catalog():
        return {a['id'] for a in request(socket, 'applications')['applications']}
    subprocess.run(['install', '-d', '-m', '755', str(applications)], user=10000, group=account.pw_gid, check=True)
    subprocess.run(['sh', '-c', 'printf "%s\\n" "[Desktop Entry]" "Type=Application" "Name=Catalog Live" "Exec=true" > "$1"', 'sh', str(entry)],
                   user=10000, group=account.pw_gid, check=True)
    try:
        await_value(catalog, lambda ids: 'test.tarot.CatalogLive.desktop' in ids, seconds=10)
        step('catalog-entry-appeared')
    finally:
        entry.unlink(missing_ok=True)
    await_value(catalog, lambda ids: 'test.tarot.CatalogLive.desktop' not in ids, seconds=10)
    step('catalog-entry-removed')
    result['clock'] = request(socket, 'status')['clock']
    # No leading zero on the hour: "0:10" after midnight, "9:41", never "09:41".
    hour = result['clock'].split(':')[0]
    assert hour == '0' or not hour.startswith('0'), result['clock']
    result['passed'] = True
    (Path(output) / 'design-check.json').write_text(json.dumps(result, indent=2) + '\n')
    return result
