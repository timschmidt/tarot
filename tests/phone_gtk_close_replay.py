"""Real editor close refusal, native touch Cancel/Save and exact file bytes.

The wrapper supplies a private user-directory tree and a temporary catalog entry.
Only synthetic fixture text enters this application's file and captured images.
"""
import json
from pathlib import Path
import subprocess
import time
from notification_replay import await_value
from phone_gesture_replay import request
from capture_motion import Motion
from dialog_button import locate
from phone_lock_replay import Keyboard
from phone_touch_keyboard import Keyboard as TouchKeyboard


def replay(socket,screen,guard,tap,state,capture,before,out,session_cgroup):
    record={'passed':False,'physical_finger_tested':False,'button_labels':[]};keyboard=None
    home=out.parent/'editor-home';document=home/'tarot-close-test.txt';original=document.read_bytes();assert original==b'Original Tarot test document\n'
    def button(label,word):
        found=locate(out/(label+'.png'),word)
        record['button_labels'].append({'capture':label,**found})
        tap(found['point'][0]/2,found['point'][1]/2)
    def window(s,identifier):return next(v for v in s['presentation']['windows'] if v['id']==identifier)
    def ordinary(s):return s.get('lock',{}).get('phase')=='Unlocked'
    def activate_prompt(identifier,label):
        s=state(label+'-card',lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        with Motion(socket,out,label+'-restore'):
            time.sleep(.4);r=window(s,identifier)['rect'];tap(r['x']+r['width']/2,r['y']+r['height']/2)
            state(label+'-active',lambda s:ordinary(s) and s['view']=={'App':identifier} and window(s,identifier)['rect']==s['app_area'])
        capture(label)
    try:
        with Motion(socket,out,'gtk-launch'):
            request(socket,'launch',id='test.tarot.GtkClose.desktop')
            s=state('gtk-close-mapped',lambda s:ordinary(s) and len(s['cards'])==len(before['cards'])+1
                and isinstance(s['view'],dict) and s['presentation']['windows'][-1]['rect']==s['app_area'])
        identifier=next(c['id'] for c in s['cards'] if c['id'] not in [c['id'] for c in before['cards']])
        processes=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                if p.stat().st_uid==10000 and (p/'exe').resolve()==Path('/usr/bin/gnome-text-editor'):
                    assert (p/'cgroup').read_text()==session_cgroup
                    assert str(home).encode() in (p/'environ').read_bytes()
                    processes.append(int(p.name))
            except (FileNotFoundError,ProcessLookupError):pass
        assert len(processes)==1;record['editor_processes']=processes
        with Motion(socket,out,'gtk-type-and-keyboard'):
            keyboard=Keyboard(guard);tap(180,180)
            state('gtk-close-keyboard',lambda s:ordinary(s) and s['keyboard_visible'])
            keyboard.key(29,True)
            try:keyboard.tap(30)
            finally:keyboard.key(29,False)
            TouchKeyboard(tap).type_ascii('tarot')
            tap(329,663);state('gtk-close-keyboard-hidden',lambda s:ordinary(s) and not s['keyboard_visible'] and window(s,identifier)['rect']==s['app_area'])
        assert document.read_bytes()==original
        capture('gtk-close-edited')
        with Motion(socket,out,'gtk-minimize'):
            request(socket,'overview');state('gtk-close-overview',lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        with Motion(socket,out,'gtk-unsaved-close-refusal'):
            request(socket,'close',id=identifier);time.sleep(.6)
            s=state('gtk-close-refused');assert len(s['cards'])==len(before['cards'])+1 and next(c for c in s['cards'] if c['id']==identifier)['closing']
        activate_prompt(identifier,'gtk-close-confirmation')
        with Motion(socket,out,'gtk-cancel'):
            button('gtk-close-confirmation','Cancel');time.sleep(.3)
        assert document.read_bytes()==original and all(Path('/proc',str(p)).exists() for p in processes)
        capture('gtk-close-cancelled')
        request(socket,'overview');state('gtk-close-second-overview',lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        request(socket,'close',id=identifier);time.sleep(.6)
        activate_prompt(identifier,'gtk-close-second-confirmation')
        with Motion(socket,out,'gtk-save-and-dismiss'):
            button('gtk-close-second-confirmation','Save')
            await_value(document.read_bytes,lambda data:data==b'tarot\n',seconds=10)
            state('gtk-close-saved-and-exited',lambda s:ordinary(s) and s['cards']==before['cards'])
        await_value(lambda:all(not Path('/proc',str(p)).exists() for p in processes),bool,seconds=10)
        request(socket,'activate',id=before['cards'][0]['id']);state('gtk-close-original-app-restored',lambda s:ordinary(s) and s['view']==before['view'] and window(s,before['cards'][0]['id'])['rect']==s['app_area'])
        record.update(passed=True,real_gtk_editor=True,same_login_cgroup=True,native_touch_keyboard_enters_text=True,
            unsaved_close_preserves_card=True,actual_touch_cancel_refuses_close=True,actual_touch_save_preserves_edited_bytes=True,
            cooperative_close=True,original_app_preserved=True)
    finally:
        if keyboard is not None:keyboard.close()
        (out/'gtk-close-validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
