#!/usr/bin/env python3
"""Live launcher catalog: adding, changing and removing a desktop entry while
the session runs reaches the shell's applications without a new login.

Uses a private XDG_DATA_HOME and an empty XDG_DATA_DIRS on a private X server.
"""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number

ENTRY="""[Desktop Entry]
Type=Application
Name={name}
Exec=true
"""


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    root=Path.cwd();runtime=tempfile.TemporaryDirectory(prefix='tarot-catalog-');work=Path(runtime.name)
    control=work/'control.sock';apps=work/'data'/'applications';apps.mkdir(parents=True);(work/'empty-data').mkdir()
    (apps/'test.first.desktop').write_text(ENTRY.format(name='First Fixture'))
    children=[];logs=[];record={'passed':False}
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def catalog():
        return {a['id']:a['name'] for a in request(control,'applications')['applications']}
    def expect(label,condition,seconds=6):
        started=time.monotonic()
        value=await_value(catalog,condition,seconds=seconds)
        record[label]={'catalog':value,'seconds':round(time.monotonic()-started,2)};return value
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None
            number=read_display_number(read);assert server.poll() is None
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work),
            XDG_CONFIG_HOME=str(work/'config'),XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty-data'),
            LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_SOCKET',None);env.pop('WAYLAND_DISPLAY',None)
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--control-socket',str(control),'--exit-after-ms','60000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool,seconds=8);assert session.poll() is None
        expect('initial',lambda c:c=={'test.first.desktop':'First Fixture'})
        # Written in place, as a package manager or Waydroid would.
        (apps/'test.second.desktop').write_text(ENTRY.format(name='Second Fixture'))
        expect('added',lambda c:c.get('test.second.desktop')=='Second Fixture')
        (apps/'test.second.desktop').write_text(ENTRY.format(name='Second Renamed'))
        expect('changed',lambda c:c.get('test.second.desktop')=='Second Renamed')
        (apps/'test.second.desktop').unlink()
        expect('removed',lambda c:c=={'test.first.desktop':'First Fixture'})
        # An atomic rename into place is seen too.
        staging=work/'data'/'staging.desktop';staging.write_text(ENTRY.format(name='Third Fixture'))
        staging.rename(apps/'test.third.desktop')
        expect('renamed-in',lambda c:c.get('test.third.desktop')=='Third Fixture')
        assert session.poll() is None
        record['passed']=True
        print('PASS: launcher catalog follows desktop entries added, changed, removed and renamed in during the session',out)
    finally:
        for p in reversed(children):
            if p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for f in logs:f.close()
        (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n');runtime.cleanup()


if __name__=='__main__':main()
