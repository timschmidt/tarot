"""Media controls by finger on the phone, against a private MPRIS fixture.

The fixture runs as the session user on Tarot's own session bus; it plays
nothing. Taps are located by OCR of the rendered dashboard.
"""
import json
import os
import time
from pathlib import Path
from dialog_button import locate
from notification_replay import Fixture
from phone_gesture_replay import request


def replay(socket, state, capture, tap, before, directory):
    r = dict(passed=False, physical_audio_used=False)
    environ = dict(f.decode().split('=', 1) for f in Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in f)
    env = dict(PATH='/usr/bin:/bin', HOME='/home/user', XDG_RUNTIME_DIR='/run/user/10000',
               DBUS_SESSION_BUS_ADDRESS=environ['DBUS_SESSION_BUS_ADDRESS'])
    folder = directory / 'mpris'
    folder.mkdir()
    os.chown(folder, 10000, 10000)
    # Frozen payload copies drop the executable bit; run a private copy as the user.
    script = folder / 'mpris_fixture.py'
    script.write_bytes(Path(__file__).with_name('mpris_fixture.py').read_bytes())
    os.chown(script, 10000, 10000)
    script.chmod(0o755)
    player = Fixture(script, folder, env, owner=folder.stat())
    try:
        def media(label, condition, seconds=15):
            return state('media-' + label, lambda s: condition(s['media']), seconds=seconds)['media']

        def calls():
            return [e for e in player.events() if e['event'] == 'call']

        def play_button(label):
            request(socket, 'notifications', open=True)
            state('media-' + label + '-dashboard', lambda s: s['notifications']['dashboard_open'])
            time.sleep(.6)
            name = 'media-' + label
            capture(name)
            target = locate(directory / (name + '.png'), 'Morning', vertical=(.07, .94), scale=1)
            row = target['recognized']
            # Captures are 2x; the OCR box is relative to the vertical crop.
            left = int(row['left']) / target['scale'] / 2
            top = (target['crop_top'] + int(row['top']) / target['scale']) / 2
            r.setdefault('targets', []).append(dict(label=label, left=left, top=top))
            # Transport row: 64-point buttons below the title and artist lines.
            return left + 99, top + 75

        m = media('discovered', lambda m: m['available'] and len(m['players']) == 1)
        assert m['players'][0]['title'] == 'Morning Song' and m['players'][0]['status'] == 'Paused'
        x, y = play_button('paused')
        tap(x, y)
        media('playing', lambda m: m['players'][0]['status'] == 'Playing' and m['pending'] is None)
        assert [c['method'] for c in calls()] == ['PlayPause']
        r['play_by_touch'] = True
        x, y = play_button('playing')
        tap(x, y)
        media('paused-again', lambda m: m['players'][0]['status'] == 'Paused' and m['pending'] is None)
        assert [c['method'] for c in calls()] == ['PlayPause', 'PlayPause']
        r['pause_by_touch'] = True
        request(socket, 'notifications', open=False)
        request(socket, 'lock')
        locked = state('media-locked', lambda s: s['lock']['phase'] == 'Locked')
        assert 'media' not in locked, 'Media text must not be exposed while protected'
        r['redacted_while_locked'] = True
        r['passed'] = True
        return r
    finally:
        if player.process.poll() is None:
            player.process.terminate()
            player.process.wait(timeout=5)
        (directory / 'media-result.json').write_text(json.dumps(r, indent=2) + '\n')
