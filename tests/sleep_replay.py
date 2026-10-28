#!/usr/bin/env python3
"""Real private-bus sleep inhibitors and protected presentation/handoff.

Never invokes Suspend and never connects to the host's system bus.
"""
import argparse,json,os,signal,subprocess,tempfile,time
from pathlib import Path
from notification_replay import Fixture,await_value
from phone_gesture_replay import request
from lock_replay import append,events,any_request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);root=Path.cwd()
    runtime=tempfile.TemporaryDirectory(prefix='tarot-sleep-');address='unix:path='+str(Path(runtime.name)/'bus');control=Path(runtime.name)/'control.sock'
    env=dict(os.environ,DBUS_SESSION_BUS_ADDRESS=address,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    report=out/'locker-events.jsonl';commands=out/'locker-commands.jsonl';commands.write_text('');app_report=out/'app-events.jsonl';app_commands=out/'app-commands.jsonl';app_commands.write_text('')
    records={};bus=fixture=session=app=None;logs=[];stopped=False
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def start_bus(name):
        proc=subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address,'--print-address=1'],stdout=subprocess.PIPE,stderr=log(name),text=True)
        assert proc.stdout.readline().startswith(address);return proc
    def state(label,condition=lambda s:True,seconds=15):
        try:value=await_value(lambda:request(control,'status'),condition,seconds=seconds)
        except AssertionError as e:records[label+'-failure']=e.args[0] if e.args else str(e);raise
        records[label]=value;return value
    def power(label,condition):return state(label,lambda s:condition(s['lock']['sleep']))
    def closed(identifier):return any(e['event']=='closed' and e['id']==identifier and e['eof'] for e in fixture.events())
    def latest_fd():return max(e['id'] for e in fixture.events() if e['event']=='inhibit')
    def own_lock():
        value=state('current-owner',lambda s:s['lock']['locker']['pid'] is not None);pid=value['lock']['locker']['pid']
        await_value(lambda:events(report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es))
        if value['lock']['phase']!='Locked':append(commands,'lock')
        return state('own-lock',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'])
    def unlock(label):
        own_lock();append(commands,'unlock');return state(label,lambda s:s['lock']['phase']=='Unlocked')
    def clipboard(label,allowed):
        before=sum(e['event']=='clipboard-read' for e in events(app_report));append(app_commands,'read-clipboard')
        value=await_value(lambda:[e for e in events(app_report) if e['event']=='clipboard-read'],lambda es:len(es)>before)[-1]
        assert value['fields']['matches']==allowed;records[label]=value
    try:
        bus=start_bus('bus-first.log');fixture=Fixture(root/'target/debug/examples/sleep-probe',out/'service-first',env)
        command=[str(root/'target/debug/tarot'),'--system-services','--platform-bus',address,'--sleep-coordination','--lock-client',str(root/'target/debug/examples/lock-probe'),'--verify-lock-blank','--control-socket',str(control),'--exit-after-ms','90000']
        for arg in ['--role','locker','--report',str(report),'--commands',str(commands)]:command.append('--lock-client-arg='+arg)
        session=subprocess.Popen(command,env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        ready=power('armed-initial',lambda s:s['available'] and s['armed'] and not s['initializing'] and s['gate']['barrier'] is None)
        assert not ready['lock']['sleep']['physical_presentation'];fd=latest_fd();assert not closed(fd)
        initial=unlock('initial-unlocked');app_env=dict(env,WAYLAND_DISPLAY=initial['wayland_display'])
        app=subprocess.Popen([str(root/'target/debug/examples/lock-probe'),'--role','app','--report',str(app_report),'--commands',str(app_commands)],env=app_env,stdout=log('app.log'),stderr=subprocess.STDOUT)
        before=state('app-active',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view']);await_value(lambda:events(app_report),lambda es:any(e['event']=='clipboard-offered' for e in es));clipboard('clipboard-before',True)
        fixture.call('spoof',start=True);time.sleep(.25);assert state('foreign-signal-ignored')['lock']['phase']=='Unlocked' and not closed(fd)
        # An actual queued D-Bus signal cannot itself close the delay FD while
        # the compositor is stopped and cannot submit/complete a protected frame.
        os.kill(session.pid,signal.SIGSTOP);stopped=True;fixture.call('prepare',start=True);time.sleep(.3);assert not closed(fd)
        os.kill(session.pid,signal.SIGCONT);stopped=False
        prepared=power('prepare-protected',lambda s:s['gate']['preparing'] and s['gate']['protected_preparations']==1 and not s['armed'])
        assert prepared['lock']['phase']!='Unlocked' and prepared['lock']['last_presented_generation']==prepared['lock']['generation'];await_value(lambda:closed(fd),bool)
        assert set(prepared)=={'ok','pid','lock'};clipboard('prepare-cached-offer-blocked',False)
        assert any_request(control,'capture',path=str(out/'denied.png'))=={'ok':False,'error':'Session is locked'};assert not (out/'denied.png').exists()
        own_lock();append(commands,'unlock');time.sleep(.2);assert state('unlock-during-preparation-refused')['lock']['phase']!='Unlocked'
        fixture.call('prepare',start=False)
        resumed=power('resume-presented-and-rearmed',lambda s:not s['gate']['preparing'] and s['gate']['barrier'] is None and s['armed'] and s['gate']['protected_resumes']>=2)
        second=latest_fd();assert second>fd and not closed(second);fixture.call('prepare',start=False);time.sleep(.2);assert latest_fd()==second and not closed(second)
        after=unlock('first-restored');assert after['cards']==before['cards'] and after['view']==before['view'];clipboard('original-offer-restored',True)
        fixture.call('prepare',start=True);power('second-cycle-protected',lambda s:s['gate']['protected_preparations']==2 and not s['armed']);await_value(lambda:closed(second),bool)
        fixture.call('resume-busy',milliseconds=1800)
        fixture.call('prepare',start=False)
        retrying=power('resume-busy-same-owner',lambda s:s['rearming'] and not s['armed'] and not s['gate']['preparing'] and s['gate']['barrier'] is None)
        epoch=retrying['lock']['sleep']['gate']['epoch'];assert retrying['lock']['phase']!='Unlocked'
        own_lock();append(commands,'unlock');time.sleep(.2)
        refused=state('unlock-during-rearm-refused');assert refused['lock']['phase']!='Unlocked' and refused['lock']['sleep']['rearming']
        rearmed=power('second-cycle-resume',lambda s:s['armed'] and not s['rearming'] and s['gate']['barrier'] is None and not s['gate']['preparing'])
        assert rearmed['lock']['sleep']['gate']['epoch']==epoch and any(e['event']=='inhibit-busy' for e in fixture.events())
        fixture.call('resume-busy',milliseconds=0)
        third=latest_fd();assert not closed(third)
        fixture.call('resume-busy',milliseconds=5000)
        fixture.call('prepare',start=True);power('retry-timeout-prepared',lambda s:s['gate']['preparing'] and not s['armed']);await_value(lambda:closed(third),bool)
        fixture.call('prepare',start=False)
        power('retry-deadline-fails-closed',lambda s:not s['available'] and s['initializing'] and s['error'] is not None)
        assert state('retry-deadline-protected')['lock']['phase']!='Unlocked'
        recovered=power('retry-deadline-recovered',lambda s:s['available'] and s['armed'] and not s['initializing'] and not s['rearming'] and s['gate']['barrier'] is None)
        assert recovered['lock']['sleep']['gate']['epoch']>epoch
        fixture.call('resume-busy',milliseconds=0);third=latest_fd();assert not closed(third)
        fixture.call('release');power('owner-lost-stays-private',lambda s:not s['available'] and s['initializing']);await_value(lambda:closed(third),bool)
        fixture.call('deny',deny=True);fixture.call('acquire');time.sleep(.6);denied=power('inhibit-denial-unavailable',lambda s:not s['available'] and s['error'] is not None);assert denied['lock']['phase']!='Unlocked'
        fixture.call('deny',deny=False);power('owner-reacquired',lambda s:s['available'] and s['armed'] and not s['initializing'] and s['gate']['barrier'] is None)
        fixture.close();fixture=None;bus.terminate();assert bus.wait(timeout=4)==0
        power('whole-bus-lost',lambda s:not s['available'] and s['initializing'])
        bus=start_bus('bus-second.log');fixture=Fixture(root/'target/debug/examples/sleep-probe',out/'service-second',env)
        power('whole-bus-replaced',lambda s:s['available'] and s['armed'] and not s['initializing'] and s['gate']['barrier'] is None)
        final=unlock('final-restored');assert final['cards']==before['cards'] and final['view']==before['view'];clipboard('final-cached-offer',True)
        append(app_commands,'exit');assert app.wait(timeout=4)==0
        last=latest_fd();assert not closed(last);request(control,'exit');records['compositor_exit']=session.wait(timeout=8);assert records['compositor_exit']==0;await_value(lambda:closed(last),bool)
        # A client connecting during an existing preparation must discover the
        # property even though its signal was sent before the subscription.
        fixture.call('prepare',start=True);last_before_connect=latest_fd()
        session=subprocess.Popen(command,env=env,stdout=log('initial-preparing.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        existing=power('already-preparing-on-connect',lambda s:s['available'] and not s['initializing'] and s['gate']['preparing'] and s['gate']['protected_preparations']==1 and not s['armed'])
        assert existing['lock']['first_presented_protected'] and existing['lock']['unprotected_presentations']==0
        assert latest_fd()==last_before_connect
        fixture.call('prepare',start=False);power('initial-preparing-resumed',lambda s:s['armed'] and not s['gate']['preparing'] and s['gate']['barrier'] is None)
        request(control,'exit');records['initial_preparing_compositor_exit']=session.wait(timeout=8);assert records['initial_preparing_compositor_exit']==0
        records['validation']={'passed':True,'real_unix_fd_transfer_and_close':True,'held_until_protected_submission':True,'stopped_compositor_cannot_release':True,'foreign_sender_ignored':True,'prepare_unlock_refused':True,'busy_rearm_same_epoch':True,'unlock_during_rearm_refused':True,'retry_deadline_fail_closed_recovery':True,'duplicate_resume_no_extra_inhibitor':True,'rearm_owner_denial_and_bus_replacement':True,'initial_preparing_property_checked':True,'no_inhibit_during_existing_preparation':True,'cached_clipboard_private_restored':True,'presentation_scope':'Nested EGL submission only; no real suspend/KMS claim','suspend_called':False}
        print('PASS: real delay FD lifetime, protected submission barrier, spoof rejection, preparation unlock refusal, resume/rearm, owner/denial/bus recovery and original app/clipboard')
    finally:
        if stopped and session is not None:os.kill(session.pid,signal.SIGCONT)
        for proc in [app,session]:
            if proc is not None and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
        if fixture is not None:fixture.close()
        if bus is not None and bus.poll() is None:bus.terminate();bus.wait(timeout=4)
        for stream in logs:stream.close()
        runtime.cleanup();(out/'status.json').write_text(json.dumps(records,indent=2)+'\n')


if __name__=='__main__':main()
