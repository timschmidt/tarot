"""Real touch/rotation/PAM Just Type on an installed ordinary-user phone session.

The controller supplies a temporary matching Reader desktop/MIME handler, its
local URI/notification fixture and exact settings restoration. No query is sent
to the Internet and no search action uses a diagnostic mutation.
"""
import hashlib
import json
from pathlib import Path
import time
from PIL import Image,ImageOps

from card_matrix import physical
from dialog_button import locate
from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

READER='org.tarot.InputProbe.desktop'


class Sender(Fixture):
    """Attach to the sender launched inside the Reader's authenticated cgroup."""
    def __init__(self, directory):
        self.report=directory/'events.jsonl';self.commands=directory/'commands.jsonl';self.sequence=0
        await_value(self.events,lambda es:any(e['event']=='ready' for e in es),seconds=10)
        assert self.report.stat().st_uid==10000 and self.commands.stat().st_uid==10000
    def close(self):
        # The Reader fixture owns this process and terminates/reaps it.
        pass


def replay(control,screen,state,tap,capture,before,out,cgroup,authenticate):
    folder=out/'just-type';assert folder.is_dir()
    result={'passed':False,'physical_finger_tested':False,'input':'native uinput/libinput touchscreen',
            'network_query_sent':False,'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'orientations':[],'target_visibility':[],'reselected_target_visibility':[],'result_targets':[]}
    original=before['cards'][0]['id'];reader=None;sender=None
    settings=Path('/home/user/.config/tarot/settings.toml');saved=settings.read_bytes()
    uri_report=folder/'uri.jsonl'
    def status():return request(control,'status')
    def observe(label,predicate=lambda s:True):return state('search-'+label,predicate)
    def dimensions(s):
        g=s['output'];w,h=g['physical_width']/g['scale'],g['physical_height']/g['scale']
        return (h,w) if g['orientation'] in ['left-up','right-up'] else (w,h)
    def down(x,y,slot=0):
        px,py=physical(status(),x,y);screen.down(px/2,py/2,slot=slot)
    def touch(x,y):
        down(x,y);screen.up();time.sleep(.1)
    def keyboard():
        s=status();width,_=dimensions(s);k=s['keyboard_rect']
        assert s['shell_text_focus'] and s['keyboard_visible']
        return Keyboard(lambda x,y:touch(5+(x-5)*(width-10)/350,k['y']+(y-444)*k['height']/240))
    def query(value,label):
        assert status()['just_type']['active']
        # The menu temporarily hides the keyboard, retiring its symbol state.
        touch(40,16);observe(label+'-menu',lambda s:s['launcher']['menu_open'])
        touch(160,60);observe(label+'-empty',lambda s:s['just_type']['active'] and not s['launcher']['menu_open']
                             and s['shell_text_focus'] and not s['launcher_query'] and s['keyboard_visible'])
        keyboard().type_ascii(value)
        return observe(label,lambda s:s['launcher_query']==value)
    def enter(label):
        s=status();width,height=dimensions(s);touch(width/2,height-18)
        observe(label+'-cards',lambda s:s['view']=='Overview' and s['presentation']['overview']==1 and not s['keyboard_visible'])
        touch(120,60)
        return observe(label,lambda s:s['just_type']['active'] and s['just_type']['origin']=='Overview' and s['keyboard_visible'])
    def rotate(orientation):
        request(control,'rotate',orientation=orientation)
        return observe('rotation-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None
                       and s['output']['presented_revision']==s['output']['revision'] and s['display_settings']['settings_saved'])
    def picture(label):capture('search-'+label)
    def result_point(word,label):
        # Installed application comments can match the same word as a setting.
        # Find the actual rendered row, scrolling only the results viewport.
        # Never repeat a failed selection or weaken its resulting state gate.
        initial=status();query_revision=initial['just_type']['revision'];output_revision=initial['output']['revision']
        for attempt in range(8):
            s=status();assert s['just_type']['active'] and s['just_type']['revision']==query_revision
            assert s['output']['revision']==output_revision and s['output']['presented_revision']==output_revision
            width,height=dimensions(s);top=84 if height<480 else 99
            bottom=(s['keyboard_rect']['y'] if s['keyboard_visible'] else height-36)-4
            assert bottom-top>20
            name=label+'-target-'+str(attempt);picture(name)
            raw=out/('search-'+name+'.png');upright=raw.with_name(raw.stem+'-upright.png')
            with Image.open(raw) as im:
                im.load()
                rotation={'normal':None,'left-up':Image.Transpose.ROTATE_270,
                          'bottom-up':Image.Transpose.ROTATE_180,'right-up':Image.Transpose.ROTATE_90}[s['output']['orientation']]
                image=im.copy() if rotation is None else im.transpose(rotation)
            assert image.size==(round(width*s['output']['scale']),round(height*s['output']['scale']))
            image.save(upright)
            # The shared dialog helper's light-text threshold merges this
            # sheet's dark text with its selected green band. Preserve the
            # actual capture and recognize a separate dark-text raster.
            text_image=upright.with_name(upright.stem+'-dark-text.png')
            ImageOps.grayscale(image).point(lambda value:0 if value<128 else 255).save(text_image)
            try:
                found=locate(text_image,word,vertical=(top/height,bottom/height))
            except AssertionError:
                if attempt==7:raise
                # Present down/motion/up across frames. A whole gesture in
                # one event batch can leave the scroll area without a drag.
                # Cross the slop promptly, before a hold could begin.
                x=width-20;y=bottom-8;end=max(top+8,y-90)
                assert y-end>14
                px,py=physical(s,x,y);screen.down(px/2,py/2)
                time.sleep(.03)
                for step in range(1,5):
                    px,py=physical(s,x,y+(end-y)*step/4)
                    screen.move(px/2,py/2);time.sleep(.02)
                time.sleep(.03)
                screen.up();time.sleep(.25)
                continue
            current=status()
            assert current['just_type']['revision']==query_revision and current['output']['revision']==output_revision
            point=tuple(v/s['output']['scale'] for v in found['point'])
            found.update(capture=upright.name,recognition_capture=text_image.name,dark_text_threshold=128,
                         orientation=s['output']['orientation'],logical_point=point,
                         query_revision=query_revision,output_revision=output_revision,scroll_attempt=attempt)
            result['result_targets'].append(found)
            return point
    def uris():return [json.loads(s) for s in uri_report.read_text().splitlines()] if uri_report.exists() else []
    def actions():return [e for e in sender.events() if e['event']=='action']
    try:
        assert status()['output']['orientation']=='normal' and not status()['notifications']['hide_previews']
        request(control,'launch',id=READER)
        s=observe('reader-mapped',lambda s:len(s['cards'])==2 and isinstance(s['view'],dict) and any(w['opacity']==1 and w['rect']==s['app_area'] for w in s['presentation']['windows']))
        reader=next(c['id'] for c in s['cards'] if c['id']!=original)
        ready=await_value(uris,lambda es:any(e['event']=='ready' for e in es),seconds=10)[0]
        for role in ['native_pid','notification_pid']:
            proc=Path('/proc',str(ready[role]));assert proc.stat().st_uid==10000 and (proc/'cgroup').read_text()==cgroup
        sender=Sender(folder/'notification')
        enter('card-entry');query('tarot search reader','app-query');picture('application-results')
        touch(64,182);observe('existing-reader-focused',lambda s:s['view']=={'App':reader} and len(s['cards'])==2 and not s['keyboard_visible'])
        enter('settings-entry');query('volume','settings-query');picture('settings-results')
        touch(*result_point('outputs','sound'));observe('sound-setting',lambda s:s['system_menu_open'] and s['system_page']=='Audio' and s['launcher_query']=='volume')
        # Touch the system scrim, which closes only the menu and preserves text.
        touch(4,400);observe('query-after-setting',lambda s:not s['system_menu_open'] and s['launcher_query']=='volume')
        notification=sender.notify(summary='Orchid reminder',body='Fictional local record',actions=['default','Open'],resident=True,timeout=0)
        query('orchid','content-query');picture('content')
        count=len(actions());touch(*result_point('reminder','content'));await_value(actions,lambda es:len(es)==count+1)
        query('orchid','stale-content-query');old=next(n['revision'] for n in status()['notifications']['items'] if n['id']==notification)
        count=len(actions());down(*result_point('reminder','stale-content'))
        sender.notify(replaces_id=notification,summary='Orchid replacement',body='Fictional replacement',actions=['default','Open'],resident=True,timeout=0)
        observe('replacement-while-held',lambda s:any(n['id']==notification and n['revision']>old for n in s['notifications']['items']))
        screen.up();time.sleep(.2);assert len(actions())==count
        touch(*result_point('replacement','fresh-content'));await_value(actions,lambda es:len(es)==count+1)
        result['stale_held_result_canceled']=True
        archived=sender.notify(summary='Archive orchid',body='Exact needle record',resident=True,timeout=0)
        for n in range(15):sender.notify(summary=f'Newer fixture {n}',body='Fictional retained record',resident=True,timeout=0)
        query('needle','fallback-query');touch(*result_point('Archive','fallback'))
        observe('dashboard-target',lambda s:s['notifications']['dashboard_open'] and (s['just_type']['selected'] or {}).get('Notification',{}).get('id')==archived)
        well=observe('dashboard-rows-ready',lambda s:s['notifications']['well']['height']==224)['notifications']['well']
        dashboard_vertical=((well['y']+48)/720,(well['y']+well['height'])/720)
        result['dashboard_well']=well
        for attempt in range(4):
            label='targeted-dashboard-'+str(attempt);picture(label)
            try:
                # Exclude the still-usable matching search row above the well.
                visible=locate(out/('search-'+label+'.png'),'Archive',vertical=dashboard_vertical);visible['capture']='search-'+label+'.png';result['target_visibility'].append(visible);break
            except AssertionError:
                if attempt==3:raise
                time.sleep(.15)
        start=well['y']+48+12;end=well['y']+well['height']-16
        assert end-start>=24
        for _ in range(14):
            s=status();px,py=physical(s,180,start);screen.down(px/2,py/2)
            for step in range(1,7):
                px,py=physical(s,180,start+(end-start)*step/6);screen.move(px/2,py/2,delay=.025)
            screen.up();time.sleep(.1)
        picture('dashboard-scrolled-away')
        result['scrolled_newer_visible']=locate(out/'search-dashboard-scrolled-away.png','Newer',vertical=dashboard_vertical)
        try:locate(out/'search-dashboard-scrolled-away.png','Archive',vertical=dashboard_vertical)
        except AssertionError:pass
        else:raise AssertionError('Target must be outside the dashboard viewport before reselection')
        selected=status()['just_type']
        touch(40,706);observe('repeat-query-preserved',lambda s:not s['notifications']['dashboard_open'] and s['launcher_query']=='needle')
        touch(*result_point('Archive','repeat-fallback'))
        observe('dashboard-reselected',lambda s:s['notifications']['dashboard_open']
                and s['just_type']['revision']==selected['revision'] and s['just_type']['selected']==selected['selected']
                and s['just_type']['selection_revision']>selected['selection_revision'])
        for attempt in range(4):
            label='reselected-dashboard-'+str(attempt);picture(label)
            try:
                visible=locate(out/('search-'+label+'.png'),'Archive',vertical=dashboard_vertical);visible['capture']='search-'+label+'.png';result['reselected_target_visibility'].append(visible);break
            except AssertionError:
                if attempt==3:raise
                time.sleep(.15)
        result['repeated_dashboard_selection']=True
        touch(40,706);observe('query-after-dashboard',lambda s:not s['notifications']['dashboard_open'] and s['launcher_query']=='needle')
        query('caf','web-prefix');keys=keyboard();touch(30,663);keys.page=1;touch(69.21429,609)
        observe('unicode-query',lambda s:s['launcher_query']=='café');keys.type_ascii(' & one=two')
        observe('web-query',lambda s:s['launcher_query']=='café & one=two');picture('provider')
        old_uris=sum(e['event']=='uri' for e in uris());touch(252,663);time.sleep(.15)
        assert sum(e['event']=='uri' for e in uris())==old_uris
        touch(180,145);await_value(uris,lambda es:any(e['event']=='uri' and e['kind']=='search' and e['matched'] for e in es),seconds=15)
        observe('uri-reader-focused',lambda s:s['view']=={'App':reader} and len(s['cards'])==2 and not s['just_type']['active'])
        enter('website-entry');query('http://example.invalid/path?q=one&x=two','website-query');picture('website')
        touch(180,145);await_value(uris,lambda es:any(e['event']=='uri' and e['kind']=='address' and e['matched'] for e in es),seconds=15)
        observe('website-reader-focused',lambda s:s['view']=={'App':reader} and len(s['cards'])==2)
        assert settings.read_bytes()==saved,'Query activity changed persisted settings'
        result['query_did_not_persist']=True
        enter('rotation-entry');query('volume','rotation-query')
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            s=rotate(orientation);width,height=dimensions(s);assert s['launcher_query']=='volume' and s['just_type']['active']
            touch(width-32,57 if height<480 else 64)
            observe('clear-'+orientation,lambda s:not s['launcher_query'] and s['keyboard_visible'])
            keyboard().type_ascii('volume');observe('typed-'+orientation,lambda s:s['launcher_query']=='volume')
            picture('orientation-'+orientation+'-'+str(len(result['orientations'])))
            touch(*result_point('outputs','sound-'+orientation+'-'+str(len(result['orientations']))))
            observe('setting-'+orientation,lambda s:s['system_menu_open'] and s['system_page']=='Audio' and s['launcher_query']=='volume')
            touch(4,height-50);observe('menu-closed-'+orientation,lambda s:not s['system_menu_open'])
            touch(100,57 if height<480 else 64);observe('field-refocused-'+orientation,lambda s:s['shell_text_focus'] and s['keyboard_visible'])
            result['orientations'].append(orientation)
        request(control,'lock');authenticate('search-real-pam')
        observe('query-after-pam',lambda s:s['just_type']['active'] and s['launcher_query']=='volume' and not s['shell_text_focus'])
        touch(100,64);observe('field-after-pam',lambda s:s['shell_text_focus'] and s['keyboard_visible'])
        picture('after-pam');result['fresh_pam_preserves_query']=True
        touch(40,16);observe('close-menu',lambda s:s['launcher']['menu_open']);touch(160,110)
        observe('origin-restored',lambda s:s['view']=='Overview' and not s['just_type']['active'] and not s['launcher_query'])
        request(control,'close',id=reader);observe('reader-closed',lambda s:len(s['cards'])==1 and s['cards'][0]['id']==original)
        request(control,'activate',id=original);observe('original-restored',lambda s:s['view']==before['view'] and s['cards']==before['cards'])
        result.update(passed=True,existing_card_focus=True,settings_and_content=True,explicit_uri_handoff=True,no_implicit_remote_enter=True)
        return result
    finally:
        (out/'just-type-validation.json').write_text(json.dumps(result,indent=2)+'\n')
