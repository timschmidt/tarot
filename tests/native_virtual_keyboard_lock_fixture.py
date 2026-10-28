#!/usr/bin/env python3
"""Run the installed compositor's protocol guard nested in the real phone login.

This is nested Wayland/EGL lock evidence, separate from protected physical KMS/PAM.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--audit',type=Path,required=True)
    args=parser.parse_args();out=args.output.resolve()
    assert os.geteuid()==10000 and out.is_dir() and out.stat().st_uid==10000
    audit=json.loads(args.audit.read_text())
    compositor=Path('/usr/bin/tarot');text=Path('/home/user/tarot-dev/bin/text-probe-r25');lock=Path('/home/user/tarot-dev/bin/lock-probe-r25')
    for key,path in [('compositor',compositor),('text',text),('lock',lock)]:
        assert hashlib.sha256(path.read_bytes()).hexdigest()==audit[key]
    display=Path(os.environ['WAYLAND_DISPLAY'])
    if not display.is_absolute():display=Path(os.environ['XDG_RUNTIME_DIR'])/display
    start={'uid':os.geteuid(),'pid':os.getpid(),'cgroup':Path('/proc/self/cgroup').read_text(),
           'audit':audit,'scope':'Installed binary nested Wayland/EGL protocol guard; physical KMS/PAM verified by separate outer session replay.'}
    (out/'started.json').write_text(json.dumps(start,indent=2)+'\n')
    with (out/'fixture.log').open('x') as log:
        result=subprocess.run(['python3','/home/user/tarot-dev/virtual_keyboard_lock_replay.py',
            '--output',str(out/'protocol'),'--compositor',str(compositor),'--text-probe',str(text),
            '--lock-probe',str(lock),'--parent-wayland',str(display)],stdout=log,stderr=subprocess.STDOUT,timeout=100)
    with (out/'crash-fixture.log').open('x') as log:
        crash=subprocess.run(['python3','/home/user/tarot-dev/virtual_keyboard_crash_replay.py',
            '--output',str(out/'crash'),'--compositor',str(compositor),'--text-probe',str(text),
            '--parent-wayland',str(display)],stdout=log,stderr=subprocess.STDOUT,timeout=45)
    crash_passed=crash.returncode==0 and json.loads((out/'crash/validation.json').read_text())['passed']
    protocol=out/'protocol/validation.json'
    passed=crash_passed and result.returncode==0 and protocol.exists() and json.loads(protocol.read_text())['passed']
    (out/'result.json').write_text(json.dumps(dict(start,passed=passed,exit=result.returncode,held_key_crash_passed=crash_passed,crash_exit=crash.returncode),indent=2)+'\n')
    raise SystemExit(0 if passed else 1)


if __name__=='__main__':main()
