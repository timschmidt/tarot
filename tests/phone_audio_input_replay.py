"""Guarded native input/route controls; no playback or recording stream is opened."""
import csv
import io
import json
import os
from pathlib import Path
import pwd
import re
import subprocess
import time

from PIL import Image, ImageOps
from dialog_button import locate
from audio_control_target import level_glyph
from lock_replay import any_request
from notification_replay import await_value
from phone_gesture_replay import request


def replay(socket, screen, tap, state, capture, before, directory, cgroup, authenticate):
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    account = pwd.getpwuid(10000)
    env = dict(HOME=account.pw_dir, XDG_RUNTIME_DIR='/run/user/10000', PATH='/usr/bin:/bin')
    result = dict(passed=False, recording_stream_opened=False, audio_playback_requested=False, targets=[])
    module = None
    module_argument = 'source_name=tarot_qa_input_r55 channels=2 description=Tarot_QA_input'

    def pactl(*args):
        return subprocess.check_output(['pactl', *map(str, args)], env=env, user=10000,
                group=account.pw_gid, extra_groups=os.getgrouplist(account.pw_name, account.pw_gid),
                text=True, timeout=8).strip()

    def assert_owned_module(index):
        rows = [line.split('\t') for line in pactl('list', 'short', 'modules').splitlines()]
        matches = [row for row in rows if row and row[0] == str(index)]
        assert len(matches) == 1 and len(matches[0]) >= 3
        assert matches[0][1] == 'module-null-source' and matches[0][2] == module_argument

    def devices(kind):
        return json.loads(pactl('-f', 'json', 'list', kind))

    def controls():
        return dict(default_input=pactl('get-default-source'), default_output=pactl('get-default-sink'),
                    sources={d['name']: dict(values=[c['value'] for c in d['volume'].values()], muted=d['mute'],
                        port=d['active_port']) for d in devices('sources')},
                    sinks={d['name']: dict(values=[c['value'] for c in d['volume'].values()], muted=d['mute'],
                        port=d['active_port']) for d in devices('sinks')})

    def audio(label, condition):
        return state('audio-input-' + label, lambda s: s['audio']['available'] and condition(s['audio']))['audio']

    def action(label, value, condition):
        request(socket, 'audio_action', action=value)
        return audio(label, lambda a: a['pending'] is None and not a['action_error'] and condition(a))

    def input_row(a):
        return next(i for i in a['inputs'] if i['id'] == input_id)

    def physical_source():
        return next(d for d in devices('sources') if d['name'] == physical_name)

    def percent():
        source = physical_source()
        return round(max(c['value'] for c in source['volume'].values()) * 100 / 65536)

    def swipe():
        screen.down(180, 570)
        for step in range(1, 13):
            screen.move(180, 570 - 25 * step)
            time.sleep(.025)
        screen.up()
        time.sleep(.3)

    def find_line(label, pattern):
        for attempt in range(8):
            name = 'audio-input-' + label + '-' + str(attempt)
            capture(name)
            image = directory / (name + '.png')
            contrast = image.with_name(image.stem + '-contrast.png')
            with Image.open(image) as picture:
                ImageOps.grayscale(picture).point(lambda value: 0 if value > 200 else 255).save(contrast)
            for variant in [image, contrast]:
                found = subprocess.run(['tesseract', str(variant), 'stdout', '--psm', '11', 'tsv'],
                                       capture_output=True, text=True, check=True, timeout=20)
                lines = {}
                for row in csv.DictReader(io.StringIO(found.stdout), delimiter='\t'):
                    if row['text'].strip() and float(row['conf']) >= 60:
                        key = tuple(row[k] for k in ['page_num', 'block_num', 'par_num', 'line_num'])
                        lines.setdefault(key, []).append(row)
                for rows in lines.values():
                    text = ' '.join(r['text'] for r in rows)
                    top = min(int(r['top']) for r in rows)
                    bottom = max(int(r['top']) + int(r['height']) for r in rows)
                    if re.fullmatch(pattern, text, re.I) and 430 < top < 1160:
                        left = min(int(r['left']) for r in rows)
                        right = max(int(r['left']) + int(r['width']) for r in rows)
                        return image, dict(text=text, top=top, bottom=bottom, point=[(left + right) / 4, (top + bottom) / 4])
            swipe()
        raise AssertionError('Rendered audio input target missing: ' + label)

    def button(label, word):
        image, line = find_line(label, r'Input level:.*')
        # Only the row directly below the observed input level can satisfy this
        # target; output controls and another input's controls are excluded.
        if word in ['+', '−']:
            target = level_glyph(image, line['bottom'], word)
        else:
            target = locate(image, word, vertical=(line['bottom'] / 1440, (line['bottom'] + 110) / 1440))
        result['targets'].append(dict(label=label, anchor=line, button=target))
        divisor = 1 if word in ['+', '−'] else 2
        tap(target['point'][0] / divisor, target['point'][1] / divisor)

    def text_button(label, text):
        _, target = find_line(label, re.escape(text))
        result['targets'].append(dict(label=label, target=target))
        tap(*target['point'])

    original = controls()
    result['original_controls'] = original
    physical = [d for d in devices('sources') if not d['monitor_source']]
    assert len(physical) == 1 and not devices('source-outputs'), 'Require one idle real input'
    physical_name = physical[0]['name']
    assert original['default_input'] == physical_name
    assert not any(d['name'] == 'tarot_qa_input_r55' for d in devices('sources'))
    ready = audio('ready', lambda a: len(a['inputs']) == 1 and len(a['outputs']) == 1)
    input_id = ready['inputs'][0]['id']
    output = ready['outputs'][0]
    result['monitor_sources_excluded'] = len(devices('sources')) > len(ready['inputs'])
    assert result['monitor_sources_excluded'] and input_id != output['id']
    assert len(ready['inputs'][0]['ports']) == len(output['ports']) == 1
    assert ready['inputs'][0]['ports'][0]['active'] and output['ports'][0]['active']
    try:
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Audio')
        # Establish a non-edge input value, then exercise actual plus/minus and
        # mute targets with independent server readback after every observation.
        action('baseline-level', {'InputVolume': dict(input=input_id, percent=40)}, lambda a: abs(input_row(a)['percent'] - 40) <= 1)
        button('plus', '+')
        audio('plus-observed', lambda a: a['pending'] is None and abs(input_row(a)['percent'] - 50) <= 1)
        assert abs(percent() - 50) <= 1
        button('minus', '−')
        audio('minus-observed', lambda a: a['pending'] is None and abs(input_row(a)['percent'] - 40) <= 1)
        assert abs(percent() - 40) <= 1
        action('unmute-baseline', {'InputMute': dict(input=input_id, muted=False)}, lambda a: not input_row(a)['muted'])
        button('mute', 'Mute')
        audio('mute-observed', lambda a: a['pending'] is None and input_row(a)['muted'])
        assert physical_source()['mute']
        button('unmute', 'Unmute')
        audio('unmute-observed', lambda a: a['pending'] is None and not input_row(a)['muted'])
        assert not physical_source()['mute']
        original_port = next(p['description'] for p in physical[0]['ports'] if p['name'] == original['sources'][physical_name]['port'])
        previous = request(socket, 'status')['audio']['generation']
        text_button('input-route', original_port)
        audio('input-route-observed', lambda a: a['generation'] > previous and a['pending'] is None and not a['action_error'])
        assert physical_source()['active_port'] == original['sources'][physical_name]['port']
        # Explicitly reselect the only output port through the adapter too; its
        # independent active-port value remains unchanged. No audio is played.
        previous = request(socket, 'status')['audio']['generation']
        action('output-route-observed', {'OutputPort': dict(output=output['id'], port=output['ports'][0]['id'])},
               lambda a: a['generation'] > previous and a['outputs'][0]['ports'][0]['active'])
        module = int(pactl('load-module', 'module-null-source', *module_argument.split()))
        two = audio('synthetic-input', lambda a: len(a['inputs']) == 2)
        synthetic_id = next(i['id'] for i in two['inputs'] if i['id'] != input_id)
        if not input_row(two)['default']:
            action('physical-default-baseline', {'DefaultInput': dict(input=input_id)}, lambda a: input_row(a)['default'])
        text_button('select-synthetic-default', 'Use for new recordings')
        audio('synthetic-default', lambda a: next(i for i in a['inputs'] if i['id'] == synthetic_id)['default'] and a['pending'] is None)
        assert pactl('get-default-source') == 'tarot_qa_input_r55'
        text_button('restore-physical-default', 'Use for new recordings')
        audio('physical-default', lambda a: input_row(a)['default'] and a['pending'] is None)
        assert pactl('get-default-source') == physical_name
        assert_owned_module(module)
        pactl('unload-module', module)
        module = None
        audio('synthetic-removed', lambda a: len(a['inputs']) == 1)
        request(socket, 'audio_action', action={'InputMute': dict(input=synthetic_id, muted=True)})
        audio('stale-input-rejected', lambda a: a['pending'] is None and bool(a['action_error']))
        request(socket, 'lock')
        protected = state('audio-input-protected', lambda s: s['lock']['phase'] == 'Locked')
        assert set(protected) == {'ok', 'pid', 'lock'}
        assert not any_request(socket, 'audio_action', action={'InputMute': dict(input=input_id, muted=True)})['ok']
        assert not any_request(socket, 'audio_action', action={'InputPort': dict(input=input_id, port=ready['inputs'][0]['ports'][0]['id'])})['ok']
        authenticate('audio-input-pam')
        result['actual_pam'] = True
        result['passed'] = True
        return result
    finally:
        try:
            # Restore values, including exact per-channel integers and mute, even
            # if an OCR or driver check fails. Do not copy a running daemon's DB.
            for name, values in original['sources'].items():
                pactl('set-source-volume', name, *values['values'])
                pactl('set-source-mute', name, int(values['muted']))
                if values['port'] is not None:
                    pactl('set-source-port', name, values['port'])
            pactl('set-default-source', original['default_input'])
            if module is not None:
                assert_owned_module(module)
                pactl('unload-module', module)
            result['controls_after'] = await_value(controls, lambda v: v == original)
            result['exact_controls_restored'] = result['controls_after'] == original
            assert not devices('source-outputs')
        except BaseException:
            result['passed'] = False
            raise
        finally:
            (directory / 'audio-input-result.json').write_text(json.dumps(result, indent=2) + '\n')
