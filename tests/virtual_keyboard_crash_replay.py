#!/usr/bin/env python3
"""A dead trusted keyboard must not leave virtual modifiers in an ordinary app."""
import argparse,hashlib,json,os,signal,subprocess,tempfile
from pathlib import Path
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--compositor',type=Path,default=Path('target/debug/tarot'))
    parser.add_argument('--text-probe',type=Path,default=Path('target/debug/examples/text-probe'))
    parser.add_argument('--parent-wayland',type=Path)
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    binary=args.compositor.resolve();probe=args.text_probe.resolve()
    runtime=tempfile.TemporaryDirectory(prefix='tarot-virtual-crash-');work=Path(runtime.name);control=work/'control.sock'
    mr=out/'method-events.jsonl';mc=out/'method-commands.jsonl';ar=out/'app-events.jsonl';ac=out/'app-commands.jsonl'
    for p in [mc,ac]:p.write_text('')
    env=dict(os.environ,XDG_RUNTIME_DIR=str(work),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
    if args.parent_wayland is not None:
        assert args.parent_wayland.is_absolute() and args.parent_wayland.is_socket()
        env['WAYLAND_DISPLAY']=str(args.parent_wayland)
        env['XDG_SESSION_TYPE']='wayland';env.pop('DISPLAY',None)
        env.pop('LIBGL_ALWAYS_SOFTWARE',None);env.pop('__EGL_VENDOR_LIBRARY_FILENAMES',None)
    children=[];logs=[];record={'passed':False,'scope':'nested protocol helper death, synthetic held key'}
    def start(command,name,environment=env):
        log=(out/(name+'.log')).open('w');logs.append(log)
        p=subprocess.Popen(command,env=environment,stdout=log,stderr=subprocess.STDOUT);children.append(p);return p
    try:
        command=[str(binary),'--control-socket',str(control),'--input-method',str(probe),'--exit-after-ms','30000']
        command.extend('--input-method-arg='+v for v in ['--role','method','--report',str(mr),'--commands',str(mc)])
        compositor=start(command,'compositor')
        await_value(lambda:control.exists() or compositor.poll() is not None,bool,seconds=10);assert compositor.poll() is None
        initial=request(control,'status');pid=initial['input_method']['pid'];assert pid
        await_value(lambda:events(mr),lambda es:any(e['event']=='ready' for e in es),seconds=6)
        app=start([str(probe),'--role','app','--report',str(ar),'--commands',str(ac)],'app',dict(env,WAYLAND_DISPLAY=initial['wayland_display']))
        await_value(lambda:request(control,'status'),lambda s:len(s['cards'])==1,seconds=6)
        with mc.open('a') as f:f.write('{"command":"virtual-key","key":30,"mods":4,"reset":false,"release":false}\n')
        before=await_value(lambda:events(ar),lambda es:any(e['event']=='key' and e['key']==30 for e in es) and [e for e in es if e['event']=='modifiers'][-1]['depressed']==4,seconds=6)
        mc.write_text('');(out/'method-before-crash.json').write_text(json.dumps(events(mr),indent=2)+'\n')
        path=Path('/proc',str(pid));assert (path/'exe').resolve()==probe.resolve()
        record.update(compositor_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),probe_sha256=hashlib.sha256(probe.read_bytes()).hexdigest(),killed_method_pid=pid,before=before)
        fd=os.pidfd_open(pid)
        try:signal.pidfd_send_signal(fd,signal.SIGKILL)
        finally:os.close(fd)
        restored=await_value(lambda:events(ar),lambda es:
            bool([e for e in es[len(before):] if e['event']=='modifiers']) and
            all([e for e in es if e['event']=='modifiers'][-1][name]==0 for name in ['depressed','latched','locked']) and
            any(e['event']=='key' and e['key']==30 and e['state']==0 for e in es[len(before):]),seconds=8)
        replacement=await_value(lambda:request(control,'status'),lambda s:s['input_method']['pid'] and s['input_method']['pid']!=pid,seconds=8)
        record.update(passed=True,after=restored,replacement_pid=replacement['input_method']['pid'])
        with ac.open('a') as f:f.write('{"command":"quit"}\n')
        assert app.wait(timeout=5)==0
        request(control,'exit');assert compositor.wait(timeout=5)==0
        print('PASS: ordinary virtual modifiers reset after helper death and managed replacement')
    finally:
        for p in reversed(children):
            if p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for f in logs:f.close()
        (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n');runtime.cleanup()


if __name__=='__main__':main()
