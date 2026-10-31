"""A real MPRIS player (GNOME Decibels) controlled by touch from Tarot's dashboard.

Playback goes to an owned null sink, so nothing is audible; the file is
generated silence. Audio defaults and controls are restored exactly.
"""
import json
import os
import struct
import subprocess
import time
import wave
from pathlib import Path
from dialog_button import locate
from notification_replay import await_value
from phone_audio_state import devices, pactl, restore, snapshot
from phone_gesture_replay import request


def replay(socket, state, capture, tap, before, directory):
    r = dict(passed=False, physical_audio_used=False, application='org.gnome.Decibels')
    folder = directory / 'media-app'
    folder.mkdir()
    os.chown(folder, 10000, 10000)
    track = folder / 'Quiet Morning.wav'
    with wave.open(str(track), 'wb') as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(8000)
        out.writeframes(struct.pack('<h', 0) * 8000 * 120)
    os.chown(track, 10000, 10000)
    original = snapshot()
    sink = 'tarot_qa_media_' + str(os.getpid())
    module = int(pactl('load-module', 'module-null-sink', 'sink_name=' + sink))
    app = None
    log = (folder / 'decibels.log').open('x')
    try:
        pactl('set-default-sink', sink)
        environ = dict(f.decode().split('=', 1) for f in Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in f)
        status = state('media-app-start')
        env = dict(PATH='/usr/bin:/bin', HOME='/home/user', USER='user', LOGNAME='user',
                   XDG_RUNTIME_DIR='/run/user/10000', WAYLAND_DISPLAY=status['wayland_display'],
                   XDG_SESSION_TYPE='wayland', XDG_CURRENT_DESKTOP='Tarot',
                   DBUS_SESSION_BUS_ADDRESS=environ['DBUS_SESSION_BUS_ADDRESS'])
        app = subprocess.Popen(['/usr/bin/org.gnome.Decibels', str(track)], env=env, user=10000, group=10000,
                               extra_groups=os.getgrouplist('user', 10000), cwd='/home/user',
                               stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        found = state('media-app-player', lambda s: any(p['identity'] for p in s['media']['players']), seconds=60)['media']
        r['players'] = found['players']
        player = found['players'][0]
        r['identity'] = player['identity']

        def streams():
            sink_index = next(d['index'] for d in devices('sinks') if d['name'] == sink)
            return [i for i in devices('sink-inputs') if i['sink'] == sink_index]

        def dashboard(label):
            request(socket, 'notifications', open=True)
            state('media-app-' + label + '-dashboard', lambda s: s['notifications']['dashboard_open'])
            time.sleep(.8)
            name = 'media-app-' + label
            capture(name)
            current = request(socket, 'status')['media']
            title = current['players'][0]['title'] or 'Nothing'
            target = locate(directory / (name + '.png'), title.split()[0], vertical=(.07, .94), scale=1)
            row = target['recognized']
            left = int(row['left']) / target['scale'] / 2
            top = (target['crop_top'] + int(row['top']) / target['scale']) / 2
            r.setdefault('targets', []).append(dict(label=label, title=title, left=left, top=top))
            # Transport row: below the title, and below the artist line when shown.
            artist = current['players'][0]['artist']
            return left + 99, top + (75 if artist else 46)

        def toggle(label, wanted):
            x, y = dashboard(label)
            tap(x, y)
            return state('media-app-' + wanted.lower(), lambda s: s['media']['players']
                         and s['media']['players'][0]['status'] == wanted and s['media']['pending'] is None, seconds=20)

        if player['status'] != 'Playing':
            toggle('start', 'Playing')
        playing = await_value(streams, lambda s: len(s) == 1 and not s[0]['corked'], seconds=20)
        r['stream_while_playing'] = dict(count=len(playing), corked=playing[0]['corked'])
        r['played_by_touch'] = player['status'] != 'Playing'
        toggle('pause', 'Paused')
        paused = await_value(streams, lambda s: all(i['corked'] for i in s), seconds=20)
        r['paused_by_touch'] = True
        r['stream_after_pause'] = [dict(corked=i['corked']) for i in paused]
        request(socket, 'notifications', open=False)
        r['passed'] = True
        return r
    finally:
        if app is not None and app.poll() is None:
            app.terminate()
            try:
                app.wait(timeout=10)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=5)
        log.close()
        try:
            r['player_removed'] = bool(await_value(lambda: request(socket, 'status').get('media', {}).get('players') == [], bool, seconds=20))
        except AssertionError:
            r['player_removed'] = False
        rows = [line.split('\t') for line in pactl('list', 'short', 'modules').splitlines() if line.split('\t')[0] == str(module)]
        assert len(rows) == 1 and rows[0][1] == 'module-null-sink'
        pactl('unload-module', str(module))
        restore(original)
        r['audio_restored'] = snapshot() == original
        (directory / 'media-app-result.json').write_text(json.dumps(r, indent=2) + '\n')
        assert r['audio_restored']
