#!/usr/bin/env python3
"""Real trusted virtual-key requests must never reach the protected locker."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from lock_replay import append, events
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--compositor',type=Path,default=Path('target/debug/tarot'))
    parser.add_argument('--text-probe',type=Path,default=Path('target/debug/examples/text-probe'))
    parser.add_argument('--lock-probe',type=Path,default=Path('target/debug/examples/lock-probe'))
    parser.add_argument('--parent-wayland',type=Path)
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    method=args.text_probe.resolve();locker=args.lock_probe.resolve()
    running=[];logs=[];record={'passed':False,'scope':'nested protocol lock, synthetic keys only'}
    runtime=tempfile.TemporaryDirectory(prefix='tarot-virtual-lock-')
    control=Path(runtime.name)/'control.sock'
    mr=out/'method-events.jsonl';mc=out/'method-commands.jsonl'
    lr=out/'locker-events.jsonl';lc=out/'locker-commands.jsonl'
    ar=out/'app-events.jsonl';ac=out/'app-commands.jsonl'
    for p in [mc,lc,ac]:p.write_text('')
    env=dict(os.environ,XDG_RUNTIME_DIR=runtime.name,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    env.pop('WAYLAND_SOCKET',None);env.pop('WAYLAND_DISPLAY',None)
    if args.parent_wayland is not None:
        assert args.parent_wayland.is_absolute() and args.parent_wayland.is_socket()
        env['WAYLAND_DISPLAY']=str(args.parent_wayland)
        env['XDG_SESSION_TYPE']='wayland';env.pop('DISPLAY',None)
        env.pop('LIBGL_ALWAYS_SOFTWARE',None);env.pop('__EGL_VENDOR_LIBRARY_FILENAMES',None)
    def start(command,name,environment=env):
        log=(out/(name+'.log')).open('w');logs.append(log)
        p=subprocess.Popen(command,env=environment,stdout=log,stderr=subprocess.STDOUT);running.append(p);return p
    def state(condition):return await_value(lambda:request(control,'status'),condition,seconds=10)
    def key(code,mods=0,reset=True,raw=False):
        count=sum(e['event']=='virtual-synced' for e in events(mr))
        with mc.open('a') as f:f.write(json.dumps({'command':'virtual-key','key':code,'mods':mods,'reset':reset,'raw':raw})+'\n')
        await_value(lambda:events(mr),lambda es:sum(e['event']=='virtual-synced' for e in es)>count,seconds=5)
    def checkpoint():
        count=sum(e['event']=='checkpoint' for e in events(lr))
        append(lc,'checkpoint')
        return await_value(lambda:events(lr),lambda es:sum(e['event']=='checkpoint' for e in es)>count,seconds=5)
    try:
        binary=args.compositor.resolve();record['compositor_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
        record['fixture_sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [method,locker]}
        command=[str(binary),'--lock-probe-unlocked-start','--control-socket',str(control),
                 '--lock-client',str(locker),'--input-method',str(method),'--exit-after-ms','90000']
        for flag,values in [('--lock-client-arg',['--role','locker','--report',str(lr),'--commands',str(lc)]),
                            ('--input-method-arg',['--role','method','--report',str(mr),'--commands',str(mc)])]:
            command.extend(flag+'='+v for v in values)
        compositor=start(command,'compositor')
        await_value(lambda:control.exists() or compositor.poll() is not None,bool,seconds=10)
        assert compositor.poll() is None
        initial=state(lambda s:s['lock']['phase']=='Unlocked')
        await_value(lambda:events(mr),lambda es:any(e['event']=='ready' for e in es),seconds=8)
        await_value(lambda:events(lr),lambda es:any(e['event']=='ready' for e in es),seconds=8)
        app=start([str(method),'--role','app','--report',str(ar),'--commands',str(ac)],'app',dict(env,WAYLAND_DISPLAY=initial['wayland_display']))
        before=state(lambda s:len(s['cards'])==1)
        key(30,4,reset=False)
        await_value(lambda:events(ar),lambda es:sum(e['event']=='key' and e['key']==30 for e in es)==2,seconds=5)
        record['ordinary_key_delivered']=True
        append(lc,'lock')
        protected=state(lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface']
            and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        boundary=checkpoint()
        record['locker_initial_events']=boundary
        assert not [e for e in boundary if e['event']=='modifiers' and any(e['fields'][k] for k in ['depressed','latched','locked'])]
        key(48,4)
        after=checkpoint()
        leaked=[e for e in after[len(boundary):] if e['event']=='key' or
                e['event']=='modifiers' and any(e['fields'][k] for k in ['depressed','latched','locked'])]
        record['protected_events']=after[len(boundary):]
        assert not leaked,{'virtual_input_reached_locker':leaked}
        record['protected_keys_and_modifiers_blocked']=True
        append(lc,'unlock')
        restored=state(lambda s:s['lock']['phase']=='Unlocked' and s['view']==before['view'])
        # No fresh modifier request is sent: denied traffic must have erased
        # the Control mask retained before locking, not merely hidden it.
        key(46,raw=True)
        found=await_value(lambda:events(ar),lambda es:sum(e['event']=='key' and e['key']==46 for e in es)==2,seconds=5)
        mods=[e for e in found if e['event']=='modifiers']
        assert mods and mods[-1]['depressed']==mods[-1]['latched']==mods[-1]['locked']==0
        record['ordinary_keys_restore_without_stuck_modifiers']=True
        with ac.open('a') as f:f.write('{"command":"quit"}\n')
        assert app.wait(timeout=5)==0
        state(lambda s:not s['cards']);request(control,'exit')
        assert compositor.wait(timeout=5)==0
        record['passed']=True
        print('PASS: trusted virtual keys/modifiers reach ordinary app, are denied at protected dispatch, and restore cleanly')
    finally:
        for p in reversed(running):
            if p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for log in logs:log.close()
        (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n')
        runtime.cleanup()


if __name__=='__main__':main()
