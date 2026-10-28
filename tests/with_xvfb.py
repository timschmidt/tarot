#!/usr/bin/env python3
"""Run a replay on a private X11 server, isolated from host desktop focus."""
import argparse
import os
from pathlib import Path
import select
import subprocess


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--xvfb',default='Xvfb')
    p.add_argument('--log',type=Path,required=True)
    p.add_argument('command',nargs=argparse.REMAINDER)
    args=p.parse_args();command=args.command
    if command and command[0]=='--':command=command[1:]
    assert command
    args.log.parent.mkdir(parents=True,exist_ok=True)
    read,write=os.pipe();server=None
    try:
        with args.log.open('x') as log:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1200x1200x24'],pass_fds=(write,),stdout=log,stderr=subprocess.STDOUT)
            os.close(write);write=None
            assert select.select([read],[],[],8)[0],'Private X server startup timed out'
            number=os.read(read,32).decode().strip()
            assert number.isdigit() and server.poll() is None
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11')
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            result=subprocess.run(command,env=env)
            raise SystemExit(result.returncode)
    finally:
        os.close(read)
        if write is not None:os.close(write)
        if server is not None and server.poll() is None:
            server.terminate()
            try:server.wait(timeout=5)
            except subprocess.TimeoutExpired:server.kill();server.wait()


if __name__=='__main__':main()
