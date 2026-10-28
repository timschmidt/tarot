#!/usr/bin/env python3
"""Direct DRM lock presentation, held-input isolation, clipboard and crash recovery."""
import argparse,fcntl,json,os,pwd,signal,struct,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from lock_replay import events,append,any_request

class Keyboard:
    def __init__(self,guard):
        self.guard=guard;self.held=set();self.fd=os.open('/dev/uinput',os.O_WRONLY|os.O_NONBLOCK);self.created=False
        iow=lambda number,size:(1<<30)|(size<<16)|(ord('U')<<8)|number
        try:
            fcntl.ioctl(self.fd,iow(100,4),1)
            for key in range(1,256):fcntl.ioctl(self.fd,iow(101,4),key)
            setup=struct.pack('@HHHH80sI',0x06,1,2,1,b'Tarot development lock keyboard',0)
            fcntl.ioctl(self.fd,iow(3,len(setup)),setup);fcntl.ioctl(self.fd,0x5501);self.created=True;time.sleep(.8)
        except BaseException:self.close();raise
    def key(self,code,down):
        self.guard();self.held.add(code) if down else self.held.discard(code)
        os.write(self.fd,struct.pack('@llHHi',0,0,1,code,int(down)));os.write(self.fd,struct.pack('@llHHi',0,0,0,0,0));time.sleep(.1)
    def tap(self,code):self.key(code,True);self.key(code,False)
    def close(self):
        if self.created:
            for code in list(self.held):self.key(code,False)
            fcntl.ioctl(self.fd,0x5502);self.created=False
        os.close(self.fd)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'));p.add_argument('--output',type=Path,required=True);p.add_argument('--locker-report',type=Path,default=Path('/home/user/tarot-dev/artifacts/lock-method.jsonl'));p.add_argument('--locker-commands',type=Path,default=Path('/home/user/tarot-dev/artifacts/lock-method.commands'));args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False);owner=args.socket.stat();assert owner.st_uid==10000;os.chown(args.output,owner.st_uid,owner.st_gid)
    account=pwd.getpwuid(owner.st_uid);groups=os.getgrouplist(account.pw_name,owner.st_gid)
    initial=await_value(lambda:request(args.socket,'status'),lambda s:s['lock']['phase']=='Unlocked' and s['lock']['locker']['pid'])
    ready=await_value(lambda:events(args.locker_report),lambda es:any(e['event']=='ready' for e in es))[-1];global_id=ready['fields']['manager_id'];assert global_id
    records={'initial':initial};screen=keyboard=app=extra=log=None;probe=Path('/home/user/tarot-dev/bin/lock-probe');app_report=args.output/'app-events.jsonl';app_commands=args.output/'app-commands.jsonl';app_commands.write_text('');os.chown(app_commands,owner.st_uid,owner.st_gid)
    env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR=str(args.socket.parent),WAYLAND_DISPLAY=initial['wayland_display']);env.pop('WAYLAND_SOCKET',None);env.pop('DISPLAY',None)
    def status(label,condition=lambda s:True,seconds=10):
        try:value=await_value(lambda:request(args.socket,'status'),condition,seconds=seconds)
        except AssertionError as error:records[label+'-failure']=error.args[0] if error.args else str(error);raise
        assert value['pid']==initial['pid'];records[label]=value;return value
    def protected(label,phase='Locked',surface=True):
        def ready(s):
            lock=s['lock'];return lock['phase']==phase and lock['blank_verified'] and lock['last_presented_generation']==lock['generation'] and lock['last_presented_surface']==surface and (lock['surface_verified'] if surface else lock['fallback_verified'])
        value=status(label,ready);assert set(value)=={'ok','pid','lock'};return value
    def command(action):append(args.locker_commands,action)
    def count(path,event):return sum(e['event']==event for e in events(path))
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(args.socket,'status')['pid']==initial['pid']
    def clip(label,allowed):
        before=count(app_report,'clipboard-read');append(app_commands,'read-clipboard');value=await_value(lambda:[e for e in events(app_report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1];assert value['fields']['matches']==allowed and ((value['fields']['bytes']>0)==allowed);records[label]=value
    def recover(label):
        old=request(args.socket,'status')['lock']['locker']['pid'];request(args.socket,'restart_locker');current=status(label+'-new-process',lambda s:s['lock']['locker']['pid'] and s['lock']['locker']['pid']!=old)['lock']['locker']['pid'];await_value(lambda:events(args.locker_report),lambda es:any(e['event']=='ready' and e['pid']==current for e in es));command('lock');return protected(label)
    try:
        # Start with the synthetic app only; never rely on a GTK text-input widget
        # for the security fixture's keyboard or clipboard event counters.
        for card in request(args.socket,'status')['cards']:request(args.socket,'close',id=card['id'])
        status('empty-session',lambda s:not s['cards'])
        log=(args.output/'app.log').open('w');app=subprocess.Popen([str(probe),'--role','app','--report',str(app_report),'--commands',str(app_commands)],env=env,user=owner.st_uid,group=owner.st_gid,extra_groups=groups,stdin=subprocess.DEVNULL,stdout=log,stderr=log,cwd=account.pw_dir)
        before=status('app-ready',lambda s:len(s['cards'])==1);assert Path(f'/proc/{initial["pid"]}').stat().st_uid==10000
        await_value(lambda:events(app_report),lambda es:any(e['event']=='clipboard-offered' for e in es));clip('clipboard-before',True)
        screen=Touchscreen(guard);keyboard=Keyboard(guard)
        keyboard.key(16,True);await_value(lambda:events(app_report),lambda es:any(e['event']=='key' for e in es))
        screen.down(180,350);await_value(lambda:events(app_report),lambda es:any(e['event']=='touch-down' for e in es))
        app_keys=count(app_report,'key');app_ups=count(app_report,'touch-up');command('lock');first=protected('first-protected')
        await_value(lambda:events(app_report),lambda es:any(e['event']=='touch-cancel' for e in es))
        screen.up();keyboard.key(16,False);time.sleep(.2)
        assert count(app_report,'key')==app_keys and count(app_report,'touch-up')==app_ups
        assert count(args.locker_report,'key')==0 and count(args.locker_report,'touch-up')==0
        enters=[e for e in events(args.locker_report) if e['event']=='keyboard-enter'];assert enters and enters[-1]['fields']['held_count']==0
        records['pre-lock-input-cancelled']={'app_key_events':app_keys,'app_touch_cancellation':True,'held_keys_not_entered':True,'held_release_not_delivered_to_locker':True}
        keyboard.tap(59);keyboard.tap(60);keyboard.tap(16);screen.down(180,350);screen.up()
        await_value(lambda:events(args.locker_report),lambda es:sum(e['event']=='key' for e in es)>=6 and any(e['event']=='touch-up' for e in es))
        assert count(app_report,'key')==app_keys and count(app_report,'touch-up')==app_ups;protected('fresh-input-only-to-locker')
        for name,fields in [('home',{}),('overview',{}),('launcher',{}),('applications',{}),('capture',{'path':str(args.output/'denied-capture.png')})]:
            denied=any_request(args.socket,name,**fields);assert not denied['ok'] and denied['error']=='Session is locked';records['denied-'+name]=denied
        clip('old-offer-blocked',False)
        denied_report=args.output/'denied-events.jsonl';result=subprocess.run([str(probe),'--role','denied','--lock-global-id',str(global_id),'--report',str(denied_report)],env=env,user=owner.st_uid,group=owner.st_gid,extra_groups=groups,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
        assert result.returncode==0 and any(e['event']=='bind-denied' for e in events(denied_report));protected('foreign-bind-denied')
        # Require a new real KMS presentation after VT reacquisition, not a stale
        # pre-pause status value. The opaque fixture must still own all input.
        presentation=protected('before-vt')['lock']['presented_frames']
        subprocess.run(['chvt','7'],check=True);await_value(lambda:Path('/sys/class/tty/tty0/active').read_text().strip(),lambda vt:vt=='tty7');time.sleep(.3)
        subprocess.run(['chvt','8'],check=True);after=status('after-vt-new-presentation',lambda s:s['lock']['presented_frames']>presentation and s['lock']['last_presented_surface'] is True and s['lock']['surface_verified']);assert after['lock']['phase']=='Locked'
        # Normal clients' round trips must not generate a perpetual locked redraw.
        time.sleep(.5);idle_before=protected('idle-before')['lock']['presented_frames'];time.sleep(.5);idle_after=protected('idle-after')['lock']['presented_frames'];assert idle_after-idle_before<=1;records['idle_presentations_in_half_second']=idle_after-idle_before
        command('destroy-lock-surface');protected('destroyed-role-blank',surface=False);clip('destroyed-role-private',False)
        command('unlock');status('first-unlock',lambda s:s['lock']['phase']=='Unlocked');clip('clipboard-restored',True)
        command('lock');protected('held-input-unlock-start');keyboard.key(16,True);screen.down(180,350);time.sleep(.2)
        # A normal app mapping/unmapping and a normal keyboard-helper crash must
        # not cancel the trusted locker's held touch or steal its key focus.
        cancelled=count(args.locker_report,'touch-cancel')
        extra_report=args.output/'extra-events.jsonl';extra_commands=args.output/'extra-commands.jsonl';extra_commands.write_text('');os.chown(extra_commands,owner.st_uid,owner.st_gid)
        extra=subprocess.Popen([str(probe),'--role','app','--report',str(extra_report),'--commands',str(extra_commands)],env=env,user=owner.st_uid,group=owner.st_gid,extra_groups=groups,stdin=subprocess.DEVNULL,stdout=log,stderr=log,cwd=account.pw_dir)
        await_value(lambda:events(extra_report),lambda es:any(e['event']=='app-mapped' for e in es));time.sleep(.2);append(extra_commands,'exit');assert extra.wait(timeout=4)==0
        helper=initial['input_method']['pid'];assert Path(f'/proc/{helper}/exe').resolve()==Path('/home/user/tarot-dev/bin/tarot-keyboard');os.kill(helper,signal.SIGKILL)
        def helpers():
            found=[]
            for proc in Path('/proc').iterdir():
                if not proc.name.isdecimal():continue
                try:
                    if proc.stat().st_uid==10000 and (proc/'exe').resolve()==Path('/home/user/tarot-dev/bin/tarot-keyboard') and int((proc/'stat').read_text().rsplit(')',1)[1].split()[1])==initial['pid']:found.append(int(proc.name))
                except (FileNotFoundError,ProcessLookupError,PermissionError):pass
            return found
        replacement=await_value(helpers,lambda ps:any(p!=helper for p in ps));time.sleep(.3)
        assert count(args.locker_report,'touch-cancel')==cancelled
        protected('lock-input-survives-app-and-helper-changes');records['helper-recovery-during-lock']={'old_pid':helper,'new_pids':replacement,'held_touch_retained':True}
        before_keys=count(app_report,'key');before_ups=count(app_report,'touch-up');command('unlock');status('held-input-unlock',lambda s:s['lock']['phase']=='Unlocked');screen.up();keyboard.key(16,False);time.sleep(.2)
        assert count(app_report,'key')==before_keys and count(app_report,'touch-up')==before_ups
        app_enters=[e for e in events(app_report) if e['event']=='keyboard-enter'];assert app_enters[-1]['fields']['held_count']==0
        records['unlock-held-input-isolated']=True
        command('lock');protected('invalid-unlock-start');command('invalid-second-unlock');protected('invalid-unlock-stays-private','Orphaned',surface=False)
        recover('after-invalid-recovery');pid=request(args.socket,'status')['lock']['locker']['pid'];assert Path(f'/proc/{pid}/exe').resolve()==probe.resolve();os.kill(pid,signal.SIGKILL)
        protected('crash-blank','Orphaned',surface=False);clip('crash-private',False)
        recover('after-crash-recovery');command('unlock');final=status('final-unlock',lambda s:s['lock']['phase']=='Unlocked');assert final['view']==before['view'];clip('final-clipboard',True)
        request(args.socket,'capture',path=str(args.output/'unlocked-restored.png'));status('capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        records['validation']={'passed':True,'compositor_uid':10000,'kms_protected_presentation':True,'opaque_frame_surface_fallback_audits':True,'held_key_touch_crossing_blocked':True,'fresh_locker_input':True,'cached_clipboard_blocked_restored':True,'foreign_bind_denied':True,'invalid_unlock_denied':True,'vt_release_reacquire_protected':True,'crash_trusted_recovery':True,'authentication_ui_implemented':False}
        print('PASS: native KMS protected frames, held key/touch isolation both directions, locker-only input, cached clipboard/capture/command blocking, denied binding, invalid unlock, VT recovery and fail-closed crash recovery')
    finally:
        if screen is not None:screen.close()
        if keyboard is not None:keyboard.close()
        if extra is not None and extra.poll() is None:
            extra.terminate()
            try:extra.wait(timeout=3)
            except subprocess.TimeoutExpired:extra.kill();extra.wait()
        if app is not None and app.poll() is None:
            app.terminate()
            try:app.wait(timeout=3)
            except subprocess.TimeoutExpired:app.kill();app.wait()
        if log is not None:log.close()
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
