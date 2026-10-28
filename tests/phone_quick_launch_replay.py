"""Native Quick Launch slice for an authenticated installed-session replay.

The caller prepares one temporary failure desktop entry and a backed-up test
configuration with Calculator as its only favorite. All input uses the existing
guarded uinput touchscreen. No PAM policy or authentication result is replaced.
"""
import hashlib
import json
from pathlib import Path
import tomllib

from phone_gesture_replay import request


CALCULATOR = 'org.gnome.Calculator.desktop'
FAILURE = 'test.tarot.QuickLaunchFailure.desktop'


def replay(socket, screen, state, tap, capture, before, out):
    records = {'passed': False, 'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'physical_finger_tested': False}

    def same_app(label):
        value = state(label, lambda s:s['view']==before['view'] and s['presentation']['quick_launch'] is None)
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

    def editor(label):
        tap(285,67)
        return state(label,lambda s:s['editing_favorites'])

    def done(label):
        tap(285,67)
        return state(label,lambda s:not s['editing_favorites'])

    try:
        ready = state('quick-native-ready',lambda s:s['favorites']['applications']==[CALCULATOR]
                      and s['view']==before['view'] and len(s['cards'])==1
                      and s['presentation']['windows'][0]['opacity']==1.0)
        assert ready['output']['orientation']=='normal' and ready['output']['scale']==2
        assert ready['cards'][0]['app_id']=='org.gnome.Calculator'
        shown=wave(42,0)
        assert shown['view']==ready['view'] and shown['cards']==ready['cards']
        assert shown['presentation']['windows']==ready['presentation']['windows']
        capture('quick-wave-native-app')
        screen.up()
        same_app('quick-existing-app-restored')
        assert request(socket,'status')['launch_feedback'] is None
        wave(2,None);screen.up();same_app('quick-outside-release-canceled')
        wave(320,4);screen.up()
        state('quick-launcher',lambda s:s['view']=='Launcher')
        editor('quick-editor')
        capture('quick-editor-before')
        apps=request(socket,'applications')['applications']
        available=[a['id'] for a in apps if a['id']!=CALCULATOR]
        index=available.index(FAILURE)
        y=226+51*index
        assert y<580,'Temporary test entry is outside the verified visible list'
        tap(165,y)
        favorite('quick-added-favorite',[CALCULATOR,FAILURE])
        tap(208,196)
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
        feedback_y=max(28,failed['keyboard_rect']['y']-152)
        tap(324,feedback_y+32)
        state('quick-failed-message-dismissed',lambda s:s['launch_feedback'] is None)
        wave(320,4);screen.up()
        state('quick-return-to-launcher',lambda s:s['view']=='Launcher')
        editor('quick-editor-again')
        tap(320,145)
        favorite('quick-removed-favorite',[CALCULATOR])
        done('quick-editor-done')
        tap(42,650)
        same_app('quick-dock-tap-restores-app')
        capture('quick-native-final')
        settings=tomllib.loads(Path('/home/user/.config/tarot/settings.toml').read_text())
        assert settings['version']==3 and settings['quick_launch']['applications']==[CALCULATOR]
        assert settings['notifications']['dnd'] and settings['display']['rotation_locked']
        records.update(passed=True,native_kms_and_uinput=True,installed_calculator=True,
            wave_preserves_native_geometry=True,release_focuses_same_card=True,outside_release_cancels=True,
            fixed_launcher=True,actual_favorite_add_move_remove=True,saved_favorites_verified=True,
            other_preference_sections_preserved=True,failed_start_keeps_original_app=True,
            restart_persistence_tested=False,password_logged=False)
    finally:
        (out/'quick-launch-validation.json').write_text(json.dumps(records,indent=2)+'\n')
    return records
