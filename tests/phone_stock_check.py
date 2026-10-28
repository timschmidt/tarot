#!/usr/bin/env python3
"""Check the authenticated stock Phosh output after development-session cleanup."""
import argparse
import hashlib
import json
import os
import pwd
import struct
import subprocess
from pathlib import Path
from phone_lock_replay import Keyboard
from phone_stock_login import processes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    account = pwd.getpwnam('user')
    before = processes()
    shell = next(p for p in before if p['uid'] == account.pw_uid and p['name'] == 'phosh')
    assert any(p['uid'] == account.pw_uid and p['name'] == 'phoc' for p in before)
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chown(args.output, account.pw_uid, account.pw_gid)
    environment = dict(item.split(b'=', 1) for item in (Path('/proc')/str(shell['pid'])/'environ').read_bytes().split(b'\0') if b'=' in item)
    runtime = environment[b'XDG_RUNTIME_DIR'].decode()
    display = environment[b'WAYLAND_DISPLAY'].decode()
    assert runtime == f'/run/user/{account.pw_uid}' and '/' not in display and display.startswith('wayland-')
    assert (Path(runtime)/display).stat().st_uid == account.pw_uid

    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty7'
        assert not Path('/run/user/10000/tarot-drm.sock').exists()
        assert shell in processes(), 'Authenticated stock shell identity changed'

    guard()
    keyboard = Keyboard(guard)
    try:
        keyboard.tap(42)  # Fresh Shift press/release wakes an idle stock output.
    finally:
        keyboard.close()
    guard()
    image = args.output/'stock-restored.png'
    result = subprocess.run(['grim', str(image)], env=dict(PATH='/usr/bin:/bin', HOME=account.pw_dir, XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=display), user=account.pw_uid, group=account.pw_gid, extra_groups=os.getgrouplist('user', account.pw_gid), capture_output=True, timeout=10, umask=0o077)
    assert result.returncode == 0, 'Stock framebuffer capture failed'
    png = image.read_bytes()
    assert png[:8] == b'\x89PNG\r\n\x1a\n' and struct.unpack('>II', png[16:24]) == (720, 1440)
    guard()
    record = {'before': before, 'after': processes(), 'capture_sha256': hashlib.sha256(png).hexdigest(), 'validation': {'passed': True, 'authenticated_phosh_uid_10000': True, 'same_shell_identity_retained': True, 'fresh_uinput_key_then_stock_capture': True, 'capture_dimensions': [720, 1440], 'manual_sensor_tested': False, 'image_visual_review_complete': False, 'password_recorded': False}}
    (args.output/'status.json').write_text(json.dumps(record, indent=2)+'\n')
    print('PASS: authenticated Phosh retained, fresh input then 720x1440 stock capture; visually inspect the image')


if __name__ == '__main__':
    main()
