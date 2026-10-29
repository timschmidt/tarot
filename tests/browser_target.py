"""Use painted synthetic targets only while their committed geometry is stable."""
import time
from PIL import Image, ImageChops


def geometry(s):
    owner=s['view'].get('App') if isinstance(s.get('view'),dict) else None
    return {'view':s.get('view'),'keyboard_visible':s.get('keyboard_visible'),
            'keyboard_rect':s.get('keyboard_rect'),'app_area':s.get('app_area'),
            'native':next((n for n in s.get('native_geometry',[]) if n['id']==owner),None),
            'visual':next((w for w in s.get('presentation',{}).get('windows',[]) if w['id']==owner),None),
            'output':s.get('output')}


def settled(s):
    g=geometry(s);native,visual,output=g['native'],g['visual'],g['output']
    if native is None or visual is None or output is None:return False
    if (output['orientation']!='normal' or output['requested'] is not None
            or output['revision']!=output['presented_revision']):return False
    expected=(dict(x=0.0,y=0.0,width=360.0,height=float(s['keyboard_rect']['y'] if s['keyboard_visible'] else 720))
              if native['fullscreen'] else s['app_area'])
    # The visual can reach its new size before the client commits that resize.
    # A fitted old buffer is not a stable target for this maximized browser.
    return (visual['rect']==expected and visual['opacity']==1 and visual['rotation']==0
            and native['geometry']['width']==expected['width']
            and native['geometry']['height']==expected['height']
            and s['presentation']['overview']==0 and s['presentation']['dock']==0)


def painted_target(name,label,state,capture,status,out,color,rejected):
    for attempt in range(5):
        before=state('browser-target-settled-'+label+'-'+str(attempt),settled)
        capture(label+'-'+str(attempt))
        with Image.open(out/(label+'-'+str(attempt)+'.png')) as source:
            image=source.convert('RGB');assert image.size==(720,1440)
            difference=ImageChops.difference(image,Image.new('RGB',image.size,color))
            red,green,blue=difference.split()
            mask=ImageChops.lighter(ImageChops.lighter(red,green),blue).point(lambda v:255 if v==0 else 0)
            bounds=mask.getbbox()
            if bounds:
                left,top,right,bottom=bounds
                area=(right-left)*(bottom-top);count=mask.histogram()[255]
                if right-left>=60 and bottom-top>=50 and count>area*.6:
                    after=status()
                    old,new=geometry(before),geometry(after)
                    if old!=new or not settled(after):
                        rejected.append({'capture':label+'-'+str(attempt),
                                         'geometry_before':old,'geometry_after':new})
                        continue
                    assert top>=64
                    assert bottom <= (after['keyboard_rect']['y']*2 if after['keyboard_visible'] else 1368)
                    return {'name':name,'capture':label+'-'+str(attempt),'bounds':bounds,
                            'count':count,'point':((left+right)/4,(top+bottom)/4),
                            'geometry_before':old,'geometry_after':new}
        time.sleep(.2)
    raise AssertionError({'target_not_stable_or_visible':name,'label':label})
