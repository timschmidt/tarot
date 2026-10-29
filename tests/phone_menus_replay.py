"""Installed menu touch/rotation/PAM and independently observed device controls.

Caller provides temporary accepting/refusing native clients and restores the
owner's exact preferences. Captures and raw reports stay private.
"""
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess
import time
from PIL import Image

from card_matrix import physical
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request
from phone_lock_replay import Keyboard


def replay(control, screen, state, tap, capture, before, out, cgroup, authenticate):
    folder=out/'menus';assert folder.is_dir()
    result={'passed':False,'physical_finger_tested':False,'orientations':[],
            'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'targets':[]}
    keys=None
    fixture_binary=Path('/home/user/tarot-dev/bin/menu-input-probe-r40')
    audit=json.loads(Path('menu-input-probe-r40-audit.json').read_text())
    assert hashlib.sha256(fixture_binary.read_bytes()).hexdigest()==audit['binary_sha256']
    result['fixture_binary_sha256']=audit['binary_sha256']
    account=pwd.getpwuid(10000);groups=os.getgrouplist(account.pw_name,account.pw_gid)
    env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR='/run/user/10000')
    for name in ['WAYLAND_SOCKET','DISPLAY','PULSE_SERVER']:env.pop(name,None)
    def pactl(*args):
        return subprocess.check_output(['pactl',*map(str,args)],env=env,text=True,user=10000,group=account.pw_gid,extra_groups=groups,timeout=8)
    original_default=pactl('get-default-sink').strip()
    def sink():return next(o for o in json.loads(pactl('-f','json','list','sinks')) if o['name']==original_default)
    original_audio=sink();volumes=[v['value'] for v in original_audio['volume'].values()];muted=original_audio['mute']
    backlight=Path('/sys/class/backlight/backlight');requested=int((backlight/'brightness').read_text());actual=int((backlight/'actual_brightness').read_text())
    assert requested==actual
    original_controls={'brightness_requested':requested,'brightness_actual':actual,
                       'default_output':original_default,'volume_values':volumes,'muted':muted}
    (folder/'original-controls.json').write_text(json.dumps(original_controls,indent=2)+'\n')
    settings=Path('/home/user/.config/tarot/settings.toml');saved=settings.read_bytes()
    def status():return request(control,'status')
    def observe(label,predicate=lambda s:True):return state('menus-'+label,predicate)
    def dimensions(s):
        g=s['output'];w,h=g['physical_width']/g['scale'],g['physical_height']/g['scale']
        return (h,w) if g['orientation'] in ['left-up','right-up'] else (w,h)
    def down(x,y):
        px,py=physical(status(),x,y);screen.down(px/2,py/2)
    def move(x,y):
        px,py=physical(status(),x,y);screen.move(px/2,py/2)
    def touch(x,y):down(x,y);screen.up();time.sleep(.1)
    def picture(label):
        observe(label+'-settled',lambda s:not s['system_menu_open'] or not s['top_menu']['motion']['moving'])
        capture('menus-'+label);s=status();path=out/('menus-'+label+'.png')
        with Image.open(path) as source:
            source.load();rotation={'normal':None,'left-up':Image.Transpose.ROTATE_270,'bottom-up':Image.Transpose.ROTATE_180,'right-up':Image.Transpose.ROTATE_90}[s['output']['orientation']]
            image=source.copy() if rotation is None else source.transpose(rotation)
        width,height=dimensions(s);scale=s['output']['scale'];assert image.size==(round(width*scale),round(height*scale))
        path=folder/(label+'-upright.png');image.save(path);return image,path,s
    def point(word,label,side='left'):
        image,path,s=picture(label);width,height=dimensions(s);scale=s['output']['scale']
        left=0 if side=='left' else max(0,width-448)
        right=min(width,240) if side=='left' else width
        bounds=tuple(round(v*scale) for v in (left,32,right,height-36))
        path=path.with_name(path.stem+'-panel.png');image.crop(bounds).save(path)
        found=locate(path,word,vertical=(0,1));p=((bounds[0]+found['point'][0])/scale,(bounds[1]+found['point'][1])/scale)
        result['targets'].append({'label':label,'word':word,'logical_point':p,'ocr':found,'output_revision':s['output']['revision']})
        return p
    def item(word,label,side='left'):touch(*point(word,label,side))
    def device(label):
        width,_=dimensions(status());touch(width-35,16)
        return observe(label,lambda s:s['system_menu_open'] and s['top_menu']['entry']=='Device')
    def activity(label):
        touch(40,16);return observe(label,lambda s:s['system_menu_open'] and s['top_menu']['entry']=='Activity')
    def dismiss(label):
        touch(20,dimensions(status())[1]-60)
        return observe(label,lambda s:not s['system_menu_open'])
    def events(label):
        path=folder/(label+'.jsonl');return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    def fixture(label):
        request(control,'launch',id='test.tarot.Menu'+label+'.desktop')
        s=observe(label+'-mapped',lambda s:len(s['cards'])==len(before['cards'])+1 and isinstance(s['view'],dict) and any(w['opacity']==1 and w['rect']==s['app_area'] for w in s['presentation']['windows']))
        identifier=next(c['id'] for c in s['cards'] if c['id'] not in [c['id'] for c in before['cards']])
        candidates=[]
        for proc in Path('/proc').iterdir():
            if not proc.name.isdecimal():continue
            try:
                if (proc/'exe').resolve()==fixture_binary and str(folder/(label+'.jsonl')).encode() in (proc/'cmdline').read_bytes().split(b'\0'):
                    assert proc.stat().st_uid==10000 and (proc/'cgroup').read_text()==cgroup
                    assert hashlib.sha256((proc/'exe').read_bytes()).hexdigest()==audit['binary_sha256']
                    candidates.append(int(proc.name))
            except (FileNotFoundError,ProcessLookupError):pass
        assert len(candidates)==1
        return identifier,candidates[0]
    try:
        assert status()['output']['orientation']=='normal'
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            request(control,'rotate',orientation=orientation)
            observe('rotation-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'] and s['display_settings']['settings_saved'])
            activity('activity-'+orientation);picture('activity-'+orientation)
            device('switched-device-'+orientation);item('display','power-'+orientation,'right')
            observe('power-'+orientation,lambda s:s['system_page']=='Power')
            item('Back','back-'+orientation,'right');observe('root-'+orientation,lambda s:s['system_page']=='Device')
            dismiss('dismiss-'+orientation);assert status()['cards']==before['cards'] and status()['view']==before['view']
            result['orientations'].append(orientation)
        initial=device('brightness-ready')
        assert initial['power']['enabled'] and initial['power']['brightness']['can_set'] and initial['power']['brightness']['pending_percent'] is None
        assert int((backlight/'actual_brightness').read_text())==actual
        image,path,s=picture('brightness-before');_,label_y=point('Brightness','brightness-label','right');scale=s['output']['scale'];width,_=dimensions(s)
        pixels=[]
        for y in range(round((label_y+12)*scale),round((label_y+66)*scale)):
            for x in range(round((width-246)*scale),round((width-12)*scale)):
                rgb=image.getpixel((x,y))[:3]
                if 175<=min(rgb) and max(rgb)<=220 and max(rgb)-min(rgb)<25:pixels.append((x,y))
        assert len(pixels)>150
        center=(sum(x for x,y in pixels)/len(pixels)/scale,sum(y for x,y in pixels)/len(pixels)/scale)
        end=(min(width-28,center[0]+45) if center[0]<width-80 else max(width-220,center[0]-45),center[1])
        down(*center);time.sleep(.06);move(*end);time.sleep(.08);screen.up()
        changed=observe('brightness-changed',lambda s:s['power']['brightness']['pending_percent'] is None and s['power']['brightness']['actual']!=actual)
        independent=int((backlight/'actual_brightness').read_text());assert independent==changed['power']['brightness']['actual']
        result['brightness']={'original_raw':actual,'observed_raw':independent,'start':center,'end':end,'driver_matches':True}
        item('Sound','sound','right');audio=observe('audio-ready',lambda s:s['system_page']=='Audio' and s['audio']['available'] and s['audio']['pending'] is None)
        assert len(audio['audio']['outputs'])==1,'This replay requires the single built-in playback output'
        output=next(o for o in audio['audio']['outputs'] if o['default']);before_percent=output['percent']
        mute_point=point('Unmute' if output['muted'] else 'Mute','volume-controls','right')
        plus=before_percent<=90;volume_point=(mute_point[0]-(90 if plus else 162),mute_point[1]);touch(*volume_point)
        target=before_percent+10 if plus else before_percent-10
        observe('volume-changed',lambda s:s['audio']['pending'] is None and any(o['id']==output['id'] and o['percent']==target for o in s['audio']['outputs']))
        independent=max(v['value'] for v in sink()['volume'].values())*100/65536;assert abs(independent-target)<=1
        result['volume']={'before_percent':before_percent,'target_percent':target,'libpulse_percent':independent,'logical_point':volume_point}
        dismiss('audio-dismissed');assert settings.read_bytes()==saved
        activity('protected-menu');request(control,'lock');authenticate('menus-actual-pam')
        observe('protected-menu-retired',lambda s:not s['system_menu_open'] and s['view']==before['view'] and s['cards']==before['cards'])
        identifier,pid=fixture('Refuse');activity('held-key-menu')
        keys=Keyboard(screen.guard)
        balanced=lambda:[e for e in events('Refuse') if e['event']=='keyboard_key_balance']
        before_keys=len(balanced());before_enters=sum(e['event']=='keyboard_enter' for e in events('Refuse'))
        keys.key(15,True);keys.key(30,True)
        dismiss('held-key-menu-retired')
        await_value(lambda:events('Refuse'),lambda es:sum(e['event']=='keyboard_enter' for e in es)>before_enters)
        keys.key(15,False);keys.key(30,False);time.sleep(.15)
        assert len(balanced())==before_keys,'A shell-owned key release reached the native client'
        keys.tap(15);await_value(lambda:len(balanced()),lambda count:count==before_keys+2)
        pair=balanced()[-2:]
        assert [e['pressed'] for e in pair]==[True,False] and pair[-1]['held']==0
        assert not any(e['unmatched_release'] for e in balanced())
        keys.close();keys=None
        result['held_shell_key_retirement']={'suppressed_releases':2,'fresh_native_pair':pair,'passed':True}
        activity('refuse-menu');item('Close','refuse-close')
        observe('refused-client-alive',lambda s:any(c['id']==identifier and c['closing'] for c in s['cards']) and not s['system_menu_open'])
        await_value(lambda:events('Refuse'),lambda es:any(e['event']=='close_requested' and e['refused'] for e in es));assert Path('/proc',str(pid)).exists()
        activity('after-refusal');item('cards','refused-show-cards');observe('refused-overview',lambda s:s['view']=='Overview' and s['presentation']['overview']==1)
        request(control,'activate',id=identifier);observe('refused-return',lambda s:s['view']=={'App':identifier} and not next(c for c in s['cards'] if c['id']==identifier)['closing'])
        activity('client-death-menu');fd=os.pidfd_open(pid)
        try:
            assert (Path('/proc',str(pid))/'cgroup').read_text()==cgroup
            assert str(folder/'Refuse.jsonl').encode() in (Path('/proc',str(pid))/'cmdline').read_bytes().split(b'\0')
            signal.pidfd_send_signal(fd,signal.SIGTERM)
        finally:os.close(fd)
        observe('client-death',lambda s:s['cards']==before['cards'] and not s['system_menu_open'])
        identifier,pid=fixture('Accept');activity('accept-menu');item('Close','accept-close')
        observe('accept-client-gone',lambda s:s['cards']==before['cards'] and not s['system_menu_open'])
        assert any(e['event']=='close_requested' and not e['refused'] for e in events('Accept'))
        request(control,'activate',id=before['cards'][0]['id']);observe('original-restored',lambda s:s['view']==before['view'] and s['cards']==before['cards'])
        assert settings.read_bytes()==saved
        result.update(passed=True,actual_pam=True,consumed_outside_contact=True,cooperative_and_refused_close=True,client_death_retires_menu=True)
    finally:
        if keys is not None:keys.close()
        for slot in list(screen.active):screen.up(slot=slot)
        (backlight/'brightness').write_text(str(requested)+'\n')
        await_value(lambda:int((backlight/'actual_brightness').read_text()),lambda value:value==actual)
        pactl('set-sink-volume',original_default,*volumes);pactl('set-sink-mute',original_default,int(muted))
        restored=sink();result['exact_original_audio_restored']=pactl('get-default-sink').strip()==original_default and [v['value'] for v in restored['volume'].values()]==volumes and restored['mute']==muted
        result['exact_original_brightness_restored']=int((backlight/'brightness').read_text())==requested and int((backlight/'actual_brightness').read_text())==actual
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
        assert result['exact_original_audio_restored'] and result['exact_original_brightness_restored']
    return result
