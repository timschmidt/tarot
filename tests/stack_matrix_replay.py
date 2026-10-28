#!/usr/bin/env python3
"""Three/five real native cards: grouping, fan pixels and actual member browsing."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time
from PIL import Image
from card_matrix import physical,run
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--xvfb',default='Xvfb');args=p.parse_args()
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=False,mode=0o700);root=Path.cwd()
    temporary=tempfile.TemporaryDirectory(prefix='tarot-stack-matrix-');work=Path(temporary.name);control=work/'control.sock'
    children=[];apps=[];logs=[];mouse=None;record={'passed':False,'compositor_sha256':hashlib.sha256((root/'target/debug/tarot').read_bytes()).hexdigest()}
    for name in ['stack_matrix_replay.py','card_matrix.py']:(out/name).write_bytes((root/'tests'/name).read_bytes())
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        s=await_value(lambda:request(control,'status'),condition,seconds=12);record[label]=s;return s
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')));state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
        with Image.open(out/(label+'.png')) as im:im.load()
    def move(s,x,y):mouse.move(*physical(s,x,y));time.sleep(.06)
    def down(s,x,y):move(s,x,y);mouse.button(True);time.sleep(.04)
    def up():mouse.button(False);time.sleep(.04)
    def launch(index,color):
        app=subprocess.Popen([str(root/'target/debug/examples/input-probe'),'--fit-output','--opaque','--solid-color',color,
            '--corner-child' if index==4 else '--no-child','--report',str(out/('client-'+str(index)+'.jsonl')),'--exit-after-seconds','180'],env=native,stdout=log('client-'+str(index)+'.log'),stderr=subprocess.STDOUT)
        children.append(app);apps.append(app)
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(server);os.close(write);write=None
            assert select.select([read],[],[],8)[0];number=os.read(read,32).decode().strip();assert number.isdigit()
        finally:
            os.close(read)
            if write is not None:os.close(write)
        runtime=work/'runtime';runtime.mkdir(mode=0o700)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(runtime),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--width','720','--height','1440','--scale','2','--control-socket',str(control),'--exit-after-ms','180000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        before=state('ready');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native=dict(env,WAYLAND_DISPLAY=before['wayland_display']);native.pop('DISPLAY',None)
        record['matrix']=run(control,state,capture,down,move,up,launch,before,out)
        assert all(app.wait(timeout=6)==0 for app in apps)
        mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=6)==0
        record.update(passed=True,all_fixture_processes_exited=True,phone_tested=False)
        print('PASS: three/five native fan pixels in every orientation, actual grouping/browsing, every member activates, independent cooperative close')
    finally:
        if mouse is not None:mouse.close()
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for f in logs:f.close()
        (out/'status.json').write_text(json.dumps(record,indent=2)+'\n');temporary.cleanup()


if __name__=='__main__':main()
