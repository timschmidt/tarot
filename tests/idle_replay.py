#!/usr/bin/env python3
"""Real Wayland idle locking/inhibitor visibility, ownership and destruction."""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from lock_replay import events,append,any_request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);root=Path.cwd();runtime=tempfile.TemporaryDirectory(prefix='tarot-idle-');control=Path(runtime.name)/'control.sock'
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    locker_report=out/'locker-events.jsonl';locker_commands=out/'locker-commands.jsonl';locker_commands.write_text('');records={};session=app=None;logs=[]
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        try:value=await_value(lambda:request(control,'status'),condition,seconds=8)
        except AssertionError as e:records[label+'-failure']=e.args[0] if e.args else str(e);raise
        records[label]=value;return value
    def inhibited(label,active):return state(label,lambda s:s['lock']['phase']=='Unlocked' and s['lock']['idle']['policy']['inhibited']==active)
    def unlock(label):
        current=state(label+'-owner',lambda s:s['lock']['locker']['pid'] is not None);pid=current['lock']['locker']['pid']
        await_value(lambda:events(locker_report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es))
        if current['lock']['phase']!='Locked':append(locker_commands,'lock')
        state(label+'-protected',lambda s:s['lock']['phase']=='Locked'
              and s['lock']['last_presented_surface'] is True
              and s['lock']['last_presented_generation']==s['lock']['generation']
              and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])
        append(locker_commands,'unlock');return state(label,lambda s:s['lock']['phase']=='Unlocked')
    def start_app(name,display):
        folder=out/name;folder.mkdir();report=folder/'events.jsonl';commands=folder/'commands.jsonl';commands.write_text('')
        app_env=dict(env,WAYLAND_DISPLAY=display)
        proc=subprocess.Popen([str(root/'target/debug/examples/lock-probe'),'--role','app','--report',str(report),'--commands',str(commands)],env=app_env,stdout=log(name+'.log'),stderr=subprocess.STDOUT)
        mapped=state(name+'-mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view']);return proc,report,commands,mapped
    def app_command(name):
        before=len(events(report));append(commands,name);await_value(lambda:events(report),lambda es:len(es)>before)
    def wait_locked(label):
        value=state(label,lambda s:s['lock']['phase']!='Unlocked' and s['lock']['last_presented_generation']==s['lock']['generation']);assert set(value)=={'ok','pid','lock'} and value['lock']['idle']['inhibitor_surfaces'] is None;return value
    try:
        command=[str(root/'target/debug/tarot'),'--idle-lock-ms','1200','--lock-client',str(root/'target/debug/examples/lock-probe'),'--control-socket',str(control),'--verify-lock-blank','--exit-after-ms','60000']
        for arg in ['--role','locker','--report',str(locker_report),'--commands',str(locker_commands)]:command.append('--lock-client-arg='+arg)
        session=subprocess.Popen(command,env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        initial=unlock('startup-unlocked');app,report,commands,before=start_app('first-app',initial['wayland_display'])
        app_command('inhibit');app_command('inhibit');one=inhibited('two-visible-inhibitors',True);time.sleep(1.6)
        assert state('visible-inhibitors-prevent-lock')['lock']['phase']=='Unlocked'
        app_command('uninhibit');inhibited('one-inhibitor-still-active',True);time.sleep(1.6);assert state('duplicate-reference-retained')['lock']['phase']=='Unlocked'
        app_command('uninhibit');inhibited('last-inhibitor-released',False);time.sleep(.55);assert state('release-grants-fresh-grace')['lock']['phase']=='Unlocked'
        wait_locked('automatic-idle-lock');assert any_request(control,'capture',path=str(out/'denied.png'))=={'ok':False,'error':'Session is locked'};assert not (out/'denied.png').exists()
        after=unlock('first-restored');assert after['cards']==before['cards'] and after['view']==before['view']
        app_command('inhibit');inhibited('visible-again',True);request(control,'home');inhibited('hidden-inhibitor-ignored',False);wait_locked('hidden-cannot-prevent-lock')
        unlock('home-restored');request(control,'activate',id=before['cards'][0]['id']);inhibited('visibility-restores-inhibition',True);time.sleep(1.6);assert state('visible-again-no-lock')['lock']['phase']=='Unlocked'
        app_command('cover');inhibited('opaque-child-occludes-inhibiting-parent',False);wait_locked('occluded-inhibitor-ignored')
        unlock('covered-restored');app_command('uncover');inhibited('uncover-restores-inhibitor',True);time.sleep(1.6);assert state('uncovered-no-lock')['lock']['phase']=='Unlocked'
        app_command('unmap');state('unmapped',lambda s:s['lock']['phase']=='Unlocked' and not s['cards'] and not s['lock']['idle']['policy']['inhibited']);wait_locked('unmapped-inhibitor-ignored')
        append(commands,'exit');assert app.wait(timeout=4)==0;empty=unlock('empty-restored');app,report,commands,_=start_app('second-app',empty['wayland_display'])
        app_command('inhibit');inhibited('second-visible',True);time.sleep(1.6);assert state('second-no-lock')['lock']['phase']=='Unlocked'
        app.terminate();assert app.wait(timeout=4) is not None;inhibited('dead-client-released',False);wait_locked('dead-client-cannot-prevent-lock');unlock('final-unlocked')
        request(control,'exit');records['compositor_exit']=session.wait(timeout=6);assert records['compositor_exit']==0
        disabled_command=[command[0],*command[3:]]
        session=subprocess.Popen(disabled_command,env=env,stdout=log('disabled-compositor.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        idle_disabled=unlock('disabled-startup');assert idle_disabled['lock']['idle']['policy'] is None
        app,report,commands,_=start_app('disabled-app',idle_disabled['wayland_display']);app_command('inhibit')
        state('disabled-retains-live-inhibitor',lambda s:s['lock']['idle']['inhibitor_surfaces']==1)
        app.terminate();app.wait(timeout=4);state('disabled-retires-dead-inhibitor',lambda s:not s['cards'] and s['lock']['idle']['inhibitor_surfaces']==0)
        time.sleep(1.6);assert state('disabled-stays-unlocked')['lock']['phase']=='Unlocked'
        request(control,'exit');records['disabled_compositor_exit']=session.wait(timeout=6);assert records['disabled_compositor_exit']==0
        records['validation']={'passed':True,'actual_wayland_idle_inhibitors':True,'duplicate_inhibitors_reference_counted':True,'visible_hidden_restored_unmapped_dead_surfaces':True,'fully_occluded_parent_ignored_and_uncover_restored':True,'dead_surface_cleanup_with_automatic_idle_disabled':True,'protected_inhibitor_metadata_redacted':True,'fresh_grace_after_inhibition':True,'automatic_protected_lock':True,'owning_unlock_restores_view':True,'control_queries_do_not_count_as_activity':True,'panel_power_tested':False,'presentation_scope':'Nested EGL submission; direct panel/wake input tested separately'}
        print('PASS: real visible/hidden/unmapped/dead Wayland inhibitors, duplicate ownership, fresh grace, automatic protected lock and restored view')
    finally:
        for proc in [app,session]:
            if proc is not None and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
        for stream in logs:stream.close()
        runtime.cleanup();(out/'status.json').write_text(json.dumps(records,indent=2)+'\n')


if __name__=='__main__':main()
