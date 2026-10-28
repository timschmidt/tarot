#!/usr/bin/env python3
"""Run the real Wayland lifecycle/resize/capture regression in an empty session."""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    runtime=tempfile.TemporaryDirectory(prefix='tarot-lifecycle-');control=Path(runtime.name)/'control.sock'
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',TAROT_TEST_CONTROL_SOCKET=str(control))
    result={};process=None
    try:
        with (args.output/'compositor.log').open('w') as log:
            process=subprocess.Popen([str(Path('target/debug/tarot').resolve()),'--control-socket',str(control),'--exit-after-ms','90000'],env=env,stdout=log,stderr=subprocess.STDOUT)
            await_value(lambda:control.exists() or process.poll() is not None,bool);assert process.poll() is None
            with (args.output/'test.log').open('w') as test_log:
                run=subprocess.run(['cargo','test','--offline','--test','wayland_lifecycle','--','--ignored','--nocapture','--test-threads=1','--skip','transient_native_input_and_pixels'],env=env,stdout=test_log,stderr=subprocess.STDOUT,timeout=60)
            result['test_exit']=run.returncode
            request(control,'exit');result['compositor_exit']=process.wait(timeout=6)
            # Debug builds may consume a GL error before later capture code
            # checks it. A new-pixel capture alone is therefore insufficient.
            result['graphics_errors']=[line for line in (args.output/'compositor.log').read_text().splitlines() if any(error in line for error in ['GL_INVALID','GL_OUT_OF_MEMORY','GL error'])]
            result['passed']=result['test_exit']==result['compositor_exit']==0 and not result['graphics_errors']
            assert result['passed'],result
        print('PASS: real lifecycle, hidden SHM resize/new pixels, callbacks, capture failure/recovery and disconnect',args.output)
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
        runtime.cleanup();(args.output/'validation.json').write_text(json.dumps(result,indent=2)+'\n')

if __name__=='__main__':main()
