#!/usr/bin/env python3
"""A hidden obsolete-size lock buffer still needs its own frame callback.

The synthetic trusted client defers its new-size buffer until that callback.
The compositor must remain opaque, deny captures and withhold full-surface
presentation proof throughout the deliberate gap. No authentication is tested.
"""
import argparse,json,os,subprocess,tempfile
from pathlib import Path
from lock_replay import append,events,any_request
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    commands,report=out/'commands.jsonl',out/'client.jsonl';commands.write_text('')
    record={};proc=None;root=Path.cwd()
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json');env.pop('WAYLAND_SOCKET',None)
    with tempfile.TemporaryDirectory(prefix='tarot-lock-resize-') as tmp:
        control=Path(tmp)/'control.sock'
        def state(label,condition):
            value=await_value(lambda:request(control,'status'),condition,seconds=3);record[label]=value;return value
        try:
            argv=[str(root/'target/debug/tarot'),'--lock-client',str(root/'target/debug/examples/lock-probe'),'--verify-lock-blank','--verify-lock-surface-rgba','336699ff','--control-socket',str(control),'--exit-after-ms','15000']
            for arg in ['--role','locker','--report',str(report),'--commands',str(commands)]:argv.append('--lock-client-arg='+arg)
            with (out/'compositor.log').open('w') as log:
                proc=subprocess.Popen(argv,env=env,stdout=log,stderr=subprocess.STDOUT)
                await_value(lambda:control.exists() or proc.poll() is not None,bool);assert proc.poll() is None
                await_value(lambda:events(report),lambda es:any(e['event']=='ready' for e in es))
                append(commands,'lock')
                first=state('initial',lambda s:s['lock']['phase']=='Locked' and s['lock']['surface_verified'] is True)
                append(commands,'defer-next-configure');await_value(lambda:events(report),lambda es:any(e['event']=='defer-armed' for e in es))
                request(control,'rotate',orientation='left-up')
                await_value(lambda:events(report),lambda es:any(e['event']=='configure-awaiting-frame' for e in es))
                # This has a three-second bound, below the readiness watchdog.
                await_value(lambda:events(report),lambda es:any(e['event']=='resize-frame-done' for e in es),seconds=3)
                gap=state('black-with-callback-delivered',lambda s:s['lock']['output']['orientation']=='left-up' and s['lock']['last_presented_surface'] is False and s['lock']['fallback_verified'] is True)
                assert gap['lock']['unprotected_presentations']==first['lock']['unprotected_presentations']==0
                assert any_request(control,'capture',path=str(out/'denied.png'))=={'ok':False,'error':'Session is locked'}
                assert not (out/'denied.png').exists()
                append(commands,'finish-resize')
                ready=state('new-size-presented',lambda s:s['lock']['last_presented_surface'] is True and s['lock']['surface_verified'] is True and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])
                assert ready['lock']['unprotected_presentations']==0
                request(control,'exit');assert proc.wait(timeout=4)==0
                record['validation']={'passed':True,'obsolete_buffer_frame_callback_received':True,'gap_stays_black_and_capture_denied':True,'full_size_surface_required_for_presentation_proof':True,'authentication_tested':False}
        except BaseException as error:
            record['validation']={'passed':False,'exception_type':type(error).__name__};raise
        finally:
            if proc is not None and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=4)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            (out/'status.json').write_text(json.dumps(record,indent=2)+'\n')
    print('PASS: stale lock buffer callback, opaque gap/capture denial, then matching-size presentation')


if __name__=='__main__':main()
