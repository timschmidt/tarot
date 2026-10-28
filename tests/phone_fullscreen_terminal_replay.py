"""Real fullscreen foot with the taller terminal input panel and fresh PAM."""
import json,time
from PIL import Image
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(control,tap,state,capture,out,cgroup,authenticate):
    folder=out/'terminal-fullscreen'
    assert not request(control,'status')['cards']
    request(control,'launch',id='test.tarot.TerminalFullscreen.desktop')
    s=state('fullscreen-terminal-mapped',lambda s:len(s['cards'])==1 and s['keyboard_visible'])
    identifier=s['cards'][0]['id']
    def viewport(label,shown):
        height=396 if shown else 720
        v=state('fullscreen-terminal-'+label,lambda s:s['view']=={'App':identifier}
            and s['keyboard_visible']==shown
            and any(w['id']==identifier and w['rect']==dict(x=0.0,y=0.0,width=360.0,height=float(height)) for w in s['presentation']['windows'])
            and any(w['id']==identifier and w['fullscreen'] and w['geometry']['height']==height for w in s['native_geometry']))
        if shown:assert v['keyboard_owner']==identifier and v['keyboard_rect']['height']==288
        return v
    rendered={}
    def visible_capture(label):
        # PTY commands can complete before foot's input-method transitions
        # settle. Require actual key pixels, not only desired geometry.
        attempts=[]
        for attempt in range(6):
            viewport(label+'-settled-'+str(attempt),True)
            name=label+'-visible-'+str(attempt);capture(name)
            with Image.open(out/(name+'.png')) as image:
                assert image.size==(720,1440)
                samples=[image.convert('RGB').getpixel(point) for point in [(40,824),(150,824),(250,824),(30,964),(170,964)]]
            attempts.append({'capture':name,'samples':samples})
            if all(all(180<=channel<=240 for channel in color) for color in samples):
                rendered[label]=attempts;return
            time.sleep(.25)
        rendered[label]=attempts
        raise AssertionError({'keyboard_pixels_absent':attempts})
    viewport('ready',True);visible_capture('terminal-fullscreen-ready')
    ready=await_value(lambda:json.loads((folder/'shell-ready.json').read_text()) if (folder/'shell-ready.json').exists() else None,bool,seconds=10)
    assert ready['uid']==10000 and ready['cgroup']==cgroup and Path('/proc',str(ready['pid']),'cgroup').read_text()==cgroup
    keyboard=Keyboard(tap)
    keyboard.type_ascii("printf '%s' 'fullscreen' > tarot-fullscreen-only");tap(250,663)
    data=folder/'shell/tarot-fullscreen-only'
    await_value(lambda:data.read_bytes() if data.exists() else None,lambda b:b==b'fullscreen',seconds=12)
    tap(329,663);viewport('hidden',False);capture('terminal-fullscreen-hidden')
    tap(329,702);viewport('reopened',True);visible_capture('terminal-fullscreen-reopened')
    tap(180,702)
    s=state('fullscreen-terminal-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and not s['keyboard_visible'])
    r=next(w['rect'] for w in s['presentation']['windows'] if w['id']==identifier)
    tap(r['x']+r['width']/2,r['y']+r['height']/2);viewport('returned',True)
    tap(28,423);tap(332,423)
    request(control,'lock')
    locked=state('fullscreen-terminal-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] and s['lock']['generation']==s['lock']['last_presented_generation'])
    assert set(locked)=={'ok','pid','lock'}
    authenticate('fullscreen-terminal-actual-pam');viewport('after-pam',True)
    keyboard=Keyboard(tap);keyboard.type_ascii('cat tarot-fullscreen-o');tap(129,423)
    keyboard.type_ascii('> after-pam.txt');tap(250,663)
    data=folder/'shell/after-pam.txt'
    await_value(lambda:data.read_bytes() if data.exists() else None,lambda b:b==b'fullscreen',seconds=12)
    assert data.stat().st_uid==10000 and Path('/proc',str(ready['pid'])).exists()
    viewport('command-complete-after-pam',True);visible_capture('terminal-fullscreen-after-pam')
    keyboard.type_ascii('exit');tap(250,663)
    state('fullscreen-terminal-gone',lambda s:not s['cards'])
    await_value(lambda:not Path('/proc',str(ready['pid'])).exists(),bool,seconds=10)
    return {'passed':True,'shell':ready,'keyboard_height':288,'fullscreen_height':396,
        'hidden_height':720,'exact_text':'fullscreen','actual_overview_and_pam':True,
        'pending_modifier_and_tools_reset':True,'cooperative_exit':True,'rendered_keyboard_samples':rendered}
