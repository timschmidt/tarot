#!/usr/bin/env python3
"""Back from the gesture strip: KEY_BACK only for a documented app integration.

A right-to-left swipe within the strip is Back. A probe posing as a Waydroid
task (app id "waydroid.*") must receive KEY_BACK (158) pressed and released;
an ordinary app must receive no key at all. Owns its private X server; never
injects host desktop input. Synthetic keys only.
"""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from hardware_replay import PrivateInput
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from card_matrix import physical
from with_xvfb import read_display_number


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    root=Path.cwd();runtime=tempfile.TemporaryDirectory(prefix='tarot-back-');work=Path(runtime.name)
    control=work/'control.sock'
    children=[];logs=[];mouse=None;record={'passed':False}
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=12);record[label]=value;return value
    def swipe_back():
        s=request(control,'status')
        mouse.move(*physical(s,300,702));time.sleep(.05);mouse.button(True)
        for x in range(280,159,-20):
            mouse.move(*physical(s,x,702));time.sleep(.03)
        mouse.button(False);time.sleep(.4)
    def keys(report):
        return [e for e in events(report) if e['event']=='keyboard_key']
    def probe(name,app_id):
        report=out/(name+'.jsonl')
        p=subprocess.Popen([str(root/'target/debug/examples/input-probe'),'--report',str(report),'--no-child',
                            '--record-key-codes','--app-id',app_id,'--exit-after-seconds','80'],
                           env=native,stdout=log(name+'.log'),stderr=subprocess.STDOUT);children.append(p)
        await_value(lambda:events(report),lambda es:any(e['event']=='keyboard_enter' for e in es),seconds=10)
        return report
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
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--control-socket',str(control),'--exit-after-ms','90000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool,seconds=8);assert session.poll() is None
        initial=state('initial');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native=dict(env,WAYLAND_DISPLAY=initial['wayland_display']);native.pop('DISPLAY',None)
        # An ordinary app keeps its own Back: the strip swipe sends it nothing.
        ordinary=probe('ordinary','org.tarot.InputProbe')
        state('ordinary-shown',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict))
        swipe_back()
        time.sleep(.5);assert keys(ordinary)==[],keys(ordinary)
        record['ordinary_keys']=0
        # A Waydroid task receives KEY_BACK pressed and released.
        android=probe('android','waydroid.org.example.app')
        shown=state('android-shown',lambda s:len(s['cards'])==2 and isinstance(s['view'],dict)
                    and any(c['app_id']=='waydroid.org.example.app' and c['id']==s['view']['App'] for c in s['cards']))
        swipe_back()
        got=await_value(lambda:keys(android),lambda ks:len(ks)>=2,seconds=6)
        assert [(k['key'],k['pressed']) for k in got[:2]]==[(158,True),(158,False)],got
        assert keys(ordinary)==[]
        assert request(control,'status')['view']==shown['view'],'Back must not change the shell view'
        record['android_keys']=got
        record['passed']=True
        print('PASS: strip right-to-left swipe sends KEY_BACK to a Waydroid task and nothing to an ordinary app',out)
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
