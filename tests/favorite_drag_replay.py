#!/usr/bin/env python3
"""Real favorite manipulation on an owned X server and private app catalog."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import tomllib

from PIL import Image, ImageChops
from dialog_button import locate
from hardware_replay import PrivateInput
from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    parser.add_argument('--scale',type=int,choices=[1,2],default=1)
    parser.add_argument('--landscape',action='store_true')
    parser.add_argument('--retained-notification',action='store_true')
    parser.add_argument('--binary',type=Path,default=Path('target/debug/tarot'))
    args=parser.parse_args();out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();temporary=tempfile.TemporaryDirectory(prefix='tarot-favorite-drag-');work=Path(temporary.name)
    width,height=(720,360) if args.landscape else (360,720)
    control=work/'control.sock';config=work/'settings.toml';catalog=work/'data/applications';catalog.mkdir(parents=True)
    (work/'empty-data').mkdir()
    original='org.tarot.InputProbe.desktop';beta='test.beta.desktop';delta='test.delta.desktop';epsilon='test.epsilon.desktop';gamma='test.gamma.desktop'
    report=out/'app.jsonl'
    config.write_text('version = 3\n[notifications]\ndnd = true\n[quick_launch]\napplications = ["'+original+'"]\n')
    for identifier,name in [(original,'Alpha'),(beta,'Beta'),(delta,'Delta'),(epsilon,'Epsilon'),(gamma,'Gamma')]:
        executable=(str(root/'target/debug/examples/input-probe')+' --exit-after-seconds 180 --report "'+str(report)+'"') if identifier==original else '/bin/false'
        (catalog/identifier).write_text('[Desktop Entry]\nType=Application\nName='+name+'\nExec='+executable+'\n')
    records={'passed':False,'scale':args.scale,'landscape':args.landscape,
        'compositor_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'actions':[],'retained_notification':args.retained_notification,'qa_source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    children=[];logs=[];mouse=session=fixture=None
    class ScaledInput(PrivateInput):
        def move(self,x,y):return super().move(x*args.scale,y*args.scale)
    def log(name):
        stream=(out/name).open('x');logs.append(stream);return stream
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=12);records[label]=value;return value
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')))
        value=state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
        with Image.open(out/(label+'.png')) as im:
            im.load();assert im.size==(width*args.scale,height*args.scale)
        return value
    def click(x,y):
        mouse.move(x,y);time.sleep(.06);mouse.button(True);time.sleep(.06);mouse.button(False);time.sleep(.1)
    def favorite(label,expected):
        value=state(label,lambda s:s['favorites']['applications']==expected and s['display_settings']['settings_saved'])
        data=tomllib.loads(config.read_text());assert data['quick_launch']['applications']==expected and data['notifications']['dnd']
        assert value['launch_feedback'] is None and len(value['cards'])==1 and value['cards'][0]['id']==native
        return value
    def launcher(label):
        request(control,'launcher')
        state(label,lambda s:s['view']=='Launcher' and not s['editing_favorites'] and not s['keyboard_visible']
            and all(w['opacity']==0 for w in s['presentation']['windows']))
    def overview(label):
        request(control,'overview');state(label,lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and s['presentation']['dock']==1)
    def tray_top():
        return height-36-request(control,'status')['notifications']['well']['height']-69-4
    def slot(index):return 3+(index+.5)*(width-6)/5,tray_top()+32
    def hold(point,label):
        mouse.move(*point);time.sleep(.08);mouse.button(True);time.sleep(.55)
        capture(label+'-held')
        found=locate(out/(label+'-held.png'),'Release',vertical=(.04,.94))
        records['actions'].append({'label':label,'held_label':found})
    def source(identifier,filtered=False):
        apps=request(control,'applications')['applications']
        if filtered:apps=[app for app in apps if 'b' in app['name'].casefold() or 'b' in app['description'].casefold() or any('b' in word.casefold() for word in app['keywords'])]
        index=[app['id'] for app in apps].index(identifier);tile=(width-32-24)/3
        top,stride=(100,80) if filtered and args.landscape else (182,112) if filtered else (148,112)
        return 16+(index%3)*(tile+8)+tile/2,top+(index//3)*stride
    def place(identifier,index,expected,label):
        before=request(control,'status')['favorites']['applications'];saved=config.read_bytes()
        hold(source(identifier),label);mouse.move(*slot(index));time.sleep(.1);capture(label+'-preview')
        assert request(control,'status')['favorites']['applications']==before and config.read_bytes()==saved
        mouse.button(False);favorite(label+'-committed',expected)
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None;number=read_display_number(read);assert server.poll() is None
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty-data'))
        env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
        if args.retained_notification:
            (work/'runtime').mkdir(mode=0o700)
            env['XDG_RUNTIME_DIR']=str(work/'runtime')
            address='unix:path='+str(work/'bus')
            env['DBUS_SESSION_BUS_ADDRESS']=address
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address],stdout=log('bus.log'),stderr=subprocess.STDOUT)
            children.append(bus)
            await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool)
            assert bus.poll() is None
        def launch(label,expected):
            nonlocal session,mouse
            session=subprocess.Popen([str(args.binary.resolve()),*(['--notifications','--publish-session-environment'] if args.retained_notification else []),'--width',str(width*args.scale),'--height',str(height*args.scale),'--scale',str(args.scale),'--config-file',str(config),'--control-socket',str(control),'--exit-after-ms','180000'],env=env,stdout=log(label+'.log'),stderr=subprocess.STDOUT)
            children.append(session)
            await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
            state(label+'-ready',lambda s:s['favorites']['applications']==expected and s['application_count']==5)
            mouse=ScaledInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool);mouse.move(width/2,height/2);time.sleep(.1)
        launch('first',[original]);request(control,'launch',id=original)
        s=state('native',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and any(w['opacity']==1 for w in s['presentation']['windows']))
        native=s['cards'][0]['id']
        if args.retained_notification:
            state('notification-ready',lambda s:s['notifications']['available'])
            fixture=Fixture(root/'target/debug/examples/notification-probe',out/'notification',env)
            identifier=fixture.notify(summary='Retained drag fixture',resident=True,timeout=0)
            state('retained-before-drag',lambda s:any(n['id']==identifier for n in s['notifications']['items'])
                and s['notifications']['well']['height']==48 and s['notifications']['banner'] is None)
        launcher('launcher-before-drag');capture('launcher-resting')
        place(beta,3,[original,beta],'add-beta')
        with Image.open(out/'launcher-resting.png') as a,Image.open(out/'add-beta-held.png') as b:
            bounds=tuple(round(v*args.scale) for v in (3,tray_top(),width-3,tray_top()+69))
            delta_image=ImageChops.difference(a.convert('RGB').crop(bounds),b.convert('RGB').crop(bounds))
            changed=sum(max(pixel)>15 for pixel in delta_image.getdata());assert changed>1000*args.scale*args.scale
            records['tray_reveal_changed_pixels']=changed
        overview('dock-before-reorder')
        x,y=slot(1);hold((x,y-31),'dock-top-reorder');mouse.move(*slot(0));time.sleep(.1)
        capture('dock-reorder-preview');assert request(control,'status')['favorites']['applications']==[original,beta]
        mouse.button(False);favorite('dock-reordered',[beta,original])
        launcher('launcher-again');place(delta,1,[beta,delta,original],'insert-delta')
        place(epsilon,3,[beta,delta,original,epsilon],'add-epsilon')
        before=config.read_bytes();hold(source(gamma),'full-tray');mouse.move(*slot(0));time.sleep(.1);capture('full-tray-preview')
        records['full_label']=locate(out/'full-tray-preview.png','full',vertical=(.04,.94))
        mouse.button(False);favorite('full-rejected',[beta,delta,original,epsilon]);assert config.read_bytes()==before
        place(delta,3,[beta,original,epsilon,delta],'reorder-existing-full')
        # The fifth position takes an application in place of Apps; dragging
        # it off the dock brings Apps back.
        hold(source(gamma),'fifth-position');mouse.move(*slot(4));capture('fifth-preview');mouse.button(False)
        state('fifth-placed',lambda s:s['favorites'].get('fifth')==gamma and s['favorites']['applications']==[beta,original,epsilon,delta])
        capture('fifth-placed')
        overview('dock-before-fifth-removal');hold(slot(4),'remove-fifth')
        mouse.move(width/2,tray_top()-32);time.sleep(.1);mouse.button(False)
        state('fifth-cleared',lambda s:s['favorites'].get('fifth') is None and s['favorites']['applications']==[beta,original,epsilon,delta])
        capture('fifth-cleared');launcher('launcher-after-fifth')
        before_status=capture('before-status-cancel')
        hold(source(gamma),'outside-cancel');mouse.move(2,0)
        after_status=capture('drag-at-status-boundary')
        if before_status['clock']==after_status['clock']:
            bounds=(0,0,width*args.scale,32*args.scale)
            with Image.open(out/'before-status-cancel.png') as a,Image.open(out/'drag-at-status-boundary.png') as b:
                difference=ImageChops.difference(a.convert('RGB').crop(bounds),b.convert('RGB').crop(bounds)).getbbox()
            records['status_boundary_changed_bounds']=difference;assert difference is None
        mouse.button(False)
        favorite('outside-unchanged',[beta,original,epsilon,delta])
        point=source(beta);mouse.move(*point);mouse.button(True);time.sleep(.04);mouse.move(point[0],point[1]+60);time.sleep(.6);mouse.button(False)
        favorite('early-motion-unchanged',[beta,original,epsilon,delta])
        hold(source(beta),'view-interruption');request(control,'overview');time.sleep(.2);mouse.button(False)
        favorite('interruption-unchanged',[beta,original,epsilon,delta]);capture('after-interruption')
        overview('dock-before-removal');hold(slot(0),'remove-beta')
        mouse.move(width/2,tray_top()-32);time.sleep(.1);capture('remove-preview');mouse.button(False)
        favorite('removed-beta',[original,epsilon,delta])
        hold(slot(0),'remove-running-favorite');mouse.move(width/2,tray_top()-32);mouse.button(False)
        favorite('running-card-retained',[epsilon,delta]);launcher('launcher-readd-original')
        place(original,0,[original,epsilon,delta],'readd-original')
        # A real launcher search opens the shared keyboard. Holding the filtered
        # tile must retain its identity as the keyboard closes and tray appears.
        click(40,16);state('search-menu',lambda s:s['launcher']['menu_open']);click(160,60);state('search-keyboard',lambda s:s['shell_text_focus'] and s['keyboard_visible'])
        mouse.tap(56);time.sleep(.15)  # X11 US key b; only Beta matches.
        capture('filtered-search-keyboard')
        hold(source(beta,filtered=True),'keyboard-visible-source')
        state('keyboard-hidden-for-drag',lambda s:not s['keyboard_visible'] and not s['shell_text_focus'])
        mouse.move(*slot(0));mouse.button(False);favorite('filtered-source-added',[beta,original,epsilon,delta])
        data=[json.loads(line) for line in report.read_text().splitlines()]
        assert not any(e['event']=='pointer_button' for e in data),'Launcher drag leaked a native app click'
        records['native_clicks_during_edit']=0
        request(control,'close',id=native);state('native-cooperative-close',lambda s:not s['cards'])
        final=config.read_bytes();mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=6)==0
        launch('restarted',[beta,original,epsilon,delta]);assert config.read_bytes()==final;capture('saved-favorites-after-restart')
        records['passed']=True;records['scope']={'actual_pointer_and_gles':True,'preview_without_persistence':True,'full_tray_and_fifth_position':True,
            'dock_top_ownership':True,'existing_identity_reordered':True,'explicit_removal_keeps_native_card':True,
            'cancel_scroll_and_view_change':True,'keyboard_visible_filtered_source':True,'saved_restart':True,'phone_kms_tested':False}
        print('PASS: real favorite hold/place/reorder/remove, cancel/full, fifth position, keyboard and restart',out)
    finally:
        if fixture is not None:fixture.close()
        if mouse is not None:mouse.close()
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for stream in logs:stream.close()
        (out/'validation.json').write_text(json.dumps(records,indent=2)+'\n');temporary.cleanup()


if __name__=='__main__':main()
