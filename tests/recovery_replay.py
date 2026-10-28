#!/usr/bin/env python3
"""Real protected recovery view, cached clipboard privacy and safe handoff."""
import argparse,json,os,stat,subprocess,tempfile
from pathlib import Path
from PIL import Image
from notification_replay import await_value
from phone_gesture_replay import request
from lock_replay import append,events,any_request

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);root=Path.cwd()
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    def run(name,capture,bad_capture=False):
        folder=out/name;folder.mkdir(mode=0o700);runtime=tempfile.TemporaryDirectory(prefix='tarot-recovery-');control=Path(runtime.name)/'control.sock'
        marker=folder/'allow';report=folder/'locker-events.jsonl';commands=folder/'locker-commands.jsonl';commands.write_text('')
        app_report=folder/'app-events.jsonl';app_commands=folder/'app-commands.jsonl';app_commands.write_text('');records={};session=app=None
        def state(label,condition=lambda s:True,seconds=15):
            try:value=await_value(lambda:request(control,'status'),condition,seconds=seconds)
            except AssertionError as e:records[label+'-failure']=e.args[0] if e.args else str(e);raise
            records[label]=value;return value
        def read_clipboard(label,allowed):
            before=sum(e['event']=='clipboard-read' for e in events(app_report));append(app_commands,'read-clipboard')
            es=await_value(lambda:[e for e in events(app_report) if e['event']=='clipboard-read'],lambda es:len(es)>before)
            assert es[-1]['fields']['matches']==allowed and (es[-1]['fields']['bytes']>0)==allowed;records[label]=es[-1]
        command=[str(root/'target/debug/tarot'),'--lock-client',str(root/'target/debug/examples/recovery-client'),'--verify-lock-blank','--recovery-screenshot',str(capture),'--control-socket',str(control),'--exit-after-ms','60000']
        for arg in ['--allow-file',str(marker),'--program',str(root/'target/debug/examples/lock-probe'),'--','--role','locker','--report',str(report),'--commands',str(commands)]:command.append('--lock-client-arg='+arg)
        try:
            if not bad_capture:marker.touch()
            with (folder/'compositor.log').open('w') as log,(folder/'app.log').open('w') as app_log:
                session=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT)
                await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
                if not bad_capture:
                    await_value(lambda:events(report),lambda es:any(e['event']=='ready' for e in es));append(commands,'lock');state('first-lock',lambda s:s['lock']['phase']=='Locked');append(commands,'unlock')
                    initial=state('unlocked',lambda s:s['lock']['phase']=='Unlocked')
                    app_env=dict(env,WAYLAND_DISPLAY=initial['wayland_display'])
                    app=subprocess.Popen([str(root/'target/debug/examples/lock-probe'),'--role','app','--report',str(app_report),'--commands',str(app_commands)],env=app_env,stdout=app_log,stderr=subprocess.STDOUT)
                    before=state('app-active',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view']);await_value(lambda:events(app_report),lambda es:any(e['event']=='clipboard-offered' for e in es))
                    read_clipboard('clipboard-before',True);marker.unlink();request(control,'lock')
                failed=state('recovery-presented',lambda s:s['lock']['recovery_ready'] and s['lock']['recovery_verified'])
                assert set(failed)=={'ok','pid','lock'} and failed['lock']['phase']=='Orphaned' and failed['lock']['locker']['consecutive_failures']==6
                assert failed['lock']['last_presented_recovery'] and not failed['lock']['last_presented_surface']
                for command,fields in [('home',{}),('overview',{}),('launcher',{}),('applications',{}),('capture',{'path':str(folder/'denied.png')})]:
                    response=any_request(control,command,**fields);assert response=={'ok':False,'error':'Session is locked'}
                assert not (folder/'denied.png').exists()
                if bad_capture:
                    state('diagnostic-failure-keeps-recovery',lambda s:s['lock']['recovery_capture_error'] is not None and s['lock']['recovery_ready'])
                else:
                    read_clipboard('cached-offer-blocked-in-recovery',False)
                    assert stat.S_IMODE(capture.stat().st_mode)==0o600
                    im=Image.open(capture).convert('RGBA');assert all(px[3]==255 for px in im.getdata());assert sum(min(px[:3])>150 for px in im.getdata())>500
                    assert im.getpixel((0,0))[:3]==(20,29,34)
                    marker.touch();request(control,'restart_locker');waiting=state('retry-awaiting-owner',lambda s:s['lock']['phase']=='Awaiting' and s['lock']['blank_verified'])
                    assert not waiting['lock']['recovery_ready'];pid=waiting['lock']['locker']['pid'];await_value(lambda:events(report),lambda es:any(e['pid']==pid and e['event']=='ready' for e in es));append(commands,'lock')
                    active=state('fresh-owner',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True);assert not active['lock']['last_presented_recovery'];read_clipboard('still-private-before-owning-unlock',False)
                    append(commands,'unlock');after=state('restored',lambda s:s['lock']['phase']=='Unlocked');assert after['cards']==before['cards'] and after['view']==before['view'];read_clipboard('original-offer-restored',True)
                    request(control,'capture',path=str(folder/'normal-restored.png'));state('normal-capture-restored',lambda s:not s['capture_pending'] and not s['capture_error'])
                    append(app_commands,'exit');assert app.wait(timeout=4)==0
                request(control,'exit');records['compositor_exit']=session.wait(timeout=5);assert records['compositor_exit']==0
                records['validation']={'passed':True,'own_opaque_recovery_view':True,'ordinary_metadata_navigation_capture_paths_blocked':True,'cached_clipboard_and_handoff':not bad_capture,'capture_failure_continues':bad_capture,'physical_retry_input_tested':False,'presentation_scope':'Nested EGL submission; native KMS/input/authentication separate.'}
        finally:
            for proc in [app,session]:
                if proc is not None and proc.poll() is None:
                    proc.terminate()
                    try:proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:proc.kill();proc.wait()
            runtime.cleanup();(folder/'status.json').write_text(json.dumps(records,indent=2)+'\n')
    run('handoff',out/'own-recovery.png')
    run('capture-failure',Path('/proc/tarot-unwritable-recovery.png'),True)
    print('PASS: real isolated recovery scene, opaque own-only capture, privacy through exhaustion/handoff, restored app/clipboard and diagnostic failure survival')

if __name__=='__main__':main()
