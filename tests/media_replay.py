#!/usr/bin/env python3
"""MPRIS media controls on an owned display/private bus; no audio output."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--binary', type=Path, default=Path('target/debug/tarot'))
    p.add_argument('--xvfb', default='Xvfb')
    a = p.parse_args()
    out = a.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.binary.resolve()
    examples = binary.parent / 'examples'
    root = Path.cwd()
    children, streams = [], []
    player = None
    record = dict(passed=False, cases=[], binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())

    def log(name):
        stream = (out / name).open('x')
        streams.append(stream)
        return stream

    def passed(case):
        record['cases'].append(case)
        print('PASS', case, flush=True)

    with tempfile.TemporaryDirectory(prefix='tarot-media-') as temporary:
        work = Path(temporary)
        (work / 'empty').mkdir()
        (work / 'runtime').mkdir(mode=0o700)
        commands = work / 'lock.commands'
        commands.write_text('')
        control = work / 'control.sock'

        def media(label, condition=lambda m: True, seconds=10):
            return await_value(lambda: request(control, 'status')['media'], condition, seconds=seconds)

        def calls():
            return [e for e in player.events() if e['event'] == 'call']

        def act(player_id, generation, action):
            request(control, 'media_action', control=dict(player=player_id, generation=generation, action=action))

        try:
            read, write = os.pipe()
            try:
                display = subprocess.Popen([a.xvfb, '-displayfd', str(write), '-nolisten', 'tcp',
                          '-noreset', '-ac', '-screen', '0', '1000x1000x24'], pass_fds=(write,),
                          stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
                children.append(display)
                os.close(write)
                write = None
                number = read_display_number(read)
            finally:
                os.close(read)
                if write is not None:
                    os.close(write)
            address = 'unix:path=' + str(work / 'bus')
            bus = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile',
                    '--address=' + address], stdout=log('bus.log'), stderr=subprocess.STDOUT)
            children.append(bus)
            await_value(lambda: (work / 'bus').exists() or bus.poll() is not None, bool)
            assert bus.poll() is None
            env = dict(os.environ, DISPLAY=':' + number, WINIT_UNIX_BACKEND='x11',
                       XDG_SESSION_TYPE='x11', LIBGL_ALWAYS_SOFTWARE='1',
                       __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',
                       XDG_RUNTIME_DIR=str(work / 'runtime'), DBUS_SESSION_BUS_ADDRESS=address,
                       XDG_CONFIG_HOME=str(work / 'empty'), XDG_CONFIG_DIRS=str(work / 'empty'),
                       XDG_DATA_HOME=str(work / 'empty'), XDG_DATA_DIRS=str(work / 'empty'))
            env.pop('WAYLAND_DISPLAY', None)
            env.pop('WAYLAND_SOCKET', None)
            player = Fixture(root / 'tests/mpris_fixture.py', out / 'player', env)
            client = subprocess.Popen([str(binary), '--reduced-motion', '--system-services',
                     '--platform-bus', address, '--backlight-root', str(work / 'empty'),
                     '--publish-session-environment', '--notifications', '--control-socket', str(control),
                     '--config-file', str(work / 'settings.toml'), '--lock-probe-unlocked-start',
                     '--lock-client', str(examples / 'lock-probe'), '--lock-client-arg=--role',
                     '--lock-client-arg=locker', '--lock-client-arg=--commands',
                     '--lock-client-arg=' + str(commands), '--lock-client-arg=--report',
                     '--lock-client-arg=' + str(out / 'locker.jsonl'), '--exit-after-ms', '120000'],
                     env=env, stdout=log('session.log'), stderr=subprocess.STDOUT)
            children.append(client)
            await_value(lambda: control.exists() or client.poll() is not None, bool)
            assert client.poll() is None

            m = media('discovered', lambda m: m['available'] and len(m['players']) == 1)
            first = m['players'][0]
            assert first['identity'] == 'Fixture Player' and first['title'] == 'Morning Song'
            assert first['artist'] == 'Example Artist' and first['status'] == 'Paused'
            assert first['can_play'] and first['can_go_next'] and not first['can_go_previous']
            passed('player-discovered-with-display-text-and-capabilities')

            player.call('set', values=dict(title='Bad‮\ttitle\u0007' + 'x' * 300, artist=['One', 'Two']))
            m = media('cleaned', lambda m: m['players'] and m['players'][0]['title'].startswith('Bad title'))
            title = m['players'][0]['title']
            assert '‮' not in title and '\u0007' not in title and len(title) == 160 and title.endswith('…')
            assert m['players'][0]['artist'] == 'One, Two'
            player.call('set', values=dict(title='Morning Song', artist=['Example Artist']))
            m = media('restored', lambda m: m['players'] and m['players'][0]['title'] == 'Morning Song')
            passed('text-cleaned-of-controls-and-bidi-overrides-and-bounded')

            before = len(calls())
            act(first['id'], m['generation'] - 1, 'PlayPause')
            act(first['id'], m['generation'], 'Previous')
            time.sleep(1.5)
            assert len(calls()) == before, 'stale and unsupported controls must not reach the player'
            passed('stale-generation-and-unsupported-control-refused')

            act(first['id'], m['generation'], 'PlayPause')
            m = media('playing', lambda m: m['players'][0]['status'] == 'Playing' and m['pending'] is None)
            new = calls()[before:]
            assert [c['method'] for c in new] == ['PlayPause'] and new[0]['interface'] == 'org.mpris.MediaPlayer2.Player'
            passed('one-control-delivered-and-state-observed-from-player')

            reads = player.call('reads')['result']['reads']
            time.sleep(3)
            assert player.call('reads')['result']['reads'] == reads, 'idle media worker must not poll'
            passed('healthy-worker-does-not-poll')

            reads = player.call('reads')['result']['reads']
            player.call('flood', count=200)
            time.sleep(2)
            flood_reads = player.call('reads')['result']['reads'] - reads
            record['flood_property_reads'] = flood_reads
            assert flood_reads < 200 * 9 / 4, flood_reads
            passed('signal-flood-coalesced')

            player.call('mode', value='hang')
            m = media('before-hang')
            act(first['id'], m['generation'], 'Next')
            media('pending', lambda m: m['pending'] is not None, seconds=3)
            m = media('timed-out', lambda m: m['pending'] is None and m['error'] is not None, seconds=8)
            player.call('mode', value='normal')
            player.call('release')
            passed('unresponsive-player-times-out-with-visible-error')

            request(control, 'notifications', open=True)
            time.sleep(1)
            request(control, 'capture', path=str(out / 'dashboard.png'))
            await_value(lambda: request(control, 'status'), lambda s: not s['capture_pending'] and not s['capture_error'])
            request(control, 'notifications', open=False)

            old = media('before-replace')
            player.call('replace-owner')
            m = media('replaced', lambda m: len(m['players']) == 1 and m['players'][0]['id'] != first['id'])
            before = len(calls())
            act(first['id'], m['generation'], 'PlayPause')
            time.sleep(1.5)
            assert len(calls()) == before, 'a control for the old exporter must not reach its replacement'
            passed('owner-replacement-retires-old-player-identity')

            player.call('set', values=dict(padding=70000))
            media('oversized-skipped', lambda m: m['players'] == [])
            player.call('set', values=dict(padding=0))
            media('back', lambda m: len(m['players']) == 1)
            passed('oversized-player-reply-skipped')

            player.call('vanish')
            media('vanished', lambda m: m['players'] == [])
            passed('player-removed-when-name-released')

            request(control, 'exit')
            assert client.wait(timeout=10) == 0
            record['passed'] = True
        finally:
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
            if player is not None and player.process.poll() is None:
                player.process.terminate()
                player.process.wait(timeout=5)
            for stream in streams:
                stream.close()
            (out / 'result.json').write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()
