#!/usr/bin/env python3
"""Exercise the visible Phrog picker with reviewed frames and real PAM.

Root-only spare-phone QA. The operator reviews each pre-credential image and
enters its coordinates/confirmation at the terminal. No greetd IPC, injected PAM
result, greeter termination or session-configuration change is performed here.
The bounded credential path supports only the spare device's numeric password.
"""
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import pwd
import struct
import subprocess
import time

from notification_replay import await_value
from phone_gesture_replay import Touchscreen
from phone_lock_replay import Keyboard
from phone_stock_login import processes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session', choices=['tarot', 'phosh'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    before = processes()
    greeter = next(p for p in before if p['name'] == 'phrog' and p['uid'] == 109)
    assert not any(p['uid'] == 10000 and p['name'] in ['phoc', 'tarot'] for p in before)
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chown(args.output, 10000, pwd.getpwuid(10000).pw_gid)
    environment = dict(x.split(b'=', 1) for x in Path('/proc', str(greeter['pid']), 'environ').read_bytes().split(b'\0') if b'=' in x)
    runtime = environment[b'XDG_RUNTIME_DIR'].decode()
    display = environment[b'WAYLAND_DISPLAY'].decode()
    assert runtime == '/run/user/109' and '/' not in display
    assert (Path(runtime)/display).stat().st_uid == 109
    env = dict(PATH='/usr/bin:/bin', HOME=pwd.getpwuid(109).pw_dir, XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=display)
    record = {'before': before, 'selected_session': args.session, 'captures': {}}
    keyboard = screen = None
    password = None

    def guard():
        assert greeter in processes(), 'Stop input if the greeter identity changes'
        assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty7'

    def capture(name):
        guard()
        time.sleep(.7)
        target = args.output/(name+'.png')
        # Root opens a private review file first; the greeter-owned client only
        # writes its inherited descriptor and needs no access to this folder.
        with target.open('xb') as stream:
            os.chmod(target, 0o600)
            os.chown(target, 10000, pwd.getpwuid(10000).pw_gid)
            subprocess.run(['grim', '-'], env=env, user=109, stdout=stream,
                           group=pwd.getpwuid(109).pw_gid, check=True, timeout=8)
        content = target.read_bytes()
        assert content[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', content[16:24]) == (720, 1440)
        record['captures'][name] = hashlib.sha256(content).hexdigest()
        print('Review pre-credential frame:', target, flush=True)

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.2)

    try:
        guard()
        keyboard = Keyboard(guard)
        screen = Touchscreen(guard)
        keyboard.tap(42)
        keyboard.tap(1)
        keyboard.tap(1)
        tap(180, 645)
        capture('picker')
        point = [int(x) for x in input('Reviewed target row logical x,y: ').split(',')]
        assert len(point) == 2 and 0 <= point[0] < 360 and 0 <= point[1] < 720
        record['reviewed_picker_point'] = point
        tap(*point)
        capture('selected')
        assert input('Confirm selected session name (tarot/phosh): ').strip() == args.session
        tap(180, 325)
        capture('authentication-empty')
        assert input('Confirm the reviewed password page is for user: ').strip() == 'user'
        password = getpass.getpass('Spare-device numeric password for visible Phrog PAM: ')
        assert password and password.isascii() and password.isdecimal()
        guard()
        for digit in password:
            keyboard.tap(11 if digit == '0' else int(digit)+1)
        # Submit as one down/up write. A successful login can replace Phrog
        # before Keyboard.tap's delayed release; no credential input may then
        # be emitted into the new compositor.
        guard()
        os.write(keyboard.fd, b''.join(struct.pack('@llHHi', 0, 0, kind, code, value)
                                     for kind, code, value in [(1, 28, 1), (0, 0, 0), (1, 28, 0), (0, 0, 0)]))
        password = None
        expected = ['tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard'] if args.session == 'tarot' else ['phoc', 'phosh']
        after = await_value(processes, lambda ps: all(any(p['uid'] == 10000 and p['name'] == name for p in ps) for name in expected), seconds=35)
        assert greeter not in after
        assert not Path('/run/user/10000/tarot-session/control.sock').exists()
        record['after'] = after
        record['default_entry_sha256'] = hashlib.sha256(Path('/usr/share/wayland-sessions', args.session+'.desktop').read_bytes()).hexdigest()
        if args.session == 'tarot':
            main = next(p for p in after if p['uid'] == 10000 and p['name'] == 'tarot')
            record['compositor_argv'] = [a.decode() for a in Path('/proc', str(main['pid']), 'cmdline').read_bytes().split(b'\0') if a]
            assert '--control-socket' not in record['compositor_argv']
            record['binaries_sha256'] = {name: hashlib.sha256(Path('/usr/bin', name).read_bytes()).hexdigest()
                                          for name in expected}
        record['validation'] = {'passed': True, 'visible_picker_and_password_page_reviewed': True,
                                'actual_greeter_authenticated_launch': True,
                                'ordinary_user_and_default_entry': True,
                                'diagnostic_socket_absent': True, 'password_recorded': False}
        print('PASS: visible Phrog picker and real authentication launched default', args.session, flush=True)
    except BaseException as error:
        record['validation'] = {'passed': False, 'exception_type': type(error).__name__}
        raise
    finally:
        password = None
        if screen is not None:
            screen.close()
        if keyboard is not None:
            # Device destruction cancels any held contact after an interrupted
            # login. Never replay its releases into a replacement compositor.
            keyboard.held.clear()
            keyboard.close()
        (args.output/'status.json').write_text(json.dumps(record, indent=2)+'\n')
        for path in args.output.iterdir():
            os.chmod(path, 0o600)
            os.chown(path, 10000, pwd.getpwuid(10000).pw_gid)
        os.chown(args.output, 10000, pwd.getpwuid(10000).pw_gid)


if __name__ == '__main__':
    main()
