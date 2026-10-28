#!/usr/bin/env python3
"""Installed preferences restart, real touch/PAM in every pose and idle timing.

Root-only authorized spare-phone QA. A numeric password lives only in controller
memory. Key coordinates come from independent empty-entry host pixel captures;
no protected phone capture or synthetic authentication result is used.
"""
import argparse
import ast
import getpass
import hashlib
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
import tomllib
from types import SimpleNamespace

from lock_replay import any_request
from notification_replay import await_value
from phone_gesture_replay import Touchscreen, request
from phone_stock_login import login, processes

BASE=Path('/home/user/tarot-dev')
CONTROL=Path('/run/user/10000/tarot-session/control.sock')
CONFIG=Path('/home/user/.config/tarot/settings.toml')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--portrait-layout',type=Path,required=True)
    parser.add_argument('--landscape-layout',type=Path,required=True)
    parser.add_argument('--touch-idle-controls',action='store_true')
    parser.add_argument('--restore-session',choices=['tarot','phosh'],default='tarot')
    args=parser.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    os.umask(0o077)
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    os.chown(args.output,10000,pwd.getpwuid(10000).pw_gid)
    layouts={False:json.loads(args.portrait_layout.read_text()),True:json.loads(args.landscape_layout.read_text())}
    assert all(len(v['first_row'])==10 for v in layouts.values())
    assert not CONFIG.is_symlink()
    original_parent_absent=not CONFIG.parent.exists()
    original=CONFIG.read_bytes() if CONFIG.exists() else None
    if CONFIG.exists():assert CONFIG.is_file() and not CONFIG.is_symlink() and CONFIG.stat().st_uid==10000 and len(original)<=65536
    record={'binaries_sha256':{n:hashlib.sha256(Path('/usr/bin',n).read_bytes()).hexdigest() for n in ['tarot','tarot-session','tarot-lock','tarot-keyboard']},
            'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'sessions':[],
            'original_settings_absent':original is None,'restore_session':args.restore_session,
            'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    screen=None;target=None;typing=False;expected_pose=None;auth_generation=None;password=None;changed_config=False

    def owned(path,blob):
        path.write_bytes(blob);os.chown(path,10000,pwd.getpwuid(10000).pw_gid);os.chmod(path,0o600)

    def save():owned(args.output/'status.json',(json.dumps(record,indent=2)+'\n').encode())

    def state(label,condition=lambda s:True,seconds=25):
        value=await_value(lambda:request(CONTROL,'status'),condition,seconds=seconds)
        if target is not None:assert value['pid']==target['pid']
        record[label]=value;save();return value

    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty7'
        assert target in processes()
        if typing:
            value=request(CONTROL,'status')['lock']
            assert value['phase']=='Locked' and value['generation']==auth_generation
            assert value['last_presented_surface'] is True and value['last_presented_generation']==auth_generation
            assert value['output']['orientation']==expected_pose and value['output']['requested'] is None and value['output']['presented_revision']==value['output']['revision']

    def tap(x,y,geometry):
        w,h=geometry['physical_width'],geometry['physical_height'];assert (w,h,geometry['scale'])==(720,1440,2)
        pose=geometry['orientation'];lw,lh=(720,360) if pose in ['left-up','right-up'] else (360,720)
        x,y=x/lw,y/lh
        x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[pose]
        screen.down(x*360,y*720);screen.up();time.sleep(.08)

    def protected(label):
        return state(label,lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'] and s['lock']['output']['requested'] is None and s['lock']['output']['revision']==s['lock']['output']['presented_revision'])

    def authenticate(label):
        nonlocal typing,expected_pose,auth_generation
        covered=protected(label+'-covered');lock=covered['lock'];geometry=lock['output']
        expected_pose=geometry['orientation'];auth_generation=lock['generation']
        landscape=expected_pose in ['left-up','right-up'];width,height=(720,360) if landscape else (360,720)
        if lock['idle']['output_off']:
            tap(width/2,height-112,geometry)
            covered=state(label+'-wake',lambda s:s['lock']['phase']=='Locked' and not s['lock']['idle']['output_off'] and s['lock']['last_presented_surface'] is True)
        covered=state(label+'-awake',lambda s:s['lock']['phase']=='Locked' and s['lock']['idle']['policy']['phase']=='Awake' and not s['lock']['idle']['output_off'] and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'])
        typing=True
        tap(width/2,height-112,geometry)
        covered=state(label+'-entry',lambda s:s['lock']['presented_frames']>covered['lock']['presented_frames'])
        template=layouts[landscape];sw,sh=template['logical_size'];old_height=min(240,sh/2);new_height=min(240,height/2)

        def key(point):
            return point[0]*width/sw,height-new_height+(point[1]-(sh-old_height))*new_height/old_height

        tap(*key(template['mode_key']),geometry)
        state(label+'-symbols',lambda s:s['lock']['presented_frames']>covered['lock']['presented_frames'])
        for digit in password:
            tap(*key(template['first_row'][(int(digit)-1)%10]),geometry)
        typing=False  # No credential input remains after submission can unlock.
        tap(width/2-88 if landscape else width/2,height/2-24 if landscape else 330,geometry)
        return state(label+'-ordinary',lambda s:s['lock']['phase']=='Unlocked' and s['lock']['unprotected_presentations']>covered['lock']['unprotected_presentations'])

    def capture(label):
        request(CONTROL,'capture',path=str(args.output/(label+'.png')))
        return state(label,lambda s:not s['capture_pending'] and not s['capture_error'])

    def begin(number):
        nonlocal target
        time.sleep(1)  # Newly returned Phrog must finish its PAM/socket setup.
        login(SimpleNamespace(output=args.output/f'login-{number}',session='tarot',diagnostics=True),password)
        first=request(CONTROL,'status');target=next(p for p in processes() if p['pid']==first['pid'])
        assert target['uid']==10000 and Path('/proc',str(target['pid']),'exe').resolve()==Path('/usr/bin/tarot')
        first=protected(f'session-{number}-first-frame')
        assert first['lock']['first_presented_protected'] is True and first['lock']['unprotected_presentations']==0
        group=Path('/proc',str(target['pid']),'cgroup').read_text()
        record['sessions'].append({'number':number,'compositor_cgroup':group,'pid':target['pid']});save()
        return first

    def end():
        nonlocal target
        if target is None:return
        if target in processes():request(CONTROL,'exit')
        old=record['sessions'][-1]['compositor_cgroup']
        await_value(processes,lambda ps:target not in ps and any(p['name']=='phrog' and p['uid']==109 for p in ps),seconds=35)
        remaining=[]
        for proc in Path('/proc').iterdir():
            if not proc.name.isdecimal():continue
            try:
                if (proc/'cgroup').read_text()==old:remaining.append(int(proc.name))
            except OSError:pass
        assert not remaining and not CONTROL.exists()
        record['sessions'][-1]['cgroup_empty_and_socket_removed']=True;target=None;save()

    try:
        password=getpass.getpass('Numeric device password for persistent-settings touch/PAM replay: ')
        assert password and password.isascii() and password.isdecimal()
        active=[p for p in processes() if p['uid']==10000 and p['name'] in ['tarot','phosh']]
        assert len(active)==1
        stock=active[0]
        record['initial_session']=stock['name']
        response=subprocess.check_output(['gdbus','call','--system','--dest','org.freedesktop.login1','--object-path','/org/freedesktop/login1','--method','org.freedesktop.login1.Manager.GetSessionByPID',str(stock['pid'])],text=True)
        path=ast.literal_eval(response.replace('objectpath ',''))[0]
        response=subprocess.check_output(['gdbus','call','--system','--dest','org.freedesktop.login1','--object-path',path,'--method','org.freedesktop.DBus.Properties.Get','org.freedesktop.login1.Session','Id'],text=True)
        sid=ast.literal_eval(response.replace('<','').replace('>',''))[0]
        props=subprocess.check_output(['loginctl','show-session',sid,'--no-pager','-p','User','-p','VTNr','-p','Remote','-p','Active'],text=True)
        assert all(x in props.splitlines() for x in ['User=10000','VTNr=7','Remote=no','Active=yes']) and stock in processes()
        subprocess.run(['loginctl','terminate-session',sid],check=True)
        await_value(processes,lambda ps:stock not in ps and any(p['name']=='phrog' and p['uid']==109 for p in ps),seconds=35)
        subprocess.run(['mkdir','-p',str(CONFIG.parent)],user=10000,group=pwd.getpwuid(10000).pw_gid,check=True)
        if original is not None:owned(args.output/'original-settings.bin',original)
        old=b'version = 1\n[notifications]\ndnd = true\nhide_previews = true\n'
        owned(CONFIG,old);changed_config=True
        screen=Touchscreen(guard)
        begin(1);first=authenticate('migration')
        assert first['output']['orientation']=='normal'
        state('version-one-notification-policy',lambda s:s['notifications']['available'] and s['notifications']['dnd'] and s['notifications']['hide_previews'])
        assert CONFIG.read_bytes()==old
        request(CONTROL,'system_menu',open=True);capture('rotation-checkbox')
        print('Review rotation-checkbox.png, then supply the checkbox center.',flush=True)
        point=[int(x) for x in input('Reviewed rotation-lock logical x,y: ').split(',')]
        assert len(point)==2 and 0<=point[0]<360 and 0<=point[1]<720
        tap(*point,first['output'])
        state('rotation-lock-by-touch',lambda s:s['rotation_locked'] and not s['sensors']['claimed'])
        idle={'lock_after_seconds':60,'screen_off_enabled':True,'off_after_seconds':60}
        if args.touch_idle_controls:
            # Scroll the actual touch menu, then let the operator review only
            # ordinary settings frames before supplying each target.
            screen.down(300,600)
            for step in range(1,13):screen.move(300,600-30*step,delay=.03)
            screen.up();time.sleep(.25)
            for label in ['idle-lock-control','idle-lock-one-minute','idle-off-control','idle-off-one-minute']:
                capture(label)
                point=[int(x) for x in input('Reviewed '+label+' logical x,y: ').split(',')]
                assert len(point)==2 and 0<=point[0]<360 and 0<=point[1]<720
                tap(*point,first['output'])
            state('idle-timeouts-by-touch',lambda s:s['display_settings']['idle']==idle)
        else:
            request(CONTROL,'idle_timeouts',settings=idle)
        request(CONTROL,'system_menu',open=False)
        request(CONTROL,'notification_action',action={'Settings':{'dnd':True,'hide_previews':False}})
        state('migrated-and-saved',lambda s:s['display_settings']['idle']==idle and s['display_settings']['settings_saved'] and s['notifications']['settings_saved'] and not s['notifications']['hide_previews'])
        disk=tomllib.loads(CONFIG.read_text());assert disk['version']==2 and disk['display']['rotation_locked'] and disk['display']['idle']==idle and disk['notifications']=={'dnd':True,'hide_previews':False}
        assert CONFIG.stat().st_uid==10000 and CONFIG.stat().st_mode&0o777==0o600
        end()
        restored=begin(2);assert restored['lock']['output']['orientation']=='normal' and restored['lock']['idle']['policy']['lock_after']['secs']==60 and restored['lock']['idle']['policy']['off_after']['secs']==60
        authenticate('restored')
        catalog=request(CONTROL,'applications')['applications'];app=next(a for a in catalog if a['id']=='org.gnome.Calculator.desktop')
        request(CONTROL,'launch',id=app['id']);before=state('native-app',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and 'App' in s['view'])
        started=time.monotonic()
        off=state('saved-timeout-panel-off',lambda s:s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase']=='Off',seconds=75)
        record['waited_for_panel_seconds']=time.monotonic()-started;assert 35<record['waited_for_panel_seconds']<75
        panels=[p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip()=='connected'];assert len(panels)==1
        assert (panels[0]/'dpms').read_text().strip()=='Off' and Path('/sys/class/backlight/backlight/bl_power').read_text().strip()=='4'
        tap(180,608,off['lock']['output']);protected('wake-protected')
        after=authenticate('idle-wake');assert after['cards']==before['cards'] and after['view']==before['view']
        request(CONTROL,'idle_timeouts',settings={'lock_after_seconds':120,'screen_off_enabled':True,'off_after_seconds':180})
        request(CONTROL,'rotate',orientation='left-up')
        state('landscape-before-save',lambda s:s['output']['orientation']=='left-up' and s['output']['revision']==s['output']['presented_revision'])
        request(CONTROL,'rotation_lock',locked=True)
        state('landscape-saved',lambda s:s['display_settings']['settings_saved'])
        assert tomllib.loads(CONFIG.read_text())['display']['locked_orientation']=='left-up'
        end()
        landscape=begin(3);assert landscape['lock']['output']['orientation']=='left-up'
        authenticate('restored-landscape')
        for pose in ['bottom-up','right-up','normal']:
            request(CONTROL,'rotate',orientation=pose)
            state('before-lock-'+pose,lambda s:s['output']['orientation']==pose and s['output']['revision']==s['output']['presented_revision'])
            request(CONTROL,'lock');authenticate('touch-pam-'+pose)
        capture('ordinary-after-all-orientations');end()
        record['validation']={'passed':True,'version_one_migrated_and_sections_preserved':True,'rotation_checkbox_touched':True,
                              'idle_timeout_controls_touched':args.touch_idle_controls,
                              'saved_idle_policy_actual_driver_off_and_pam_wake':True,'locked_landscape_restored_before_first_frame':True,
                              'actual_touch_keyboard_and_pam_all_orientations':True,'all_session_cgroups_empty':True,'password_recorded':False}
    except BaseException as error:
        record['validation']={'passed':False,'exception_type':type(error).__name__}
        raise
    finally:
        typing=False
        try:end()
        except BaseException as error:record['session_cleanup_error']=type(error).__name__
        if screen is not None:screen.close()
        if changed_config and target is None:
            try:
                if original is None:CONFIG.unlink(missing_ok=True)
                else:owned(CONFIG,original)
                record['original_settings_restored']=(not CONFIG.exists()) if original is None else CONFIG.read_bytes()==original
                if original_parent_absent:CONFIG.parent.rmdir()
            except BaseException as error:record['settings_restore_error']=type(error).__name__
        if password and not any(p['name']==args.restore_session and p['uid']==10000 for p in processes()) and target is None:
            try:login(SimpleNamespace(output=args.output/'restored-session-login',session=args.restore_session,diagnostics=False),password)
            except BaseException as error:record['session_restore_error']=type(error).__name__
        record['stock_restored']=any(p['name']=='phosh' and p['uid']==10000 for p in processes())
        record['session_restored']=target is None and any(p['name']==args.restore_session and p['uid']==10000 for p in processes()) and (args.restore_session!='tarot' or not CONTROL.exists())
        clean=record['session_restored'] and target is None and (not changed_config or record.get('original_settings_restored',False)) and not any(key.endswith('_error') for key in record)
        record.setdefault('validation',{'passed':False})['passed'] &= clean
        password=None;save()


if __name__=='__main__':main()
