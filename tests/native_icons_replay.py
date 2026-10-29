#!/usr/bin/env python3
"""Installed icon lookup, worker completion, GPU pixels and favorite gestures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib

from PIL import Image
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def color_count(image, bounds, color, scale):
    box=tuple(round(v*scale) for v in bounds)
    return sum(all(abs(a-b)<8 for a,b in zip(pixel,color)) for pixel in image.crop(box).getdata())


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--xvfb',default='Xvfb')
    p.add_argument('--scale',type=int,choices=[1,2],default=1)
    args=p.parse_args();out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();children=[];logs=[];mouse=None
    record={'passed':False,'scale':args.scale,'phone_kms_tested':False,'actions':[],
            'compositor_sha256':hashlib.sha256((root/'target/debug/tarot').read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='tarot-native-icons-') as temporary:
        work=Path(temporary);data=work/'data';catalog=data/'applications';catalog.mkdir(parents=True)
        (work/'empty').mkdir();control=work/'control.sock';config=work/'settings.toml'
        def svg(path,color):
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="{color}"/></svg>')
        theme=data/'icons/Fixture';theme.mkdir(parents=True)
        (theme/'index.theme').write_text('[Icon Theme]\nInherits=Parent\nDirectories=one,two\n[one]\nSize=58\nType=Fixed\n[two]\nSize=58\nScale=2\nType=Fixed\n')
        for directory,color in [('one',(0,255,0,255)),('two',(0,255,255,255))]:
            path=theme/directory/'scaled.png';path.parent.mkdir(parents=True)
            Image.new('RGBA',(58,58),color).save(path)
        parent=data/'icons/Parent';parent.mkdir(parents=True)
        (parent/'index.theme').write_text('[Icon Theme]\nInherits=Fixture\nDirectories=apps\n[apps]\nSize=58\n')
        svg(parent/'apps/red.svg','#ff0000')
        hicolor=data/'icons/hicolor';hicolor.mkdir(parents=True)
        (hicolor/'index.theme').write_text('[Icon Theme]\nDirectories=apps\n[apps]\nSize=58\n')
        svg(hicolor/'apps/orange.svg','#ff8000');svg(work/'absolute.svg','#0000ff')
        imported=work/'import.svg'
        imported.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#ffff00"/><image href="/tmp/private.png" width="20" height="20"/></svg>')
        applications=[('test.iconA.desktop','A Red','red'),('test.iconB.desktop','B Scale','scaled'),
            ('test.iconC.desktop','C Absolute',str(work/'absolute.svg')),('test.iconD.desktop','D Missing','absent'),
            ('test.iconE.desktop','E Import',str(imported)),('test.calendar.desktop','F Calendar','absent'),
            ('test.iconG.desktop','G Hicolor','orange')]
        for identifier,name,icon in applications:
            (catalog/identifier).write_text(f'[Desktop Entry]\nType=Application\nName={name}\nExec=/bin/false\nIcon={icon}\n')
        config.write_text('version = 4\n[quick_launch]\napplications = ["test.iconA.desktop"]\n')
        def log(name):
            stream=(out/name).open('x');logs.append(stream);return stream
        def state(label,predicate=lambda s:True):
            s=await_value(lambda:request(control,'status'),predicate,seconds=12)
            record['actions'].append({'label':label,'state':s});return s
        def capture(label):
            request(control,'capture',path=str(out/(label+'.png')))
            state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
            with Image.open(out/(label+'.png')) as im:im.load();return im.convert('RGB')
        def move(x,y):mouse.move(x*args.scale,y*args.scale)
        try:
            read,write=os.pipe()
            try:
                server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
                children.append(server);os.close(write);write=None;number=read_display_number(read)
            finally:
                os.close(read)
                if write is not None:os.close(write)
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',HOME=str(work),XDG_DATA_HOME=str(data),XDG_DATA_DIRS=str(work/'empty'),TAROT_ICON_THEME='Fixture')
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            session=subprocess.Popen([str(root/'target/debug/tarot'),'--width',str(360*args.scale),'--height',str(720*args.scale),'--scale',str(args.scale),'--config-file',str(config),'--control-socket',str(control),'--exit-after-ms','90000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT);children.append(session)
            await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
            state('catalog',lambda s:s['application_count']==7)
            mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool);move(180,350)
            request(control,'launcher')
            s=state('resolved',lambda s:s['view']=='Launcher' and s['application_icons']['ready']==4 and s['application_icons']['missing']==2 and s['application_icons']['pending']==0)
            assert s['application_icons']['raster_bytes']==4*(58*args.scale)**2*4
            image=capture('launcher-icons');counts={}
            for label,bounds,color in [('red',(28,108,87,153),(255,0,0)),('scale',(150,100,210,160),(0,255,255) if args.scale==2 else (0,255,0)),('absolute',(274,108,332,153),(0,0,255)),('hicolor',(28,332,87,377),(255,128,0))]:
                counts[label]=color_count(image,bounds,color,args.scale);assert counts[label]>500*args.scale**2,(label,counts)
            record['pixel_counts']=counts
            before=config.read_bytes();move(180,140);time.sleep(.08);mouse.button(True);time.sleep(.6)
            move(180,420);time.sleep(.1);image=capture('lifted-native-icon')
            assert color_count(image,(154,374,206,426),(0,255,255) if args.scale==2 else (0,255,0),args.scale)>500*args.scale**2
            assert config.read_bytes()==before
            move(3+1.5*354/5,643);time.sleep(.1);mouse.button(False)
            state('favorite-committed',lambda s:s['favorites']['applications']==['test.iconA.desktop','test.iconB.desktop'] and s['display_settings']['settings_saved'])
            assert tomllib.loads(config.read_text())['quick_launch']['applications']==['test.iconA.desktop','test.iconB.desktop']
            request(control,'home');state('home',lambda s:s['view']=='Home');time.sleep(.4)
            image=capture('quick-launch-native-icons')
            assert color_count(image,(12,620,65,667),(255,0,0),args.scale)>350*args.scale**2
            assert color_count(image,(82,615,133,670),(0,255,255) if args.scale==2 else (0,255,0),args.scale)>500*args.scale**2
            request(control,'exit');assert session.wait(timeout=10)==0
            record['passed']=True
            print('PASS: inherited/hicolor/absolute SVG, scale-specific PNG, import/missing fallback, lifted icon and saved Quick Launch pixels')
        finally:
            if mouse is not None:mouse.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:child.wait(timeout=5)
                    except subprocess.TimeoutExpired:child.kill();child.wait()
            for stream in logs:stream.close()
            (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n')


if __name__=='__main__':main()
