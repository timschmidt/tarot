"""Native card-mode and menu-touch checks; no stream or call is opened.

An optional saved-mode check delegates final restoration to its controller after
normal login. Otherwise the original logical mode/device controls are restored.
"""
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import re
import select
import signal
import socket as sockets
import struct
import subprocess
import sys
import time

from PIL import Image, ImageOps
from lock_replay import any_request
from phone_audio_state import devices, pactl, restore, snapshot, peer_identity, start_standard_server, stop_verified_server
from phone_gesture_replay import request


def replay(socket, screen, tap, state, capture, before, directory, cgroup, authenticate, save_for_restart=False):
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    original = snapshot()
    cards = devices('cards')
    assert len(cards) == 1 and not devices('sink-inputs') and not devices('source-outputs')
    card_name = cards[0]['name']
    original_profile = cards[0]['active_profile']
    earpiece = 'HiFi (Earpiece, Mic)'
    assert original_profile == 'HiFi (Mic, Speaker)'
    assert cards[0]['profiles'][earpiece]['available'] and cards[0]['profiles']['off']['available']
    labels = {name: data['description'] for name, data in cards[0]['profiles'].items()}
    result = dict(passed=False, recording_stream_opened=False, audio_playback_requested=False,
                  targets=[], modes=[], original_controls=original, saved_profile=None)

    def audio(label, condition=lambda a: True):
        return state('audio-profiles-' + label, lambda s: condition(s['audio']))['audio']

    def current_profile():
        return next(c for c in devices('cards') if c['name'] == card_name)['active_profile']

    def current_card(a):
        assert a['available'] and len(a['cards']) == 1
        return a['cards'][0]

    def active_id(a):
        active = [p['id'] for p in current_card(a)['profiles'] if p['active']]
        assert len(active) == 1
        return active[0]

    def swipe(direction):
        start, end = (570, 240) if direction == 'up' else (280, 580)
        screen.down(180, start)
        for step in range(1, 13):
            screen.move(180, start + (end - start) * step / 12)
        screen.up()
        time.sleep(.2)

    def image(label):
        name = 'audio-profiles-' + label
        capture(name)
        return directory / (name + '.png')

    def recognized(path, text):
        normalize = lambda value: re.sub(r'\s+', '', value).casefold()
        expected = normalize(text)
        contrast = path.with_name(path.stem + '-contrast.png')
        with Image.open(path) as source:
            assert source.size == (720, 1440)
            ImageOps.grayscale(source).point(lambda v: 0 if v > 200 else 255).save(contrast)
        for variant in [path, contrast]:
            output = subprocess.run(['tesseract', str(variant), 'stdout', '--psm', '11', 'tsv'],
                                    capture_output=True, text=True, check=True, timeout=20)
            lines = {}
            for row in csv.DictReader(io.StringIO(output.stdout), delimiter='\t'):
                if row['text'].strip() and float(row['conf']) >= 60:
                    key = tuple(row[k] for k in ['page_num', 'block_num', 'par_num', 'line_num'])
                    lines.setdefault(key, []).append(row)
            for rows in lines.values():
                actual = ' '.join(r['text'] for r in rows)
                left, top = min(int(r['left']) for r in rows), min(int(r['top']) for r in rows)
                right = max(int(r['left']) + int(r['width']) for r in rows)
                bottom = max(int(r['top']) + int(r['height']) for r in rows)
                if normalize(actual) == expected and 430 < top < bottom < 1310:
                    return dict(text=actual, bounds=[left, top, right, bottom],
                                point=[(left + right) / 4, (top + bottom) / 4])
        return None

    def find_text(label, text):
        # Begin at the top on each search. Mode changes can replace preceding
        # device groups; searching only down from an old offset can miss a row.
        for _ in range(7):
            swipe('down')
        for attempt in range(12):
            path = image(label + '-' + str(attempt))
            target = recognized(path, text)
            if target is not None:
                result['targets'].append(dict(label=label, target=target))
                return path, target
            swipe('up')
        raise AssertionError('Rendered audio mode target missing: ' + text)

    def tap_text(label, text):
        _, target = find_text(label, text)
        tap(*target['point'])

    def open_modes():
        request(socket, 'system_menu', open=False)
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Audio')
        tap_text('expand-modes', 'Audio device modes')

    def select_mode(name):
        same = current_profile() == name
        prior = audio('before-mode-' + name)
        previous = active_id(prior)
        tap_text('choose-mode-' + name, labels[name])
        observed = audio('mode-' + name, lambda a: a['available'] and a['pending'] is None and
                         not a['action_error'] and a['generation'] > prior['generation'] and
                         ((active_id(a) == previous) if same else (active_id(a) != previous)))
        assert current_profile() == name
        result['modes'].append(dict(profile=name, observed_profile_id=active_id(observed),
                                    outputs=len(observed['outputs']), inputs=len(observed['inputs'])))
        return observed

    def paused_roundtrip():
        # The peer credentials identify the real server at the normal user's
        # local endpoint. No protocol/authentication bytes or samples are read.
        with sockets.socket(sockets.AF_UNIX) as peer:
            peer.connect('/run/user/10000/pulse/native')
            pid, uid, _ = struct.unpack('3i', peer.getsockopt(sockets.SOL_SOCKET, sockets.SO_PEERCRED, 12))
        proc = Path('/proc', str(pid))
        assert uid == 10000 and proc.stat().st_uid == uid
        assert os.readlink(proc / 'exe') == '/usr/bin/pulseaudio'
        assert (proc / 'cgroup').read_text() == cgroup
        identity = dict(pid=pid, start=(proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
                        sha256=hashlib.sha256((proc / 'exe').read_bytes()).hexdigest(), cgroup=cgroup)
        assert not devices('sink-inputs') and not devices('source-outputs')
        old = audio('before-paused-server')
        old_card, old_profile = current_card(old)['id'], active_id(old)
        fd = os.pidfd_open(pid)
        read, write = os.pipe()
        watchdog = None
        try:
            watchdog = subprocess.Popen([sys.executable, str(Path(__file__).parent / 'phone_resume_watchdog.py'),
                                         str(fd), str(write), '5'], pass_fds=(fd, write),
                                        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
            os.close(write)
            write = None
            assert select.select([read], [], [], 2)[0] and os.read(read, 1) == b'R'
            assert proc.stat().st_uid == uid and (proc / 'cgroup').read_text() == cgroup
            assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == identity['start']
            assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == identity['sha256']
            signal.pidfd_send_signal(fd, signal.SIGSTOP)
            request(socket, 'audio_action', action={'CardProfile': dict(card=old_card, profile=old_profile)})
            audio('paused-pending', lambda a: a['pending'] is not None and active_id(a) == old_profile)
            request(socket, 'home')
            assert request(socket, 'status')['view'] == 'Home'
            audio('paused-timeout', lambda a: not a['available'] and a['pending'] is None and bool(a['action_error']))
        finally:
            try:
                signal.pidfd_send_signal(fd, signal.SIGCONT)
            except ProcessLookupError:
                pass
            if watchdog is not None:
                assert watchdog.wait(timeout=7) == 0
            os.close(fd)
            os.close(read)
            if write is not None:
                os.close(write)
        fresh = audio('server-recovered', lambda a: a['available'] and len(a['cards']) == 1 and a['epoch'] > old['epoch'])
        assert current_card(fresh)['id'] != old_card and active_id(fresh) != old_profile
        assert current_profile() == original_profile
        request(socket, 'audio_action', action={'CardProfile': dict(card=old_card, profile=old_profile)})
        audio('old-mode-rejected', lambda a: a['pending'] is None and bool(a['action_error']))
        result['paused_server'] = dict(identity=identity, automatic_resume_watchdog=True,
                                      actual_process_restarted=False, timeout_reconnect=True,
                                      stale_card_rejected=True, ui_responsive=True)
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Audio')
        return fresh

    try:
        initial = audio('ready', lambda a: a['available'] and len(a['cards']) == 1 and len(a['outputs']) == len(a['inputs']) == 1)
        assert len(current_card(initial)['profiles']) == len(cards[0]['profiles'])
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Audio')
        # Repeat a horizontal label drag that previously selected explanatory
        # text. The body glyph box contains no selected-green paint afterward.
        _, target = find_text('label-probe', 'Input mute controls the sound server.')
        left, top, right, bottom = target['bounds']
        start = ((left + 5) / 2, (top + bottom) / 4)
        finish = (min(right - 8, left + 300) / 2, start[1])
        screen.down(*start)
        for step in range(1, 13):
            screen.move(start[0] + (finish[0] - start[0]) * step / 12, start[1])
        screen.up()
        path = image('label-dragged')
        with Image.open(path) as picture:
            area = picture.convert('RGB').crop((left, top, right, bottom))
            selected = sum(pixel == (95, 115, 91) for pixel in area.getdata())
        assert selected == 0, 'Shell explanation text was selected by the drag'
        result['label_selection_pixels'] = selected
        # Exercise a same-value command: the UI reservation must still retire.
        prior = audio('before-same-mode')
        request(socket, 'audio_action', action={'CardProfile': dict(card=current_card(prior)['id'], profile=active_id(prior))})
        audio('same-mode-completed', lambda a: a['generation'] > prior['generation'] and a['pending'] is None and not a['action_error'])
        unavailable = next(p for p in current_card(initial)['profiles'] if not p['available'])
        request(socket, 'audio_action', action={'CardProfile': dict(card=current_card(initial)['id'], profile=unavailable['id'])})
        audio('unavailable-mode-rejected', lambda a: a['pending'] is None and bool(a['action_error']))
        assert current_profile() == original_profile
        open_modes()
        ear = select_mode(earpiece)
        before_off = dict(outputs=[d['id'] for d in ear['outputs']], inputs=[d['id'] for d in ear['inputs']])
        off = select_mode('off')
        # The distribution's always-sink module can create a dummy sink when
        # the physical card is Off. Verify the card and physical endpoints;
        # do not disable that distro policy to obtain an empty global list.
        off_card = next(c for c in devices('cards') if c['name'] == card_name)
        off_sinks, off_sources = devices('sinks'), devices('sources')
        assert off_card['active_profile'] == 'off'
        assert not off['inputs']
        assert all(d['name'] == 'auto_null' and d['driver'] == 'module-null-sink.c' for d in off_sinks)
        assert all(d['name'] == 'auto_null.monitor' for d in off_sources)
        assert len(off['outputs']) == len(off_sinks)
        result['off_inventory'] = dict(card=off_card['name'], profile=off_card['active_profile'],
                                      sinks=[dict(name=d['name'], driver=d['driver']) for d in off_sinks],
                                      sources=[d['name'] for d in off_sources], physical_endpoints=0)
        back = select_mode(original_profile)
        assert len(back['outputs']) == len(back['inputs']) == 1
        assert all(d['id'] not in before_off['outputs'] for d in back['outputs'])
        assert all(d['id'] not in before_off['inputs'] for d in back['inputs'])
        for value in [{'Mute': dict(output=before_off['outputs'][0], muted=True)},
                      {'InputMute': dict(input=before_off['inputs'][0], muted=True)}]:
            request(socket, 'audio_action', action=value)
            audio('old-endpoint-rejected-' + next(iter(value)), lambda a: a['pending'] is None and bool(a['action_error']))
        result['off_and_device_replacement'] = True
        fresh = paused_roundtrip()
        open_modes()
        select_mode(earpiece)
        time.sleep(2)
        old = peer_identity()
        old_state = audio('before-live-restart')
        assert stop_verified_server(old)
        audio('server-exited', lambda a: not a['available'] and not a['cards'])
        replacement = start_standard_server(before['pid'])
        fresh = audio('live-server-recovered', lambda a: a['available'] and len(a['cards']) == 1 and
                      a['epoch'] > old_state['epoch'] and len(a['outputs']) == len(a['inputs']) == 1)
        assert (replacement['pid'], replacement['start']) != (old['pid'], old['start'])
        assert replacement['sha256'] == old['sha256']
        assert current_card(fresh)['id'] != current_card(old_state)['id']
        assert current_profile() == earpiece, 'Selected mode must survive a real server restart'
        request(socket, 'audio_action', action={'CardProfile': dict(card=current_card(old_state)['id'], profile=active_id(old_state))})
        audio('old-restarted-card-rejected', lambda a: a['pending'] is None and bool(a['action_error']))
        result['live_server_restart'] = dict(original=old, replacement=replacement,
                                            selected_mode_persisted=True, stale_card_rejected=True)
        open_modes()
        select_mode(original_profile)
        fresh = audio('before-protected-mode')
        request(socket, 'lock')
        protected = state('audio-profiles-protected', lambda s: s['lock']['phase'] == 'Locked')
        assert set(protected) == {'ok', 'pid', 'lock'}
        assert not any_request(socket, 'audio_action', action={'CardProfile': dict(card=current_card(fresh)['id'], profile=active_id(fresh))})['ok']
        authenticate('audio-profiles-pam')
        result['actual_pam'] = True
        if save_for_restart:
            open_modes()
            select_mode(earpiece)
            assert current_profile() == earpiece
            result['saved_profile'] = dict(card=card_name, profile=earpiece)
            time.sleep(2)  # Let callbacks settle; graceful daemon teardown flushes pending saves.
        result['passed'] = True
        return result
    finally:
        failed = sys.exc_info()[0] is not None
        if failed:
            result['passed'] = False
        try:
            if not (result['passed'] and save_for_restart and result['saved_profile']):
                result['controls_after'] = restore(original)
                result['exact_controls_restored'] = result['controls_after'] == original
            else:
                result['restoration_deferred_to_controller'] = True
            assert not devices('sink-inputs') and not devices('source-outputs')
        except BaseException:
            result['passed'] = False
            raise
        finally:
            (directory / 'audio-profiles-result.json').write_text(json.dumps(result, indent=2) + '\n')
