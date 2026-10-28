#!/usr/bin/env python3
"""Bounded real PAM + private egui touch authentication on the spare phone.

Prompts on the administrator's terminal before starting the harness. Never puts
credentials into files, argv, environment, status, screenshots of clear text or
logs. Python/PAM/OS copies are not a zeroization guarantee. The compositor and
locker remain unprivileged; root creates only temporary test input devices.
"""
import argparse,getpass,json,os,pwd,signal,struct,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_lock_replay import Keyboard
from phone_touch_keyboard import Keyboard as TouchKeyboard
from lock_replay import any_request

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    assert not Path('/run/user/10000/tarot-drm.sock').exists()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    password=getpass.getpass('Device password for ephemeral authentication test: ')
    assert password and all(' '<=c<='~' for c in password),'Replay supports printable ASCII; no password was recorded'
    account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid);groups=os.getgrouplist('user',account.pw_gid);base=Path('/home/user/tarot-dev');socket=Path('/run/user/10000/tarot-drm.sock');capture_dir=base/'artifacts/locker-ui'
    records={};screen=keyboard=harness=None;log=None
    def state(label,condition=lambda s:True,seconds=15):
        value=await_value(lambda:request(socket,'status'),condition,seconds=seconds);records[label]=value;return value
    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8'
        assert request(socket,'status')['pid']==compositor_pid
    def tap(x,y):screen.down(x,y);time.sleep(.05);screen.up();time.sleep(.08)
    def snapshot(pid,name):
        path=capture_dir/f'{pid}-{name}.png';await_value(path.exists,bool,seconds=20);return str(path)
    def authenticate(text):
        value=text.encode();payload=struct.pack('<H',len(value))+value
        result=subprocess.run([str(base/'bin/tarot-lock'),'--pam-worker'],input=payload,user=account.pw_uid,group=account.pw_gid,extra_groups=groups,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
        assert result.stdout==result.stderr==b'';return result.returncode
    try:
        # Direct helper checks prove actual account authentication independently
        # of input coordinates and the graphics client; no credential is logged.
        records['pam_wrong_rejected']=authenticate('tarot-fixture-wrong-password')==1
        records['pam_real_accepted']=authenticate(password)==0
        assert records['pam_wrong_rejected'] and records['pam_real_accepted']
        env=dict(os.environ,TAROT_DRM_SECONDS='180',TAROT_LOCAL_SESSION='1',TAROT_KEYBOARD='1',TAROT_LOCKER='1')
        log=(args.output/'harness.log').open('w');harness=subprocess.Popen(['sh',str(base/'phone-drm-smoke.sh')],env=env,stdout=log,stderr=subprocess.STDOUT,cwd=base)
        await_value(lambda:socket.exists() or harness.poll() is not None,bool);assert harness.poll() is None
        initial=state('protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['blank_verified'])
        assert set(initial)=={'ok','pid','lock'};assert initial['lock']['startup_reserved'] and initial['lock']['first_presented_protected'] is True and initial['lock']['unprotected_presentations']==0;compositor_pid=initial['pid'];locker_pid=initial['lock']['locker']['pid'];snapshot(locker_pid,'cover')
        screen=Touchscreen(guard);keyboard=Keyboard(guard)
        # Kill the real GL client while a rejected password is still in PAM.
        # Its private worker must die with it, and cannot later unlock anything.
        tap(180,608);snapshot(locker_pid,'entry-empty');keyboard.tap(16);keyboard.tap(28)
        def workers():
            children=Path(f'/proc/{locker_pid}/task/{locker_pid}/children')
            return [int(p) for p in children.read_text().split() if Path(f'/proc/{p}/cmdline').read_bytes().split(b'\0')[1:2]==[b'--pam-worker']]
        worker_pid=await_value(workers,bool,seconds=3)[0]
        os.kill(locker_pid,signal.SIGKILL)
        await_value(lambda:Path(f'/proc/{worker_pid}').exists(),lambda alive:not alive,seconds=4)
        records['inflight_worker_dies_with_locker']=True
        recovered=state('automatic-replacement',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['locker']['pid'] not in (None,locker_pid))
        assert recovered['lock']['orphaned_presentations']>0 and recovered['lock']['locker']['recoveries']>=1
        locker_pid=recovered['lock']['locker']['pid'];snapshot(locker_pid,'cover')
        tap(180,608);snapshot(locker_pid,'entry-empty')
        # Real physical-keyboard path: one synthetic wrong character and Enter.
        keyboard.tap(16);snapshot(locker_pid,'entry-masked');keyboard.tap(28)
        snapshot(locker_pid,'rejected');state('wrong-stays-private',lambda s:s['lock']['phase']=='Locked')
        for name,fields in [('overview',{}),('applications',{}),('capture',{'path':str(args.output/'denied.png')})]:
            result=any_request(socket,name,**fields);assert not result['ok'];records['denied-'+name]=result
        assert not (args.output/'denied.png').exists();time.sleep(3)
        # Correct text then Cancel must not authenticate or unlock. Escape/Cancel
        # resets the keyboard layout before a fresh complete touch entry.
        touch=TouchKeyboard(lambda x,y:tap(x,y+28));touch.type_ascii(password)
        tap(180,385);time.sleep(.15);assert request(socket,'status')['lock']['phase']=='Locked'
        records['correct_entry_cancel_retains_lock']=True
        tap(180,608);TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        final=state('authenticated',lambda s:s['lock']['phase']=='Unlocked',seconds=20)
        await_value(lambda:Path(f'/proc/{locker_pid}').exists(),lambda alive:not alive)
        assert final['lock']['phase']=='Unlocked';records['touch_password_authenticated']=True
        # Check that ordinary apps/chrome and their capture path recover.
        request(socket,'capture',path=str(args.output/'unlocked.png'));state('capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        first_cards=final['cards'];first_generation=final['lock']['generation'];old_locker_pid=locker_pid
        tap(310,14);state('menu-before-user-lock',lambda s:s['system_menu_open'] and s['system_page']=='Power')
        request(socket,'capture',path=str(args.output/'menu-before-lock.png'));state('menu-captured',lambda s:not s['capture_pending'] and not s['capture_error'])
        tap(180,366)
        user_locked=state('touch-menu-lock',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']>first_generation)
        locker_pid=user_locked['lock']['locker']['pid'];assert locker_pid!=old_locker_pid
        before=user_locked['lock']['generation'];request(socket,'lock');assert request(socket,'status')['lock']['generation']==before
        tap(180,608);snapshot(locker_pid,'entry-empty');TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        again=state('second-authenticated-unlock',lambda s:s['lock']['phase']=='Unlocked',seconds=20)
        assert again['cards']==first_cards and again['view']==final['view'] and not again['system_menu_open']
        await_value(lambda:Path(f'/proc/{locker_pid}').exists(),lambda alive:not alive)
        records['normal_touch_lock_and_second_authentication']=True
        records['validation']={'startup_protected_from_first_kms_frame':True,'normal_touch_lock_and_repeat_authentication':True,'automatic_recovery':True,'passed':True,'same_user_pam_authentication_and_account':True,'real_gles_lock_surface':True,'kms_presentation':True,'physical_wrong_rejected':True,'touch_correct_cancel_and_authenticate':True,'crash_trusted_recovery':True,'password_logged':False,'idle_suspend_packaging_complete':False}
        print('PASS: real unprivileged PAM, GL lock UI, physical wrong-password rejection, touch Cancel and authenticated unlock, crash recovery and normal capture restoration')
    finally:
        password=None
        if screen is not None:screen.close()
        if keyboard is not None:keyboard.close()
        if socket.exists():
            try:request(socket,'exit')
            except (OSError,AssertionError):pass
        if harness is not None:
            try:records['harness_exit']=harness.wait(timeout=45)
            except subprocess.TimeoutExpired:records['harness_exit']='timeout; inspect bounded watchdog'
        if log is not None:log.close()
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
