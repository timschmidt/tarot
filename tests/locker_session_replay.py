#!/usr/bin/env python3
"""Real egui locker startup, protected rendering and crash/replacement (no auth)."""
import argparse,json,os,signal,subprocess,tempfile
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from lock_replay import any_request

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);captures=out/'client-ui';captures.mkdir(mode=0o700)
    runtime=tempfile.TemporaryDirectory(prefix='tarot-locker-');control=Path(runtime.name)/'control.sock';root=Path.cwd();records={};process=None
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition);records[label]=value;return value
    try:
        with (out/'compositor.log').open('w') as log:
            process=subprocess.Popen([str(root/'target/debug/tarot'),'--lock-client',str(root/'target/debug/tarot-lock'),'--lock-client-arg=--capture-dir','--lock-client-arg='+str(captures),'--verify-lock-blank','--control-socket',str(control),'--exit-after-ms','60000'],env=env,stdout=log,stderr=subprocess.STDOUT)
            await_value(lambda:control.exists() or process.poll() is not None,bool);assert process.poll() is None
            first=state('protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
            assert set(first)=={'ok','pid','lock'} and first['lock']['blank_verified'];assert first['lock']['startup_reserved'] and first['lock']['first_presented_protected'] is True and first['lock']['unprotected_presentations']==0;pid=first['lock']['locker']['pid']
            await_value(lambda:(captures/f'{pid}-cover.png').exists(),bool)
            for command in ['home','overview','launcher','applications']:
                denied=any_request(control,command);assert not denied['ok'] and denied['error']=='Session is locked'
            os.kill(pid,signal.SIGKILL);state('orphaned',lambda s:s['lock']['phase']=='Orphaned' and s['lock']['last_presented_surface'] is False and s['lock']['fallback_verified'])
            again=state('recovered',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['locker']['pid']!=pid)
            await_value(lambda:(captures/f"{again['lock']['locker']['pid']}-cover.png").exists(),bool)
            request(control,'exit');code=process.wait(timeout=5);assert code==0
            records['validation']={'passed':True,'compositor_exit':code,'real_egui_gles_locker':True,'private_status':True,'crash_replacement':True,'authentication_attempted':False,'presentation_scope':'Nested EGL submission only; native KMS/authentication tested separately.'}
        print('PASS: real egui/EGL lock cover, private status, command denial, opaque crash fallback and trusted replacement')
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        runtime.cleanup();(out/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
