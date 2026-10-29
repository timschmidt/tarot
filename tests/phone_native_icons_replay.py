"""Real icon pixels and touch in the installed authenticated phone session.

Caller supplies seven temporary entries, a selected fixture page, and restores
the owner's exact settings. No icon/favorite/page mutation uses diagnostics.
"""
import hashlib
import json
from pathlib import Path
import time
import tomllib
from PIL import Image
from card_matrix import physical
from phone_gesture_replay import request

IDS=[f'test.tarot.Icon{n:02}.desktop' for n in range(6)]+['test.tarot.Icon06.calendar.desktop']


def replay(control,screen,state,tap,capture,before,out,authenticate):
    result={'passed':False,'physical_finger_tested':False,'input':'native uinput/libinput touchscreen',
            'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'orientations':[]}
    settings=Path('/home/user/.config/tarot/settings.toml')
    def status():return request(control,'status')
    def observe(label,predicate=lambda s:True):return state('icons-'+label,predicate)
    def move(x,y):
        px,py=physical(status(),x,y);screen.move(px/2,py/2)
    def down(x,y):
        px,py=physical(status(),x,y);screen.down(px/2,py/2)
    def picture(label):
        capture('icons-'+label)
        with Image.open(out/('icons-'+label+'.png')) as image:image.load();return image.convert('RGB')
    def count(image,s,bounds,color):
        total=0
        for y in range(round(bounds[1]),round(bounds[3])):
            for x in range(round(bounds[0]),round(bounds[2])):
                px,py=physical(s,x,y);pixel=image.getpixel((px,py))
                total+=all(abs(a-b)<8 for a,b in zip(pixel,color))
        return total
    def launcher(label):
        request(control,'launcher')
        return observe(label,lambda s:s['view']=='Launcher' and s['launcher']['settings']['selected']==2
                       and not s['keyboard_visible'] and all(w['opacity']==0 for w in s['presentation']['windows']))
    try:
        catalog={a['id']:a for a in request(control,'applications')['applications']}
        assert all(identifier in catalog for identifier in IDS)
        assert catalog[IDS[3]]['icon']==catalog['org.gnome.Calculator.desktop']['icon']
        launcher('launcher')
        ready=observe('resolved',lambda s:s['application_icons']['ready']==5 and s['application_icons']['missing']==2
                      and s['application_icons']['pending']==0)
        baseline_stats=ready['application_icons'];result['resolved_cache']=baseline_stats
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            request(control,'rotate',orientation=orientation)
            s=observe('rotation-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None
                      and s['output']['revision']==s['output']['presented_revision'] and s['display_settings']['settings_saved'])
            g=s['output'];width=(g['physical_height'] if orientation in ['left-up','right-up'] else g['physical_width'])/g['scale']
            image=picture('launcher-'+orientation+'-'+str(len(result['orientations'])))
            counts=[]
            tile=(width-16)/3
            for index,color in enumerate([(255,0,0),(0,255,0),(0,0,255)]):
                center=tile/2+index*(tile+8)
                pixels=count(image,s,(center-28,102,center+28,158),color)
                assert pixels>500,(orientation,index,pixels)
                counts.append(pixels)
            assert s['application_icons']==baseline_stats
            result['orientations'].append({'orientation':orientation,'color_pixels':counts})
        saved=settings.read_bytes();down(180,140);time.sleep(.6);move(180,420);time.sleep(.1)
        s=observe('held');image=picture('lifted')
        assert count(image,s,(154,374,206,426),(0,255,0))>500
        assert settings.read_bytes()==saved
        move(3+1.5*354/5,643);time.sleep(.1);screen.up()
        favorite=['org.gnome.Calculator.desktop',IDS[1]]
        observe('favorite-saved',lambda s:s['favorites']['applications']==favorite and s['display_settings']['settings_saved'])
        assert tomllib.loads(settings.read_text())['quick_launch']['applications']==favorite
        request(control,'home');s=observe('home',lambda s:s['view']=='Home');time.sleep(.4)
        image=picture('quick-launch');assert count(image,s,(82,615,133,670),(0,255,0))>500
        launcher('before-lock');request(control,'lock');authenticate('icons-pam')
        s=observe('after-pam',lambda s:s['view']=='Launcher' and s['launcher']['settings']['selected']==2)
        assert s['application_icons']==baseline_stats and s['favorites']['applications']==favorite
        picture('after-pam');result['fresh_pam_preserves_cache']=True
        request(control,'activate',id=before['cards'][0]['id'])
        observe('restored-native-card',lambda s:s['view']==before['view'] and s['cards']==before['cards'])
        result['persisted_favorites']=favorite;result['passed']=True
        return result
    finally:
        (out/'native-icons-validation.json').write_text(json.dumps(result,indent=2)+'\n')
