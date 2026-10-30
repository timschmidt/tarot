#!/usr/bin/env python3
"""Actual top-menu input, capture, client ownership and cooperative-close checks."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from PIL import Image
from dialog_button import locate
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--compositor', type=Path)
    parser.add_argument('--scale', type=int, choices=[1, 2], default=1)
    parser.add_argument('--landscape', action='store_true')
    parser.add_argument('--require-export-peer', action='store_true',
                        help='Require a retained live process handle for the real native client')
    args = parser.parse_args()
    out = args.output.resolve(); out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd(); width, height = (720, 360) if args.landscape else (360, 720)
    compositor_binary = (args.compositor or root/'target/debug/tarot').resolve()
    records = {'passed': False, 'phone_kms_tested': False, 'scale': args.scale,
               'landscape': args.landscape, 'actions': [],
               'compositor_sha256': hashlib.sha256(compositor_binary.read_bytes()).hexdigest()}
    sources = {}; pending = [Path(__file__).resolve()]; snapshot = out/'qa-sources'; snapshot.mkdir()
    while pending:
        source = pending.pop()
        if source.name in sources: continue
        payload = source.read_bytes(); (snapshot/source.name).write_bytes(payload)
        sources[source.name] = hashlib.sha256(payload).hexdigest()
        for node in ast.walk(ast.parse(payload)):
            modules = [a.name for a in node.names] if isinstance(node, ast.Import) else [node.module] if isinstance(node, ast.ImportFrom) and node.module else []
            for module in modules:
                candidate = source.parent/(module.split('.')[0]+'.py')
                if candidate.is_file(): pending.append(candidate)
    records['executed_sources_sha256'] = sources
    children = []; logs = []; mouse = None
    with tempfile.TemporaryDirectory(prefix='tarot-menus-') as temporary:
        work = Path(temporary); control = work/'control.sock'; config = work/'settings.toml'
        for directory in ['runtime', 'data/applications', 'empty', 'config']:
            (work/directory).mkdir(parents=True, exist_ok=True, mode=0o700)
        original = 'version = 3\n[notifications]\ndnd = true\n'
        config.write_text(original)
        def log(name):
            stream = (out/name).open('x'); logs.append(stream); return stream
        def state(label, predicate=lambda s: True):
            value = await_value(lambda: request(control, 'status'), predicate, seconds=10)
            records['actions'].append({'label': label, 'state': value}); return value
        def capture(label):
            state(label+'-settled', lambda s: not s['system_menu_open'] or not s['top_menu']['motion']['moving'])
            path = out/(label+'.png'); request(control, 'capture', path=str(path))
            state(label, lambda s: not s['capture_pending'] and not s['capture_error'])
            with Image.open(path) as image:
                image.load(); assert image.size == (width*args.scale, height*args.scale)
            return path
        def click(x, y):
            mouse.move(x*args.scale, y*args.scale); time.sleep(.04)
            mouse.button(True); time.sleep(.06); mouse.button(False); time.sleep(.1)
        def item(word, label, side='left'):
            path = capture(label)
            with Image.open(path) as image:
                bounds = (0, 32*args.scale, min(width, 240)*args.scale, (height-36)*args.scale) if side == 'left' else ((width-min(448, width))*args.scale, 32*args.scale, width*args.scale, (height-36)*args.scale)
                cropped = image.crop(bounds); cropped_path = out/(label+'-panel.png'); cropped.save(cropped_path)
            found = locate(cropped_path, word, vertical=(0, 1))
            point = ((bounds[0]+found['point'][0])/args.scale, (bounds[1]+found['point'][1])/args.scale)
            records['actions'].append({'label': label+'-visible-target', 'point': point, 'ocr': found})
            click(*point)
        def app_events(path):
            return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
        def launch_native(label, refuse=False):
            report = out/(label+'.jsonl')
            command = [str(root/'target/debug/examples/input-probe'), '--report', str(report), '--exit-after-seconds', '180', '--no-child', '--audit-key-balance']
            if refuse: command.append('--refuse-close')
            process = subprocess.Popen(command, env=native_env, stdout=log(label+'.log'), stderr=subprocess.STDOUT)
            children.append(process)
            value = state(label+'-mapped', lambda s: isinstance(s['view'], dict) and len(s['cards']) == 1 and bool(s['presentation']['windows']) and s['presentation']['windows'][0]['opacity'] == 1)
            await_value(lambda: any(e['event'] == 'keyboard_enter' for e in app_events(report)), bool)
            if args.require_export_peer:
                assert value['native_export_peers'] == [value['cards'][0]['id']]
                records['native_export_peer_verified'] = True
            return process, report, value['cards'][0]['id']
        try:
            read, write = os.pipe()
            try:
                xvfb = subprocess.Popen([args.xvfb, '-displayfd', str(write), '-nolisten', 'tcp', '-noreset', '-ac', '-screen', '0', '1600x1600x24'], pass_fds=(write,), stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
                children.append(xvfb); os.close(write); write = None; number = read_display_number(read)
            finally:
                os.close(read)
                if write is not None: os.close(write)
            env = dict(os.environ, DISPLAY=':'+number, WINIT_UNIX_BACKEND='x11', XDG_SESSION_TYPE='x11', LIBGL_ALWAYS_SOFTWARE='1', __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json', XDG_RUNTIME_DIR=str(work/'runtime'), XDG_DATA_HOME=str(work/'data'), XDG_DATA_DIRS=str(work/'empty'), XDG_CONFIG_HOME=str(work/'config'), XDG_CONFIG_DIRS=str(work/'empty'))
            env.pop('WAYLAND_DISPLAY', None); env.pop('WAYLAND_SOCKET', None)
            session = subprocess.Popen([str(compositor_binary), '--width', str(width*args.scale), '--height', str(height*args.scale), '--scale', str(args.scale), '--config-file', str(config), '--control-socket', str(control), '--exit-after-ms', '180000'], env=env, stdout=log('session.log'), stderr=subprocess.STDOUT)
            children.append(session)
            await_value(lambda: control.exists() or session.poll() is not None, bool); assert session.poll() is None
            ready = state('ready')
            mouse = PrivateInput(session.pid, xvfb, env['DISPLAY']); await_value(mouse.find, bool)
            native_env = dict(env, WAYLAND_DISPLAY=ready['wayland_display']); native_env.pop('DISPLAY', None)
            click(40, 16); state('home-activity', lambda s: s['system_menu_open'] and s['top_menu']['entry'] == 'Activity')
            capture('home-activity')
            click(width-35, 16); state('switched-device', lambda s: s['system_menu_open'] and s['top_menu']['entry'] == 'Device')
            capture('device-root')
            item('display', 'device-power', 'right'); state('power-detail', lambda s: s['system_page'] == 'Power')
            detail=capture('power-detail')
            # Traverse the actual enabled widgets to the visible detail Back
            # control. Enter must reach egui rather than menu-row selection.
            with Image.open(detail) as image:
                left=max(0,width-448)*args.scale;top=32*args.scale
                path=out/'detail-back-panel.png';image.crop((left,top,width*args.scale,(height-36)*args.scale)).save(path)
            found=locate(path,'Back',vertical=(0,1))
            back=((left+found['point'][0])/args.scale,(top+found['point'][1])/args.scale)
            focused=False
            for index in range(1,17):
                mouse.tap(23);picture=capture('detail-tab-'+str(index))
                with Image.open(picture) as image:
                    # egui paints a light focus outline above the label. Keep
                    # this band outside the letters and adjacent controls.
                    band=image.crop(tuple(round(v*args.scale) for v in (back[0]-40,back[1]-30,back[0]+35,back[1]-18)))
                    outline=sum(min(pixel[:3])>210 for pixel in band.getdata())
                if outline>40*args.scale:
                    focused=True;break
            assert focused,'The actual Back button must receive visible keyboard focus'
            records['actions'].append({'label':'focused-detail-back','tab_presses':index,'outline_pixels':outline,'ocr':found})
            mouse.tap(36)
            state('back-device', lambda s: s['system_menu_open'] and s['system_page'] == 'Device')
            mouse.tap(116); state('keyboard-selected-first', lambda s: s['top_menu']['selected'] == {'Page':'Wifi'})
            mouse.tap(36); state('keyboard-wifi', lambda s: s['system_page'] == 'Wifi')
            capture('wifi-detail'); mouse.tap(9); state('wifi-back', lambda s: s['system_page'] == 'Device')
            mouse.tap(9); state('root-dismissed', lambda s: not s['system_menu_open'] and s['view'] == 'Home')
            assert config.read_text() == original
            app, report, identifier = launch_native('refusing-client', True)
            enter_count = sum(e['event'] == 'keyboard_enter' for e in app_events(report))
            key_events = lambda: [e for e in app_events(report) if e['event'] == 'keyboard_key_balance']
            prior_keys = len(key_events())
            mouse.key(50, True)
            await_value(lambda: len(key_events()), lambda count: count == prior_keys+1)
            assert key_events()[-1]['pressed'] and key_events()[-1]['held'] == 1
            click(40, 16); state('native-menu', lambda s: s['system_menu_open'] and s['top_menu']['owner'] == {'App':identifier})
            await_value(lambda: any(e['event'] == 'keyboard_leave' for e in app_events(report)), bool)
            mouse.key(50, False)
            # Both an egui key and a text-only key stay owned by the retiring
            # shell. Release them after the native client regains focus.
            before = len(key_events())
            mouse.key(23, True); mouse.key(38, True)
            click(width-16, height*.6)
            state('held-key-menu-retired', lambda s: not s['system_menu_open'] and s['view'] == {'App':identifier})
            await_value(lambda: sum(e['event'] == 'keyboard_enter' for e in app_events(report)) > enter_count, bool)
            mouse.key(23, False); mouse.key(38, False); time.sleep(.15)
            assert len(key_events()) == before, 'A shell-owned physical release reached the client'
            mouse.tap(23)
            await_value(lambda: len(key_events()), lambda count: count == before+2)
            tail = key_events()[-2:]
            assert [e['pressed'] for e in tail] == [True,False] and tail[-1]['held'] == 0
            assert not any(e['unmatched_release'] for e in key_events())
            records['actions'].append({'label':'held-shell-key-retirement','suppressed_releases':2,'native_key_released_inside_menu':True,'fresh_native_pair':tail})
            # A native key still held when focus returns must stay retired too.
            before = len(key_events())
            enter_count = sum(e['event'] == 'keyboard_enter' for e in app_events(report))
            mouse.key(50, True)
            await_value(lambda: len(key_events()), lambda count: count == before+1)
            assert key_events()[-1]['pressed'] and key_events()[-1]['held'] == 1
            click(40,16); state('held-native-key-menu', lambda s: s['system_menu_open'])
            click(width-16,height*.6); state('held-native-key-retired', lambda s: not s['system_menu_open'])
            await_value(lambda: sum(e['event'] == 'keyboard_enter' for e in app_events(report)) > enter_count, bool)
            mouse.key(50,False); time.sleep(.15)
            assert len(key_events()) == before+1
            mouse.tap(23)
            await_value(lambda: len(key_events()), lambda count: count == before+3)
            pair = key_events()[-2:]
            assert [e['pressed'] for e in pair] == [True,False] and pair[-1]['held'] == 0
            assert not any(e['unmatched_release'] for e in key_events())
            records['actions'].append({'label':'held-native-key-through-menu-retirement','suppressed_releases':1,'fresh_native_pair':pair})
            click(40,16); state('native-menu-reopened', lambda s: s['system_menu_open'])
            item('Close', 'cooperative-close')
            state('close-refused', lambda s: not s['system_menu_open'] and s['cards'][0]['closing'])
            await_value(lambda: any(e['event'] == 'close_requested' and e['refused'] for e in app_events(report)), bool)
            assert app.poll() is None
            await_value(lambda: sum(e['event'] == 'keyboard_enter' for e in app_events(report)) > enter_count, bool)
            capture('refused-close-client-alive')
            click(40, 16); state('menu-after-refusal', lambda s: s['system_menu_open'])
            item('cards', 'show-cards'); state('overview', lambda s: s['view'] == 'Overview' and s['presentation']['overview'] == 1)
            click(width/2, height*.45); state('returned-refusing-card', lambda s: s['view'] == {'App':identifier} and not s['cards'][0]['closing'])
            click(40, 16); state('before-death', lambda s: s['system_menu_open'])
            app.terminate(); app.wait(timeout=5)
            state('client-death-dismissed', lambda s: not s['cards'] and s['view'] == 'Home' and not s['system_menu_open'])
            app, report, identifier = launch_native('accepting-client')
            click(40, 16); state('accepting-menu', lambda s: s['system_menu_open'])
            item('Close', 'accepted-close')
            state('accepted-client-gone', lambda s: not s['cards'] and s['view'] == 'Home')
            assert app.wait(timeout=5) == 0
            assert any(e['event'] == 'close_requested' and not e['refused'] for e in app_events(report))
            assert config.read_text() == original
            records['passed'] = True
            print('PASS: rendered app/device menus, hierarchy, keyboard selection, cooperative/refused close, focus and client death')
        finally:
            if mouse is not None: mouse.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try: child.wait(timeout=5)
                    except subprocess.TimeoutExpired: child.kill(); child.wait()
            for stream in logs: stream.close()
            (out/'validation.json').write_text(json.dumps(records, indent=2)+'\n')


if __name__ == '__main__': main()
