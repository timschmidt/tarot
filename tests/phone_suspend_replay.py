#!/usr/bin/env python3
"""Actual RTC-woken elogind suspend cycles with Tarot KMS/PAM/input checks.

Authenticate at the hidden terminal prompt, then --detach keeps the ephemeral
test alive across USB pauses. No credential is stored in files/argv/environment.
"""
import argparse,getpass,hashlib,json,os,pwd,signal,subprocess,sys,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_lock_replay import Keyboard
from phone_touch_keyboard import Keyboard as TouchKeyboard
from phone_suspend_probe import RTC,clocks,stats,kernel_lines
from lock_replay import append,events,any_request


def run(args,password,account):
    base=Path('/home/user/tarot-dev');socket=Path('/run/user/10000/tarot-drm.sock');captures=base/'artifacts/locker-ui';groups=os.getgrouplist('user',account.pw_gid)
    records={'pid':os.getpid(),'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'cycles':[],'kernel_before':kernel_lines()};screen=keyboard=harness=app=log=app_log=None;pid=None;owned_alarm=None
    expected_profile=json.loads((base/'artifacts/persistence-restore-final/status.json').read_text())['original_profile_hash']
    report=args.output/'app-events.jsonl';commands=args.output/'app-commands.jsonl'
    def save(final=False):
        (args.output/('status.json' if final else 'running.json')).write_text(json.dumps(records,indent=2)+'\n')
        if final:(args.output/'running.json').unlink(missing_ok=True)
    def state(label,condition=lambda s:True,seconds=25):
        try:value=await_value(lambda:request(socket,'status'),condition,seconds=seconds)
        except AssertionError as e:records[label+'-failure']=e.args[0] if e.args else str(e);raise
        if pid is not None:assert value['pid']==pid
        records[label]=value;return value
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(socket,'status')['pid']==pid
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.08)
    def authenticate(label):
        covered=state(label+'-cover',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        locker=covered['lock']['locker']['pid'];await_value((captures/f'{locker}-cover.png').exists,bool)
        tap(180,608);await_value((captures/f'{locker}-entry-empty.png').exists,bool)
        TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        state(label+'-authenticated',lambda s:s['lock']['phase']=='Unlocked')
        # Authentication can finish before the first ordinary KMS frame. An
        # app touch must target an actually restored scene, not the lock frame
        # whose surface placements were deliberately cleared for protection.
        value=state(label+'-ordinary-presented',lambda s:s['lock']['phase']=='Unlocked' and s['lock']['unprotected_presentations']>covered['lock']['unprotected_presentations'])
        await_value(lambda:Path(f'/proc/{locker}').exists(),lambda alive:not alive);return value
    def count(name):return sum(e['event']==name for e in events(report))
    def clipboard(label,allowed):
        assert app.poll() is None,'Synthetic app exited before clipboard probe'
        before=count('clipboard-read');append(commands,'read-clipboard');value=await_value(lambda:[e for e in events(report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1];assert value['fields']['matches']==allowed;records[label]=value
    def clear_alarm():
        nonlocal owned_alarm
        current=(RTC/'wakealarm').read_text().strip()
        if current and current==owned_alarm:(RTC/'wakealarm').write_text('0')
        owned_alarm=None
    def network():
        profile=subprocess.check_output(['nmcli','-g','GENERAL.CON-UUID','device','show','wlan0'],text=True,timeout=5).strip()
        devices=subprocess.check_output(['nmcli','-t','-f','DEVICE,STATE','device'],text=True,timeout=5)
        return {'original_profile':hashlib.sha256(profile.encode()).hexdigest()==expected_profile,'wifi_connected':'wlan0:connected' in devices,'usb_connected':'usb0:connected' in devices}
    save()
    try:
        commands.write_text('');os.chown(commands,account.pw_uid,account.pw_gid)
        records['active_harness_bound_seconds']=args.harness_seconds;records['rtc_window_seconds']=args.seconds
        records['network_before']=await_value(network,lambda n:all(n.values()),seconds=45)
        env=dict(os.environ,TAROT_DRM_SECONDS=str(args.harness_seconds),TAROT_SUSPEND_RELIABILITY='1' if args.harness_seconds>300 else '0',TAROT_LOCAL_SESSION='1',TAROT_KEYBOARD='1',TAROT_LOCKER='1',TAROT_SLEEP_COORDINATION='1',TAROT_IDLE_PANEL='0',TAROT_LOCK_PROBE='0',TAROT_LOCK_RECOVERY='0')
        log=(args.output/'harness.log').open('w');harness=subprocess.Popen(['sh',str(base/'phone-drm-smoke.sh')],env=env,stdout=log,stderr=subprocess.STDOUT,cwd=base)
        await_value(lambda:socket.exists() or harness.poll() is not None,bool);assert harness.poll() is None
        initial=state('initial',lambda s:s['lock']['phase']=='Locked' and s['lock']['sleep']['available'] and s['lock']['sleep']['armed'] and not s['lock']['sleep']['initializing'] and s['lock']['sleep']['gate']['barrier'] is None)
        pid=initial['pid'];assert initial['lock']['first_presented_protected'] is True and initial['lock']['unprotected_presentations']==0
        screen=Touchscreen(guard);keyboard=Keyboard(guard);first=authenticate('startup')
        for card in first['cards']:request(socket,'close',id=card['id'])
        state('empty',lambda s:not s['cards'])
        env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR=str(socket.parent),WAYLAND_DISPLAY=first['wayland_display']);env.pop('WAYLAND_SOCKET',None);env.pop('DISPLAY',None)
        records['app_bound_seconds']=args.harness_seconds
        app_log=(args.output/'app.log').open('w');app=subprocess.Popen([str(base/'bin/lock-probe'),'--role','app','--report',str(report),'--commands',str(commands),'--exit-after-seconds',str(args.harness_seconds)],env=env,user=account.pw_uid,group=account.pw_gid,extra_groups=groups,stdout=app_log,stderr=app_log,stdin=subprocess.DEVNULL,cwd=account.pw_dir)
        before=state('app-active',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view']);await_value(lambda:events(report),lambda es:any(e['event']=='clipboard-offered' for e in es));clipboard('clipboard-before',True)
        for index in range(1,args.cycles+1):
            folder=args.output/f'cycle-{index:02}';folder.mkdir(mode=0o700);os.chown(folder,account.pw_uid,account.pw_gid)
            cycle={'number':index,'before_stats':stats(),'before_clocks':clocks(),'before_status':request(socket,'status')};records['cycles'].append(cycle);records['stage']=f'cycle-{index:02}-preparing';save()
            if index==1:
                keyboard.key(28,True);screen.down(180,350);await_value(lambda:(count('key'),count('touch-down')),lambda v:v[0]>0 and v[1]>0);cycle['held_before']={'keys':count('key'),'ups':count('touch-up')}
            assert not (RTC/'wakealarm').read_text().strip()
            with (folder/'rtcwake.log').open('w') as alarm_log:
                result=subprocess.run(['rtcwake','-u','-d','/dev/rtc0','-m','no','-s',str(args.seconds),'-v'],stdout=alarm_log,stderr=subprocess.STDOUT,timeout=4)
            assert result.returncode==0;owned_alarm=(RTC/'wakealarm').read_text().strip();assert owned_alarm;cycle['owned_alarm']=owned_alarm;save()
            helper_report=folder/'observer.jsonl'
            with (folder/'helper.log').open('w') as helper_log:
                result=subprocess.run([str(base/'bin/suspend-request'),'--control-socket',str(socket),'--pid',str(pid),'--report',str(helper_report)],stdout=helper_log,stderr=subprocess.STDOUT,timeout=50)
            cycle['helper_exit']=result.returncode
            if helper_report.exists():os.chown(helper_report,account.pw_uid,account.pw_gid);cycle['observer']=[json.loads(line) for line in helper_report.read_text().splitlines()]
            cycle['after_stats']=stats();cycle['after_clocks']=clocks();save()
            assert result.returncode==0
            proof=cycle['observer'][0];assert proof['observed'] is not None and proof['observation_error'] is None and proof['elapsed_boottime_us']<2_000_000
            assert cycle['after_stats']['success']==cycle['before_stats']['success']+1 and cycle['after_stats']['fail']==cycle['before_stats']['fail']
            mono=cycle['after_clocks']['monotonic']-cycle['before_clocks']['monotonic'];boot=cycle['after_clocks']['boottime']-cycle['before_clocks']['boottime'];cycle['suspended_seconds']=boot-mono;assert cycle['suspended_seconds']>=3
            clear_alarm();cycle['alarm_cleared']=not (RTC/'wakealarm').read_text().strip();assert cycle['alarm_cleared']
            prior=cycle['before_status']['lock']['sleep']['gate']
            resumed=state(f'cycle-{index:02}-protected-resume',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['sleep']['armed'] and not s['lock']['sleep']['gate']['preparing'] and s['lock']['sleep']['gate']['barrier'] is None and s['lock']['sleep']['gate']['protected_preparations']>prior['protected_preparations'] and s['lock']['sleep']['gate']['protected_resumes']>prior['protected_resumes'])
            assert set(resumed)=={'ok','pid','lock'} and resumed['lock']['last_presented_generation']==resumed['lock']['generation']
            assert resumed['lock']['sleep']['gate']['epoch']==prior['epoch'] and not resumed['lock']['sleep']['rearming'],'Resume must re-arm without replacing the manager connection'
            if index==1:
                await_value(lambda:events(report),lambda es:any(e['event']=='touch-cancel' for e in es));screen.up();keyboard.key(28,False);time.sleep(.15)
                assert count('key')==cycle['held_before']['keys'] and count('touch-up')==cycle['held_before']['ups']
                assert not (captures/f"{resumed['lock']['locker']['pid']}-entry-empty.png").exists();cycle['held_release_suppressed']=True
            clipboard(f'cycle-{index:02}-private-cached-offer',False)
            assert any_request(socket,'capture',path=str(folder/'denied.png'))=={'ok':False,'error':'Session is locked'};assert not (folder/'denied.png').exists()
            after=authenticate(f'cycle-{index:02}');assert after['cards']==before['cards'] and after['view']==before['view'];clipboard(f'cycle-{index:02}-restored-offer',True)
            records['stage']=f'cycle-{index:02}-restored-input';save()
            request(socket,'capture',path=str(folder/'normal-restored.png'));state(f'cycle-{index:02}-capture',lambda s:not s['capture_pending'] and not s['capture_error'])
            keys=count('key');downs=count('touch-down');ups=count('touch-up');keyboard.tap(16)
            screen.down(180,350)
            await_value(lambda:count('touch-down'),lambda n:n>downs)
            screen.up()
            await_value(lambda:(count('key'),count('touch-up')),lambda v:v[0]>keys and v[1]>ups)
            cycle['fresh_app_input']={'key_events':count('key')-keys,'touch_downs':count('touch-down')-downs,'touch_ups':count('touch-up')-ups}
            cycle['network_restored']=await_value(network,lambda n:all(n.values()),seconds=45)
            cycle['validated']=True;cycle['mem_sleep']=Path('/sys/power/mem_sleep').read_text().strip();records['stage']=f'cycle-{index:02}-complete';save()
        append(commands,'exit');records['app_exit']=app.wait(timeout=4);assert records['app_exit']==0
        records['kernel_after']=kernel_lines();records['validation']={'passed':True,'actual_kernel_suspend_cycles':args.cycles,'rtc_wake':True,'independent_delay_observer_before_kernel_sleep':True,'matching_protected_kms_and_resume_rearm':True,'same_manager_epoch_each_resume':True,'held_key_touch_crossing_suppressed':True,'fresh_uinput_touch_pam_each_resume':True,'same_app_view_cached_offer_each_cycle':True,'fresh_native_key_touch_and_capture_each_cycle':True,'original_wifi_profile_and_usb_each_cycle':True,'manual_touch_sensor_tested':False,'password_logged':False,'carrier_wake_tested':False,'power_key_wake_tested':False}
        print('PASS:',args.cycles,'actual RTC-woken suspend cycles, pre-sleep KMS proof, protected resume, real touch/PAM and original app/clipboard',flush=True)
    except BaseException as error:
        records['app_exit_at_failure']=app.poll() if app is not None else None
        try:records['failure_status']=request(socket,'status') if socket.exists() else None
        except (OSError,AssertionError):records['failure_status']=None
        records['validation']={'passed':False,'completed_cycles':sum(c.get('validated',False) for c in records['cycles']),'failure_stage':records.get('stage'),'exception_type':type(error).__name__}
        raise
    finally:
        password=None
        # If a helper failed before its request settled, preserve its recovery
        # alarm until that deadline has passed; never cancel a pending wake.
        if owned_alarm:
            deadline=int(owned_alarm)+3
            while stats()['success']==records['cycles'][-1]['before_stats']['success'] and stats()['fail']==records['cycles'][-1]['before_stats']['fail'] and time.time()<deadline:time.sleep(.1)
            clear_alarm()
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
        records['alarm_after_cleanup']=(RTC/'wakealarm').read_text().strip();save(True)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--stock-evidence',type=Path,required=True);p.add_argument('--cycles',type=int,default=1);p.add_argument('--seconds',type=int,default=15);p.add_argument('--detach',action='store_true');args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text() and 1<=args.cycles<=20 and 8<=args.seconds<=20
    args.harness_seconds=300 if args.cycles==1 else 900
    assert args.cycles*(args.seconds+15)+60<args.harness_seconds,'Run must fit the bounded native harness'
    assert not Path('/run/user/10000/tarot-drm.sock').exists() and not Path('/home/user/tarot-dev/artifacts/locker-ui').exists() and not (RTC/'wakealarm').read_text().strip()
    stock=json.loads(args.stock_evidence.read_text());assert stock['validation']['passed'] and stock['validation']['kernel_suspend_invoked'] and stock['before']['boot_id']==Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False);account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid)
    password=getpass.getpass('Device password for ephemeral real-suspend/PAM test: ');assert password and all(' '<=c<='~' for c in password),'Printable ASCII test entry only; no password recorded'
    signal.signal(signal.SIGHUP,signal.SIG_IGN)
    if args.detach:
        child=os.fork()
        if child:
            password=None;print('Detached Tarot suspend replay PID',child,flush=True);return
        os.setsid();null=os.open('/dev/null',os.O_RDONLY);os.dup2(null,0);os.close(null)
        log=os.open(args.output/'controller.log',os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.chown(args.output/'controller.log',account.pw_uid,account.pw_gid);os.dup2(log,1);os.dup2(log,2);os.close(log)
    run(args,password,account)


if __name__=='__main__':main()
