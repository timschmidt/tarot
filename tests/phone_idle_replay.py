#!/usr/bin/env python3
"""Bounded real panel off/on, wake-input isolation, inhibitors and real PAM.

Root creates ephemeral input devices and emits one elogind signal-only cycle.
Compositor/apps/locker/PAM remain UID 10000. No kernel suspend is invoked.
"""
import argparse,getpass,json,os,pwd,signal,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_lock_replay import Keyboard
from phone_touch_keyboard import Keyboard as TouchKeyboard
from lock_replay import append,events,any_request
from phone_sleep_replay import MANAGER


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    base=Path('/home/user/tarot-dev');socket=Path('/run/user/10000/tarot-drm.sock');captures=base/'artifacts/locker-ui'
    panels=[p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip()=='connected'];assert len(panels)==1
    assert not socket.exists() and not captures.exists()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid);groups=os.getgrouplist('user',account.pw_gid)
    password=getpass.getpass('Device password for ephemeral idle/panel authentication test: ')
    assert password and all(' '<=c<='~' for c in password),'Replay supports printable ASCII only; password was not recorded'
    records={};screen=keyboard=harness=app=log=app_log=guard_process=None;pid=None;prepare_sent=False
    report=args.output/'app-events.jsonl';commands=args.output/'app-commands.jsonl'
    def state(label,condition=lambda s:True,seconds=20):
        try:value=await_value(lambda:request(socket,'status'),condition,seconds=seconds)
        except AssertionError as e:records[label+'-failure']=e.args[0] if e.args else str(e);raise
        if pid is not None:assert value['pid']==pid
        records[label]=value;return value
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(socket,'status')['pid']==pid
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.08)
    def panel(label,off):
        def read():return {'dpms':(panels[0]/'dpms').read_text().strip(),'backlight_power':int(Path('/sys/class/backlight/backlight/bl_power').read_text()),'brightness':int(Path('/sys/class/backlight/backlight/brightness').read_text())}
        value=await_value(read,lambda s:s['dpms']==('Off' if off else 'On') and s['backlight_power']==(4 if off else 0));records[label]=value;return value
    def off(label):
        value=state(label,lambda s:s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase']=='Off')
        assert set(value)=={'ok','pid','lock'} and value['lock']['phase']!='Unlocked' and value['lock']['last_presented_generation']==value['lock']['generation'];panel(label+'-driver',True);return value
    def awake(label):
        value=state(label,lambda s:not s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase']=='Awake');panel(label+'-driver',False);return value
    def authenticate(label):
        covered=state(label+'-cover',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['idle']['policy']['phase']=='Awake')
        locker=covered['lock']['locker']['pid'];await_value((captures/f'{locker}-cover.png').exists,bool)
        tap(180,608);await_value((captures/f'{locker}-entry-empty.png').exists,bool)
        TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        return state(label+'-authenticated',lambda s:s['lock']['phase']=='Unlocked')
    def count(name):return sum(e['event']==name for e in events(report))
    def clipboard(label,allowed):
        before=count('clipboard-read');append(commands,'read-clipboard');value=await_value(lambda:[e for e in events(report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1];assert value['fields']['matches']==allowed;records[label]=value
    def cover_untouched():
        current=request(socket,'status');locker=current['lock']['locker']['pid'];assert current['lock']['phase']=='Locked' and not (captures/f'{locker}-entry-empty.png').exists()
    try:
        commands.write_text('');os.chown(commands,account.pw_uid,account.pw_gid)
        env=dict(os.environ,TAROT_DRM_SECONDS='180',TAROT_LOCAL_SESSION='1',TAROT_KEYBOARD='1',TAROT_LOCKER='1',TAROT_SLEEP_COORDINATION='1',TAROT_IDLE_PANEL='1',TAROT_LOCK_PROBE='0',TAROT_LOCK_RECOVERY='0')
        log=(args.output/'harness.log').open('w');harness=subprocess.Popen(['sh',str(base/'phone-drm-smoke.sh')],env=env,stdout=log,stderr=subprocess.STDOUT,cwd=base)
        await_value(lambda:socket.exists() or harness.poll() is not None,bool);assert harness.poll() is None
        initial=state('initial',lambda s:s['lock']['phase']=='Locked' and s['lock']['sleep']['available'] and not s['lock']['sleep']['initializing'] and s['lock']['sleep']['gate']['barrier'] is None)
        pid=initial['pid'];assert initial['lock']['first_presented_protected'] is True and initial['lock']['unprotected_presentations']==0;panel('initial-driver',False)
        screen=Touchscreen(guard);keyboard=Keyboard(guard);first=authenticate('startup')
        for card in first['cards']:request(socket,'close',id=card['id'])
        state('empty',lambda s:not s['cards'])
        app_env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR=str(socket.parent),WAYLAND_DISPLAY=first['wayland_display']);app_env.pop('WAYLAND_SOCKET',None);app_env.pop('DISPLAY',None)
        app_log=(args.output/'app.log').open('w');app=subprocess.Popen([str(base/'bin/lock-probe'),'--role','app','--report',str(report),'--commands',str(commands)],env=app_env,user=account.pw_uid,group=account.pw_gid,extra_groups=groups,stdout=app_log,stderr=app_log,stdin=subprocess.DEVNULL,cwd=account.pw_dir)
        before=state('app-active',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view']);append(commands,'inhibit')
        state('visible-inhibitor',lambda s:s['lock']['idle']['policy']['inhibited']);time.sleep(10.7)
        assert state('inhibitor-survives-both-deadlines')['lock']['phase']=='Unlocked';panel('inhibited-driver',False)
        append(commands,'cover');state('opaque-child-occludes-native-inhibitor',lambda s:not s['lock']['idle']['policy']['inhibited'])
        append(commands,'uncover');state('uncover-restores-native-inhibitor',lambda s:s['lock']['idle']['policy']['inhibited'])
        subprocess.run(['chvt','7'],check=True);state('inactive-seat-ignores-inhibitor',lambda s:not s['lock']['idle']['policy']['inhibited'])
        subprocess.run(['chvt','8'],check=True);state('active-seat-restores-inhibitor',lambda s:s['lock']['idle']['policy']['inhibited']);panel('returned-seat-driver',False)
        await_value(lambda:events(report),lambda es:any(e['event']=='clipboard-offered' for e in es));clipboard('clipboard-before',True)
        append(commands,'uninhibit');state('inhibitor-released',lambda s:not s['lock']['idle']['policy']['inhibited'])
        keyboard.key(28,True);screen.down(180,350);await_value(lambda:(count('key'),count('touch-down')),lambda v:v[0]>0 and v[1]>0);keys=count('key');ups=count('touch-up')
        locked=state('automatic-idle-lock',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        await_value(lambda:events(report),lambda es:any(e['event']=='touch-cancel' for e in es));first_off=off('first-panel-off')
        frames=first_off['lock']['presented_frames'];time.sleep(.5);assert request(socket,'status')['lock']['presented_frames']==frames
        screen.up();keyboard.key(28,False);time.sleep(.15);assert request(socket,'status')['lock']['idle']['output_off'];assert count('key')==keys and count('touch-up')==ups
        assert any_request(socket,'capture',path=str(args.output/'denied.png'))=={'ok':False,'error':'Session is locked'};assert not (args.output/'denied.png').exists();clipboard('off-clipboard-private',False)
        # A puck contact used only to wake must not also open private entry.
        screen.down(180,608);awake('touch-wake-frame');screen.up();time.sleep(.15);cover_untouched()
        restored=authenticate('touch-wake');assert restored['cards']==before['cards'] and restored['view']==before['view'];clipboard('touch-wake-offer-restored',True)
        off('second-panel-off');keyboard.key(28,True);awake('key-wake-frame');keyboard.key(28,False);time.sleep(.15);cover_untouched();authenticate('key-wake')
        # Destroy the waking touchscreen while its first contact is held. A new
        # device must supply a fresh contact before private entry is possible.
        off('third-panel-off');screen.down(180,608);screen.close();screen=None;awake('removed-device-wake');time.sleep(.15);cover_untouched();screen=Touchscreen(guard);authenticate('replacement-device')
        # A system sleep boundary also wakes a powered-off panel to complete its
        # own fresh KMS barrier; it still cannot authenticate or expose apps.
        off('fourth-panel-off')
        fallback='sleep 40; exec gdbus call --system --dest org.freedesktop.login1 --object-path /org/freedesktop/login1 --method org.freedesktop.login1.Manager.SendPrepareForSleep false'
        guard_process=subprocess.Popen(['sh','-c',fallback],start_new_session=True,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        prepare_sent=True;subprocess.run(MANAGER+['org.freedesktop.login1.Manager.SendPrepareForSleep','true'],check=True,stdout=subprocess.DEVNULL,timeout=8)
        state('sleep-prepare-wakes-protected-panel',lambda s:s['lock']['sleep']['gate']['protected_preparations']==1 and not s['lock']['sleep']['armed'] and s['lock']['idle']['policy']['phase']=='Awake');panel('system-wake-driver',False)
        subprocess.run(MANAGER+['org.freedesktop.login1.Manager.SendPrepareForSleep','false'],check=True,stdout=subprocess.DEVNULL,timeout=8);prepare_sent=False
        state('sleep-resume-rearmed',lambda s:s['lock']['sleep']['armed'] and s['lock']['sleep']['gate']['barrier'] is None and not s['lock']['sleep']['gate']['preparing'])
        final=authenticate('system-wake');assert final['cards']==before['cards'] and final['view']==before['view'];clipboard('final-original-offer',True)
        key_count=count('key');up_count=count('touch-up');keyboard.tap(16);tap(180,350);await_value(lambda:(count('key'),count('touch-up')),lambda v:v[0]>key_count and v[1]>up_count)
        request(socket,'capture',path=str(args.output/'normal-restored.png'));state('normal-capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        append(commands,'exit');assert app.wait(timeout=4)==0
        records['validation']={'passed':True,'native_visible_idle_inhibitor':True,'opaque_occlusion_and_inactive_seat_ignore_inhibitor':True,'timed_lock_and_real_panel_off':True,'driver_dpms_and_backlight_readback':True,'no_compositor_presentations_while_off':True,'held_releases_do_not_wake_or_reach_app':True,'wake_touch_and_key_consumed_until_new_frame':True,'removed_wake_device_fresh_contact':True,'sleep_boundary_wakes_protected_panel':True,'real_pam_after_each_wake':True,'same_app_view_original_clipboard_restored':True,'password_logged':False,'kernel_suspend_tested':False,'physical_mouse_tested':False}
        print('PASS: native visible inhibitor, timed protected panel off, driver readback, held/touch/key/device-loss wake isolation, sleep boundary, real PAM and original app/clipboard')
    finally:
        password=None;cleanup_sent=not prepare_sent
        if prepare_sent:
            try:subprocess.run(MANAGER+['org.freedesktop.login1.Manager.SendPrepareForSleep','false'],check=True,stdout=subprocess.DEVNULL,timeout=8);cleanup_sent=True
            except (OSError,subprocess.SubprocessError) as error:records['resume_cleanup_error']=str(error)
        if cleanup_sent and guard_process is not None and guard_process.poll() is None:os.killpg(guard_process.pid,signal.SIGTERM);guard_process.wait(timeout=3)
        if screen is not None:screen.close()
        if keyboard is not None:keyboard.close()
        if app is not None and app.poll() is None:
            app.terminate()
            try:app.wait(timeout=3)
            except subprocess.TimeoutExpired:app.kill();app.wait()
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
