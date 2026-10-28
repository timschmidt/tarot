#!/usr/bin/env python3
"""Quick Launch with real X11/Wayland input on a private Xvfb display.

Isolates the application catalog and preferences. Exercises an actual native
client, gesture preview/commit/cancel, favorite editing and saved restart.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time

from PIL import Image, ImageChops

from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    parser.add_argument('--scale',type=int,choices=[1,2],default=1)
    args=parser.parse_args();out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();temporary=tempfile.TemporaryDirectory(prefix='tarot-quick-launch-');work=Path(temporary.name)
    control=work/'control.sock';config=work/'settings.toml';catalog=work/'data/applications';catalog.mkdir(parents=True)
    (work/'empty-data').mkdir()
    report=out/'app.jsonl'
    app_id='org.tarot.InputProbe.desktop'
    config.write_text('version = 3\n[notifications]\ndnd = true\n[quick_launch]\napplications = ["'+app_id+'"]\n')
    for identifier,name in [(app_id,'Alpha Probe'),('test.beta.desktop','Beta'),('test.gamma.desktop','Gamma'),('test.delta.desktop','Delta'),('org.gnome.Calendar.desktop','Calendar')]:
        executable=(str(root/'target/debug/examples/input-probe')+' --exit-after-seconds 180 --report "'+str(report)+'"') if identifier==app_id else '/bin/false'
        (catalog/identifier).write_text('[Desktop Entry]\nType=Application\nName='+name+'\nExec='+executable+'\n')
    records={'validation':{'passed':False},'scale':args.scale};children=[];logs=[];mouse=session=None
    class ScaledInput(PrivateInput):
        def move(self,x,y):
            super().move(x*args.scale,y*args.scale)
    def log(name):
        f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=8);records[label]=value;return value
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')))
        value=state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
        with Image.open(out/(label+'.png')) as image:
            image.load()  # Read the finished PNG, not an image-preview impression.
            rgb=image.convert('RGB')
        for role,bounds,minimum in [('clock',(144,0,216,32),50),('right_status',(250,0,360,32),10)]:
            bounds=tuple(v*args.scale for v in bounds)
            bright=sum(min(pixel)>150 for pixel in rgb.crop(bounds).getdata())
            records.setdefault('status_pixels',{})[label+'-'+role]=bright
            assert bright>=minimum*args.scale*args.scale,(label,role,bright,value['clock'])
    def click(x,y):
        mouse.move(x,y);time.sleep(.08);mouse.button(True);time.sleep(.07);mouse.button(False);time.sleep(.12)
    def wave(x=42,y=610):
        mouse.move(180,706);time.sleep(.05);mouse.button(True);time.sleep(.05)
        mouse.move(180,610)
        state('wave-preview',lambda s:s['presentation']['quick_launch'] is not None)
        mouse.move(x,y)
        return state('wave-selected',lambda s:s['presentation']['quick_launch'] is not None
                     and s['presentation']['quick_launch']['selected']==(0 if x==42 else 4 if x==320 else None))
    try:
        read,write=os.pipe()
        try:
            xvfb=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(xvfb);os.close(write);write=None
            assert select.select([read],[],[],8)[0]
            number=os.read(read,32).decode().strip();assert number.isdigit() and xvfb.poll() is None
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty-data'))
        env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
        def launch(label):
            nonlocal session,mouse
            session=subprocess.Popen([str(root/'target/debug/tarot'),'--width',str(360*args.scale),'--height',str(720*args.scale),'--scale',str(args.scale),'--config-file',str(config),'--control-socket',str(control),'--input-method',str(root/'target/debug/tarot-keyboard'),'--exit-after-ms','120000'],env=env,stdout=log(label+'.log'),stderr=subprocess.STDOUT)
            children.append(session)
            await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
            initial=state(label+'-ready',lambda s:s.get('favorites',{}).get('applications')==[app_id] and s['application_count']==5)
            mouse=ScaledInput(session.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
            mouse.move(180,350);time.sleep(.2)
            return initial
        launch('first');capture('home-five-positions')
        click(42,650)
        app=state('native-app',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict)
                  and s['presentation']['windows'][0]['opacity']==1.0)
        assert app['cards'][0]['app_id']=='org.tarot.InputProbe'
        click(180,350)
        def input_events():
            return [json.loads(line) for line in report.read_bytes().split(b'\n')[:-1] if line]
        count=lambda:sum(e['event']=='pointer_button' for e in input_events())
        await_value(count,lambda n:n>=2)
        native_buttons=count()
        capture('native-app-before-wave')
        before_wave=records['native-app-before-wave']
        shown=wave();assert shown['view']==app['view'] and shown['cards']==app['cards']
        assert shown['presentation']['windows']==app['presentation']['windows']
        capture('wave-over-native-app')
        if before_wave['clock']==shown['clock']:
            bounds=(144*args.scale,0,360*args.scale,32*args.scale)
            ordinary=Image.open(out/'native-app-before-wave.png').convert('RGB').crop(bounds)
            overlay=Image.open(out/'wave-over-native-app.png').convert('RGB').crop(bounds)
            changed=ImageChops.difference(ordinary,overlay).getbbox()
            records['status_during_wave']={'changed_bounds':changed,'scale':args.scale}
            assert changed is None,'The clock and observed status must remain visible during Quick Launch'
        mouse.button(False)
        assert count()==native_buttons
        state('favorite-focuses-same-app',lambda s:s['view']==app['view'] and s['presentation']['quick_launch'] is None)
        assert len(state('no-duplicate-launch')['cards'])==1
        # Empty/outside release must return to the same app, not overview.
        wave(2,610);mouse.button(False)
        state('outside-release-cancels',lambda s:s['view']==app['view'] and s['presentation']['quick_launch'] is None)
        wave(320,610);mouse.button(False)
        launcher=state('wave-launcher',lambda s:s['view']=='Launcher' and all(w['opacity']==0.0 for w in s['presentation']['windows']))
        assert launcher['calendar_day']==datetime.datetime.now().day
        capture('launcher-live-calendar')
        # Editor uses real UI presses; reordered identities, not labels, persist.
        click(285,67);state('editor-open',lambda s:s['editing_favorites'])
        capture('editor-before')
        click(165,259);state('second-favorite-added',lambda s:s['favorites']['applications']==[app_id,'test.beta.desktop'])
        click(208,225);state('favorite-reordered',lambda s:s['favorites']['applications']==['test.beta.desktop',app_id])
        click(285,67);state('editor-done-before-failure',lambda s:not s['editing_favorites'])
        request(control,'activate',id=app['cards'][0]['id'])
        state('app-before-failed-favorite',lambda s:s['view']==app['view'] and s['presentation']['windows'][0]['opacity']==1.0)
        wave(42,610);mouse.button(False)
        failed=state('failed-favorite-keeps-app',lambda s:s['launch_feedback'] is not None and not s['launch_feedback']['pending'])
        assert failed['view']==app['view'] and failed['cards']==app['cards']
        capture('failed-favorite-message')
        buttons=count();click(324,failed['keyboard_rect']['y']-120)
        state('launch-message-dismissed',lambda s:s['launch_feedback'] is None)
        assert count()==buttons
        wave(320,610);mouse.button(False);state('launcher-after-failure',lambda s:s['view']=='Launcher')
        click(285,67);state('editor-again',lambda s:s['editing_favorites'])
        click(320,174);state('favorite-removed',lambda s:s['favorites']['applications']==[app_id])
        click(285,67);state('editor-done',lambda s:not s['editing_favorites'])
        saved=await_value(config.read_text,lambda text:'org.tarot.InputProbe.desktop' in text and 'test.beta.desktop' not in text)
        assert 'dnd = true' in saved
        records['saved_preferences']=saved
        mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=5)==0
        launch('restarted');capture('saved-dock-after-restart')
        assert 'dnd = true' in config.read_text()
        text_report=out/'text.jsonl';text_commands=out/'text.commands';text_commands.write_text('')
        app_env=dict(env,WAYLAND_DISPLAY=state('text-display')['wayland_display'])
        text=subprocess.Popen([str(root/'target/debug/examples/text-probe'),'--role','app','--report',str(text_report),'--commands',str(text_commands)],env=app_env,stdout=log('text.log'),stderr=subprocess.STDOUT)
        children.append(text)
        text_state=state('native-keyboard',lambda s:s['keyboard_visible'] and len(s['cards'])==1 and s['presentation']['windows'][0]['opacity']==1.0)
        wave(42,610);capture('wave-over-native-keyboard')
        mouse.move(2,610);mouse.button(False)
        restored=state('keyboard-after-cancel',lambda s:s['view']==text_state['view'] and s['presentation']['quick_launch'] is None)
        assert restored['keyboard_rect']==text_state['keyboard_rect'] and restored['keyboard_visible']
        click(21,restored['keyboard_rect']['y']+52)
        def text_events():
            return [json.loads(line) for line in text_report.read_bytes().split(b'\n')[:-1] if line]
        await_value(text_events,lambda es:any(e['event']=='applied' and e['text']=='hello q' for e in es))
        request(control,'launcher');state('launcher-before-search',lambda s:s['view']=='Launcher')
        click(80,116);state('search-focused',lambda s:s['shell_text_focus'])
        wave(320,610);capture('wave-over-shell-keyboard');mouse.button(False)
        state('search-focus-after-wave',lambda s:s['view']=='Launcher' and s['shell_text_focus'] and s['presentation']['quick_launch'] is None)
        mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=5)==0
        records['validation']=dict(passed=True,private_display_and_catalog=True,native_client=True,
            wave_preserves_geometry_and_view=True,release_focuses_existing_app=True,
            outside_release_cancels=True,fixed_launcher=True,actual_editor_add_move_remove=True,failed_launch_feedback_without_input_leak=True,
            favorites_survive_restart=True,other_preferences_preserved=True,native_and_shell_keyboard_wave=True,typing_after_wave_cancel=True,live_calendar_gles=True,status_pixels_verified=True,phone_kms_tested=False)
        print('PASS: native app Quick Launch wave, commit/cancel, fixed launcher, actual favorite editing and saved restart')
    finally:
        if mouse is not None:mouse.close()
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for f in logs:f.close()
        (out/'status.json').write_text(json.dumps(records,indent=2)+'\n');temporary.cleanup()


if __name__=='__main__':main()
