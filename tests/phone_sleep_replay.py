#!/usr/bin/env python3
"""Native KMS barriers with real elogind inhibitor/signals, without suspend.

Uses elogind 255's root-only SendPrepareForSleep signal helper. A detached,
bounded recovery process sends false if the replay is interrupted. No kernel
sleep request, daemon replacement, package or persistent policy change occurs.
"""
import argparse,ast,getpass,json,os,pwd,re,signal,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_touch_keyboard import Keyboard as TouchKeyboard


MANAGER=['gdbus','call','--system','--dest','org.freedesktop.login1','--object-path','/org/freedesktop/login1','--method']


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    base=Path('/home/user/tarot-dev');socket=Path('/run/user/10000/tarot-drm.sock');captures=base/'artifacts/locker-ui'
    assert not socket.exists() and not captures.exists()
    introspection=subprocess.check_output(['gdbus','introspect','--system','--dest','org.freedesktop.login1','--object-path','/org/freedesktop/login1'],text=True)
    assert 'SendPrepareForSleep' in introspection and 'PreparingForSleep = false' in introspection
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid)
    password=getpass.getpass('Device password for ephemeral sleep-barrier authentication test: ')
    assert password and all(' '<=c<='~' for c in password),'Replay supports printable ASCII only; password was not recorded'
    records={};harness=screen=log=recovery=None;pid=None
    def state(label,condition=lambda s:True,seconds=20):
        try:value=await_value(lambda:request(socket,'status'),condition,seconds=seconds)
        except AssertionError as error:records[label+'-failure']=error.args[0] if error.args else str(error);raise
        if pid is not None:assert value['pid']==pid
        records[label]=value;return value
    def power(label,condition):return state(label,lambda s:condition(s['lock']['sleep']))
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(socket,'status')['pid']==pid
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.08)
    def authenticate(label):
        locked=state(label+'-cover',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        locker=locked['lock']['locker']['pid'];await_value((captures/f'{locker}-cover.png').exists,bool)
        tap(180,608);await_value((captures/f'{locker}-entry-empty.png').exists,bool)
        TouchKeyboard(lambda x,y:tap(x,y+28)).type_ascii(password);tap(180,330)
        return state(label+'-authenticated',lambda s:s['lock']['phase']=='Unlocked')
    def inhibitors():
        value=subprocess.check_output(MANAGER+['org.freedesktop.login1.Manager.ListInhibitors'],text=True)
        value=re.sub(r'@a\(ssssuu\)\s*','',re.sub(r'uint32\s+','',value))
        return [entry for entry in ast.literal_eval(value)[0] if entry[1]=='Tarot']
    def armed(expected):
        found=await_value(inhibitors,lambda entries:bool(entries)==expected)
        if expected:assert len(found)==1 and found[0][0]=='sleep' and found[0][3]=='delay' and found[0][4:]==(10000,pid)
        return found
    def prepare(start):
        subprocess.run(MANAGER+['org.freedesktop.login1.Manager.SendPrepareForSleep',str(start).lower()],check=True,stdout=subprocess.DEVNULL,timeout=8)
    def vt(number):
        subprocess.run(['chvt',str(number)],check=True);await_value(lambda:Path('/sys/class/tty/tty0/active').read_text().strip(),lambda name:name=='tty'+str(number));time.sleep(.15)
    try:
        assert not inhibitors()
        # The root process is ephemeral; this detached guard survives loss of
        # the administrator's SSH terminal and exits after one false signal.
        fallback='sleep 90; exec gdbus call --system --dest org.freedesktop.login1 --object-path /org/freedesktop/login1 --method org.freedesktop.login1.Manager.SendPrepareForSleep false'
        recovery=subprocess.Popen(['sh','-c',fallback],start_new_session=True,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        env=dict(os.environ,TAROT_DRM_SECONDS='180',TAROT_LOCAL_SESSION='1',TAROT_KEYBOARD='1',TAROT_LOCKER='1',TAROT_SLEEP_COORDINATION='1',TAROT_LOCK_PROBE='0',TAROT_LOCK_RECOVERY='0')
        log=(args.output/'harness.log').open('w');harness=subprocess.Popen(['sh',str(base/'phone-drm-smoke.sh')],env=env,stdout=log,stderr=subprocess.STDOUT,cwd=base)
        await_value(lambda:socket.exists() or harness.poll() is not None,bool);assert harness.poll() is None
        initial=power('initial-armed',lambda s:s['available'] and s['armed'] and not s['initializing'] and s['gate']['barrier'] is None)
        pid=initial['pid'];assert initial['lock']['sleep']['physical_presentation'] and initial['lock']['first_presented_protected'] is True and initial['lock']['unprotected_presentations']==0;records['initial-real-inhibitor']=armed(True)
        screen=Touchscreen(guard);before=authenticate('startup')
        # Actual seat pause prevents KMS completion. A newly received prepare
        # signal must reserve protection but retain the real daemon's delay FD.
        vt(7);prepare(True)
        paused=power('prepare-awaits-real-kms',lambda s:s['gate']['preparing'] and s['gate']['barrier'] is not None and s['armed'])
        normal_count=paused['lock']['unprotected_presentations']
        assert set(paused)=={'ok','pid','lock'} and paused['lock']['phase']!='Unlocked';records['held-during-seat-pause']=armed(True)
        time.sleep(.3);assert request(socket,'status')['lock']['sleep']['gate']['protected_preparations']==0
        vt(8)
        prepared=power('prepare-kms-completed',lambda s:s['gate']['preparing'] and s['gate']['protected_preparations']==1 and not s['armed'])
        assert prepared['lock']['last_presented_generation']==prepared['lock']['generation'] and prepared['lock']['unprotected_presentations']==normal_count;records['released-after-kms']=armed(False)
        # Resume has a separate fresh-frame barrier. Its next-cycle inhibitor
        # must stay open when that resume frame completes.
        vt(7);prepare(False)
        paused=power('resume-awaits-real-kms',lambda s:not s['gate']['preparing'] and s['gate']['barrier'] is not None and s['armed'])
        assert paused['lock']['sleep']['gate']['barrier']['purpose']=='Resume';records['rearmed-before-resume-frame']=armed(True)
        vt(8);power('resume-kms-completed',lambda s:not s['gate']['preparing'] and s['gate']['barrier'] is None and s['armed']);armed(True)
        final=authenticate('after-signaled-resume');assert final['cards']==before['cards'] and final['view']==before['view']
        request(socket,'capture',path=str(args.output/'normal-restored.png'));state('normal-capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
        records['validation']={'passed':True,'real_elogind_delay_inhibitor_uid_10000':True,'real_authoritative_elogind_signals':True,'prepare_fd_held_across_inactive_vt':True,'released_only_after_new_protected_kms':True,'resume_barrier_and_next_cycle_inhibitor':True,'real_pam_before_and_after':True,'cards_view_capture_restored':True,'actual_kernel_suspend_tested':False,'password_logged':False}
        print('PASS: real elogind inhibitor/signals, paused-seat KMS barriers, release/rearm, real PAM and original session; no kernel suspend invoked')
    finally:
        password=None
        # Always undo the emitted prepare signal, even after an assertion.
        cleanup_sent=False
        try:prepare(False);cleanup_sent=True
        except (OSError,subprocess.SubprocessError) as error:records['resume_signal_cleanup_error']=str(error)
        if cleanup_sent and recovery is not None and recovery.poll() is None:os.killpg(recovery.pid,signal.SIGTERM);recovery.wait(timeout=3)
        if screen is not None:screen.close()
        if socket.exists():
            try:request(socket,'exit')
            except (OSError,AssertionError):pass
        if harness is not None:
            try:records['harness_exit']=harness.wait(timeout=45)
            except subprocess.TimeoutExpired:records['harness_exit']='timeout; inspect bounded watchdog'
        if log is not None:log.close()
        records['final_tarot_inhibitors']=armed(False)
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')


if __name__=='__main__':main()
