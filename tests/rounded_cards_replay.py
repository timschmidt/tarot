#!/usr/bin/env python3
"""Real native card clipping and input: two opaque clients, one with a corner subsurface.

Uses a private X server; the test never injects input into the host desktop.
Pixels must reveal the lower native client through the upper rounded corner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time
from PIL import Image
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--scale',type=int,choices=[1,2],default=2)
    p.add_argument('--xvfb',default='Xvfb')
    p.add_argument('--opaque',action='store_true')
    args=p.parse_args();out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();temp=tempfile.TemporaryDirectory(prefix='tarot-card-clipping-');work=Path(temp.name)
    socket=work/'control.sock';record={'validation':{'passed':False},'scale':args.scale,'opaque':args.opaque};children=[];logs=[];mouse=None;apps={}
    record['replay_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    record['compositor_sha256']=hashlib.sha256((root/'target/debug/tarot').read_bytes()).hexdigest()
    record['probe_sha256']=hashlib.sha256((root/'target/debug/examples/input-probe').read_bytes()).hexdigest()
    (out/'replay-source.py').write_bytes(Path(__file__).read_bytes())
    def log(name):
        f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        v=await_value(lambda:request(socket,'status'),condition,seconds=12);record[label]=v;return v
    def window(s,identifier):return next(w for w in s['presentation']['windows'] if w['id']==identifier)
    def capture(name):
        request(socket,'capture',path=str(out/(name+'.png')))
        state(name,lambda s:not s['capture_pending'] and not s['capture_error'])
        with Image.open(out/(name+'.png')) as im:im.load();return im.convert('RGB')
    def physical(s,x,y):
        g=s['output'];w,h=g['physical_width'],g['physical_height'];scale=g['scale']
        swap=g['orientation'] in ['left-up','right-up'];lw,lh=((h/scale,w/scale) if swap else (w/scale,h/scale))
        x,y=x/lw,y/lh
        x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[g['orientation']]
        return round(x*w),round(y*h)
    def move(s,x,y):mouse.move(*physical(s,x,y));time.sleep(.08)
    def click(s,x,y):move(s,x,y);mouse.button(True);time.sleep(.04);mouse.button(False)
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None
            assert select.select([read],[],[],8)[0];number=os.read(read,32).decode().strip();assert number.isdigit()
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--width',str(360*args.scale),'--height',str(720*args.scale),'--scale',str(args.scale),'--control-socket',str(socket),'--exit-after-ms','180000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT)
        children.append(session);await_value(lambda:socket.exists() or session.poll() is not None,bool);assert session.poll() is None
        ready=state('ready');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native_env=dict(env,WAYLAND_DISPLAY=ready['wayland_display']);native_env.pop('DISPLAY',None)
        ids=[]
        for name,color,extra in [('red','e91d31',[]),('blue','2255dd',['--corner-child','--refuse-close'])]:
            app=subprocess.Popen([str(root/'target/debug/examples/input-probe'),'--fit-output','--solid-color',color,'--report',str(out/(name+'.jsonl')),'--exit-after-seconds','180',*(['--opaque'] if args.opaque else []),*extra],env=native_env,stdout=log(name+'.log'),stderr=subprocess.STDOUT)
            children.append(app);apps[name]=app
            s=state(name+'-mapped',lambda s:len(s['cards'])==len(ids)+1 and s['presentation']['windows'][-1]['opacity']==1.0)
            ids.append(next(c['id'] for c in s['cards'] if c['id'] not in ids))
        red,blue=ids
        request(socket,'overview');s=state('overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        # Stack through the real gesture recognizer rather than a test-only API.
        b=window(s,blue)['rect'];move(s,b['x']+b['width']/2,300);mouse.button(True)
        state('arrange-ready',lambda s:(s['presentation']['arrange'] or {}).get('id')==blue)
        move(s,20,300)
        state('stack-target',lambda s:((s['presentation']['arrange'] or {}).get('target') or {}).get('Into',{}).get('anchor')==red)
        mouse.button(False)
        state('stacked',lambda s:len({c['stack'] for c in s['cards']})==1 and s['presentation']['arrange'] is None)
        samples=[]
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            request(socket,'rotate',orientation=orientation)
            s=state('rotation-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'])
            # Client configure/commit and release-to-settle are asynchronous.
            time.sleep(.45)
            s=state('settled-'+orientation,lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
            b=window(s,blue);r=b['rect'];radius=b['corner_radius'];assert radius>0
            image=capture('stack-'+orientation)
            corner=(r['x']+1,r['y']+1);inside=(r['x']+radius+2,r['y']+1)
            cp=image.getpixel(physical(s,*corner));ip=image.getpixel(physical(s,*inside))
            assert cp[0]>cp[1]*2 and cp[0]>cp[2]*2,('corner must reveal red native client',orientation,cp,corner,s)
            assert ip[2]>ip[0]*2 and ip[2]>ip[1]*1.3,('inner edge remains blue subsurface',orientation,ip,inside)
            shadow=image.getpixel(physical(s,r['x']-4,r['y']+20))
            assert 90<shadow[0]<230 and shadow[0]>2*max(shadow[1:]),('shadow must dim the red client behind, not repaint wallpaper',orientation,shadow)
            samples.append(dict(orientation=orientation,corner=cp,inside=ip,shadow=shadow,radius=radius))
            click(s,*corner)
            focused=state('corner-selects-red-'+orientation,lambda s:s['view']=={'App':red} and window(s,red)['corner_radius']==0 and window(s,red)['rect']==s['app_area'])
            assert len(focused['cards'])==2
            request(socket,'activate',id=blue);state('blue-before-overview-'+orientation,lambda s:s['view']=={'App':blue} and window(s,blue)['corner_radius']==0)
            request(socket,'overview');state('overview-again-'+orientation,lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        record['samples']=samples
        request(socket,'close',id=blue)
        def refused():
            rows=(out/'blue.jsonl').read_bytes().split(b'\n')[:-1]
            return any(json.loads(row).get('event')=='close_requested' and json.loads(row).get('refused') for row in rows)
        await_value(refused,bool)
        state('close-refused',lambda s:len(s['cards'])==2 and next(c for c in s['cards'] if c['id']==blue)['closing'])
        assert apps['blue'].poll() is None,'A close request must not terminate a refusing client'
        capture('close-refused')
        apps['blue'].terminate();apps['blue'].wait(timeout=5)
        state('client-exit-removes-only-its-card',lambda s:len(s['cards'])==1 and s['cards'][0]['id']==red)
        request(socket,'close',id=red)
        state('cooperative-close',lambda s:not s['cards'])
        assert apps['red'].wait(timeout=5)==0
        capture('after-close')
        mouse.close();mouse=None;request(socket,'exit');assert session.wait(timeout=5)==0
        record['validation']=dict(passed=True,real_gles=True,native_shm_clients=True,corner_subsurface_clipped=True,lower_native_client_visible_through_corner=True,actual_corner_tap_selects_lower_card=True,all_output_orientations=True,cooperative_close=True,refused_close_preserves_client_and_card=True,client_exit_isolated=True,phone_kms_tested=False)
        print('PASS: rounded native card/subsurface pixels and actual corner selection in every orientation')
    finally:
        if mouse is not None:mouse.close()
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for f in logs:f.close()
        (out/'status.json').write_text(json.dumps(record,indent=2)+'\n');temp.cleanup()


if __name__=='__main__':main()
