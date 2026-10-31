#!/usr/bin/env python3
"""Keyboard button over a client without text-input: overlay, key events, no resize.

Waydroid binds no text-input and recreates any window it is asked to resize,
so the keyboard opened by hand for such a client must overlay it. A private
input-probe without text-input stands in for it. Owns its private X server;
never injects host desktop input. Synthetic keys only.
"""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from hardware_replay import PrivateInput
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from card_matrix import physical
from with_xvfb import read_display_number


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    root=Path.cwd();runtime=tempfile.TemporaryDirectory(prefix='tarot-key-events-');work=Path(runtime.name)
    control=work/'control.sock';report=out/'probe-events.jsonl'
    children=[];logs=[];mouse=None;record={'passed':False}
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=12);record[label]=value;return value
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')))
        state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
    def click(x,y):
        s=request(control,'status');mouse.move(*physical(s,x,y));time.sleep(.05)
        mouse.button(True);time.sleep(.05);mouse.button(False);time.sleep(.12)
    def acked(s):
        return [size for g in s['native_geometry'] for size in g['acked_sizes'] if size]
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None
            number=read_display_number(read);assert server.poll() is None
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work),
            XDG_CONFIG_HOME=str(work/'config'),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_SOCKET',None);env.pop('WAYLAND_DISPLAY',None)
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--control-socket',str(control),'--input-method',str(root/'target/debug/tarot-keyboard'),'--exit-after-ms','90000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool,seconds=8);assert session.poll() is None
        initial=state('initial');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native=dict(env,WAYLAND_DISPLAY=initial['wayland_display']);native.pop('DISPLAY',None)
        probe=subprocess.Popen([str(root/'target/debug/examples/input-probe'),'--report',str(report),'--audit-key-balance','--no-child','--exit-after-seconds','80'],env=native,stdout=log('probe.log'),stderr=subprocess.STDOUT);children.append(probe)
        ready=state('probe-ready',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and not s['keyboard_visible'] and acked(s))
        full=max(h for _,h in acked(ready));record['full_height']=full
        await_value(lambda:events(report),lambda es:any(e['event']=='keyboard_enter' for e in es),seconds=6)
        # The probe has no text-input, so only the Keyboard button opens the
        # keyboard, as an overlay that reserves no space.
        click(329,702)
        shown=state('keyboard-overlay',lambda s:s['keyboard_visible'] and s['keyboard_overlay'] and s['keyboard_owner']==s['view']['App'])
        rect=shown['keyboard_rect'];assert rect['height']>0 and abs(rect['y']+rect['height']-(720-36))<1,rect
        assert shown['layout']['keyboard_height']==0 if 'layout' in shown else True
        time.sleep(.6);capture('keyboard-overlay')
        after=state('no-resize');assert min(h for _,h in acked(after))==full,acked(after)
        assert len(after['cards'])==1 and after['cards'][0]['id']==ready['cards'][0]['id']
        # Taps become key events delivered to the focused probe.
        start=len(events(report))
        Keyboard(click).type_ascii('ab')
        keys=await_value(lambda:[e for e in events(report)[start:] if e['event']=='keyboard_key_balance'],
                         lambda es:sum(e['pressed'] for e in es)>=2 and sum(not e['pressed'] for e in es)>=2,seconds=6)
        assert not any(e['unmatched_release'] for e in keys),keys
        record['key_events']=len(keys)
        still=state('still-overlay',lambda s:s['keyboard_visible'] and s['keyboard_overlay'])
        assert min(h for _,h in acked(still))==full
        # Hide closes it and clears the overlay.
        click(329,663)
        state('hidden',lambda s:not s['keyboard_visible'] and not s['keyboard_overlay'])
        record['passed']=True
        print('PASS: hand-opened keyboard overlays a client without text-input, sends key events and never resizes it',out)
    finally:
        if mouse is not None:mouse.close()
        for p in reversed(children):
            if p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for f in logs:f.close()
        (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n');runtime.cleanup()


if __name__=='__main__':main()
