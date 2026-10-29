#!/usr/bin/env python3
"""Restricted real Wayland locking, cached clipboard privacy and fail-closed recovery."""
import argparse,json,os,signal,socket,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request


def any_request(control_path,command,**fields):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(3);stream.connect(str(control_path));stream.sendall((json.dumps(dict(command=command,**fields))+'\n').encode());return json.loads(stream.makefile().readline())

def events(path):return [json.loads(line) for line in path.read_bytes().split(b'\n')[:-1] if line] if path.exists() else []
def append(path,action):
    with path.open('a') as file:file.write(json.dumps({'action':action})+'\n');file.flush()

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args();args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    runtime=tempfile.TemporaryDirectory(prefix='tarot-lock-');control=Path(runtime.name)/'control.sock'
    root=Path.cwd();probe=root/'target/debug/examples/lock-probe';locker_commands=args.output/'locker-commands.jsonl';app_commands=args.output/'app-commands.jsonl'
    locker_report=args.output/'locker-events.jsonl';app_report=args.output/'app-events.jsonl';scheduled=args.output/'scheduled-capture.png'
    locker_commands.write_text('');app_commands.write_text('');env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    command=[str(root/'target/debug/tarot'),'--lock-probe-unlocked-start','--lock-client',str(probe),'--verify-lock-blank','--verify-lock-surface-rgba','336699ff','--control-socket',str(control),'--screenshot',str(scheduled),'--screenshot-after-ms','4000','--exit-after-ms','180000']
    for arg in ['--role','locker','--report',str(locker_report),'--commands',str(locker_commands)]:command.append('--lock-client-arg='+arg)
    processes=[];logs=[];records={};validation={'passed':False};session=None;started=time.monotonic()
    def log(name):file=(args.output/name).open('w');logs.append(file);return file
    def status(label,condition=lambda s:True,seconds=10):
        try:value=await_value(lambda:request(control,'status'),condition,seconds=seconds)
        except AssertionError as error:records[label+'-failure']=error.args[0] if error.args else str(error);raise
        records[label]=value;return value
    def protected(label,phase='Locked',surface=True):
        def ready(s):
            lock=s['lock'];return lock['phase']==phase and lock['blank_verified'] and lock['last_presented_generation']==lock['generation'] and lock['last_presented_surface']==surface and (lock['surface_verified'] if surface else lock['fallback_verified'])
        value=status(label,ready);assert set(value)=={'ok','pid','lock'};return value
    def locker_pid():return request(control,'status')['lock']['locker']['pid']
    def command_locker(action):append(locker_commands,action)
    def read_clipboard(label,allowed):
        before=sum(e['event']=='clipboard-read' for e in events(app_report));append(app_commands,'read-clipboard')
        value=await_value(lambda:[e for e in events(app_report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1]
        assert value['fields']['matches']==allowed and ((value['fields']['bytes']>0)==allowed);records[label]=value
    def recover(label):
        old=locker_pid();assert request(control,'restart_locker')['ok']
        current=status(label+'-process',lambda s:s['lock']['locker']['pid'] and s['lock']['locker']['pid']!=old)['lock']['locker']['pid']
        await_value(lambda:events(locker_report),lambda es:any(e['pid']==current and e['event']=='ready' for e in es));command_locker('lock');return protected(label)
    try:
        session=subprocess.Popen(command,env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);processes.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        initial=status('initial',lambda s:s['lock']['phase']=='Unlocked');ready=await_value(lambda:events(locker_report),lambda es:any(e['event']=='ready' for e in es))[-1];global_id=ready['fields']['manager_id'];assert global_id
        app_env=dict(env,WAYLAND_DISPLAY=initial['wayland_display'])
        app=subprocess.Popen([str(probe),'--role','app','--report',str(app_report),'--commands',str(app_commands)],env=app_env,stdout=log('app.log'),stderr=subprocess.STDOUT);processes.append(app)
        before=status('app-ready',lambda s:len(s['cards'])==1);await_value(lambda:events(app_report),lambda es:any(e['event']=='clipboard-offered' for e in es))
        assert next(e for e in events(app_report) if e['event']=='ready')['fields']['manager_id'] is None
        read_clipboard('clipboard-before-lock',True)
        command_locker('lock');first=protected('first-protected');generation=first['lock']['generation']
        await_value(lambda:events(locker_report),lambda es:any(e['event']=='locked' for e in es))
        for name,fields in [('home',{}),('overview',{}),('launcher',{}),('applications',{}),('activate',{'id':before['cards'][0]['id']}),('capture',{'path':str(args.output/'denied-capture.png')}),('power_action',{'action':{'Brightness':{'percent':40}}})]:
            value=any_request(control,name,**fields);assert not value['ok'] and value['error']=='Session is locked';records['denied-'+name]=value
        assert not any_request(control,'restart_locker')['ok']
        read_clipboard('cached-offer-blocked',False)
        denied_report=args.output/'denied-events.jsonl'
        denied=subprocess.run([str(probe),'--role','denied','--lock-global-id',str(global_id),'--report',str(denied_report)],env=app_env,stdout=log('denied.log'),stderr=subprocess.STDOUT,timeout=5)
        assert denied.returncode==0 and any(e['event']=='bind-denied' for e in events(denied_report));protected('denied-bind-retains-lock')
        # Ensure a capture scheduled before locking cannot run during protection.
        while time.monotonic()-started<4.5:time.sleep(.05)
        assert not scheduled.exists() and not (args.output/'denied-capture.png').exists()
        command_locker('destroy-lock-surface');protected('destroyed-role-blank',surface=False)
        read_clipboard('destroyed-role-still-private',False)
        # A verified fallback frame alone cannot authorize unlocking after the
        # trusted role has been destroyed. Require rejection and replacement.
        command_locker('unlock');protected('destroyed-role-unlock-rejected','Orphaned',surface=False)
        read_clipboard('destroyed-role-rejected-still-private',False)
        replaced=recover('destroyed-role-trusted-recovery');assert replaced['lock']['generation']>generation
        generation=replaced['lock']['generation']
        command_locker('unlock');unlocked=status('first-unlock',lambda s:s['lock']['phase']=='Unlocked');assert unlocked['view']==before['view'];read_clipboard('clipboard-restored',True)
        request(control,'capture',path=str(args.output/'unlocked-restored.png'));status('capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        command_locker('lock');second=protected('second-protected');assert second['lock']['generation']>generation
        command_locker('invalid-second-unlock');protected('invalid-unlock-stays-protected','Orphaned',surface=False)
        await_value(lambda:events(locker_report),lambda es:any(e['event']=='invalid-unlock-rejected' for e in es))
        recovered=recover('after-protocol-error-recovery');assert recovered['lock']['generation']>second['lock']['generation']
        pid=locker_pid();assert Path(f'/proc/{pid}/exe').resolve()==probe.resolve();os.kill(pid,signal.SIGKILL)
        protected('crash-falls-back','Orphaned',surface=False);read_clipboard('crash-still-private',False)
        recover('after-crash-recovery');command_locker('unlock');final=status('final-unlock',lambda s:s['lock']['phase']=='Unlocked');assert final['view']==before['view'];read_clipboard('final-clipboard-restored',True)
        append(app_commands,'exit');assert app.wait(timeout=4)==0
        status('app-removed',lambda s:not s['cards']);request(control,'exit');validation['compositor_exit']=session.wait(timeout=5);assert validation['compositor_exit']==0
        validation.update(passed=True,scheduled_capture_blocked=True,ordinary_bind_denied=True,cached_clipboard_blocked_and_restored=True,invalid_unlock_rejected=True,crash_and_trusted_recovery=True,nested_presentation_scope='Opaque framebuffer audited; nested acknowledgement follows EGL submission. Physical KMS presentation is a separate native gate.')
        print('PASS: private lock manager, opaque frame/surface/fallback audits, redacted status, capture/command denial, cached clipboard isolation, invalid-unlock rejection, crash and trusted recovery')
    finally:
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=5)
                except subprocess.TimeoutExpired:process.kill();process.wait()
        for file in logs:file.close()
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n');(args.output/'validation.json').write_text(json.dumps(validation,indent=2)+'\n');runtime.cleanup()

if __name__=='__main__':main()
