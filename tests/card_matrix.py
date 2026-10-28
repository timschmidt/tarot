"""Independent rendered-pixel and real-input scenario for three/five native cards."""
import json
from contextlib import nullcontext
from capture_motion import Motion
import math
from pathlib import Path
import time
from PIL import Image
from phone_gesture_replay import request

COLORS=['e91d31','1db931','1db9b9','b91db9','2255dd']


def physical(s,x,y):
    g=s['output'];w,h=g['physical_width'],g['physical_height'];scale=g['scale']
    swap=g['orientation'] in ['left-up','right-up'];lw,lh=((h/scale,w/scale) if swap else (w/scale,h/scale))
    x,y=x/lw,y/lh
    x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[g['orientation']]
    return round(x*w),round(y*h)


def inside(v,x,y,margin=0):
    r=v['rect'];cx=r['x']+r['width']/2;cy=r['y']+r['height']/2;a=math.radians(-v['rotation'])
    dx,dy=x-cx,y-cy;x=cx+math.cos(a)*dx-math.sin(a)*dy;y=cy+math.sin(a)*dx+math.cos(a)*dy
    radius=v['corner_radius'];qx=abs(x-cx)-r['width']/2+radius;qy=abs(y-cy)-r['height']/2+radius
    return math.hypot(max(qx,0),max(qy,0))+min(max(qx,qy),0)-radius<=-margin


def visible_point(s,identifier,exclude=None):
    windows=[v for v in s['presentation']['windows'] if v['opacity']>.01 and v['id']!=exclude]
    target=next(v for v in windows if v['id']==identifier)
    r=target['rect'];area=s['app_area']
    for y in [r['y']+r['height']/2,*range(round(area['y']+60),round(area['y']+area['height']-90),6)]:
        for x in [r['x']+r['width']/2,*range(6,round(area['width']-6),4)]:
            if not 4<=x<=area['width']-4 or not area['y']+4<=y<=area['y']+area['height']-4:continue
            if not inside(target,x,y,4):continue
            top=next((v for v in reversed(windows) if inside(v,x,y)),None)
            if top and top['id']==identifier:return x,y
    return None


def run(socket,state,capture,down,move,up,launch,before,out):
    record={'passed':False,'scenarios':[],'active_pixels':[]};ids=[];base={c['id'] for c in before['cards']}
    def window(s,identifier):return next(v for v in s['presentation']['windows'] if v['id']==identifier)
    def picture(label):
        capture(label)
        with Image.open(out/(label+'.png')) as im:im.load();return im.convert('RGB')
    def ordinary(s):return s.get('lock',{}).get('phase')=='Unlocked'
    def settled(label):
        s=state(label,lambda s:ordinary(s) and s['view']=='Overview' and s['presentation']['overview']==1.0)
        time.sleep(.4);return state(label+'-settled',lambda s:ordinary(s) and s['view']=='Overview')
    try:
        for index,color in enumerate(COLORS):
            launch(index,color)
            s=state('matrix-mapped-'+str(index),lambda s:ordinary(s) and len(s['cards'])==len(base)+len(ids)+1
                and isinstance(s['view'],dict) and s['presentation']['windows'][-1]['rect']==s['app_area'])
            identifier=next(c['id'] for c in s['cards'] if c['id'] not in base and c['id'] not in ids);ids.append(identifier)
            request(socket,'overview');s=settled('matrix-overview-'+str(index))
            if index:
                r=window(s,identifier)['rect'];down(s,r['x']+r['width']/2,r['y']+r['height']/2)
                s=state('matrix-hold-'+str(index),lambda s:(s['presentation']['arrange'] or {}).get('id')==identifier)
                target=ids[-2];point=visible_point(s,target,exclude=identifier)
                assert point is not None,('Adjacent folded group is not reachable',index,s)
                move(s,*point)
                state('matrix-drop-target-'+str(index),lambda s:((s['presentation']['arrange'] or {}).get('target') or {}).get('Into',{}).get('anchor')==target)
                up();s=state('matrix-joined-'+str(index),lambda s:s['presentation']['arrange'] is None
                    and len({c['stack'] for c in s['cards'] if c['id'] in ids})==1)
                s=settled('matrix-joined-overview-'+str(index))
            if len(ids) not in [3,5]:continue
            count=len(ids)
            for orientation in ['normal','left-up','bottom-up','right-up','normal']:
                request(socket,'rotate',orientation=orientation)
                state('matrix-rotation-'+str(count)+'-'+orientation,lambda s:ordinary(s) and s['output']['orientation']==orientation
                    and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'])
                s=settled('matrix-frame-'+str(count)+'-'+orientation)
                visible=[v for v in s['presentation']['windows'] if v['id'] in ids and v['opacity']>.01]
                assert len(visible)==3 and sorted(v['rotation'] for v in visible)==[-5.0,0.0,5.0]
                im=picture('matrix-'+str(count)+'-'+orientation);samples=[]
                for v in visible:
                    point=visible_point(s,v['id']);assert point is not None,(count,orientation,v,s)
                    actual=im.getpixel(physical(s,*point));expected=tuple(bytes.fromhex(COLORS[ids.index(v['id'])]))
                    # Black material shadows preserve chroma. Reject another
                    # client, chrome/wallpaper and an unrotated texture sample.
                    assert max(actual)>60 and max(abs(a/max(actual)-e/max(expected)) for a,e in zip(actual,expected))<.15,(count,orientation,v['id'],actual,expected,point)
                    samples.append({'id':v['id'],'rotation':v['rotation'],'point':point,'pixel':actual})
                record['scenarios'].append({'members':count,'orientation':orientation,'visible':samples,'retained_ids':ids.copy()})
            for offset,expected_id in enumerate(reversed(ids)):
                s=state('matrix-before-activate-'+str(count)+'-'+str(expected_id),lambda s:ordinary(s) and s['selected']==expected_id)
                r=window(s,expected_id)['rect'];point=(r['x']+r['width']/2,r['y']+r['height']/2)
                down(s,*point);up()
                s=state('matrix-active-'+str(count)+'-'+str(expected_id),lambda s:ordinary(s) and s['view']=={'App':expected_id}
                    and window(s,expected_id)['rotation']==0 and window(s,expected_id)['rect']==s['app_area'])
                im=picture('matrix-active-'+str(count)+'-'+str(expected_id));r=s['app_area'];actual=im.getpixel(physical(s,r['x']+r['width']/2,r['y']+r['height']/2));expected=tuple(bytes.fromhex(COLORS[ids.index(expected_id)]))
                assert max(abs(a-e) for a,e in zip(actual,expected))<=2,(expected_id,actual,expected)
                record['active_pixels'].append({'members':count,'id':expected_id,'pixel':actual})
                request(socket,'overview');s=settled('matrix-return-'+str(count)+'-'+str(expected_id))
                if offset+1<count:
                    r=window(s,expected_id)['rect'];point=(r['x']+r['width']/2,r['y']+r['height']/2)
                    origins={v['id']:v['rect']['x'] for v in s['presentation']['windows'] if v['opacity']>.01}
                    down(s,*point);move(s,point[0]+48,point[1])
                    state('matrix-browse-preview-'+str(count)+'-'+str(expected_id),lambda q:q['presentation']['arrange'] is None
                        and all(abs(window(q,identifier)['rect']['x']-x-48)<.75 for identifier,x in origins.items()))
                    # Readback before the first move can turn a scripted swipe
                    # into a real 420ms hold. Record only after axis ownership.
                    with Motion(socket,out,'stack-five-browse') if count==5 and offset==0 else nullcontext():
                        time.sleep(.24)
                        up();state('matrix-after-browse-'+str(count)+'-'+str(expected_id))
                        state('matrix-browsed-'+str(count)+'-'+str(expected_id),lambda s:ordinary(s) and s['view']=='Overview' and s['selected']==ids[count-offset-2])
                        settled('matrix-browse-settled-'+str(count)+'-'+str(expected_id))
        for identifier in reversed(ids):request(socket,'close',id=identifier)
        state('matrix-clients-closed',lambda s:ordinary(s) and s['cards']==before['cards'])
        if before['cards']:
            request(socket,'activate',id=before['cards'][0]['id'])
            state('matrix-original-app-restored',lambda s:ordinary(s) and s['view']==before['view']
                and window(s,before['cards'][0]['id'])['rect']==s['app_area'])
        record.update(passed=True,real_input_grouping=True,all_orientations=True,three_and_five_native_cards=True,
            three_visible_previews=True,actual_pixels_for_each_visible_member=True,actual_browse_reaches_every_member=True,
            original_app_preserved=True,cooperative_close=True)
    finally:(out/'stack-matrix-validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
