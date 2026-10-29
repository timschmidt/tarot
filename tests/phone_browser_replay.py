"""Real native Firefox UI input with a loopback DOM observer, no WebDriver."""
import hashlib
import json
import os
from pathlib import Path
import signal
import time
from PIL import Image, ImageChops
from browser_fixture import COLORS, EXPECTED
from browser_observations import latest_reports
from dialog_button import locate
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from phone_stock_login import processes


def replay(control,tap,state,capture,out,cgroup,authenticate,inspect_only=False,focus_cycles=0):
    assert type(focus_cycles) is int and 0<=focus_cycles<=20
    folder=out/'browser';report=folder/'events.jsonl'
    result={'passed':False,'inspect_only':inspect_only,'physical_finger_tested':False,
            'webdriver_input_used':False,'target_pixels':[],'overview_cycles':[]}
    def observe(event,condition=lambda e:True,seconds=30):
        return await_value(lambda:[e for e in events(report) if e['event']==event and condition(e)],bool,seconds=seconds)[-1]
    def exact(field,text,page='main'):
        index=EXPECTED.index(text)
        def matches(es):
            latest=latest_reports(es).get(page)
            return latest is not None and field in latest['fields'] and latest['fields'][field]['state']==index
        try:return await_value(lambda:events(report),matches,seconds=15)
        except AssertionError:
            latest=latest_reports(events(report))
            # Preserve the rendered keyboard only when all field contents are
            # still recognized synthetic states. This happens after failure,
            # without changing the timing or acceptance of any input.
            if all(f['state']>=0 for e in latest.values() for f in e['fields'].values()):
                safe_capture('browser-exact-state-failure')
                result['failure_snapshot']={p:e['fields'] for p,e in latest.items()}
            raise
    def safe_capture(label):
        latest=latest_reports(events(report))
        unknown=[{'page':page,'field':name,'length':field['length']} for page,event in latest.items()
                 for name,field in event['fields'].items() if field['state']==-1]
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
    def overview_return(suffix=''):
        tap(180,702)
        s=state('browser-overview'+suffix,lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and not s['keyboard_visible'])
        rect=next(w['rect'] for w in s['presentation']['windows'] if w['id']==identifier)
        tap(rect['x']+rect['width']/2,rect['y']+rect['height']/2)
        state('browser-returned'+suffix,lambda s:s['view']=={'App':identifier})
        click('primary','browser-refocus'+suffix);keyboard('returned-keyboard'+suffix)
        Keyboard(tap).type_ascii('x');exact('primary','tarotx')
        result['overview_cycles'].append({'label':suffix or 'original','passed':True})
    def queued_taps():
        # Stop only our verified ordinary-user keyboard, queue actual touchscreen
        # contacts, and resume it even if input fails. This deliberately batches
        # taps without WebDriver, text injection or slowing the acceptance input.
        target('primary','browser-before-key-burst')
        candidates=[p for p in processes() if p['uid']==10000 and p['name']=='tarot-keyboard'
                    and Path('/proc',str(p['pid']),'cgroup').read_text()==cgroup]
        assert len(candidates)==1
        helper=candidates[0];proc=Path('/proc',str(helper['pid']))
        digest=hashlib.sha256((proc/'exe').read_bytes()).hexdigest()
        assert (proc/'exe').resolve()==Path('/usr/bin/tarot-keyboard')
        assert digest==hashlib.sha256(Path('/usr/bin/tarot-keyboard').read_bytes()).hexdigest()
        fd=os.pidfd_open(helper['pid']);stopped=False
        try:
            assert helper in processes() and proc.stat().st_uid==10000
            assert (proc/'cgroup').read_text()==cgroup
            assert request(control,'status')['lock']['phase']=='Unlocked'
            signal.pidfd_send_signal(fd,signal.SIGSTOP);stopped=True
            await_value(lambda:(proc/'status').read_text(),lambda s:'State:\tT' in s,seconds=5)
            result['queued_taps']={'helper':helper,'loaded_sha256':digest,'passed':False,
                                   'actual_touch_contacts':3,'helper_stopped_before_contacts':True}
            Keyboard(tap).type_ascii('caf')
        finally:
            try:
                if stopped:signal.pidfd_send_signal(fd,signal.SIGCONT)
            finally:os.close(fd)
        exact('primary','caf')
        result['queued_taps']['passed']=True
        safe_capture('browser-key-burst-retained')
        for expected in ['ca','c','']:
            tap(330,609);exact('primary',expected)
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
        queued_taps()
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
        overview_return()
        for cycle in range(focus_cycles):
            tap(330,609);exact('primary','tarot')
            overview_return('-repeat-'+str(cycle+1))
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
        click('back','browser-back');returned=observe('pageshow',lambda e:e['page']=='main' and e['observed']>second['observed'])
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
    finally:
        snapshots=events(report)
        result['observer']={'heartbeats':sum(e['event']=='heartbeat' for e in snapshots),
            'reported_transport_failures':max((e.get('transportFailures',0) for e in snapshots),default=0),
            'receipt_minus_browser_timestamp_ms_max':max((e.get('report_delivery_ms',0) for e in snapshots),default=0)}
        result['last_field_metadata']={page:event['fields'] for page,event in latest_reports(snapshots).items()}
        result['last_document_metadata']={page:{key:event.get(key) for key in ['documentFocused','visibility','sequence','observed']} for page,event in latest_reports(snapshots).items()}
        result['last_report_age_ms']={page:time.time()*1000-event['observed'] for page,event in latest_reports(snapshots).items()}
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
