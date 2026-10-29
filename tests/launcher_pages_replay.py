#!/usr/bin/env python3
"""Named Launcher pages through real controls on a private X display/catalog."""
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

from dialog_button import locate
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--xvfb',default='Xvfb')
    args=p.parse_args();out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();children=[];logs=[];mouse=session=None
    records={'passed':False,'phone_kms_tested':False,'actions':[],
        'compositor_sha256':hashlib.sha256((root/'target/debug/tarot').read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='tarot-launcher-pages-') as temporary:
        work=Path(temporary);control=work/'control.sock';config=work/'settings.toml'
        catalog=work/'data/applications';catalog.mkdir(parents=True);(work/'empty').mkdir()
        for n in range(36):
            (catalog/f'test.app{n:02}.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=App {n:02}\nExec=/bin/false\n')
        original='version = 3\n[notifications]\ndnd = true\n[quick_launch]\napplications = ["test.app00.desktop"]\n'
        config.write_text(original)
        def log(name):
            stream=(out/name).open('x');logs.append(stream);return stream
        def state(label,condition=lambda s:True):
            s=await_value(lambda:request(control,'status'),condition,seconds=10)
            records['actions'].append({'label':label,'state':s});return s
        def capture(label):
            request(control,'capture',path=str(out/(label+'.png')))
            return state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
        def click(x,y):
            mouse.move(x,y);time.sleep(.06);mouse.button(True);time.sleep(.06);mouse.button(False);time.sleep(.12)
        def menu(label):
            click(40,16);state(label+'-open',lambda s:s['launcher']['menu_open']);capture(label)
        def menu_item(word,label):
            capture(label)
            with Image.open(out/(label+'.png')) as source:
                source.crop((0,32,240,684)).save(out/(label+'-panel.png'))
            found=locate(out/(label+'-panel.png'),word,vertical=(0,1))
            records['actions'].append({'label':label+'-visible-target','ocr':found})
            click(found['point'][0],32+found['point'][1])
        def page_menu(label,name):
            menu(label);menu_item('pages',label+'-pages')
            state(label+'-page-list',lambda s:s['top_menu']['panel']=='Pages')
            menu_item(name,label+'-page')
            state(label+'-page-actions',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Page' in s['top_menu']['panel'])
        def swipe(start,end):
            mouse.move(*start);mouse.button(True);time.sleep(.06);mouse.move(*end);time.sleep(.1);mouse.button(False)
        def saved(label,condition):
            s=state(label,lambda s:s['launcher']['saved'] and condition(s['launcher']['settings']))
            data=tomllib.loads(config.read_text());assert data['version']==4
            assert data['notifications']['dnd'] and data['quick_launch']['applications']==['test.app00.desktop']
            assert data['launcher']==s['launcher']['settings']
            assert not s['cards'] and s['launch_feedback'] is None
            return s
        try:
            read,write=os.pipe()
            try:
                xvfb=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1200x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
                children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
            finally:
                os.close(read)
                if write is not None:os.close(write)
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty'))
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            def launch(label):
                nonlocal session,mouse
                session=subprocess.Popen([str(root/'target/debug/tarot'),'--width','360','--height','720','--config-file',str(config),'--control-socket',str(control),'--exit-after-ms','180000'],env=env,stdout=log(label+'.log'),stderr=subprocess.STDOUT);children.append(session)
                await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
                state(label+'-ready',lambda s:s['application_count']==36)
                mouse=PrivateInput(session.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool);mouse.move(180,350)
                request(control,'launcher');state(label+'-launcher',lambda s:s['view']=='Launcher' and s['presentation']['overview']==0)
            launch('first');capture('initial-inbox');assert config.read_text()==original
            menu('create-menu');click(160,110)
            state('new-name-field',lambda s:s['launcher']['editing_name'] and s['shell_text_focus'] and s['keyboard_visible'])
            for x,y in [(58,504),(300,504),(132,504),(300,557)]:click(x,y)
            capture('typed-page-name');click(260,186)
            work_id=saved('page-created',lambda p:len(p['pages'])==2 and p['pages'][1]['name']=='work')['launcher']['settings']['selected']
            capture('empty-work-page')
            swipe((90,300),(260,300));saved('swipe-to-inbox',lambda p:p['selected']==p['inbox']);time.sleep(.4)
            # The page scroll owns vertical movement and cannot select a page.
            swipe((180,450),(180,220));capture('vertically-scrolled-inbox')
            assert request(control,'status')['launcher']['settings']['selected']==1
            # Return the first row to view without depending on scroll inertia.
            for _ in range(4):
                swipe((180,160),(180,640));time.sleep(.3)
            time.sleep(.5);capture('inbox-restored-top')
            mouse.move(58,148);mouse.button(True);time.sleep(.6);mouse.move(180,148);time.sleep(.1);mouse.button(False)
            saved('same-page-reorder',lambda p:p['pages'][0]['applications'][:3]==['test.app01.desktop','test.app00.desktop','test.app02.desktop'])
            mouse.move(180,148);mouse.button(True);time.sleep(.6);capture('held-icon')
            before=config.read_bytes();mouse.move(350,300);time.sleep(.8);capture('edge-work-preview')
            assert config.read_bytes()==before,'Edge preview persisted before release'
            mouse.move(180,150);time.sleep(.15);mouse.button(False)
            saved('identity-moved-to-work',lambda p:p['selected']==work_id and next(x for x in p['pages'] if x['id']==work_id)['applications']==['test.app00.desktop'])
            capture('work-with-linked-favorite')
            page_menu('rename-menu','work');menu_item('Rename','rename-command')
            state('rename-focus',lambda s:s['launcher']['editing_name'] and s['shell_text_focus'])
            for code in [43,32,58,26]:mouse.tap(code)  # home, replacing selected old name
            time.sleep(.2);click(260,186)
            saved('renamed-home',lambda p:next(x for x in p['pages'] if x['id']==work_id)['name']=='home')
            page_menu('reorder-menu','home');menu_item('earlier','reorder-command')
            saved('page-reordered',lambda p:p['pages'][0]['id']==work_id and p['inbox']==1)
            click(180,650);state('menu-dismissed',lambda s:not s['launcher']['menu_open'])
            final=config.read_bytes();mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=6)==0
            launch('restarted');assert config.read_bytes()==final
            saved('saved-restart',lambda p:p['selected']==work_id and p['pages'][0]['name']=='home' and p['pages'][0]['applications']==['test.app00.desktop'])
            capture('after-restart');page_menu('remove-menu','home')
            menu_item('Remove','remove-command')
            saved('page-removed',lambda p:len(p['pages'])==1 and p['pages'][0]['applications']==[f'test.app{n:02}.desktop' for n in range(1,36)]+['test.app00.desktop'])
            records['passed']=True
            print('PASS: menu/OSK page creation, horizontal paging, vertical scroll, edge hold/drop, rename/reorder, saved restart and removal')
        finally:
            if mouse is not None:mouse.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:child.wait(timeout=5)
                    except subprocess.TimeoutExpired:child.kill();child.wait()
            for stream in logs:stream.close()
            (out/'validation.json').write_text(json.dumps(records,indent=2)+'\n')


if __name__=='__main__':main()
