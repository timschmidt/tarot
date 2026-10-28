#!/usr/bin/env python3
"""Hardware keys on an owned Xvfb display and private PulseAudio server.

Never injects power/media keys into the host display or controls host audio.
The managed lock probe is synthetic: this proves input/dispatch isolation,
not native KMS power changes, physical buttons or PAM authentication.
"""
import argparse
import ctypes as c
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time

from lock_replay import append, events, any_request
from notification_replay import await_value
from phone_gesture_replay import request
from x11_replay_input import X11Input


class PrivateInput(X11Input):
    def __init__(self, pid, server, display):
        assert server.poll() is None
        self.server = server
        super().__init__(pid, display=display)
        self.keys = set()
        self.x.XQueryKeymap.argtypes = [c.c_void_p, c.c_void_p]
        self.xtest.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]

    def find(self):
        # Xvfb deliberately has no desktop/WM and therefore no _NET_CLIENT_LIST.
        queue = [self.root]
        found = []
        for _ in range(4096):
            if not queue:
                break
            node = queue.pop()
            if self.property(node, '_NET_WM_PID') == [self.pid]:
                found.append(node)
            root, parent, count = c.c_ulong(), c.c_ulong(), c.c_uint()
            children = c.POINTER(c.c_ulong)()
            assert self.x.XQueryTree(self.display, node, c.byref(root), c.byref(parent), c.byref(children), c.byref(count))
            try:
                queue.extend(children[i] for i in range(count.value))
            finally:
                if children:
                    self.x.XFree(children)
        assert not queue and len(found) <= 1
        self.window = found[0] if found else None
        return self.window

    def key(self, code, pressed):
        assert self.server.poll() is None, 'Private X server exited'
        assert self.property(self.window, '_NET_WM_PID') == [self.pid]
        if pressed:
            assert code not in self.keys and self.focus()[0] == self.window
            bitmap = c.create_string_buffer(32)
            self.x.XQueryKeymap(self.display, bitmap)
            assert not bitmap.raw[code // 8] & (1 << (code % 8)), 'Key already held'
            self.keys.add(code)
        else:
            assert code in self.keys
            self.keys.remove(code)
        assert self.xtest.XTestFakeKeyEvent(self.display, code, int(pressed), 0)
        self.x.XSync(self.display, 0)

    def tap(self, code):
        self.key(code, True)
        self.key(code, False)

    def close(self):
        for code in list(self.keys):
            self.key(code, False)
        super().close()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--xvfb', default='Xvfb')
    p.add_argument('--pulse-daemon', default='pulseaudio')
    p.add_argument('--pulse-modules', type=Path)
    p.add_argument('--pulse-libraries', type=Path)
    args = p.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    temporary = tempfile.TemporaryDirectory(prefix='tarot-hardware-')
    work = Path(temporary.name)
    control, pulse_socket = work/'control.sock', work/'pulse.sock'
    server = 'unix:' + str(pulse_socket)
    commands, locker_report, app_report = out/'locker.commands', out/'locker.jsonl', out/'app.jsonl'
    commands.write_text('')
    app_commands = out/'app.commands'
    app_commands.write_text('')
    records = {'validation': {'passed': False}}
    logs, children = [], []
    input_device = daemon = compositor = None

    def log(name):
        stream = (out/name).open('w')
        logs.append(stream)
        return stream

    def state(label, condition=lambda s: True, seconds=8):
        value = await_value(lambda: request(control, 'status'), condition, seconds=seconds)
        records[label] = value
        return value

    def pactl(*args):
        return subprocess.check_output(['pactl', '--server', server, *map(str, args)], text=True)

    def output(name):
        return next(o for o in json.loads(pactl('-f', 'json', 'list', 'sinks')) if o['name'] == name)

    def percent(name):
        return round(max(v['value'] for v in output(name)['volume'].values())*100/65536)

    def observed(label, name, value):
        result = state(label, lambda s: s.get('audio', {}).get('pending') is None
                       and any(o['default'] and o['percent'] == value for o in s.get('audio', {}).get('outputs', [])))
        assert percent(name) == value
        return result

    def unlock(label):
        append(commands, 'unlock')
        return state(label, lambda s: s['lock']['phase'] == 'Unlocked')

    try:
        # A separately owned X server has no host desktop's global key grabs.
        read_fd, write_fd = os.pipe()
        try:
            xvfb = subprocess.Popen([args.xvfb, '-displayfd', str(write_fd), '-nolisten', 'tcp',
                                     '-noreset', '-ac', '-screen', '0', '1000x1000x24'],
                                    pass_fds=(write_fd,), stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
            children.append(xvfb)
            os.close(write_fd)
            write_fd = None
            assert select.select([read_fd], [], [], 8)[0], 'Private Xvfb startup timed out'
            number = os.read(read_fd, 32).decode().strip()
            assert number.isdigit() and xvfb.poll() is None
        finally:
            os.close(read_fd)
            if write_fd is not None:
                os.close(write_fd)
        env = dict(os.environ, DISPLAY=':'+number, WINIT_UNIX_BACKEND='x11', XDG_SESSION_TYPE='x11',
                   LIBGL_ALWAYS_SOFTWARE='1', __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_DISPLAY', None)
        env.pop('WAYLAND_SOCKET', None)
        pulse_env = dict(env, PULSE_RUNTIME_PATH=str(work/'runtime'), PULSE_STATE_PATH=str(work/'state'), PULSE_CONFIG_PATH=str(work))
        for folder in ['runtime', 'state']:
            (work/folder).mkdir(mode=0o700)
        if args.pulse_libraries:
            pulse_env['LD_LIBRARY_PATH'] = str(args.pulse_libraries.resolve())
        if args.pulse_modules:
            pulse_env['LD_LIBRARY_PATH'] = str(args.pulse_modules.resolve()) + ':' + pulse_env.get('LD_LIBRARY_PATH', '')
        config = work/'default.pa'
        config.write_text('load-module module-native-protocol-unix socket='+str(pulse_socket)+' auth-anonymous=yes\n')
        command = [args.pulse_daemon, '-n', '--daemonize=no', '--use-pid-file=no', '--exit-idle-time=-1',
                   '--disable-shm=yes', '--log-target=stderr', '--file='+str(config)]
        if args.pulse_modules:
            command += ['--dl-search-path='+str(args.pulse_modules.resolve())]
        daemon = subprocess.Popen(command, env=pulse_env, stdout=log('pulse.log'), stderr=subprocess.STDOUT)
        children.append(daemon)
        await_value(lambda: pulse_socket.exists() or daemon.poll() is not None, bool)
        assert daemon.poll() is None
        pactl('load-module', 'module-null-sink', 'sink_name=tarot_first')
        pactl('load-module', 'module-null-sink', 'sink_name=tarot_second')
        pactl('set-default-sink', 'tarot_first')
        pactl('set-sink-volume', 'tarot_first', '50%')
        pactl('set-sink-volume', 'tarot_second', '25%')
        command = [str(root/'target/debug/tarot'), '--hardware-keys', '--idle-lock-ms', '600000',
                   '--system-services', '--platform-bus', 'unix:path='+str(work/'absent-bus'),
                   '--audio-server', server, '--control-socket', str(control), '--verify-lock-blank',
                   '--lock-client', str(root/'target/debug/examples/lock-probe'), '--exit-after-ms', '90000']
        for arg in ['--role', 'locker', '--report', str(locker_report), '--commands', str(commands)]:
            command.append('--lock-client-arg='+arg)
        compositor = subprocess.Popen(command, env=env, stdout=log('compositor.log'), stderr=subprocess.STDOUT)
        children.append(compositor)
        await_value(lambda: control.exists() or compositor.poll() is not None, bool)
        assert compositor.poll() is None
        await_value(lambda: events(locker_report), lambda es: any(e['event'] == 'ready' for e in es))
        append(commands, 'lock')
        protected = state('startup', lambda s: s['lock']['phase'] == 'Locked' and s['lock']['last_presented_surface'] is True)
        assert set(protected) == {'ok', 'pid', 'lock'} and protected['lock']['unprotected_presentations'] == 0
        input_device = PrivateInput(compositor.pid, xvfb, env['DISPLAY'])
        await_value(input_device.find, bool)
        input_device.move(180, 350)
        time.sleep(.2)
        before = len(events(locker_report))
        input_device.tap(123)
        await_value(lambda: percent('tarot_first'), lambda n: n == 55)
        assert set(state('volume-while-locked')) == {'ok', 'pid', 'lock'}
        assert not any(e['event'] == 'key' for e in events(locker_report)[before:])
        initial = unlock('startup-unlocked')
        state('audio-ready', lambda s: s['audio']['available'] and s['hardware_keys'])
        app_env = dict(env, WAYLAND_DISPLAY=initial['wayland_display'])
        app = subprocess.Popen([str(root/'target/debug/examples/lock-probe'), '--role', 'app',
                                '--report', str(app_report), '--commands', str(app_commands)],
                               env=app_env, stdout=log('app.log'), stderr=subprocess.STDOUT)
        children.append(app)
        app_state = state('app-mapped', lambda s: len(s['cards']) == 1
                          and s['presentation']['windows'][0]['opacity'] > .99)
        input_device.move(180, 350)
        await_value(lambda: events(app_report), lambda es: any(e['event'] == 'pointer-enter' for e in es))
        input_device.button(True)
        await_value(lambda: events(app_report), lambda es: any(e['event'] == 'button' and e['fields']['state'] == 'Value(Pressed)' for e in es))
        input_device.button(False)
        await_value(lambda: events(app_report), lambda es: sum(e['event'] == 'button' for e in es) >= 2)
        input_device.tap(38)  # X11 key 38 is evdev A (30), with the XKB offset.
        await_value(lambda: events(app_report), lambda es: any(e['event'] == 'key' and e['fields']['code'] == 30 for e in es))
        before = len(events(app_report))
        input_device.tap(123)
        input_device.tap(123)
        feedback = observed('two-fast-presses', 'tarot_first', 65)
        assert feedback['volume_feedback']['percent'] == 65
        # Feedback neither steals keyboard focus nor forwards its overlay click.
        input_device.tap(38)
        await_value(lambda: events(app_report)[before:], lambda es: any(e['event'] == 'key' and e['fields']['code'] == 30 for e in es))
        clicks = sum(e['event'] == 'button' for e in events(app_report))
        input_device.move(180, 60)
        input_device.button(True)
        input_device.button(False)
        time.sleep(.15)
        assert sum(e['event'] == 'button' for e in events(app_report)) == clicks
        # Delayed RPC shows only observed values and coalesces queued steps.
        os.kill(daemon.pid, signal.SIGSTOP)
        input_device.tap(123)
        pending = state('pending-readback', lambda s: s['audio']['pending'] is not None)
        assert pending['volume_feedback']['percent'] == 65
        input_device.tap(123)
        os.kill(daemon.pid, signal.SIGCONT)
        observed('delayed-confirmed', 'tarot_first', 75)
        input_device.key(122, True)
        await_value(lambda: percent('tarot_first'), lambda n: n <= 60)
        input_device.key(122, False)
        time.sleep(.2)
        released = state('released', lambda s: s['audio']['pending'] is None)['audio']
        time.sleep(.6)
        assert state('no-repeat-after-release')['audio']['outputs'] == released['outputs']
        # A held key belongs to one output identity; switching default cancels it.
        input_device.key(123, True)
        await_value(lambda: percent('tarot_first'), lambda n: n > next(o['percent'] for o in released['outputs'] if o['default']))
        pactl('set-default-sink', 'tarot_second')
        observed('default-changed-during-hold', 'tarot_second', 25)
        time.sleep(.8)
        assert percent('tarot_second') == 25
        input_device.key(123, False)
        input_device.tap(123)
        observed('fresh-key-new-output', 'tarot_second', 30)
        input_device.tap(121)
        state('muted', lambda s: s['audio']['pending'] is None and any(o['default'] and o['muted'] for o in s['audio']['outputs']))
        assert output('tarot_second')['mute']
        input_device.tap(122)
        observed('volume-preserves-mute', 'tarot_second', 25)
        assert output('tarot_second')['mute']
        assert not any(e['event'] == 'key' and e['fields']['code'] in [113, 114, 115, 116] for e in events(app_report))
        # Nested power locks; only the direct DRM backend can power a panel off.
        input_device.key(124, True)
        armed = state('power-reserves-protection', lambda s: s['lock']['phase'] == 'Awaiting')
        owner = armed['lock']['locker']['pid']
        await_value(lambda: events(locker_report), lambda es: any(e['event'] == 'ready' and e['pid'] == owner for e in es))
        append(commands, 'lock')
        locked = state('power-lock', lambda s: s['lock']['phase'] == 'Locked' and s['lock']['last_presented_surface'] is True)
        generation = locked['lock']['generation']
        assert locked['lock']['idle']['policy']['phase'] == 'Awake'
        time.sleep(.8)
        assert state('held-power-once')['lock']['generation'] == generation
        input_device.key(124, False)
        restored = unlock('power-unlocked')
        assert restored['cards'] == app_state['cards'] and restored['view'] == app_state['view']
        state('feedback-expires', lambda s: s['volume_feedback'] is None)
        request(control, 'exit')
        assert compositor.wait(timeout=5) == 0
        records['validation'] = dict(passed=True, isolated_display=True, isolated_audio=True,
                                    protected_volume_input_private=True, fast_steps_and_delayed_readback=True,
                                    native_client_keyboard_focus_preserved=True, release_stops_repeat=True,
                                    default_change_cancels_hold=True, fresh_key_targets_new_default=True,
                                    mute_preserved=True, nested_power_locks_once=True,
                                    native_kms_tested=False, physical_buttons_tested=False, pam_tested=False)
        print('PASS: isolated power/volume keys, protected/client input isolation, observed feedback, queued/repeated volume and default-output changes')
    finally:
        if input_device is not None:
            input_device.close()
        if daemon is not None and daemon.poll() is None:
            os.kill(daemon.pid, signal.SIGCONT)
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        for stream in logs:
            stream.close()
        (out/'status.json').write_text(json.dumps(records, indent=2)+'\n')
        temporary.cleanup()


if __name__ == '__main__':
    main()
