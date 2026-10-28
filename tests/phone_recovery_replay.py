#!/usr/bin/env python3
"""Bounded native recovery retry/input isolation with real same-user PAM.

The administrator supplies the device password through getpass only. Root owns
temporary uinput devices; the compositor, application and PAM locker use UID
10000. No password is recorded in files, arguments, environment or reports.
"""
import argparse,getpass,json,os,pwd,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_lock_replay import Keyboard
from phone_touch_keyboard import Keyboard as TouchKeyboard
from lock_replay import events,append,any_request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    base=Path('/home/user/tarot-dev');socket=Path('/run/user/10000/tarot-drm.sock');marker=base/'artifacts/recovery-allow';captures=base/'artifacts/locker-ui'
    assert not socket.exists() and not captures.exists() and not (base/'artifacts/recovery-own.png').exists() and not marker.exists()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid);groups=os.getgrouplist('user',account.pw_gid)
    password=getpass.getpass('Device password for ephemeral recovery authentication test: ')
    assert password and all(' '<=c<='~' for c in password),'Replay supports printable ASCII; password was not recorded'
    records={};screen=keyboard=harness=app=log=app_log=None;compositor_pid=None
    app_report=args.output/'app-events.jsonl';app_commands=args.output/'app-commands.jsonl'
    def state(label,condition=lambda s:True,seconds=20):
        try:value=await_value(lambda:request(socket,'status'),condition,seconds=seconds)
        except AssertionError as error:records[label+'-failure']=error.args[0] if error.args else str(error);raise
        if compositor_pid is not None:assert value['pid']==compositor_pid
        records[label]=value;return value
    def allow():
        marker.touch(mode=0o600,exist_ok=False);os.chown(marker,account.pw_uid,account.pw_gid)
    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8'
        assert request(socket,'status')['pid']==compositor_pid
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.08)
    def count(name):return sum(e['event']==name for e in events(app_report))
    def clipboard(label,allowed):
        before=count('clipboard-read');append(app_commands,'read-clipboard')
        value=await_value(lambda:[e for e in events(app_report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1]
        assert value['fields']['matches']==allowed and (value['fields']['bytes']>0)==allowed;records[label]=value
    def recovery(label):
        value=state(label,lambda s:s['lock']['recovery_ready'] and s['lock']['recovery_verified'])
        lock=value['lock'];assert set(value)=={'ok','pid','lock'} and lock['phase']=='Orphaned'
        assert lock['last_presented_recovery'] and not lock['last_presented_surface'] and lock['locker']['consecutive_failures']==6
        return value['lock']['generation']
    def same_generation(generation):
        time.sleep(.3);value=request(socket,'status');assert value['lock']['recovery_ready'] and value['lock']['generation']==generation
    def authenticate(label):
        locked=state(label+'-cover',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        locker_pid=locked['lock']['locker']['pid'];assert Path(f'/proc/{locker_pid}').stat().st_uid==10000
        await_value((captures/f'{locker_pid}-cover.png').exists,bool)
        tap(180,608);await_value((captures/f'{locker_pid}-entry-empty.png').exists,bool)
        TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        value=state(label+'-authenticated',lambda s:s['lock']['phase']=='Unlocked')
        await_value(lambda:Path(f'/proc/{locker_pid}').exists(),lambda alive:not alive)
        return value
    try:
        allow();app_commands.write_text('');os.chown(app_commands,account.pw_uid,account.pw_gid)
        env=dict(os.environ,TAROT_DRM_SECONDS='180',TAROT_LOCAL_SESSION='1',TAROT_KEYBOARD='1',TAROT_LOCK_RECOVERY='1',TAROT_LOCKER='0',TAROT_LOCK_PROBE='0')
        log=(args.output/'harness.log').open('w');harness=subprocess.Popen(['sh',str(base/'phone-drm-smoke.sh')],env=env,stdout=log,stderr=subprocess.STDOUT,cwd=base)
        await_value(lambda:socket.exists() or harness.poll() is not None,bool);assert harness.poll() is None
        initial=state('first-protected-presentation',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        compositor_pid=initial['pid'];assert initial['lock']['first_presented_protected'] is True and initial['lock']['unprotected_presentations']==0
        screen=Touchscreen(guard);keyboard=Keyboard(guard);unlocked=authenticate('initial')
        for card in unlocked['cards']:request(socket,'close',id=card['id'])
        state('empty-session',lambda s:not s['cards'])
        app_env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR=str(socket.parent),WAYLAND_DISPLAY=unlocked['wayland_display']);app_env.pop('WAYLAND_SOCKET',None);app_env.pop('DISPLAY',None)
        app_log=(args.output/'app.log').open('w');app=subprocess.Popen([str(base/'bin/lock-probe'),'--role','app','--report',str(app_report),'--commands',str(app_commands)],env=app_env,user=account.pw_uid,group=account.pw_gid,extra_groups=groups,stdout=app_log,stderr=app_log,stdin=subprocess.DEVNULL,cwd=account.pw_dir)
        before=state('app-active',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view'])
        await_value(lambda:events(app_report),lambda es:any(e['event']=='clipboard-offered' for e in es));clipboard('clipboard-before',True)
        keyboard.key(28,True);screen.down(180,350)
        await_value(lambda:events(app_report),lambda es:any(e['event']=='key' for e in es) and any(e['event']=='touch-down' for e in es))
        app_keys=count('key');app_ups=count('touch-up');marker.unlink();request(socket,'lock');generation=recovery('exhausted-presented')
        await_value(lambda:events(app_report),lambda es:any(e['event']=='touch-cancel' for e in es))
        screen.up();keyboard.key(28,False);same_generation(generation)
        assert count('key')==app_keys and count('touch-up')==app_ups
        for name,fields in [('home',{}),('overview',{}),('launcher',{}),('applications',{}),('capture',{'path':str(args.output/'denied.png')})]:
            assert any_request(socket,name,**fields)=={'ok':False,'error':'Session is locked'}
        assert not (args.output/'denied.png').exists();clipboard('cached-offer-blocked',False)
        keyboard.tap(59);keyboard.tap(60);keyboard.tap(16);same_generation(generation)
        assert count('key')==app_keys
        # A second finger cancels the first button press; no release can retry.
        screen.down(180,388);screen.down(50,200,slot=1);screen.up(slot=1);screen.up();same_generation(generation)
        tap(50,200);same_generation(generation)
        # Destroy the device while the button is held, without emitting an up.
        screen.down(180,388);screen.close();screen=None;time.sleep(.3);same_generation(generation)
        screen=Touchscreen(guard);same_generation(generation)
        records['canceled_and_removed_contacts_cannot_retry']=True
        # Both actual physical-keyboard and touch retries reset the bounded
        # budget but must remain protected when the configured client fails.
        keyboard.tap(28);state('physical-enter-retry',lambda s:s['lock']['generation']>generation)
        generation=recovery('keyboard-retry-exhausted')
        tap(180,388);state('fresh-touch-retry',lambda s:s['lock']['generation']>generation)
        generation=recovery('touch-retry-exhausted')
        assert count('key')==app_keys and count('touch-up')==app_ups
        # Hold one recovery contact across a keyboard-triggered replacement.
        # The release must not become a fresh touch in the new private locker.
        allow();screen.down(180,388);keyboard.key(28,True)
        replaced=state('real-replacement-presented',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        assert replaced['lock']['generation']>generation and not replaced['lock']['recovery_ready'] and not replaced['lock']['last_presented_recovery']
        screen.up();keyboard.key(28,False);time.sleep(.2)
        assert request(socket,'status')['lock']['phase']=='Locked';clipboard('still-private-before-pam',False)
        after=authenticate('recovery');assert after['cards']==before['cards'] and after['view']==before['view'];clipboard('original-offer-restored',True)
        keys=count('key');ups=count('touch-up');keyboard.tap(16);tap(180,350)
        await_value(lambda:(count('key'),count('touch-up')),lambda counts:counts[0]>keys and counts[1]>ups)
        request(socket,'capture',path=str(args.output/'normal-restored.png'));state('normal-capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        append(app_commands,'exit');assert app.wait(timeout=4)==0
        # Exercise the freshly reset ordinary egui context and a real text-input
        # field/keyboard after authenticated recovery, with brightness restored.
        result=subprocess.run(['python3',str(base/'phone_power_replay.py'),'--output',str(args.output/'normal-power-text')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=40)
        (args.output/'normal-power-text.log').write_text(result.stdout);assert result.returncode==0
        records['validation']={'passed':True,'native_kms_recovery_presentation':True,'first_frame_protected':True,'exhaustion_stays_private':True,'held_release_canceled':True,'multitouch_and_device_removal_cancel':True,'physical_enter_and_fresh_touch_retry':True,'new_owner_handoff_private':True,'real_same_user_pam_required':True,'cards_view_cached_offer_restored':True,'normal_power_modal_text_keyboard_regression':True,'password_logged':False,'mouse_path_tested':False,'idle_suspend_packaging_complete':False}
        print('PASS: native recovery presentation, canceled/held/device-loss input, keyboard/touch retry, private handoff, real PAM, restored app/clipboard and ordinary modal/text input')
    finally:
        password=None
        if screen is not None:screen.close()
        if keyboard is not None:keyboard.close()
        if app is not None and app.poll() is None:
            app.terminate()
            try:app.wait(timeout=3)
            except subprocess.TimeoutExpired:app.kill();app.wait()
        marker.unlink(missing_ok=True)
        if socket.exists():
            try:request(socket,'exit')
            except (OSError,AssertionError):pass
        if harness is not None:
            try:records['harness_exit']=harness.wait(timeout=45)
            except subprocess.TimeoutExpired:records['harness_exit']='timeout; inspect bounded watchdog'
        if app_log is not None:app_log.close()
        if log is not None:log.close()
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')


if __name__=='__main__':main()
