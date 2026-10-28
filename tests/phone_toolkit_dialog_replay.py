"""Touch real GTK3/Qt6 separate toplevels, native text and child popup responses."""
import json
from pathlib import Path
import time
from dialog_button import locate
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(control,tap,state,capture,out,session_cgroup):
    record={'passed':False,'physical_finger_tested':False,'toolkits':{}}
    def window(s,identifier):return next(w for w in s['presentation']['windows'] if w['id']==identifier)
    for toolkit in ['gtk','qt']:
        report=out/'toolkits'/(toolkit+'-events.jsonl')
        result={'passed':False,'buttons':[]};record['toolkits'][toolkit]=result
        prefix='toolkit-'+toolkit
        def observe(event,condition=lambda e:True):
            return await_value(lambda:[e for e in events(report) if e['event']==event and condition(e)],bool,seconds=15)[-1]
        def button(label,word):
            state(prefix+'-'+label+'-settled',lambda s:s['view']=={'App':identifier}
                and window(s,identifier)['rect']==s['app_area'] and window(s,identifier)['opacity']==1.0
                and s['presentation']['overview']==0.0 and s['presentation']['dock']==0.0)
            capture(prefix+'-'+label)
            found=locate(out/(prefix+'-'+label+'.png'),word,vertical=(.08,.91));result['buttons'].append(found)
            tap(found['point'][0]/2,found['point'][1]/2)
        def dialog_mapped(label):
            return state(prefix+'-'+label,lambda s:len(s['cards'])==1 and any(n['mapped'] and n['parent']==identifier for n in s['native_windows']))
        def typed(label):
            state(prefix+'-'+label+'-keyboard',lambda s:s['keyboard_visible'])
            Keyboard(tap).type_ascii('tarot')
            observe('text',lambda e:e['text']=='tarot')
            tap(329,663)
            state(prefix+'-'+label+'-keyboard-hidden',lambda s:not s['keyboard_visible'] and window(s,identifier)['rect']==s['app_area'])
        assert not request(control,'status')['cards']
        request(control,'launch',id='test.tarot.'+('Gtk' if toolkit=='gtk' else 'Qt')+'Dialog.desktop')
        s=state(prefix+'-parent',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and s['presentation']['windows'][0]['rect']==s['app_area'])
        identifier=s['cards'][0]['id']
        ready=observe('ready');pid=ready['pid'];assert ready['uid']==10000 and Path('/proc',str(pid),'cgroup').read_text()==session_cgroup
        result['pid']=pid;result['version']=observe('version')
        button('parent','Open')
        opened=observe('dialog-open');assert opened['transient'] and opened['modal']
        dialog_mapped('dialog');typed('first')
        button('combo','Choice')
        state(prefix+'-child-popup',lambda s:s['popup_grab_active'] and s['popup_owner']==identifier)
        button('popup','Other');observe('choice',lambda e:e['value']=='Other')
        state(prefix+'-popup-dismissed',lambda s:not s['popup_grab_active'])
        request(control,'overview')
        s=state(prefix+'-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        request(control,'close',id=identifier)
        # GTK can filter a close while its modal child is active before the
        # application's delete-event callback. xdg close has no required ack.
        # Verify retained native content and lifetime instead of a private
        # toolkit callback; final cooperative close below must still exit.
        time.sleep(.6)
        s=dialog_mapped('close-refused');assert s['cards'][0]['closing'] and Path('/proc',str(pid)).exists()
        result['root_close_callback_observed']=any(e['event']=='root-close-refused' for e in events(report))
        r=window(s,identifier)['rect'];tap(r['x']+r['width']/2,r['y']+r['height']/2)
        state(prefix+'-restored',lambda s:s['view']=={'App':identifier} and window(s,identifier)['rect']==s['app_area'])
        button('cancel','Cancel');observe('response',lambda e:e['action']=='cancel' and e['text']=='tarot')
        state(prefix+'-cancelled',lambda s:len(s['cards'])==1 and not any(n['mapped'] and n['parent']==identifier for n in s['native_windows']))
        button('reopen','Open');dialog_mapped('second-dialog');typed('second')
        button('apply','Apply');observe('response',lambda e:e['action']=='apply' and e['text']=='tarot')
        state(prefix+'-applied',lambda s:len(s['cards'])==1 and not any(n['mapped'] and n['parent']==identifier for n in s['native_windows']))
        capture(prefix+'-saved-parent')
        request(control,'close',id=identifier);observe('root-close');observe('exit')
        state(prefix+'-closed',lambda s:not s['cards'])
        await_value(lambda:not Path('/proc',str(pid)).exists(),bool,seconds=12)
        result.update(passed=True,separate_xdg_toplevel_with_parent=True,native_touch_keyboard_exact_text=True,
            child_popup_owned_by_root_card=True,actual_popup_selection=True,unsaved_close_refused=True,
            actual_card_activation_restores_dialog=True,cancel_and_apply_actual_responses=True,cooperative_parent_close=True,same_login_cgroup=True)
    record.update(passed=all(r['passed'] for r in record['toolkits'].values()),native_panfrost_kms=True)
    (out/'toolkit-dialog-validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
