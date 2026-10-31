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
from notification_replay import await_value
from phone_gesture_replay import request

PACKAGE = 'org.fdroid.fdroid'
ENTRY = 'waydroid.' + PACKAGE + '.desktop'


def replay(socket, state, capture, before, directory):
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
        r['cards_after'] = [c for c in request(socket, 'status')['cards'] if c['id'] not in {c['id'] for c in before['cards']}]
        (directory / 'waydroid-result.json').write_text(json.dumps(r, indent=2) + '\n')
