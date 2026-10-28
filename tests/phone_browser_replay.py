"""Real native Firefox UI input with a loopback DOM observer, no WebDriver."""
import hashlib
import json
from pathlib import Path
import time
from PIL import Image, ImageChops
from browser_fixture import COLORS, EXPECTED
from dialog_button import locate
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(control,tap,state,capture,out,cgroup,authenticate,inspect_only=False):
    folder=out/'browser';report=folder/'events.jsonl'
    result={'passed':False,'inspect_only':inspect_only,'physical_finger_tested':False,
            'webdriver_input_used':False,'target_pixels':[]}
    def observe(event,condition=lambda e:True,seconds=30):
        return await_value(lambda:[e for e in events(report) if e['event']==event and condition(e)],bool,seconds=seconds)[-1]
    def exact(field,text,page='main'):
        index=EXPECTED.index(text)
        def matches(es):
            found=[e['fields'][field] for e in es if e.get('page')==page and field in e.get('fields',{})]
            return bool(found) and found[-1]['state']==index
        return await_value(lambda:events(report),matches,seconds=15)
    def safe_capture(label):
        latest={}
        for event in events(report):
            if 'fields' in event:latest[event['page']]=event['fields']
        unknown=[{'page':page,'field':name,'length':field['length']} for page,fields in latest.items()
                 for name,field in fields.items() if field['state']==-1]
        assert not unknown,{'unexpected_content_capture_suppressed':unknown}
        capture(label)
    def target(name,label):
        def settled(s):
            if not isinstance(s['view'],dict) or 'App' not in s['view']:return False
            owner=s['view']['App']
            native=next((n for n in s['native_geometry'] if n['id']==owner),None)
            visual=next((w for w in s['presentation']['windows'] if w['id']==owner),None)
            if native is None or visual is None:return False
            expected=(dict(x=0.0,y=0.0,width=360.0,height=float(s['keyboard_rect']['y'] if s['keyboard_visible'] else 720))
                      if native['fullscreen'] else s['app_area'])
            return visual['rect']==expected and visual['opacity']==1 and s['presentation']['overview']==0 and s['presentation']['dock']==0
        for attempt in range(5):
            state('browser-target-settled-'+label+'-'+str(attempt),settled)
            safe_capture(label+'-'+str(attempt))
            with Image.open(out/(label+'-'+str(attempt)+'.png')) as source:
                image=source.convert('RGB');assert image.size==(720,1440)
                difference=ImageChops.difference(image,Image.new('RGB',image.size,COLORS[name]))
                red,green,blue=difference.split()
                mask=ImageChops.lighter(ImageChops.lighter(red,green),blue).point(lambda value:255 if value==0 else 0)
                bounds=mask.getbbox()
                if bounds:
                    left,top,right,bottom=bounds
                    area=(right-left)*(bottom-top);count=mask.histogram()[255]
                    if right-left>=60 and bottom-top>=50 and count>area*.6:
                        point=((left+right)/4,(top+bottom)/4)
                        s=request(control,'status')
                        assert s['output']['orientation']=='normal'
                        assert top>=64
                        assert bottom <= (s['keyboard_rect']['y']*2 if s['keyboard_visible'] else 1368)
                        result['target_pixels'].append({'name':name,'capture':label+'-'+str(attempt),'bounds':bounds,'count':count,'point':point})
                        return point
            time.sleep(.2)
        raise AssertionError({'target_not_visible':name,'label':label})
    def click(name,label):tap(*target(name,label))
    def keyboard(label):
        s=state('browser-'+label,lambda s:s['keyboard_visible'] and s['keyboard_rect']['height']==240)
        assert s['keyboard_owner']==identifier
        return s
    def hide(label):
        tap(329,663);state('browser-'+label,lambda s:not s['keyboard_visible'])
    try:
        assert not request(control,'status')['cards']
        request(control,'launch',id='test.tarot.Browser.desktop')
        s=state('browser-mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict),seconds=75)
        identifier=s['cards'][0]['id'];result['card']=identifier
        start=observe('started');assert start['uid']==10000 and start['cgroup']==cgroup
        proc=Path('/proc',str(start['browser_pid']))
        assert proc.stat().st_uid==10000 and (proc/'cgroup').read_text()==cgroup
        assert str(start['profile']).encode() in (proc/'cmdline').read_bytes().split(b'\0')
        result['browser']={**start,'exe':str((proc/'exe').resolve()),'loaded_sha256':hashlib.sha256((proc/'exe').read_bytes()).hexdigest()}
        observe('ready',lambda e:e['page']=='main',seconds=60)
        click('primary','browser-primary');keyboard('text-keyboard')
        Keyboard(tap).type_ascii('tarot');exact('primary','tarot')
        safe_capture('browser-first-typed')
        if inspect_only:
            hide('inspect-hidden');request(control,'close',id=identifier)
            state('browser-inspect-gone',lambda s:not s['cards'])
            observe('exit',lambda e:e['code']==0,seconds=30)
            result.update(passed=True,native_browser_launch_and_text=True)
            return result
        # Shared-keyboard Backspace and same field through overview/PAM.
        Keyboard(tap).type_ascii('x');exact('primary','tarotx');tap(330,609);exact('primary','tarot')
        tap(180,702)
        s=state('browser-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and not s['keyboard_visible'])
        rect=next(w['rect'] for w in s['presentation']['windows'] if w['id']==identifier)
        tap(rect['x']+rect['width']/2,rect['y']+rect['height']/2)
        state('browser-returned',lambda s:s['view']=={'App':identifier})
        click('primary','browser-refocus');keyboard('returned-keyboard');Keyboard(tap).type_ascii('x');exact('primary','tarotx')
        request(control,'lock')
        protected=state('browser-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        authenticate('browser-actual-pam')
        state('browser-after-pam',lambda s:s['view']=={'App':identifier})
        click('primary','browser-after-pam-field');keyboard('after-pam-keyboard');Keyboard(tap).type_ascii('z');exact('primary','tarotxz')
        hide('after-pam-hidden')
        click('password','browser-private-field');keyboard('private-keyboard')
        Keyboard(tap).type_ascii('demo42');exact('password','demo42');safe_capture('browser-private-typed')
        hide('private-hidden')
        click('navigate','browser-navigate');second=observe('ready',lambda e:e['page']=='second')
        state('browser-second-title',lambda s:any(c['id']==identifier and 'second' in c['title'] for c in s['cards']))
        # Firefox offers to save the synthetic login on navigation. Dismiss
        # its real Chrome UI; an outside click is consumed by that popup.
        dismiss=None
        for attempt in range(4):
            name='browser-password-save-prompt-'+str(attempt);safe_capture(name)
            # The underlined mnemonic can join "Not now" into one OCR word.
            # Enlarge the response region, retaining the 80% confidence gate.
            try:dismiss=locate(out/(name+'.png'),'now',vertical=(.82,.89),alternatives=('notnow',),scale=2)
            except AssertionError:
                time.sleep(.25);continue
            break
        assert dismiss is not None,'Expected Firefox synthetic-password save prompt'
        result['password_save_prompt_dismissal']=dismiss
        tap(dismiss['point'][0]/2,dismiss['point'][1]/2)
        click('back','browser-back');returned=observe('pageshow',lambda e:e['page']=='main' and e['monotonic']>second['monotonic'])
        result['history_returned_fields']=returned.get('fields')
        click('popup','browser-popup-button');observe('ready',lambda e:e['page']=='popup')
        popup=state('browser-popup-mapped',lambda s:len(s['native_geometry'])>=2)
        result['popup_windows']=popup['native_geometry']
        click('popup-input','browser-popup-field');state('browser-popup-keyboard',lambda s:s['keyboard_visible'])
        Keyboard(tap).type_ascii('popup');exact('popup-input','popup',page='popup');hide('popup-hidden')
        click('close-popup','browser-popup-close')
        state('browser-popup-gone',lambda s:len(s['native_geometry'])==1 and s['view']=={'App':identifier})
        click('fullscreen','browser-fullscreen-button');observe('fullscreen',lambda e:e['page']=='main' and e['fullscreen'])
        state('browser-fullscreen',lambda s:any(n['id']==identifier and n['fullscreen'] and n['geometry']['height']==720 for n in s['native_geometry']))
        click('bottom','browser-fullscreen-bottom');s=keyboard('fullscreen-keyboard')
        state('browser-fullscreen-reserved',lambda s:any(n['id']==identifier and n['fullscreen'] and n['geometry']['height']==444 for n in s['native_geometry']))
        click('bottom','browser-fullscreen-reserved-field');Keyboard(tap).type_ascii('caf')
        tap(30,663);tap(69.21429,609);exact('bottom','café')
        safe_capture('browser-fullscreen-typed');hide('fullscreen-hidden')
        click('exit-fullscreen','browser-leave-fullscreen')
        state('browser-restored-window',lambda s:any(n['id']==identifier and not n['fullscreen'] for n in s['native_geometry']))
        request(control,'close',id=identifier);state('browser-gone',lambda s:not s['cards'])
        observe('exit',lambda e:e['code']==0,seconds=30)
        result.update(passed=True,native_browser_launch_and_text=True,unicode_and_backspace=True,
            overview_and_actual_pam=True,password_field=True,navigation_and_back=True,
            popup_input_and_cooperative_close=True,fullscreen_bottom_field=True)
    finally:(folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
