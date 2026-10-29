"""Touch-only favorite organization in the real authenticated phone session."""
import hashlib
import json
from pathlib import Path
import time
import tomllib

from card_matrix import physical
from dialog_button import locate
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

CALCULATOR='org.gnome.Calculator.desktop'
BETA='test.tarot.FavoriteBeta.desktop'
DELTA='test.tarot.FavoriteDelta.desktop'
EPSILON='test.tarot.FavoriteEpsilon.desktop'
GAMMA='test.tarot.FavoriteGamma.desktop'
PREFIX='tarot favorite'


def replay(control,screen,state,tap,capture,before,out,authenticate):
    folder=out/'favorite-drag';assert folder.is_dir()
    result={'passed':False,'physical_finger_tested':False,'input':'native uinput/libinput touchscreen',
        'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'holds':[],'orientations':[]}
    identifier=before['cards'][0]['id'];query=''
    settings=Path('/home/user/.config/tarot/settings.toml')
    def status():return request(control,'status')
    def size(s):
        g=s['output'];w,h=g['physical_width']/g['scale'],g['physical_height']/g['scale']
        return (h,w) if g['orientation'] in ['left-up','right-up'] else (w,h)
    def down(point):
        x,y=physical(status(),*point);screen.down(x/2,y/2)
    def move(point):
        x,y=physical(status(),*point);screen.move(x/2,y/2)
    def touch(point):
        down(point);screen.up()
    def favorite(label,expected):
        s=state('favorite-'+label,lambda s:s['favorites']['applications']==expected and s['display_settings']['settings_saved'])
        assert [c['id'] for c in s['cards']]==[identifier] and s['launch_feedback'] is None
        saved=tomllib.loads(settings.read_text());assert saved['quick_launch']['applications']==expected
        assert saved['notifications']['dnd'] and saved['display']['rotation_locked']
        return s
    def launcher(label,desired=None):
        wanted=query if desired is None else desired
        request(control,'launcher')
        s=state('favorite-'+label,lambda s:s['view']=='Launcher' and not s['editing_favorites']
            and s['presentation']['overview']==0 and all(w['opacity']==0 for w in s['presentation']['windows']))
        if not s['just_type']['active']:
            touch((40,16));state('favorite-'+label+'-menu',lambda s:s['launcher']['menu_open'])
            touch((160,60))
            s=state('favorite-'+label+'-search-open',lambda s:s['just_type']['active'] and s['shell_text_focus'] and not s['launcher']['menu_open'])
        width,height=size(s);field_y=57 if height<480 else 64
        # Reopening the active Just Type menu offers Clear search, not the
        # former app-filter entry. Leaving search also retires its query.
        # Restore this finite fixture query through actual oriented keys.
        if s['launcher_query']!=wanted:
            touch((width-32,field_y))
            s=state('favorite-'+label+'-empty-query',lambda s:not s['launcher_query'] and s['shell_text_focus'] and s['keyboard_visible'])
            k=s['keyboard_rect']
            Keyboard(lambda x,y:touch((5+(x-5)*(width-10)/350,k['y']+(y-444)*k['height']/240))).type_ascii(wanted)
        elif not s['shell_text_focus']:
            touch((100,field_y))
        return state('favorite-'+label+'-query-ready',lambda s:s['launcher_query']==wanted and s['shell_text_focus'] and s['keyboard_visible'])
    def overview(label):
        request(control,'overview')
        return state('favorite-'+label,lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and s['presentation']['dock']==1)
    def rotate(orientation):
        request(control,'rotate',orientation=orientation)
        return state('favorite-rotation-'+orientation,lambda s:s['output']['orientation']==orientation
            and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision']
            and s['display_settings']['settings_saved'])
    def set_query(value,label):
        nonlocal query
        assert status()['output']['orientation']=='normal'
        launcher(label+'-launcher',value)
        state('favorite-'+label+'-exact-query',lambda s:s['launcher_query']==value)
        query=value
        capture('favorite-'+label+'-search')
    def source(app_id):
        words=query.split();apps=request(control,'applications')['applications']
        apps=[a for a in apps if all(word in (' '.join([a['name'],a['description'],*a['keywords']])).casefold() for word in words)]
        index=[a['id'] for a in apps].index(app_id);s=status();width,height=size(s)
        compact=height<480
        tile=(width-32-24)/3;point=(16+(index%3)*(tile+8)+tile/2,(100 if compact else 182)+(index//3)*(56 if compact else 112))
        assert point[1]<height-36 and index<6,(app_id,index,query)
        return point
    def slot(index):
        width,height=size(status());return 3+(index+.5)*(width-6)/5,height-36-69-4+32
    def hold(point,label):
        down(point);time.sleep(.6);capture('favorite-'+label+'-held')
        s=status();entry={'label':label,'orientation':s['output']['orientation'],'point':point}
        if s['output']['orientation']=='normal':
            entry['recognized']=locate(out/('favorite-'+label+'-held.png'),'Release',vertical=(.04,.94))
        result['holds'].append(entry)
    def place(app_id,index,expected,label):
        launcher(label+'-launcher');previous=status()['favorites']['applications'];saved=settings.read_bytes()
        hold(source(app_id),label);state('favorite-'+label+'-keyboard-hidden',lambda s:not s['keyboard_visible'])
        move(slot(index));capture('favorite-'+label+'-preview')
        assert status()['favorites']['applications']==previous and settings.read_bytes()==saved
        screen.up();return favorite(label+'-committed',expected)
    def remove(index,expected,label):
        overview(label+'-overview');hold(slot(index),label)
        width,height=size(status());move((width/2,height-36-69-4-32));capture('favorite-'+label+'-target')
        screen.up();return favorite(label+'-removed',expected)
    try:
        state('favorite-ready',lambda s:s['favorites']['applications']==[CALCULATOR] and s['view']==before['view']
            and any(w['id']==identifier and w['opacity']==1 and w['rect']==s['app_area'] for w in s['presentation']['windows']))
        assert status()['output']['orientation']=='normal'
        set_query(PREFIX,'initial');place(BETA,3,[CALCULATOR,BETA],'add-beta')
        overview('dock-reorder');x,y=slot(1);hold((x,y-31),'top-of-tray');move(slot(0));capture('favorite-dock-reorder-preview')
        assert status()['favorites']['applications']==[CALCULATOR,BETA]
        screen.up();favorite('dock-reordered',[BETA,CALCULATOR])
        place(DELTA,1,[BETA,DELTA,CALCULATOR],'insert-delta')
        place(EPSILON,3,[BETA,DELTA,CALCULATOR,EPSILON],'add-epsilon')
        launcher('before-full');saved=settings.read_bytes();hold(source(GAMMA),'full-tray');move(slot(0));capture('favorite-full-tray')
        result['full_label']=locate(out/'favorite-full-tray.png','full',vertical=(.04,.94))
        screen.up();favorite('full-rejected',[BETA,DELTA,CALCULATOR,EPSILON]);assert settings.read_bytes()==saved
        expected=[BETA,CALCULATOR,EPSILON,DELTA]
        place(DELTA,3,expected,'existing-in-full-tray')
        hold(source(GAMMA),'fixed-apps');move(slot(4));screen.up();favorite('fixed-apps-rejected',expected)
        assert status()['view']=='Launcher'
        hold(source(GAMMA),'outside');move((2,0));capture('favorite-status-boundary');screen.up();favorite('outside-canceled',expected)
        hold(source(BETA),'second-contact');screen.down(200,400,slot=1);screen.up(slot=1);screen.up()
        favorite('second-contact-canceled',expected);capture('favorite-after-second-contact')
        hold(source(BETA),'protected-interruption');request(control,'lock')
        protected=state('favorite-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface']
            and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        screen.up();authenticate('favorite-actual-pam')
        state('favorite-after-pam',lambda s:s['view']=='Launcher' and not s['keyboard_visible'])
        favorite('protected-canceled',expected);capture('favorite-after-pam')
        hold(source(BETA),'rotation-interruption');rotate('left-up');screen.up();favorite('rotation-canceled',expected);rotate('normal')
        remove(0,[CALCULATOR,EPSILON,DELTA],'remove-beta')
        remove(0,[EPSILON,DELTA],'remove-running-shortcut')
        set_query('calculator','calculator');place(CALCULATOR,0,[CALCULATOR,EPSILON,DELTA],'readd-calculator')
        remove(2,[CALCULATOR,EPSILON],'remove-delta');remove(1,[CALCULATOR],'remove-epsilon')
        set_query(PREFIX,'orientation-query')
        # Hide the keyboard with its real control before rotation-only cases.
        tap(329,663);state('favorite-orientation-keyboard-hidden',lambda s:not s['keyboard_visible'])
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            rotate(orientation)
            place(BETA,0,[BETA,CALCULATOR],'oriented-add-'+orientation)
            remove(0,[CALCULATOR],'oriented-remove-'+orientation)
            result['orientations'].append(orientation)
        set_query(PREFIX,'landscape-query');rotate('left-up')
        # Output reconfiguration cancels the old shell input context. Reopen
        # search through its painted landscape control, using oriented touch.
        down((100,57));screen.up()
        state('favorite-landscape-keyboard',lambda s:s['shell_text_focus'] and s['keyboard_visible'] and s['keyboard_rect']['height']==180)
        assert status()['launcher_query']==PREFIX
        capture('favorite-landscape-filtered-search')
        place(BETA,0,[BETA,CALCULATOR],'landscape-keyboard-source')
        remove(0,[CALCULATOR],'landscape-restore');rotate('normal')
        set_query('','clear-query');tap(329,663);state('favorite-query-cleared',lambda s:not s['keyboard_visible'])
        request(control,'activate',id=identifier)
        state('favorite-original-app-restored',lambda s:s['view']==before['view'] and s['presentation']['overview']==0
            and s['presentation']['dock']==0 and any(w['id']==identifier and w['rect']==s['app_area'] for w in s['presentation']['windows']))
        favorite('restored-for-next-replay',[CALCULATOR]);capture('favorite-final-native-app')
        result.update(passed=True,native_kms_and_touch=True,preview_without_persistence=True,full_and_fixed_rejection=True,
            same_identity_reorder=True,explicit_removal_preserves_native_card=True,second_contact_cancellation=True,
            protected_cancellation_and_actual_pam=True,rotation_cancellation=True,landscape_keyboard_source=True,
            original_favorites_and_query_restored=True,other_preferences_preserved=True)
    finally:(folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
