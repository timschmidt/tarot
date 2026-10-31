"""Native Quick Launch slice for an authenticated installed-session replay.

The caller prepares one temporary failure desktop entry and a backed-up test
configuration with Calculator as its only favorite. All input uses the existing
guarded uinput touchscreen. No PAM policy or authentication result is replaced.
"""
import hashlib
import datetime
import json
from pathlib import Path
import tomllib
import time
from PIL import Image
from dialog_button import locate

from phone_gesture_replay import request


CALCULATOR = 'org.gnome.Calculator.desktop'
FAILURE = 'test.tarot.QuickLaunchFailure.desktop'


def replay(socket, screen, state, tap, capture, before, out):
    records = {'passed': False, 'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'physical_finger_tested': False}

    def same_app(label):
        value = state(label, lambda s:s['view']==before['view'] and s['presentation']['quick_launch'] is None
                      and s['presentation']['overview']==0.0 and s['presentation']['dock']==0.0
                      and s['presentation']['windows'][0]['rect']==s['app_area'])
        assert value['cards']==before['cards']
        return value

    def wave(x, selected):
        screen.down(180,706)
        screen.move(180,610)
        state('quick-wave-armed', lambda s:s['presentation']['quick_launch'] is not None)
        screen.move(x,610)
        return state('quick-wave-selected',lambda s:s['presentation']['quick_launch'] is not None
                     and s['presentation']['quick_launch']['selected']==selected)

    def favorite(label, expected):
        return state(label,lambda s:s['favorites']['applications']==expected
                     and s['display_settings']['settings_saved'])

    def add_visible_application(identifier, label):
        catalog = request(socket, 'applications')['applications']
        application = next(a for a in catalog if a['id'] == identifier)
        favorites = request(socket, 'status')['favorites']['applications']
        assert application['unavailable'] is None and identifier not in favorites
        assert sum(a['name']==application['name'] and a['id'] not in favorites for a in catalog)==1, 'Rendered application name must identify one available choice'
        legacy_y = 259+51*[a['id'] for a in catalog if a['id']!=CALCULATOR].index(identifier)
        word = application['name'].split()[-1]
        for attempt in range(32):
            name = label+'-visible-'+str(attempt)
            capture(name)
            s = request(socket, 'status')
            assert s['editing_favorites'] and s['output']['orientation'] == 'normal'
            scale = s['output']['scale']
            width = s['output']['physical_width']/scale
            height = s['output']['physical_height']/scale
            bottom = height-36-s['notifications']['well']['height']-69-8
            assert bottom > 148
            panel = out/(name+'-editor.png')
            with Image.open(out/(name+'.png')) as image:
                image.crop((round(16*scale),round(100*scale),round((width-16)*scale),round(bottom*scale))).save(panel)
            try:
                found = locate(panel, word, vertical=(0,1))
            except AssertionError:
                if attempt == 31:
                    raise
                # Keep successive scans overlapping and stop momentum before
                # scanning. A full-height fling can skip the desired row.
                start = bottom-24
                end = start-min(120, (bottom-100)*.3)
                screen.down(width/2,start)
                for step in range(1,7):
                    screen.move(width/2,start+(end-start)*step/6,delay=.05)
                time.sleep(.2)
                screen.move(width/2,end,delay=.05)
                screen.up();time.sleep(.1)
                continue
            point = (16+found['point'][0]/scale,100+found['point'][1]/scale)
            records.setdefault('visible_application_targets',[]).append({'id':identifier,'point':point,'viewport_bottom':bottom,'legacy_y':legacy_y,'recognition':found})
            tap(*point)
            return

    def editor(label):
        tap(40,16)
        state(label+'-menu',lambda s:s['launcher']['menu_open'])
        tap(160,160)
        shown = state(label,lambda s:s['editing_favorites'])
        # Reopening retains egui's scroll offset. Return the favorite controls
        # to the top after the bounded application scan before using them.
        bottom = shown['output']['physical_height']/shown['output']['scale']-36-shown['notifications']['well']['height']-69-8
        screen.down(180,124)
        for step in range(1,7):
            screen.move(180,124+(bottom-24-124)*step/6,delay=.05)
        time.sleep(.2)
        screen.move(180,bottom-24,delay=.05)
        screen.up();time.sleep(.1)
        return shown

    def done(label):
        tap(285,67)
        return state(label,lambda s:not s['editing_favorites'])

    try:
        ready = state('quick-native-ready',lambda s:s['favorites']['applications']==[CALCULATOR]
                      and s['view']==before['view'] and len(s['cards'])==1
                      and s['presentation']['windows'][0]['opacity']==1.0)
        assert ready['output']['orientation']=='normal' and ready['output']['scale']==2
        assert ready['cards'][0]['app_id']=='org.gnome.Calculator'
        request(socket,'overview')
        state('quick-packet-card-view',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        capture('quick-packet-card-view')
        request(socket,'activate',id=before['cards'][0]['id'])
        # Opacity stays one during maximization; it cannot prove the card has
        # reached the app area. Calculator also reactivates native text input.
        ready=state('quick-app-before-wave',lambda s:s['view']==before['view']
                    and s['keyboard_visible'] and s['presentation']['overview']==0.0
                    and s['presentation']['dock']==0.0
                    and s['presentation']['windows'][0]['rect']==s['app_area'])
        shown=wave(42,0)
        assert shown['view']==ready['view'] and shown['cards']==ready['cards']
        assert shown['presentation']['windows']==ready['presentation']['windows']
        capture('quick-wave-native-app')
        screen.up()
        same_app('quick-existing-app-restored')
        assert request(socket,'status')['launch_feedback'] is None
        wave(2,None);screen.up();same_app('quick-outside-release-canceled')
        wave(320,4);screen.up()
        state('quick-launcher',lambda s:s['view']=='Launcher' and s['presentation']['overview']==0.0
              and all(w['opacity']==0.0 for w in s['presentation']['windows']))
        capture('quick-launcher-without-tray')
        editor('quick-editor')
        capture('quick-editor-before')
        add_visible_application(FAILURE,'quick-add-failure')
        favorite('quick-added-favorite',[CALCULATOR,FAILURE])
        tap(208,225)
        favorite('quick-reordered-favorite',[FAILURE,CALCULATOR])
        done('quick-done-before-failure')
        request(socket,'activate',id=before['cards'][0]['id'])
        state('quick-original-app-active',lambda s:s['view']==before['view']
              and s['presentation']['windows'][0]['opacity']==1.0)
        wave(42,0);screen.up()
        failed=state('quick-failed-launch',lambda s:s['launch_feedback'] is not None
                     and not s['launch_feedback']['pending'])
        assert failed['view']==before['view'] and failed['cards']==before['cards']
        capture('quick-failed-start-message')
        feedback_y=max(32,failed['keyboard_rect']['y']-145)
        tap(324,feedback_y+32)
        state('quick-failed-message-dismissed',lambda s:s['launch_feedback'] is None)
        wave(320,4);screen.up()
        state('quick-return-to-launcher',lambda s:s['view']=='Launcher')
        editor('quick-editor-again')
        tap(320,174)
        favorite('quick-removed-favorite',[CALCULATOR])
        # The packet's sample date must be replaced by the real local day.
        calendar='org.gnome.Calendar.desktop'
        add_visible_application(calendar,'quick-add-calendar')
        dated=favorite('quick-calendar-added',[CALCULATOR,calendar])
        assert dated['calendar_day']==datetime.datetime.now().day
        done('quick-editor-done-calendar')
        request(socket,'overview')
        state('quick-live-calendar-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
        capture('quick-live-calendar-in-tray')
        request(socket,'launcher')
        state('quick-calendar-launcher',lambda s:s['view']=='Launcher'
              and all(w['opacity']==0.0 for w in s['presentation']['windows']))
        editor('quick-calendar-editor')
        tap(320,225)
        favorite('quick-calendar-removed',[CALCULATOR])
        done('quick-editor-done')
        wave(42,0);screen.up()
        same_app('quick-favorite-restores-app')
        capture('quick-native-final')
        settings=tomllib.loads(Path('/home/user/.config/tarot/settings.toml').read_text())
        assert settings['version']==6 and settings['quick_launch']['applications']==[CALCULATOR]
        assert settings['notifications']['dnd'] and settings['display']['rotation_locked']
        records.update(passed=True,native_kms_and_uinput=True,installed_calculator=True,
            wave_preserves_native_geometry=True,release_focuses_same_card=True,outside_release_cancels=True,
            fixed_launcher=True,actual_favorite_add_move_remove=True,saved_favorites_verified=True,live_local_calendar_day=True,
            other_preference_sections_preserved=True,failed_start_keeps_original_app=True,
            restart_persistence_tested=False,password_logged=False)
    finally:
        (out/'quick-launch-validation.json').write_text(json.dumps(records,indent=2)+'\n')
    return records
