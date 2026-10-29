"""Named pages through native touch in an authenticated installed Tarot session.

Caller creates eighteen numbered temporary desktop entries, backs up settings,
provides real PAM authentication and restores the exact original files afterward.
This replay never sets page/name/placement state through the control socket.
"""
import hashlib
import json
import time
import tomllib
from pathlib import Path

from card_matrix import physical
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

IDS=[f'test.tarot.Launcher{n:02}.desktop' for n in range(18)]


def replay(control,screen,state,tap,capture,before,out,authenticate):
    folder=out/'launcher-pages'
    assert folder.is_dir()
    settings=Path('/home/user/.config/tarot/settings.toml')
    result={'passed':False,'physical_finger_tested':False,
            'input':'native uinput/libinput touchscreen',
            'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'orientations':[],'cancellations':[]}
    favorite_before=request(control,'status')['favorites']
    def status():return request(control,'status')
    def dimensions(s):
        g=s['output'];w,h=g['physical_width']/g['scale'],g['physical_height']/g['scale']
        return (h,w) if g['orientation'] in ['left-up','right-up'] else (w,h)
    def down(x,y,slot=0):
        px,py=physical(status(),x,y);screen.down(px/2,py/2,slot=slot)
    def move(x,y):
        px,py=physical(status(),x,y);screen.move(px/2,py/2)
    def touch(x,y):
        down(x,y);screen.up();time.sleep(.1)
    def observe(label,condition=lambda s:True):
        return state('pages-'+label,condition)
    def picture(label):capture('pages-'+label)
    def saved(label,condition):
        s=observe(label,lambda s:s['launcher']['saved'] and condition(s['launcher']['settings']))
        data=tomllib.loads(settings.read_text())
        assert data['version']==4 and data['launcher']==s['launcher']['settings']
        assert s['favorites']==favorite_before
        assert data['quick_launch']['applications']==favorite_before['applications']
        assert data['notifications']['dnd'] and data['display']['rotation_locked']
        assert s['cards']==before['cards'] and s['launch_feedback'] is None
        return s
    def menu(label):
        touch(40,16);observe(label+'-menu',lambda s:s['launcher']['menu_open']);picture(label+'-menu')
    def launcher(label):
        request(control,'home');request(control,'launcher')
        return observe(label,lambda s:s['view']=='Launcher' and not s['shell_text_focus']
                       and not s['launcher']['search_open'] and s['presentation']['overview']==0
                       and all(w['opacity']==0 for w in s['presentation']['windows']))
    def swipe(start,end):
        down(*start);move(*end);time.sleep(.1);screen.up();time.sleep(.4)
    def hold(x,y,label):
        down(x,y);time.sleep(.6);picture(label+'-hold')
    def rotate(orientation):
        request(control,'rotate',orientation=orientation)
        return observe('rotation-'+orientation,lambda s:s['output']['orientation']==orientation
                       and s['output']['requested'] is None
                       and s['output']['presented_revision']==s['output']['revision']
                       and s['display_settings']['settings_saved'])
    def page(pages,identifier):return next(p for p in pages['pages'] if p['id']==identifier)
    def type_name(text):
        s=status();width,_=dimensions(s);keyboard=s['keyboard_rect']
        assert s['shell_text_focus'] and s['launcher']['editing_name']
        # Transform the existing English key centers within the actual keyboard
        # rectangle for landscape; all submitted input remains real touch.
        Keyboard(lambda x,y:touch(5+(x-5)*(width-10)/350,
                                 keyboard['y']+(y-444)*keyboard['height']/240)).type_ascii(text)
    def edit_name(label):
        menu(label);touch(315,284)  # second of the two verified page rows
        return observe(label+'-field',lambda s:s['launcher']['editing_name']
                       and s['shell_text_focus'] and s['keyboard_visible'])
    try:
        catalog=request(control,'applications')['applications']
        assert [a['id'] for a in catalog[:18]]==IDS,'Numbered fixtures must lead the unfiltered grid'
        initial=launcher('initial-launcher')
        original=initial['launcher']['settings']
        assert len(original['pages'])==1 and original['selected']==original['inbox']
        inbox=original['inbox'];picture('initial-inbox')
        menu('create');touch(160,110)
        observe('new-name',lambda s:s['launcher']['editing_name'] and s['shell_text_focus']
                and s['keyboard_visible'] and s['keyboard_rect']['height']==240)
        type_name('work');picture('typed-work');touch(260,186)
        created=saved('created-work',lambda p:len(p['pages'])==2 and p['pages'][1]['name']=='work')
        work=created['launcher']['settings']['selected'];assert work!=inbox
        picture('empty-work')
        swipe((90,300),(260,300));saved('swiped-to-inbox',lambda p:p['selected']==inbox)
        # Start a new header contact while the previous swipe is settling.
        down(270,300);move(70,300);screen.up()
        released=time.monotonic();touch(40,16)
        observe('menu-interrupted-settle',lambda s:s['launcher']['menu_open']
                and s['launcher']['settings']['selected']==work)
        result['menu_interrupt_elapsed_seconds']=time.monotonic()-released
        touch(40,16);observe('interrupt-menu-closed',lambda s:not s['launcher']['menu_open'])
        swipe((90,300),(260,300));saved('returned-to-inbox',lambda p:p['selected']==inbox)
        # Same-page placement uses the painted first two cells. Exact identities
        # are checked afterward, rather than assuming the dragged icon's label.
        hold(58,148,'same-page');move(180,148);time.sleep(.1);screen.up()
        saved('same-page-reordered',lambda p:page(p,inbox)['applications'][:3]==[IDS[1],IDS[0],IDS[2]])
        baseline=settings.read_bytes()
        hold(180,148,'edge');move(350,300);time.sleep(.8);picture('edge-work-preview')
        assert settings.read_bytes()==baseline,'Edge hover saved a preview before release'
        move(180,150);time.sleep(.1);screen.up()
        saved('placed-in-work',lambda p:p['selected']==work and page(p,work)['applications']==[IDS[0]])
        picture('work-with-linked-favorites')
        # Cancel a preview after switching to its adjacent page.
        baseline=settings.read_bytes();hold(58,148,'outside-cancel');move(10,300);time.sleep(.8)
        picture('canceled-edge-preview');move(2,0);screen.up()
        assert settings.read_bytes()==baseline
        saved('outside-canceled',lambda p:p['selected']==work and page(p,work)['applications']==[IDS[0]])
        result['cancellations'].append('outside-edge-preview')
        hold(58,148,'second-contact');move(180,150);down(200,400,slot=1)
        screen.up(slot=1);screen.up()
        saved('second-contact-canceled',lambda p:p==tomllib.loads(baseline.decode())['launcher'])
        assert settings.read_bytes()==baseline
        result['cancellations'].append('second-contact')
        hold(58,148,'protected');request(control,'lock')
        protected=observe('protected',lambda s:s['lock']['phase']=='Locked'
                          and s['lock']['last_presented_surface']
                          and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        screen.up();authenticate('pages-actual-pam')
        saved('protected-canceled',lambda p:p==tomllib.loads(baseline.decode())['launcher'])
        result['cancellations'].append('protected-with-actual-pam')
        hold(58,148,'rotation');rotate('left-up');screen.up();rotate('normal')
        saved('rotation-canceled',lambda p:p==tomllib.loads(baseline.decode())['launcher'])
        result['cancellations'].append('rotation')
        # Repeated swipes in all output orientations retain page/favorite IDs.
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            s=rotate(orientation);width,height=dimensions(s)
            touch(26,64);saved('arrow-inbox-'+orientation,lambda p:p['selected']==inbox)
            y=min(200,height-70);swipe((width*.75,y),(width*.25,y))
            saved('swiped-work-'+orientation,lambda p:p['selected']==work)
            result['orientations'].append(orientation);picture('swipe-'+orientation)
        # A vertical page scroll must not change page selection or launch an app.
        touch(26,64);saved('scroll-inbox',lambda p:p['selected']==inbox)
        baseline=settings.read_bytes();swipe((180,450),(180,220));picture('vertical-scroll')
        assert settings.read_bytes()==baseline
        saved('scroll-page-unchanged',lambda p:p['selected']==inbox)
        touch(334,64);saved('return-to-work',lambda p:p['selected']==work)
        # Actual landscape name editing must leave its actions above the OSK.
        rotate('left-up');edit_name('landscape-rename');type_name('side')
        picture('landscape-name');width,_=dimensions(status());touch(width-70,58)
        saved('landscape-name-saved',lambda p:page(p,work)['name']=='side')
        rotate('normal')
        baseline=settings.read_bytes();edit_name('rename-cancel');type_name('abandoned');touch(90,186)
        saved('name-canceled',lambda p:page(p,work)['name']=='side')
        assert settings.read_bytes()==baseline
        edit_name('rename-accept');type_name('personal');touch(245,663)  # touch Enter
        saved('name-accepted',lambda p:page(p,work)['name']=='personal')
        menu('reorder');touch(210,284)
        saved('work-first',lambda p:p['pages'][0]['id']==work and p['inbox']==inbox)
        touch(180,650);observe('reorder-menu-dismissed',lambda s:not s['launcher']['menu_open'])
        menu('remove');touch(160,340)
        removed=saved('work-removed',lambda p:len(p['pages'])==1 and p['pages'][0]['id']==inbox
                      and IDS[0] in p['pages'][0]['applications'])
        assert removed['launcher']['menu_open']
        touch(160,110);observe('replacement-name',lambda s:s['launcher']['editing_name'] and s['shell_text_focus'])
        type_name('saved');touch(260,186)
        replacement=saved('replacement-page',lambda p:len(p['pages'])==2 and p['pages'][1]['name']=='saved')
        replacement_id=replacement['launcher']['settings']['selected'];assert replacement_id>work
        # Move the real favorite's original catalog entry into the saved page.
        # Search spans every page and keeps the source reachable above the OSK.
        menu('favorite-source');touch(160,60)
        observe('favorite-search',lambda s:s['launcher']['search_open'] and s['shell_text_focus'])
        Keyboard(touch).type_ascii('calculator')
        observe('favorite-query',lambda s:s['launcher_query']=='calculator')
        matches=[a for a in catalog if 'calculator' in ' '.join([a['name'],a['description'],*a['keywords']]).lower()]
        assert matches[0]['id']=='org.gnome.Calculator.desktop'
        hold(70,182,'favorite-source');observe('favorite-keyboard-hidden',lambda s:not s['keyboard_visible'])
        move(180,150);time.sleep(.1);screen.up()
        final=saved('replacement-placed',lambda p:p['selected']==replacement_id and page(p,replacement_id)['applications']==['org.gnome.Calculator.desktop'])
        result['persisted_settings']=final['launcher']['settings']
        result['settings_sha256']=hashlib.sha256(settings.read_bytes()).hexdigest()
        picture('final-saved-page')
        request(control,'activate',id=before['cards'][0]['id'])
        observe('original-app-restored',lambda s:s['view']==before['view'] and s['presentation']['overview']==0
                and s['presentation']['dock']==0 and s['cards']==before['cards'])
        # The moved favorite still focuses its original native card.
        down(180,706);move(180,610)
        observe('moved-favorite-wave',lambda s:s['presentation']['quick_launch'] is not None)
        move(42,610);observe('moved-favorite-selected',lambda s:s['presentation']['quick_launch'] is not None and s['presentation']['quick_launch']['selected']==0)
        screen.up()
        observe('moved-favorite-focus',lambda s:s['view']==before['view'] and s['cards']==before['cards'] and s['presentation']['quick_launch'] is None)
        result.update(passed=True,same_page_and_cross_page_placement=True,preview_without_persistence=True,
                      vertical_scroll_ownership=True,real_name_keyboard_and_enter=True,
                      landscape_name_controls=True,reorder_and_remove=True,identities_not_reused=True,
                      favorite_identities_preserved=True,protected_privacy=True)
    finally:
        # An assertion may interrupt a held test contact. Release only contacts
        # owned by this temporary input device before the controller cleans up.
        for slot in list(screen.active):screen.up(slot=slot)
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
