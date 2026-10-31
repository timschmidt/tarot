"""An Android app launched from Tarot's catalog through Waydroid becomes one card.

Requires the Waydroid container manager running with its session stopped, and
the app's exported desktop entry in the catalog, so the launch also exercises
Waydroid's lazy session start. The first window can be Android's boot screen,
so the cards are read once Android reports the app's own task. Records the new cards, Android's task list
before and after Tarot closes the card, and always stops the Waydroid session
again as the session user.
"""
import json
import os
import re
import subprocess
import time
from pathlib import Path
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

PACKAGE = 'org.fdroid.fdroid'
ENTRY = 'waydroid.' + PACKAGE + '.desktop'


def text_input(r, socket, tap, capture, directory, waydroid):
    """Type into an Android field with Tarot's keyboard in key-event mode.

    Waydroid has no text-input protocol, so Tarot's keyboard is opened by hand
    and sends key events. Android's own keyboard, which Waydroid would show as a
    separate card, is disabled for the run (show_ime_with_hard_keyboard alone
    does not stop it) and its enabled set is restored.
    """
    def word(label, text, vertical=(.07, .97), alternatives=()):
        capture(label)
        target = locate(directory / (label + '.png'), text, vertical=vertical, scale=1, alternatives=alternatives)
        r.setdefault('text_targets', []).append(dict(label=label, word=target['word'], point=target['point']))
        tap(target['point'][0] / 2, target['point'][1] / 2)
        return target

    def shell(*command):
        return waydroid('shell', '--', *command, user=False, timeout=30)

    def observe(label):
        time.sleep(4)
        capture(label)
        ime = [line.strip() for line in shell('dumpsys', 'input_method').splitlines() if 'mInputShown' in line]
        status = request(socket, 'status')
        return dict(tarot_keyboard=status['keyboard_visible'], cards=len(status['cards']),
                    android_ime_shown=any('mInputShown=true' in line for line in ime))

    enabled = [line.strip() for line in shell('ime', 'list', '-s').splitlines() if line.strip()]
    default = shell('settings', 'get', 'secure', 'default_input_method').strip()
    r['android_imes_before'] = dict(enabled=enabled, default=default)
    try:
        for ime in enabled:
            shell('ime', 'disable', ime)
        r['android_imes_disabled'] = [line.strip() for line in shell('ime', 'list', '-s').splitlines() if line.strip()]
        word('waydroid-search-tab', 'Search', vertical=(.8, .97))
        time.sleep(3)
        word('waydroid-search-field', 'Search...', vertical=(.08, .25), alternatives=('Search…', 'Search..'))
        r['field_focused'] = observe('waydroid-field-focused')
        def sample():
            status = request(socket, 'status')
            return dict(at=round(time.monotonic() - started, 2), view=status['view'],
                        keyboard_visible=status['keyboard_visible'], keyboard_owner=status['keyboard_owner'],
                        cards=[(c['id'], c['title'], c['app_id']) for c in status['cards']])
        started = time.monotonic()
        r['before_keyboard'] = sample()
        word('waydroid-keyboard-button', 'Keyboard', vertical=(.94, 1.0))
        timeline = []
        for _ in range(16):
            timeline.append(sample())
            time.sleep(.25)
        r['keyboard_timeline'] = timeline
        r['tarot_keyboard_opened'] = bool(await_value(lambda: request(socket, 'status')['keyboard_visible'], bool, seconds=10))
        capture('waydroid-keyboard-open')
        status = request(socket, 'status')
        r['keyboard_owner'] = status['keyboard_owner']
        r['view_at_keyboard'] = status['view']
        Keyboard(tap).type_ascii('fdroid')
        r['after_typing'] = observe('waydroid-typed')
        status = request(socket, 'status')
        r['geometry_with_keyboard'] = [g for g in status['native_geometry'] if g['id'] == status['view'].get('App')]
        try:
            locate(directory / 'waydroid-typed.png', 'fdroid', vertical=(.07, .4), scale=1)
            r['typed_text_visible'] = True
        except AssertionError:
            r['typed_text_visible'] = False
    finally:
        for ime in enabled:
            shell('ime', 'enable', ime)
        if default not in ('null', ''):
            shell('ime', 'set', default)
        after = [line.strip() for line in shell('ime', 'list', '-s').splitlines() if line.strip()]
        r['android_imes_restored'] = (sorted(after) == sorted(enabled)
                                      and shell('settings', 'get', 'secure', 'default_input_method').strip() == default)


def replay(socket, state, capture, before, directory, tap=None):
    r = dict(passed=False, entry=ENTRY)
    environ = dict(f.decode().split('=', 1) for f in Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in f)
    env = dict(PATH='/usr/bin:/bin', HOME='/home/user', USER='user', LOGNAME='user',
               XDG_RUNTIME_DIR='/run/user/10000', DBUS_SESSION_BUS_ADDRESS=environ['DBUS_SESSION_BUS_ADDRESS'])

    def waydroid(*args, user=True, timeout=60):
        extra = dict(user=10000, group=10000, extra_groups=os.getgrouplist('user', 10000), env=env) if user else {}
        return subprocess.run(['waydroid', *args], capture_output=True, text=True, timeout=timeout, **extra).stdout

    def status():
        return dict(re.findall(r'^([^:\n]+):\s*(.*)$', waydroid('status'), re.M))

    def bounds():
        # Android's own idea of the app task's window, to compare with the card.
        waydroid('container', 'unfreeze', user=False, timeout=20)
        out = waydroid('shell', '--', 'dumpsys', 'activity', 'activities', user=False, timeout=30)
        return [line.strip() for line in out.splitlines() if PACKAGE in line or 'mBounds=' in line][:12]

    def tasks():
        # Android freezes the container once nothing is shown; thaw it to read.
        waydroid('container', 'unfreeze', user=False, timeout=20)
        try:
            out = waydroid('shell', '--', 'dumpsys', 'activity', 'activities', user=False, timeout=30)
        except subprocess.TimeoutExpired:
            return 'timeout'
        return sorted(set(re.findall(r'Task\{\w+ #(\d+) type=(\w+) A=\d+:([\w.]+)', out)))

    def network():
        # NetworkManager keeps runtime profiles for Waydroid's bridge and veth
        # while they exist; the controller's baseline compares them.
        runtime = Path('/run/NetworkManager/system-connections')
        return dict(bridge=Path('/sys/class/net/waydroid0').exists(),
                    profiles=sorted(p.name for p in runtime.iterdir()) if runtime.is_dir() else [])

    r['network_before'] = network()
    try:
        initial = status()
        r['waydroid_before'] = initial
        assert initial.get('Session') == 'STOPPED', initial
        manager = subprocess.run(['rc-service', 'waydroid-container', 'status'], capture_output=True, text=True, timeout=20)
        r['container_manager'] = manager.stdout.strip()
        assert manager.returncode == 0, manager.stdout
        catalog = request(socket, 'applications')['applications']
        app = next(a for a in catalog if a['id'] == ENTRY)
        r['catalog_entry'] = dict(name=app['name'], icon=app['icon'])
        cards = {c['id'] for c in before['cards']}
        started = time.monotonic()
        request(socket, 'launch', id=ENTRY)
        state('waydroid-card', lambda s: len(s['cards']) > len(cards), seconds=180)
        r['first_card_seconds'] = round(time.monotonic() - started, 1)
        # The first window can be Android's boot screen; wait for the app's own task.
        boot = request(socket, 'status')
        r['boot_cards'] = [c for c in boot['cards'] if c['id'] not in cards]
        # Waydroid sizes Android's display from this window's early configures.
        r['boot_geometry'] = [g for g in boot['native_geometry'] if g['id'] not in cards]
        capture('waydroid-boot')
        await_value(tasks, lambda t: t != 'timeout' and any(row[2] == PACKAGE for row in t), seconds=180)
        r['app_task_seconds'] = round(time.monotonic() - started, 1)
        # Let windows settle; any extra window would appear here.
        time.sleep(10)
        current = request(socket, 'status')
        new = [c for c in current['cards'] if c['id'] not in cards]
        r['new_cards'] = new
        r['native_geometry'] = [g for g in current['native_geometry'] if g['id'] in [c['id'] for c in new]]
        r['android_bounds'] = bounds()
        if tap is not None:
            try:
                text_input(r, socket, tap, capture, directory, waydroid)
            except Exception as error:
                r['text_input_error'] = '%s: %s' % (type(error).__name__, str(error)[:300])
        r['view'] = current['view']
        r['android_tasks_open'] = tasks()
        capture('waydroid-card')
        assert len(new) == 1, new
        card = new[0]
        r['one_card'] = True
        request(socket, 'close', id=card['id'])
        state('waydroid-card-closed', lambda s: all(c['id'] != card['id'] for c in s['cards']), seconds=30)
        time.sleep(5)
        r['android_tasks_after_close'] = tasks()
        r['passed'] = True
        return r
    finally:
        try:
            waydroid('session', 'stop')
            r['waydroid_after'] = await_value(status, lambda s: s.get('Session') == 'STOPPED', seconds=60)
        except Exception as error:
            r['session_stop_error'] = type(error).__name__
            r['passed'] = False
        try:
            r['network_after'] = await_value(network, lambda n: n == r['network_before'], seconds=90)
        except AssertionError:
            r['network_after'] = network()
            r['passed'] = False
        r['cards_after'] = [c for c in request(socket, 'status')['cards'] if c['id'] not in {c['id'] for c in before['cards']}]
        (directory / 'waydroid-result.json').write_text(json.dumps(r, indent=2) + '\n')
