#!/usr/bin/env python3
"""Nested output rotation: protected configure, real pixels and client input.

Uses a scoped XTEST connection to its own X11-hosted compositor window. The
synthetic lock fixture never authenticates; device KMS/PAM/sensor tests remain
separate gates. No host system service is controlled by this replay.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from PIL import Image
from lock_replay import append, events, any_request
from notification_replay import await_value
from phone_gesture_replay import request
from x11_replay_input import X11Input


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    runtime = tempfile.TemporaryDirectory(prefix='tarot-orientation-')
    control = Path(runtime.name)/'control.sock'
    commands, locker_report, app_report = out/'locker.commands', out/'locker.jsonl', out/'app.jsonl'
    commands.write_text('')
    env = dict(os.environ, WINIT_UNIX_BACKEND='x11', XDG_SESSION_TYPE='x11', LIBGL_ALWAYS_SOFTWARE='1', __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    env.pop('WAYLAND_DISPLAY', None)
    env.pop('WAYLAND_SOCKET', None)
    proc = client = mouse = None
    record = {'orientations': []}

    def state(condition=lambda s: True):
        return await_value(lambda: request(control, 'status'), condition, seconds=12)

    def rotate(orientation):
        request(control, 'rotate', orientation=orientation)
        return state(lambda s: (s.get('output') or s['lock']['output'])['orientation'] == orientation and
                     (s.get('output') or s['lock']['output'])['requested'] is None and
                     (s.get('output') or s['lock']['output'])['presented_revision'] == (s.get('output') or s['lock']['output'])['revision'])

    def physical(point, geometry):
        w, h = geometry['physical_width'], geometry['physical_height']
        swap = geometry['orientation'] in ['left-up', 'right-up']
        lw, lh = (h, w) if swap else (w, h)
        x, y = point[0]/lw, point[1]/lh
        # Independent inverse of panel-normal input: output point -> host pixel.
        x, y = {'normal': (x, y), 'left-up': (y, 1-x), 'bottom-up': (1-x, 1-y), 'right-up': (1-y, x)}[geometry['orientation']]
        return x*w, y*h

    try:
        with (out/'compositor.log').open('w') as log:
            argv = [str(root/'target/debug/tarot'), '--control-socket', str(control), '--input-method', str(root/'target/debug/tarot-keyboard'), '--lock-client', str(root/'target/debug/examples/lock-probe'), '--verify-lock-blank', '--verify-lock-surface-rgba', '336699ff', '--exit-after-ms', '90000']
            for value in ['--role', 'locker', '--report', str(locker_report), '--commands', str(commands)]:
                argv.append('--lock-client-arg='+value)
            proc = subprocess.Popen(argv, env=env, stdout=log, stderr=subprocess.STDOUT)
            await_value(lambda: control.exists() or proc.poll() is not None, bool)
            assert proc.poll() is None
            state(lambda s: s['lock']['phase'] == 'Awaiting' and s['lock']['blank_verified'])
            append(commands, 'lock')
            state(lambda s: s['lock']['phase'] == 'Locked' and s['lock']['surface_verified'])
            for orientation in ['left-up', 'bottom-up', 'right-up', 'normal']:
                value = rotate(orientation)
                geometry = value['lock']['output']
                width, height = geometry['physical_width'], geometry['physical_height']
                expected = (height, width) if orientation in ['left-up', 'right-up'] else (width, height)
                await_value(lambda: events(locker_report), lambda es: any(e['event'] == 'configured' and (e['fields']['width'], e['fields']['height']) == expected for e in es))
                value = state(lambda s: s['lock']['surface_verified'] is True)
                assert value['lock']['unprotected_presentations'] == 0
                assert any_request(control, 'capture', path=str(out/'denied.png')) == {'ok': False, 'error': 'Session is locked'}
                record['orientations'].append({'orientation': orientation, 'protected': value})
            append(commands, 'unlock')
            first = state(lambda s: s['lock']['phase'] == 'Unlocked')
            mouse = X11Input(proc.pid)
            await_value(mouse.find, bool)
            client_env = dict(env, WAYLAND_DISPLAY=first['wayland_display'])
            client_env.pop('DISPLAY', None)
            with (out/'client.log').open('w') as client_log:
                client = subprocess.Popen([str(root/'target/debug/examples/input-probe'), '--report', str(app_report)], env=client_env, stdout=client_log, stderr=subprocess.STDOUT)
                state(lambda s: len(s['cards']) == 1)
                for orientation in ['normal', 'left-up', 'bottom-up', 'right-up', 'normal']:
                    value = rotate(orientation)
                    time.sleep(.3)
                    value = state()
                    record['pending'] = {'orientation': orientation, 'state': value}
                    geometry = value['output']
                    rect = value['presentation']['windows'][0]['rect']
                    factor = min(rect['width']/600, rect['height']/1000)
                    samples = []
                    for cx, cy in [(423, 437), (383, 887), (563, 677)]:
                        point = (rect['x']+(rect['width']-600*factor)/2+(cx-20)*factor,
                                 rect['y']+(rect['height']-1000*factor)/2+(cy-30)*factor)
                        px, py = physical(point, geometry)
                        start = len(events(app_report))
                        record['pending']['point'] = [px, py]
                        record['pending']['host_input'] = mouse.move(px, py)
                        received = await_value(lambda: events(app_report)[start:], lambda es: any(e['event'] in ['pointer_enter', 'pointer_motion'] for e in es))
                        motion = next(e for e in reversed(received) if e['event'] in ['pointer_enter', 'pointer_motion'])
                        assert motion['target'] == 'root' and abs(motion['x']-cx) < 4 and abs(motion['y']-cy) < 4, (orientation, motion, (cx, cy))
                        mouse.button(True)
                        mouse.button(False)
                        await_value(lambda: events(app_report)[start:], lambda es: sum(e['event']=='pointer_button' for e in es)==2)
                        samples.append({'physical': [px, py], 'expected_client': [cx, cy], 'received': motion})
                    path = out/f'app-{orientation}-{len(record["orientations"]):02}.png'
                    request(control, 'capture', path=str(path))
                    state(lambda s: not s['capture_pending'] and not s['capture_error'])
                    image = Image.open(path).convert('RGBA')
                    for sample in samples:
                        px, py = sample['physical']
                        pixel = image.getpixel((round(px), round(py)))
                        cx, cy = sample['expected_client']
                        expected = (cx//3, cy//5, 70, 255)
                        assert all(abs(a-b) <= 4 for a,b in zip(pixel, expected)), (orientation, sample, pixel, expected)
                        sample['pixel'] = pixel
                    record['orientations'].append({'orientation': orientation, 'geometry': geometry, 'samples': samples})
                # Without another physical motion, reconfiguration must move
                # the logical pointer to the new position under that same pixel.
                last_pixel = samples[-1]['physical']
                start = len(events(app_report))
                unmoved = rotate('bottom-up')
                received = await_value(lambda: events(app_report)[start:], lambda es: any(e['event']=='pointer_enter' for e in es))
                actual = next(e for e in reversed(received) if e['event']=='pointer_enter')
                rect = unmoved['presentation']['windows'][0]['rect']
                factor = min(rect['width']/600, rect['height']/1000)
                lw, lh = unmoved['output']['physical_width'], unmoved['output']['physical_height']
                lx, ly = lw-round(last_pixel[0]), lh-round(last_pixel[1])
                cx = (lx-rect['x']-(rect['width']-600*factor)/2)/factor+20
                cy = (ly-rect['y']-(rect['height']-1000*factor)/2)/factor+30
                assert actual['target']=='root' and abs(actual['x']-cx)<4 and abs(actual['y']-cy)<4, (actual, (cx,cy))
                record['stationary_pointer_reprojected'] = {'actual':actual,'expected_client':[cx,cy]}
                value = rotate('normal')
                geometry = value['output']
                rect = value['presentation']['windows'][0]['rect']
                factor = min(rect['width']/600, rect['height']/1000)
                point = (rect['x']+(rect['width']-600*factor)/2+(423-20)*factor, rect['y']+(437-30)*factor)
                # A held native button must leave its old surface and its
                # physical release must not complete that obsolete click.
                start = len(events(app_report))
                mouse.move(*physical(point, geometry))
                mouse.button(True)
                await_value(lambda: events(app_report)[start:], lambda es: any(e['event']=='pointer_button' and e['pressed'] for e in es))
                rotated = rotate('left-up')
                await_value(lambda: events(app_report)[start:], lambda es: any(e['event']=='pointer_leave' for e in es))
                mouse.button(False)
                time.sleep(.15)
                assert not any(e['event']=='pointer_button' and not e['pressed'] for e in events(app_report)[start:])
                record['held_native_button_canceled'] = True
                client.terminate()
                client.wait(timeout=5)
                client = None
            state(lambda s: not s['cards'])
            text_commands, text_report = out/'text.commands', out/'text.jsonl'
            text_commands.write_text('')
            with (out/'text.log').open('w') as text_log:
                client = subprocess.Popen([str(root/'target/debug/examples/text-probe'), '--role', 'app', '--commands', str(text_commands), '--report', str(text_report)], env=client_env, stdout=text_log, stderr=subprocess.STDOUT)
                first_text = state(lambda s: len(s['cards']) == 1 and s['keyboard_visible'] and s['keyboard_rect']['height'] > 0)
                helper = first_text['input_method']['pid']
                expected_text = 'hello '
                record['keyboard_rotations'] = []
                for orientation in ['normal', 'left-up', 'bottom-up', 'right-up', 'normal']:
                    value = rotate(orientation)
                    time.sleep(.2)
                    value = state(lambda s: s['keyboard_visible'])
                    assert value['input_method']['pid'] == helper
                    rect = value['keyboard_rect']
                    point = (rect['x']+rect['width']/20+3, rect['y']+rect['height']*.2+4)
                    mouse.move(*physical(point, value['output']))
                    mouse.button(True)
                    time.sleep(.08)
                    mouse.button(False)
                    expected_text += 'q'
                    await_value(lambda: events(text_report), lambda es: any(e['event']=='applied' and e['text']==expected_text for e in es))
                    record['keyboard_rotations'].append({'orientation': orientation, 'keyboard_rect': rect, 'expected_text': expected_text})
                # Hold another key over an orientation change: no character
                # may be committed by its stale release in the resized layout.
                mouse.move(*physical(point, value['output']))
                mouse.button(True)
                time.sleep(.08)
                value = rotate('left-up')
                mouse.button(False)
                time.sleep(.25)
                assert [e for e in events(text_report) if e['event']=='applied'][-1]['text'] == expected_text
                record['held_keyboard_key_canceled'] = True
                value = state(lambda s: s['keyboard_visible'])
                rect = value['keyboard_rect']
                point = (rect['x']+rect['width']/20+3, rect['y']+rect['height']*.2+4)
                mouse.move(*physical(point, value['output']))
                mouse.button(True)
                time.sleep(.08)
                mouse.button(False)
                expected_text += 'q'
                await_value(lambda: events(text_report), lambda es: any(e['event']=='applied' and e['text']==expected_text for e in es))
                record['keyboard_after_canceled_hold'] = expected_text
                client.terminate()
                client.wait(timeout=5)
                client = None
            mouse.close()
            mouse = None
            request(control, 'exit')
            record['compositor_exit'] = proc.wait(timeout=5)
            assert record['compositor_exit'] == 0
            record['validation'] = {'passed': True, 'protected_configure_and_opacity': True, 'real_client_pointer_and_pixel_agreement': True, 'native_kms_tested': False, 'actual_pam_tested': False, 'sensor_tested': False}
        print('PASS: nested protected rotation, actual pixels and client input agree in all orientations')
    finally:
        if mouse:
            mouse.close()
        for child in [client, proc]:
            if child and child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        runtime.cleanup()
        (out/'status.json').write_text(json.dumps(record, indent=2)+'\n')


if __name__ == '__main__':
    main()
