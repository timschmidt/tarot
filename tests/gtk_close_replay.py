#!/usr/bin/env python3
"""Real GTK editor close/cancel/save with private display, bus and user directories."""
import argparse
import csv
import io
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time
from PIL import Image
from hardware_replay import PrivateInput
from capture_motion import Motion
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--xvfb',default='Xvfb')
    p.add_argument('--editor',default='/usr/bin/gnome-text-editor')
    p.add_argument('--tesseract',default='tesseract')
    args=p.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False,mode=0o700)
    root=Path.cwd();temporary=tempfile.TemporaryDirectory(prefix='tarot-gtk-close-');work=Path(temporary.name)
    control=work/'control.sock';children=[];groups=[];logs=[];mouse=None
    records={'validation':{'passed':False},'compositor_sha256':hashlib.sha256((root/'target/debug/tarot').read_bytes()).hexdigest()}
    (out/'replay-source.py').write_bytes(Path(__file__).read_bytes())
    def log(name):
        f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        s=await_value(lambda:request(control,'status'),condition,seconds=10);records[label]=s;return s
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')))
        state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
        with Image.open(out/(label+'.png')) as im:im.load()
    def click(x,y):
        mouse.move(x*2,y*2);time.sleep(.06);mouse.button(True);time.sleep(.06);mouse.button(False)
    def button(label,word):
        found=locate(out/(label+'.png'),word,args.tesseract)
        records.setdefault('button_labels',[]).append({'capture':label,**found})
        click(found['point'][0]/2,found['point'][1]/2)
    def chord(key):
        mouse.key(37,True)
        try:mouse.tap(key)
        finally:mouse.key(37,False)
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None
            assert select.select([read],[],[],8)[0];number=os.read(read,32).decode().strip();assert number.isdigit()
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',GSETTINGS_BACKEND='keyfile',GTK_A11Y='none',GTK_USE_PORTAL='0')
        for key,name in [('HOME','home'),('XDG_RUNTIME_DIR','runtime'),('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache'),('XDG_STATE_HOME','state')]:
            folder=work/name;folder.mkdir(mode=0o700);env[key]=str(folder)
        env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
        env['DBUS_SESSION_BUS_ADDRESS']='unix:path='+str(work/'bus')
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--width','720','--height','1440','--scale','2','--control-socket',str(control),'--exit-after-ms','120000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT)
        children.append(session);await_value(lambda:control.exists() or session.poll() is not None,bool);assert session.poll() is None
        initial=state('ready');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native=dict(env,WAYLAND_DISPLAY=initial['wayland_display'],GDK_BACKEND='wayland');native.pop('DISPLAY',None)
        bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+env['DBUS_SESSION_BUS_ADDRESS']],env=native,stdout=log('bus.log'),stderr=subprocess.STDOUT,start_new_session=True)
        children.append(bus);groups.append(bus);await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
        subprocess.run(['gsettings','set','org.gnome.TextEditor','restore-session','false'],env=native,check=True,capture_output=True)
        assert subprocess.check_output(['gsettings','get','org.gnome.TextEditor','restore-session'],env=native,text=True).strip()=='false'
        records['editor_version']=subprocess.check_output([args.editor,'--version'],env=native,text=True)
        document=work/'tarot-close-test.txt';original=b'Original Tarot test document\n';document.write_bytes(original)
        app=subprocess.Popen([args.editor,'--ignore-session',str(document)],env=native,stdout=log('editor.log'),stderr=subprocess.STDOUT,start_new_session=True)
        children.append(app);groups.append(app)
        s=state('editor-mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and s['presentation']['windows'][0]['rect']==s['app_area'])
        identifier=s['cards'][0]['id'];time.sleep(.4)
        def original_word():
            capture('editor-ready')
            ready=subprocess.run([args.tesseract,str(out/'editor-ready.png'),'stdout','--psm','3','tsv'],capture_output=True,text=True,check=True,timeout=20)
            return [r for r in csv.DictReader(io.StringIO(ready.stdout),delimiter='\t') if r['text']=='Original' and float(r['conf'])>=80]
        rows=await_value(original_word,lambda rows:len(rows)==1,seconds=10)
        row=rows[0];click((int(row['left'])+int(row['width'])/2)/2,(int(row['top'])+int(row['height'])/2)/2)
        time.sleep(.2);chord(38)
        for key in [28,38,27,32,28]:mouse.tap(key);time.sleep(.04)
        time.sleep(.3);assert document.read_bytes()==original and app.poll() is None
        capture('edited')
        with Motion(control,out,'gtk-minimize'):
            request(control,'overview');state('edited-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        capture('edited-card')
        request(control,'close',id=identifier);time.sleep(.6)
        s=state('close-requested');assert app.poll() is None and len(s['cards'])==1 and s['cards'][0]['id']==identifier and s['cards'][0]['closing'],s
        capture('close-requested-card')
        v=s['presentation']['windows'][0]['rect'];click(v['x']+v['width']/2,v['y']+v['height']/2)
        state('close-prompt-active',lambda s:s['view']=={'App':identifier} and s['presentation']['windows'][0]['rect']==s['app_area'])
        capture('close-confirmation')
        button('close-confirmation','Cancel');time.sleep(.3)
        assert document.read_bytes()==original and app.poll() is None
        capture('cancelled-unsaved')
        request(control,'overview');state('cancelled-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        request(control,'close',id=identifier);time.sleep(.6)
        s=state('second-close-requested');assert len(s['cards'])==1 and app.poll() is None
        v=s['presentation']['windows'][0]['rect'];click(v['x']+v['width']/2,v['y']+v['height']/2)
        state('second-close-active',lambda s:s['view']=={'App':identifier} and s['presentation']['windows'][0]['rect']==s['app_area'])
        capture('second-close-confirmation');button('second-close-confirmation','Save')
        await_value(document.read_bytes,lambda data:data==b'tarot\n',seconds=8)
        state('saved-cooperative-close',lambda s:not s['cards']);assert app.wait(timeout=8)==0
        mouse.close();mouse=None;request(control,'exit');assert session.wait(timeout=6)==0
        records['validation']={'passed':True,'real_gtk_editor':True,'private_display_bus_and_user_directories':True,'unsaved_close_retains_same_card':True,'actual_card_click_reopens_prompt':True,'actual_cancel_button_refuses_close':True,'actual_save_button_preserves_edited_bytes':True,'saved_close_is_cooperative':True,'phone_tested':False}
        print('PASS: real GTK unsaved close, card reactivation, Cancel, saved bytes and cooperative close')
    finally:
        if mouse is not None:mouse.close()
        for child in reversed(children):
            if child.poll() is None:
                if child in groups:
                    assert os.getpgid(child.pid)==child.pid
                    os.killpg(child.pid,signal.SIGTERM)
                else:child.terminate()
                try:child.wait(timeout=5)
                except subprocess.TimeoutExpired:child.kill();child.wait()
        for f in logs:f.close()
        (out/'status.json').write_text(json.dumps(records,indent=2)+'\n');temporary.cleanup()


if __name__=='__main__':main()
