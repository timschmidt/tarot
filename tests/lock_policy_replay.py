#!/usr/bin/env python3
"""Default startup protection, repeated user locks and bounded locker recovery."""
import argparse,json,os,signal,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from lock_replay import append,events,any_request

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);root=Path.cwd();records={}
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    def scenario(name,program,arguments,run):
        folder=out/name;folder.mkdir(mode=0o700);runtime=tempfile.TemporaryDirectory(prefix='tarot-lock-policy-');control=Path(runtime.name)/'control.sock';data={};proc=None
        def state(label,condition=lambda s:True,seconds=12):
            try:value=await_value(lambda:request(control,'status'),condition,seconds=seconds)
            except AssertionError as error:data[label+'-failure']=error.args[0] if error.args else str(error);raise
            data[label]=value;return value
        command=[str(root/'target/debug/tarot'),'--lock-client',str(program),'--verify-lock-blank','--control-socket',str(control),'--exit-after-ms','60000']
        for arg in arguments:command.append('--lock-client-arg='+str(arg))
        try:
            with (folder/'compositor.log').open('w') as log:
                proc=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT)
                await_value(lambda:control.exists() or proc.poll() is not None,bool);assert proc.poll() is None
                run(control,state,data)
                request(control,'exit');data['compositor_exit']=proc.wait(timeout=5);assert data['compositor_exit']==0
        finally:
            if proc is not None and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            runtime.cleanup();(folder/'status.json').write_text(json.dumps(data,indent=2)+'\n');records[name]=data
    commands=out/'commands.jsonl';report=out/'events.jsonl';commands.write_text('')
    def repeated(control,state,data):
        started=time.monotonic()
        first=state('startup-awaiting',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'])
        assert set(first)=={'ok','pid','lock'};lock=first['lock'];assert lock['startup_reserved'] and lock['first_presented_protected'] is True and lock['unprotected_presentations']==0
        await_value(lambda:events(report),lambda es:any(e['event']=='ready' for e in es))
        for command in ['home','overview','applications']:
            response=any_request(control,command);assert not response['ok'] and response['error']=='Session is locked'
        time.sleep(.3);append(commands,'lock');one=state('first-lock',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation']);append(commands,'unlock');state('first-unlock',lambda s:s['lock']['phase']=='Unlocked')
        time.sleep(1);request(control,'lock');second=state('user-request-armed',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'])
        pid=second['lock']['locker']['pid'];assert pid!=one['lock']['locker']['pid']
        await_value(lambda:events(report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es))
        # The old generation's five-second startup timer cannot kill the fresh
        # child, which intentionally has not requested its protocol lock yet.
        while time.monotonic()-started<5.5:time.sleep(.05)
        waiting=state('old-watchdog-stale');assert waiting['lock']['phase']=='Awaiting' and waiting['lock']['locker']['pid']==pid
        append(commands,'lock');state('second-lock',lambda s:s['lock']['phase']=='Locked')
        append(commands,'destroy-lock-surface')
        replaced=state('destroyed-role-replaced',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['locker']['pid'] not in (None,pid) and s['lock']['blank_verified'])
        pid=replaced['lock']['locker']['pid'];await_value(lambda:events(report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es));append(commands,'lock');state('role-replacement-locked',lambda s:s['lock']['phase']=='Locked')
        before=state('before-crash');os.kill(pid,signal.SIGKILL)
        recovered=state('automatic-replacement',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['locker']['pid'] not in (None,pid) and s['lock']['blank_verified'])
        assert recovered['lock']['orphaned_presentations']>0 and recovered['lock']['locker']['recoveries']>=1
        new_pid=recovered['lock']['locker']['pid'];await_value(lambda:events(report),lambda es:any(e['pid']==new_pid and e['event']=='ready' for e in es));append(commands,'lock')
        state('replacement-lock',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation']);append(commands,'unlock');state('second-unlock',lambda s:s['lock']['phase']=='Unlocked')
        request(control,'lock');third=state('third-user-lock',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['locker']['pid'] not in (None,new_pid))
        third_pid=third['lock']['locker']['pid'];await_value(lambda:events(report),lambda es:any(e['pid']==third_pid and e['event']=='ready' for e in es));append(commands,'lock');state('third-ack',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation']);append(commands,'unlock');state('third-unlock',lambda s:s['lock']['phase']=='Unlocked')
        data['validation']={'passed':True,'startup_black_before_protocol_request':True,'fresh_client_per_user_lock':True,'old_timer_ignored':True,'automatic_crash_replacement':True,'destroyed_role_replaced':True,'repeated_lock_unlock':True,'presentation_scope':'Nested EGL submission; native KMS checked separately.'}
    scenario('repeated',root/'target/debug/examples/lock-probe',['--role','locker','--report',report,'--commands',commands],repeated)
    def exhausted(control,state,data):
        value=state('exhausted',lambda s:s['lock']['phase']=='Orphaned' and s['lock']['locker']['consecutive_failures']==6 and not s['lock']['locker']['restart_pending'] and s['lock']['recovery_ready'] and s['lock']['recovery_verified'],seconds=15)
        assert value['lock']['first_presented_protected'] is True and value['lock']['unprotected_presentations']==0
        generation=value['lock']['locker']['generation'];time.sleep(.6);assert state('still-protected')['lock']['locker']['generation']==generation
        request(control,'restart_locker');state('operator-retry',lambda s:s['lock']['locker']['generation']>generation)
        data['validation']={'passed':True,'six_failed_starts_stop_automatic_recovery':True,'protection_retained':True,'explicit_operator_retry':True}
    scenario('crash-budget','/bin/false',[],exhausted)
    scenario('spawn-error',out/'missing-locker',[],exhausted)
    def deadline(control,state,data):
        initial=state('unresponsive-start',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['blank_verified']);pid=initial['lock']['locker']['pid']
        changed=state('startup-deadline-recovery',lambda s:s['lock']['locker']['recoveries']>=1 and s['lock']['locker']['pid'] not in (None,pid) and s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'],seconds=8)
        assert changed['lock']['unprotected_presentations']==0
        await_value(lambda:Path(f'/proc/{pid}').exists(),lambda alive:not alive)
        data['validation']={'passed':True,'unresponsive_child_terminated':True,'no_ordinary_startup_scene':True,'automatic_replacement':True}
    scenario('startup-deadline','/bin/sleep',['60'],deadline)
    silent_commands=out/'silent-commands.jsonl';silent_report=out/'silent-events.jsonl';silent_commands.write_text('')
    def no_buffer(control,state,data):
        initial=state('black-before-request',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'])
        pid=initial['lock']['locker']['pid'];await_value(lambda:events(silent_report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es));append(silent_commands,'lock')
        state('acknowledged-without-client-buffer',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is False and s['lock']['fallback_verified'])
        state('missing-buffer-replaced',lambda s:s['lock']['locker']['recoveries']>=1 and s['lock']['locker']['pid'] not in (None,pid) and s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'],seconds=8)
        data['validation']={'passed':True,'acknowledged_black_without_ui':True,'missing_client_buffer_replaced':True}
    scenario('missing-buffer',root/'target/debug/examples/lock-probe',['--role','locker','--no-lock-buffer','--report',silent_report,'--commands',silent_commands],no_buffer)
    (out/'validation.json').write_text(json.dumps({'passed':all(d['validation']['passed'] for d in records.values()),'scenarios':list(records),'authentication_fixture_only':True},indent=2)+'\n')
    print('PASS: protected default startup, repeated user locks, stale timer isolation, opaque automatic crash recovery, six-failure budget and unresponsive startup deadline')

if __name__=='__main__':main()
