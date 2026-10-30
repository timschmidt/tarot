#!/usr/bin/env python3
"""Check real transient pixels/input on an owned Xvfb, never the host desktop.

The Rust Wayland fixture owns its buffers, surfaces and event assertions. This
driver supplies scoped XTEST clicks and keys over a private QA-only socket.
The managed lock probe checks protocol protection, not physical KMS or PAM.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import tempfile
import threading
import time

from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from lock_replay import append, events
from with_xvfb import read_display_number


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--xvfb', default='Xvfb')
    p.add_argument('--scale', type=int, choices=[1, 2], default=2)
    p.add_argument('--case', choices=['transient','viewport','notifications'],default='transient')
    p.add_argument('--tarot', type=Path, default=Path('target/debug/tarot'))
    p.add_argument('--fixture-bin', type=Path)
    p.add_argument('--keyboard', type=Path, default=Path('target/debug/tarot-keyboard'))
    args = p.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    record = {'passed': False, 'scale': args.scale, 'case':args.case,'input': []}
    tarot = args.tarot.resolve()
    record['compositor_sha256'] = hashlib.sha256(tarot.read_bytes()).hexdigest()
    record['fixture_source_sha256'] = hashlib.sha256((root/'tests/wayland_lifecycle.rs').read_bytes()).hexdigest()
    temporary = tempfile.TemporaryDirectory(prefix='tarot-transient-')
    work = Path(temporary.name)
    control, inputs = work/'control.sock', work/'input.sock'
    runtime = work/'runtime'
    runtime.mkdir(mode=0o700)
    processes, logs = [], []
    stop = threading.Event()
    worker = None
    listener = None

    def log(name):
        f = (out/name).open('w')
        logs.append(f)
        return f

    try:
        read, write = os.pipe()
        try:
            server = subprocess.Popen([args.xvfb, '-displayfd', str(write), '-nolisten', 'tcp', '-noreset', '-ac', '-screen', '0', '1600x1600x24'], pass_fds=(write,), stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
            processes.append(server)
            os.close(write)
            write = None
            number = read_display_number(read)
            assert number.isdigit() and server.poll() is None
        finally:
            os.close(read)
            if write is not None:
                os.close(write)
        display = ':'+number
        env = dict(os.environ, DISPLAY=display, WINIT_UNIX_BACKEND='x11', XDG_SESSION_TYPE='x11', XDG_RUNTIME_DIR=str(runtime), LIBGL_ALWAYS_SOFTWARE='1', __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json', TAROT_TEST_CONTROL_SOCKET=str(control), TAROT_TEST_INPUT_SOCKET=str(inputs), TAROT_TEST_CAPTURE_DIR=str(out))
        env.pop('WAYLAND_DISPLAY', None)
        env.pop('WAYLAND_SOCKET', None)
        if args.case == 'notifications':
            address = 'unix:path='+str(work/'bus')
            env['DBUS_SESSION_BUS_ADDRESS'] = address
            bus = subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address],stdout=log('bus.log'),stderr=subprocess.STDOUT)
            processes.append(bus)
            await_value(lambda: (work/'bus').exists() or bus.poll() is not None, bool)
            assert bus.poll() is None
        lock_report, lock_commands = out/'locker-events.jsonl', out/'locker-commands.jsonl'
        lock_commands.write_text('')
        command = [str(tarot), '--width', str(360*args.scale), '--height', str(720*args.scale), '--scale', str(args.scale), '--control-socket', str(control), '--lock-probe-unlocked-start', '--lock-client', str(root/'target/debug/examples/lock-probe'), '--verify-lock-blank', '--verify-lock-surface-rgba', '336699ff', '--exit-after-ms', '180000']
        if args.case == 'notifications':
            keyboard = args.keyboard.resolve()
            record['keyboard_sha256'] = hashlib.sha256(keyboard.read_bytes()).hexdigest()
            command.extend(['--notifications','--publish-session-environment','--config-file',str(work/'settings.toml'),'--input-method',str(keyboard)])
        for arg in ['--role', 'locker', '--report', str(lock_report), '--commands', str(lock_commands)]:
            command.append('--lock-client-arg='+arg)
        session = subprocess.Popen(command, env=env, stdout=log('compositor.log'), stderr=subprocess.STDOUT)
        processes.append(session)
        await_value(lambda: control.exists() or session.poll() is not None, bool)
        assert session.poll() is None
        record['loaded_compositor_sha256'] = hashlib.sha256(Path('/proc',str(session.pid),'exe').read_bytes()).hexdigest()
        assert record['loaded_compositor_sha256'] == record['compositor_sha256']
        listener = socket.socket(socket.AF_UNIX)
        listener.bind(str(inputs))
        listener.listen(1)
        listener.settimeout(.25)

        def serve():
            mouse = None
            try:
                mouse = PrivateInput(session.pid, server, display)
                await_value(mouse.find, bool)
                while not stop.is_set():
                    try:
                        conn, _ = listener.accept()
                    except socket.timeout:
                        continue
                    with conn:
                        try:
                            message = json.loads(conn.makefile('rb').readline(65536))
                            assert session.poll() is None and server.poll() is None
                            if message['command'] == 'lock':
                                request(control, 'lock')
                                owner = await_value(lambda: request(control, 'status'), lambda s: s['lock']['locker']['pid'] is not None)
                                pid = owner['lock']['locker']['pid']
                                await_value(lambda: events(lock_report), lambda es: any(e['pid'] == pid and e['event'] == 'ready' for e in es))
                                append(lock_commands, 'lock')
                                record['protected'] = await_value(lambda: request(control, 'status'), lambda s: s['lock']['phase'] == 'Locked' and s['lock']['surface_verified'] and s['lock']['last_presented_surface'] and s['lock']['last_presented_generation'] == s['lock']['generation'])
                                record['input'].append(message)
                                conn.sendall(b'{"ok":true}\n')
                                continue
                            if message['command'] == 'unlock':
                                append(lock_commands, 'unlock')
                                await_value(lambda: request(control, 'status'), lambda s: s['lock']['phase'] == 'Unlocked')
                                record['input'].append(message)
                                conn.sendall(b'{"ok":true}\n')
                                continue
                            s = request(control, 'status')
                            assert s['lock']['phase'] == 'Unlocked'
                            assert message['command'] == 'click' and message['output'] == s['output']
                            g = s['output']
                            w, h, scale, orientation = g['physical_width'], g['physical_height'], g['scale'], g['orientation']
                            lw, lh = (h/scale, w/scale) if orientation in ['left-up', 'right-up'] else (w/scale, h/scale)
                            x, y = message['x']/lw, message['y']/lh
                            x, y = {'normal': (x, y), 'left-up': (y, 1-x), 'bottom-up': (1-x, 1-y), 'right-up': (1-y, x)}[orientation]
                            assert 0 < x < 1 and 0 < y < 1
                            mouse.move(round(x*w), round(y*h))
                            time.sleep(.04)
                            mouse.button(True)
                            time.sleep(.035)
                            mouse.button(False)
                            time.sleep(.05)
                            if message['key']:
                                mouse.tap(38)  # synthetic fixture letter a
                            time.sleep(.04)
                            record['input'].append(message)
                            conn.sendall(b'{"ok":true}\n')
                        except BaseException as error:
                            conn.sendall((json.dumps({'ok': False, 'error': str(error)})+'\n').encode())
                            raise
            except BaseException as error:
                record['input_error'] = repr(error)
            finally:
                if mouse is not None:
                    mouse.close()

        worker = threading.Thread(target=serve, name='transient-private-input')
        worker.start()
        with (out/'test.log').open('w') as test_log:
            test_name = {'transient':'transient_native_input_and_pixels','viewport':'viewport_crop_scale_commit_and_subsurface_input','notifications':'notification_well_keeps_oversized_client_input_and_popup_alignment'}[args.case]
            if args.fixture_bin:
                fixture_bin = args.fixture_bin.resolve()
                record['fixture_binary_sha256'] = hashlib.sha256(fixture_bin.read_bytes()).hexdigest()
                test_command = [str(fixture_bin), test_name, '--ignored', '--nocapture']
            else:
                test_command = ['cargo', 'test', '--offline', '--locked', '--test', 'wayland_lifecycle', test_name, '--', '--ignored', '--nocapture']
            test = subprocess.run(test_command, env=env, stdout=test_log, stderr=subprocess.STDOUT, timeout=120)
        record['test_exit'] = test.returncode
        if args.case == 'notifications' and test.returncode == 0:
            helper = request(control, 'status')['input_method']
            assert helper['generation'] == 1 and not helper['error'] and not helper['restart_pending']
            record['keyboard_status'] = helper
            record['loaded_keyboard_sha256'] = hashlib.sha256(Path('/proc',str(helper['pid']),'exe').read_bytes()).hexdigest()
            assert record['loaded_keyboard_sha256'] == record['keyboard_sha256']
        request(control, 'exit')
        record['compositor_exit'] = session.wait(timeout=6)
        record['graphics_errors'] = [line for line in (out/'compositor.log').read_text().splitlines() if any(e in line for e in ['GL_INVALID', 'GL_OUT_OF_MEMORY', 'GL error'])]
        assert record['test_exit'] == record['compositor_exit'] == 0 and 'input_error' not in record and not record['graphics_errors'], record
        assert len(record['input']) == {'transient':40,'viewport':7,'notifications':40}[args.case], record
        record['passed'] = True
        print('PASS:',args.case,'native pixels and actual input, scale', args.scale, out)
    finally:
        stop.set()
        if worker is not None:
            worker.join(timeout=6)
            assert not worker.is_alive(), 'QA input worker did not stop'
        if listener is not None:
            listener.close()
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        for f in logs:
            f.close()
        temporary.cleanup()
        (out/'validation.json').write_text(json.dumps(record, indent=2)+'\n')


if __name__ == '__main__':
    main()
