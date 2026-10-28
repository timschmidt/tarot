"""Actual fullscreen toolkit entry above the native keyboard; synthetic text only."""
import time
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(control,tap,state,capture,observe,window,identifier,prefix,authenticate):
    prefix+='-fullscreen-text'
    result={'passed':False,'physical_finger_tested':False}
    def bounds(parent_height,after=0.0):
        return observe('fullscreen-field-bounds',lambda e:e['parent_width']==360
            and e['parent_height']==parent_height and e['monotonic']>=after)
    # Use trailing blank space when refocusing so appended text keeps its caret.
    def field_point(field):return field['x']+field['width']-12,field['y']+field['height']/2
    def above(label,after):
        s=state(prefix+'-'+label,lambda s:s['keyboard_visible']
            and window(s,identifier)['rect']['y']==0.0
            and window(s,identifier)['rect']['height']==s['keyboard_rect']['y']
            and any(n['id']==identifier and n['geometry']['height']==s['keyboard_rect']['y'] for n in s.get('native_geometry',[])))
        native=next(n for n in s['native_geometry'] if n['id']==identifier)
        assert native['fullscreen'] and native['geometry']['width']==360
        assert native['geometry']['height']==s['keyboard_rect']['y'],native
        assert s['keyboard_owner']==identifier
        field=bounds(int(s['keyboard_rect']['y']),after)
        assert field['y']>=0 and field['y']+field['height']<=s['keyboard_rect']['y'],field
        result[label]={'field':field,'after':after,'viewport':window(s,identifier)['rect'],'keyboard':s['keyboard_rect']}
        return s,field
    def full(label):
        return state(prefix+'-'+label,lambda s:not s['keyboard_visible']
            and window(s,identifier)['rect']==dict(x=0.0,y=0.0,width=360.0,height=720.0)
            and any(n['id']==identifier and n['fullscreen'] and n['geometry']['height']==720 for n in s['native_geometry']))
    initial=bounds(720);assert initial['y']>444,initial
    result['initial_field']=initial
    after=time.monotonic();tap(*field_point(initial))
    state(prefix+'-keyboard-started',lambda s:s['keyboard_visible'])
    # Capture the actual obstruction on the old payload before waiting for
    # the corrected viewport. Do not call toolkit resize/focus callbacks.
    capture(prefix+'-keyboard-arrived')
    _,field=above('first-reserved',after)
    tap(*field_point(field));Keyboard(tap).type_ascii('tarot')
    observe('text',lambda e:e.get('field')=='fullscreen' and e['text']=='tarot')
    capture(prefix+'-typed')
    after=time.monotonic();tap(329,663);full('hidden');bounds(720,after)
    capture(prefix+'-hidden')
    after=time.monotonic();tap(329,702);_,field=above('reopened',after);tap(*field_point(field))
    Keyboard(tap).type_ascii('x')
    observe('text',lambda e:e.get('field')=='fullscreen' and e['text']=='tarotx')
    tap(180,702)
    s=state(prefix+'-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0 and not s['keyboard_visible'])
    r=window(s,identifier)['rect'];after=time.monotonic();tap(r['x']+r['width']/2,r['y']+r['height']/2)
    _,field=above('returned',after);tap(*field_point(field));Keyboard(tap).type_ascii('z')
    observe('text',lambda e:e.get('field')=='fullscreen' and e['text']=='tarotxz')
    request(control,'lock')
    s=state(prefix+'-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] and s['lock']['last_presented_generation']==s['lock']['generation'])
    assert set(s)=={'ok','pid','lock'}
    after=time.monotonic();authenticate(prefix+'-actual-pam')
    _,field=above('after-pam',after);tap(*field_point(field));Keyboard(tap).type_ascii('a')
    observe('text',lambda e:e.get('field')=='fullscreen' and e['text']=='tarotxza')
    capture(prefix+'-after-pam')
    after=time.monotonic();tap(329,663);full('final-hidden');bounds(720,after)
    result.update(passed=True,actual_bottom_field_visible=True,exact_text='tarotxza',
        hide_reopen_restores_viewport=True,overview_restores_same_field=True,actual_pam_restores_field=True)
    return result
