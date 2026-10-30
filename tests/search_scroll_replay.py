#!/usr/bin/env python3
"""Focused search result scrolling through actual nested input and rendered selection."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from PIL import Image,ImageOps
from hardware_replay import PrivateInput
from dialog_button import locate
from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from with_xvfb import read_display_number

LETTERS = dict(zip('qwertyuiopasdfghjklzxcvbnm', [24,25,26,27,28,29,30,31,32,33,38,39,40,41,42,43,44,45,46,52,53,54,55,56,57,58]))
PUNCTUATION = {' ': (65, False), ':': (47, True), '/': (61, False), '.': (60, False), '?': (61, True), '=': (21, False), '&': (16, True)}


def dashboard_band(path,scale):
    """Find the reserved dashboard's painted rows, excluding foreground results.

    Fixed height fractions can cut through both visible titles in landscape.
    The tested Nocturne panel/row colors establish the actual viewport instead.
    """
    with Image.open(path) as source:
        image=source.convert('RGB');width,height=image.size
    edges=[round(12*scale),round(width-12*scale)]
    panel=[y for y in range(round(32*scale),round(height-28*scale))
           if all(image.getpixel((x,y))==(24,32,36) for x in edges)]
    assert panel,'The opaque dashboard panel must be visible'
    columns=[round(width*f) for f in (.45,.68,.9)]
    rows=[y for y in range(min(panel),max(panel)+1)
          if sum(image.getpixel((x,y))==(35,40,44) for x in columns)>=2]
    assert rows and max(rows)-min(rows)>=24*scale,'The actual dashboard row viewport must be visible'
    top,bottom=min(rows),max(rows)+1
    return {'vertical':(top/height,bottom/height),'pixel_bounds':(top,bottom),
            'panel_pixel_bounds':(min(panel),max(panel)+1),'row_scan_columns':columns}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--xvfb', default='Xvfb')
    p.add_argument('--scale', type=int, choices=[1,2], default=1)
    p.add_argument('--landscape', action='store_true')
    p.add_argument('--scroll-mode',choices=['drag','wheel'],required=True)
    p.add_argument('--application-count',type=int,choices=[1,3],required=True)
    p.add_argument('--binary',type=Path,default=Path('target/debug/tarot'))
    args = p.parse_args(); out = args.output.resolve(); out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd(); width,height = (720,360) if args.landscape else (360,720)
    records = {'passed': False, 'scale': args.scale, 'landscape': args.landscape,
               'phone_kms_tested': False, 'network_queries_sent': False, 'actions': [],
               'compositor_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'application_count':args.application_count}
    source_root=Path(__file__).resolve().parent; snapshots=out/'qa-sources';snapshots.mkdir()
    pending=[Path(__file__).resolve(),source_root/'just_type_uri_fixture.py'];sources={}
    while pending:
        path=pending.pop()
        if path.name in sources:continue
        payload=path.read_bytes();sources[path.name]=hashlib.sha256(payload).hexdigest();(snapshots/path.name).write_bytes(payload)
        for node in ast.walk(ast.parse(payload)):
            modules=[a.name for a in node.names] if isinstance(node,ast.Import) else [node.module] if isinstance(node,ast.ImportFrom) and node.module else []
            for module in modules:
                candidate=source_root/(module.split('.')[0]+'.py')
                if candidate.is_file() and candidate.name not in sources:pending.append(candidate)
    records['executed_sources_sha256']=sources
    children=[]; logs=[]; mouse=fixture=session=None
    with tempfile.TemporaryDirectory(prefix='tarot-just-type-') as temporary:
        work=Path(temporary);control=work/'control.sock';config=work/'settings.toml'
        catalog=work/'data/applications';catalog.mkdir(parents=True);(work/'empty').mkdir();(work/'config').mkdir()
        original='version = 3\n[notifications]\ndnd = true\nhide_previews = false\n'
        config.write_text(original)
        uri_socket=work/'uri.sock';uri_report=out/'uri.jsonl';native_report=out/'native.jsonl'
        identifier='org.tarot.InputProbe.desktop'
        executable=f'"{sys.executable}" "{root}/tests/just_type_uri_fixture.py" --socket "{uri_socket}" --report "{uri_report}" --probe "{root}/target/debug/examples/input-probe" --native-report "{native_report}" %u'
        (catalog/identifier).write_text('[Desktop Entry]\nType=Application\nName=Reader\nKeywords=read;books;\nExec='+executable+'\nMimeType=x-scheme-handler/http;x-scheme-handler/https;\n')
        for n in range(15):
            (catalog/f'test.extra{n}.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=Z Extra {n:02}\nExec=/bin/false\n'+('Keywords=volume;\n' if n<args.application_count else ''))
        (work/'config/tarot-mimeapps.list').write_text('[Default Applications]\nx-scheme-handler/http='+identifier+';\nx-scheme-handler/https='+identifier+';\n')
        def log(name):
            stream=(out/name).open('x');logs.append(stream);return stream
        def state(label, predicate=lambda s:True):
            value=await_value(lambda:request(control,'status'),predicate,seconds=12)
            records['actions'].append({'label':label,'state':value});return value
        def capture(label):
            request(control,'capture',path=str(out/(label+'.png')))
            value=state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
            with Image.open(out/(label+'.png')) as image:
                image.load();assert image.size==(width*args.scale,height*args.scale)
            return value
        def dashboard_word(label,word):
            path=out/(label+'.png');band=dashboard_band(path,args.scale)
            records.setdefault('dashboard_bands',{})[label]=band
            return locate(path,word,vertical=band['vertical'])
        def click(x,y):
            mouse.move(x*args.scale,y*args.scale);time.sleep(.05);mouse.button(True);time.sleep(.06);mouse.button(False);time.sleep(.1)
        def type_ascii(value):
            for ch in value:
                code,shift=(LETTERS[ch],False) if ch in LETTERS else PUNCTUATION[ch]
                if shift:mouse.key(50,True)
                mouse.tap(code)
                if shift:mouse.key(50,False)
                time.sleep(.025)
        def query(value,label):
            click(width-32,57 if args.landscape else 64)
            state(label+'-clear',lambda s:not s['launcher_query'] and s['shell_text_focus'])
            type_ascii(value)
            return state(label,lambda s:s['launcher_query']==value)
        def open_from_cards(label):
            mouse.tap(67)  # F1
            state(label+'-cards',lambda s:s['view']=='Overview' and s['presentation']['overview']==1)
            click(120,60);return state(label,lambda s:s['just_type']['active'] and s['shell_text_focus'])
        def uri_events():
            return [json.loads(x) for x in uri_report.read_text().splitlines()] if uri_report.exists() else []
        def uri_count():return sum(e['event']=='uri' for e in uri_events())
        try:
            read,write=os.pipe()
            try:
                xvfb=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
            finally:
                os.close(read)
                if write is not None:os.close(write)
            address='unix:path='+str(work/'bus')
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+address],stdout=log('bus.log'),stderr=subprocess.STDOUT);children.append(bus)
            await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty'),XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'),DBUS_SESSION_BUS_ADDRESS=address)
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            session=subprocess.Popen([str(args.binary.resolve()),'--width',str(width*args.scale),'--height',str(height*args.scale),'--scale',str(args.scale),'--config-file',str(config),'--publish-session-environment','--notifications','--control-socket',str(control),'--exit-after-ms','240000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT);children.append(session)
            await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
            mouse=PrivateInput(session.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool);mouse.move(180,300)
            state('ready',lambda s:s['application_count']==16 and s['notifications']['available'])
            # The first physical keyboard character starts a query in Home.
            mouse.tap(54);state('home-first-character',lambda s:s['just_type']['active'] and s['launcher_query']=='c')
            capture('home-search');assert config.read_text()==original
            mouse.tap(9);state('home-search-closed',lambda s:s['view']=='Home' and not s['just_type']['active'])
            request(control,'launch',id=identifier)
            first=state('original-native',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and bool(s['presentation']['windows']) and s['presentation']['windows'][0]['opacity']==1)
            native=first['cards'][0]['id'];await_value(uri_socket.exists,bool)
            open_from_cards('card-entry');query('reader','application-query');capture('application-results')
            click(64,100 if args.landscape else 182)
            state('existing-card-focused',lambda s:s['view']=={'App':native} and len(s['cards'])==1 and not s['keyboard_visible'])
            open_from_cards('settings-entry');query('volume','settings-query');capture('settings-results')
            assert args.landscape and args.scale==2
            query_revision=request(control,'status')['just_type']['revision']
            output_revision=request(control,'status')['output']['revision']
            found=None
            for attempt in range(8):
                label='volume-target-'+str(attempt);sample=capture(label)
                assert sample['launcher_query']=='volume' and sample['just_type']['active']
                assert sample['just_type']['revision']==query_revision and sample['output']['revision']==output_revision and sample['output']['presented_revision']==output_revision
                top=84;bottom=sample['keyboard_rect']['y']-4
                path=out/(label+'.png');text=out/(label+'-dark.png')
                with Image.open(path) as image:ImageOps.grayscale(image).point(lambda value:0 if value<128 else 255).save(text)
                try:found=locate(text,'outputs',vertical=(top/height,bottom/height))
                except AssertionError:
                    if attempt==7:raise
                    x=width-128;y=bottom-8;end=max(top+8,y-90)
                    mouse.move(x*args.scale,y*args.scale)
                    time.sleep(.04)
                    if args.scroll_mode=='wheel':
                        for down in [True,False]:mouse.xtest.XTestFakeButtonEvent(mouse.display,5,int(down),0)
                        mouse.x.XSync(mouse.display,0)
                    else:
                        mouse.button(True)
                        if args.scroll_mode=='drag':time.sleep(.03)
                        for step in range(1,5):
                            mouse.move(x*args.scale,(y+(end-y)*step/4)*args.scale)
                            if args.scroll_mode=='drag':time.sleep(.02)
                        if args.scroll_mode=='drag':time.sleep(.03)
                        mouse.button(False)
                    time.sleep(.25)
                    continue
                records['focused_scroll']={'mode':args.scroll_mode,'attempt':attempt,'ocr':found,'query_revision':sample['just_type']['revision'],'output_revision':sample['output']['revision']}
                assert sample['keyboard_visible'] and sample['shell_text_focus']
                click(found['point'][0]/args.scale,found['point'][1]/args.scale)
                break
            state('sound-setting' ,lambda s:s['system_menu_open'] and s['system_page']=='Audio' and s['launcher_query']=='volume')
            records['passed']=True;print('PASS: focused narrow viewport scroll',args.scroll_mode);return
        finally:
            if fixture is not None:fixture.close()
            if mouse is not None:mouse.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:child.wait(timeout=5)
                    except subprocess.TimeoutExpired:child.kill();child.wait()
            for stream in logs:stream.close()
            (out/'status.json').write_text(json.dumps(records,indent=2)+'\n')
    print('PASS: Just Type actual-input/rendering/local content and explicit isolated GIO handoff')


if __name__=='__main__':main()
