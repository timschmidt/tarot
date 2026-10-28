"""Native KMS card pixels and rounded selection inside the real PAM login session.

The caller supplies two temporary desktop entries for the audited ARM fixture.
Input stays on its existing guarded touchscreen. Calculator survives the slice.
"""
import hashlib
import json
from pathlib import Path
import time
from PIL import Image
from phone_gesture_replay import request

RED='test.tarot.CardRed.desktop'
BLUE='test.tarot.CardBlue.desktop'
PROBE=Path('/home/user/tarot-dev/bin/rounded-input-probe')


def replay(socket, screen, state, tap, capture, before, out, session_cgroup):
    record={'passed':False,'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'physical_finger_tested':False,'samples':[],'processes':[]}
    ids=[]
    audit=json.loads(Path('/home/user/tarot-dev/rounded-probe-r15-audit.json').read_text())
    assert hashlib.sha256(PROBE.read_bytes()).hexdigest()==audit['binary_sha256']
    def window(s,identifier):return next(w for w in s['presentation']['windows'] if w['id']==identifier)
    def physical(s,x,y):
        g=s['output'];w,h=g['physical_width'],g['physical_height'];scale=g['scale']
        swap=g['orientation'] in ['left-up','right-up'];lw,lh=((h/scale,w/scale) if swap else (w/scale,h/scale))
        x,y=x/lw,y/lh
        x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[g['orientation']]
        return round(x*w),round(y*h)
    def move(s,x,y):
        x,y=physical(s,x,y);screen.move(x/2,y/2)
    def press(s,x,y):
        x,y=physical(s,x,y);screen.down(x/2,y/2)
    def click(s,x,y):
        x,y=physical(s,x,y);tap(x/2,y/2)
    def picture(label):
        capture(label)
        with Image.open(out/(label+'.png')) as im:im.load();return im.convert('RGB')
    def ordinary(s):return s.get('lock',{}).get('phase')=='Unlocked'
    try:
        catalog=request(socket,'applications')['applications']
        assert all(any(a['id']==identifier for a in catalog) for identifier in [RED,BLUE])
        for identifier in [RED,BLUE]:
            request(socket,'launch',id=identifier)
            s=state('rounded-mapped-'+str(len(ids)),lambda s:ordinary(s) and len(s['cards'])==len(before['cards'])+len(ids)+1
                    and s['presentation']['windows'][-1]['opacity']==1.0)
            ids.append(next(c['id'] for c in s['cards'] if c['id'] not in ids and c['id'] not in [c['id'] for c in before['cards']]))
        red,blue=ids
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                if p.stat().st_uid==10000 and (p/'exe').resolve()==PROBE:
                    cgroup=(p/'cgroup').read_text();assert cgroup==session_cgroup
                    loaded=hashlib.sha256((p/'exe').read_bytes()).hexdigest();assert loaded==audit['binary_sha256']
                    record['processes'].append({'pid':int(p.name),'cgroup':cgroup,'binary_sha256':loaded})
            except (FileNotFoundError,ProcessLookupError):pass
        assert len(record['processes'])==2
        request(socket,'overview');s=state('rounded-overview',lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        b=window(s,blue)['rect'];press(s,b['x']+b['width']/2,300)
        state('rounded-arrange-ready',lambda s:(s['presentation']['arrange'] or {}).get('id')==blue)
        move(s,20,300)
        state('rounded-stack-target',lambda s:((s['presentation']['arrange'] or {}).get('target') or {}).get('Into',{}).get('anchor')==red)
        screen.up()
        state('rounded-stacked',lambda s:next(c for c in s['cards'] if c['id']==red)['stack']==next(c for c in s['cards'] if c['id']==blue)['stack'] and s['presentation']['arrange'] is None)
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            request(socket,'rotate',orientation=orientation)
            state('rounded-rotation-'+orientation,lambda s:ordinary(s) and s['output']['orientation']==orientation
                  and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'])
            time.sleep(.45)
            s=state('rounded-settled-'+orientation,lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
            v=window(s,blue);r=v['rect'];radius=v['corner_radius'];assert radius>0
            image=picture('rounded-pixels-'+orientation)
            corner=(r['x']+1,r['y']+1);inside=(r['x']+radius+2,r['y']+1)
            cp=image.getpixel(physical(s,*corner));ip=image.getpixel(physical(s,*inside))
            shadow=image.getpixel(physical(s,r['x']-4,r['y']+20))
            assert cp[0]>2*max(cp[1:]),(orientation,'corner',cp)
            assert ip[2]>2*ip[0] and ip[2]>1.3*ip[1],(orientation,'inner subsurface',ip)
            assert 90<shadow[0]<230 and shadow[0]>2*max(shadow[1:]),(orientation,'shadow',shadow)
            record['samples'].append({'orientation':orientation,'corner':cp,'inside':ip,'shadow':shadow,'radius':radius})
            click(s,*corner)
            state('rounded-corner-selects-red-'+orientation,lambda s:ordinary(s) and s['view']=={'App':red}
                  and window(s,red)['corner_radius']==0 and window(s,red)['rect']==s['app_area'])
            request(socket,'activate',id=blue)
            state('rounded-blue-active-'+orientation,lambda s:ordinary(s) and s['view']=={'App':blue} and window(s,blue)['corner_radius']==0)
            request(socket,'overview')
            state('rounded-overview-again-'+orientation,lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        for identifier in [blue,red]:request(socket,'close',id=identifier)
        state('rounded-clients-closed',lambda s:ordinary(s) and s['cards']==before['cards'])
        request(socket,'activate',id=before['cards'][0]['id'])
        state('rounded-original-calculator-restored',lambda s:ordinary(s) and s['view']==before['view']
              and window(s,before['cards'][0]['id'])['corner_radius']==0
              and window(s,before['cards'][0]['id'])['rect']==s['app_area'])
        assert all(not Path('/proc',str(p['pid'])).exists() for p in record['processes'])
        record.update(passed=True,native_panfrost_kms=True,opaque_xrgb_clients=True,
            corner_subsurface_clipped=True,lower_native_client_visible_through_corner=True,
            actual_touch_selects_lower_card=True,all_orientations=True,shadow_on_lower_native_client=True,
            original_app_preserved=True,cooperative_close_and_child_cleanup=True)
    finally:
        (out/'rounded-cards-validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
