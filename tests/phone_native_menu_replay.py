"""Real installed GTK/Rust-egui exports, finger input, rotation and PAM.

The controller stages catalog entries and owned stdin FIFOs; commands mutate
exports only. Every action invocation below requires an actual menu contact.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import time

from PIL import Image
from card_matrix import physical
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request


def replay(control, screen, state, capture, before, out, cgroup, authenticate):
    folder = out/'native-menus'
    audit = json.loads((folder/'fixtures.json').read_text())
    result = {'passed': False, 'physical_finger_tested': False, 'producers': {},
              'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'targets': []}
    def status(): return request(control, 'status')
    def observe(label, predicate=lambda s: True): return state('native-menu-'+label, predicate)
    def dimensions(s):
        g=s['output'];w,h=g['physical_width']/g['scale'],g['physical_height']/g['scale']
        return (h,w) if g['orientation'] in ['left-up','right-up'] else (w,h)
    def down(x,y):
        px,py=physical(status(),x,y);screen.down(px/2,py/2)
    def move(x,y):
        px,py=physical(status(),x,y);screen.move(px/2,py/2)
    def touch(x,y): down(x,y);screen.up();time.sleep(.1)
    def rotate(orientation):
        request(control,'rotate',orientation=orientation)
        return observe('rotation-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'] and s['display_settings']['settings_saved'])
    def picture(label):
        observe(label+'-settled',lambda s:not s['system_menu_open'] or not s['top_menu']['motion']['moving'])
        capture('native-menu-'+label);s=status();path=out/('native-menu-'+label+'.png')
        with Image.open(path) as source:
            rotation={'normal':None,'left-up':Image.Transpose.ROTATE_270,'bottom-up':Image.Transpose.ROTATE_180,'right-up':Image.Transpose.ROTATE_90}[s['output']['orientation']]
            image=source.copy() if rotation is None else source.transpose(rotation)
        w,h=dimensions(s);scale=s['output']['scale'];assert image.size==(round(w*scale),round(h*scale))
        image.save(folder/(label+'-upright.png'));return image,s
    def point(word,label):
        image,s=picture(label);scale=s['output']['scale'];_,height=dimensions(s);rgb=image.convert('RGB');bottom=round((height-36)*scale)
        for y in range(round(48*scale),bottom):
            red,green,blue=rgb.getpixel((round(8*scale),y))
            if not (30<=red<=85 and 0<=green-red<=16 and 0<=blue-red<=16):bottom=y;break
        rejected=[];found=None
        # The phone's OCR model can merge a check mark with a dimmed label.
        # Retry only preprocessing of the same captured pixels, omitting the
        # check column; never retry a selection or lower recognition confidence.
        for left in [12,40]:
            bounds=(round(left*scale),round(40*scale),round(232*scale),bottom-round(4*scale));assert bounds[3]>bounds[1]+40
            path=folder/(label+'-panel-'+str(left)+'.png');image.crop(bounds).save(path)
            try:found=locate(path,word,vertical=(0,1));break
            except AssertionError as error:rejected.append({'bounds':bounds,'error':str(error)})
        assert found is not None,(word,rejected)
        p=((bounds[0]+found['point'][0])/scale,(bounds[1]+found['point'][1])/scale)
        result['targets'].append({'label':label,'word':word,'point':p,'bounds':bounds,'ocr':found,'rejected_crops':rejected,'output_revision':s['output']['revision']});return p
    def row(word,label): touch(*point(word,label))
    def prefs(label):
        touch(40,16);observe(label+'-open',lambda s:s['system_menu_open']);row('Preferences',label+'-preferences')
        return observe(label+'-submenu',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Native' in s['top_menu']['panel'])
    def events(label):
        path=folder/(label+'.jsonl');return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    def callbacks(label): return [e for e in events(label) if e['event']=='action']
    def mutate(label,command):
        path=folder/(label+'.fifo');fd=os.open(path,os.O_WRONLY|os.O_NONBLOCK|os.O_NOFOLLOW)
        try:
            meta=os.fstat(fd);assert stat.S_ISFIFO(meta.st_mode) and meta.st_uid==10000 and stat.S_IMODE(meta.st_mode)==0o600
            assert command in ['flip','disable'];assert os.write(fd,(command+'\n').encode())==len(command)+1
        finally: os.close(fd)
    try:
        for label in ['Gtk','Egui']:
            rotate('normal');request(control,'launch',id='test.tarot.NativeMenu'+label+'.desktop')
            mapped=observe(label+'-mapped',lambda s:len(s['cards'])==len(before['cards'])+1 and isinstance(s['view'],dict) and s['app_menu']['status']=='Available')
            owner=next(c['id'] for c in mapped['cards'] if c['id'] not in [v['id'] for v in before['cards']]);assert mapped['app_menu']['owner']==owner
            candidates=[]
            for p in Path('/proc').iterdir():
                if not p.name.isdecimal():continue
                try:
                    if str(folder/(label+'.jsonl')).encode() in (p/'cmdline').read_bytes().split(b'\0'):
                        assert p.stat().st_uid==10000 and (p/'cgroup').read_text()==cgroup
                        actual=hashlib.sha256((p/'exe').read_bytes()).hexdigest();assert actual==audit[label]['executable_sha256'];candidates.append({'pid':int(p.name),'executable_sha256':actual})
                except (FileNotFoundError,ProcessLookupError):pass
            assert len(candidates)==1
            case={'process':candidates[0], 'orientations':[]};result['producers'][label]=case
            prefs(label+'-choose');row('Choose',label+'-night');await_value(lambda:callbacks(label),lambda es:len(es)==1);observe(label+'-choose-closed',lambda s:not s['system_menu_open'])
            prefs(label+'-toggle');row('Toggle',label+'-flag');await_value(lambda:callbacks(label),lambda es:len(es)==2)
            checked=observe(label+'-checked',lambda s:not s['system_menu_open'] and s['app_menu']['status']=='Available' and s['app_menu']['nodes'][0]['kind']['Submenu'][1]['checked'])
            case['orientations'].append('normal');source=checked['app_menu']['stamp']['source'];w,h=dimensions(checked)
            touch(w/2,h-18);overview=observe(label+'-cards',lambda s:s['view']=='Overview' and s['presentation']['overview']==1)
            window=next(v for v in overview['presentation']['windows'] if v['id']==owner);r=window['rect'];touch(r['x']+r['width']/2,r['y']+r['height']/2)
            observe(label+'-returned',lambda s:s['view']=={'App':owner} and s['app_menu']['status']=='Available' and s['app_menu']['stamp']['source']!=source and s['presentation']['overview']==0)
            rotate('left-up');case['orientations'].append('left-up');prefs(label+'-held');x,y=point('Choose',label+'-held-choice');down(x,y);time.sleep(.08)
            held=status();mutate(label,'flip')
            changed=observe(label+'-held-invalidated',lambda s:s['app_menu']['status']=='Available' and s['app_menu']['stamp']!=held['app_menu']['stamp']);assert changed['top_menu']['revision']!=held['top_menu']['revision'] and changed['app_menu']['nodes']==held['app_menu']['nodes']
            screen.up();time.sleep(.15);assert len(callbacks(label))==2
            row('Preferences',label+'-fresh-preferences');observe(label+'-fresh-submenu',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Native' in s['top_menu']['panel']);row('Choose',label+'-fresh-choice');await_value(lambda:callbacks(label),lambda es:len(es)==3);observe(label+'-fresh-closed',lambda s:not s['system_menu_open'])
            expected=[{'event':'action','name':'choose','target':'night'},{'event':'action','name':'toggle','target':None},{'event':'action','name':'choose','target':'night'}]
            if label=='Egui':
                prefs(label+'-window');down(160,245)
                for step in range(1,7):move(160,245-step*15);time.sleep(.02)
                time.sleep(.1);screen.up();time.sleep(.2);row('Window',label+'-seven');await_value(lambda:callbacks(label),lambda es:len(es)==4);observe(label+'-window-closed',lambda s:not s['system_menu_open']);expected.append({'event':'action','name':'window','target':7})
            mutate(label,'disable');observe(label+'-disabled-update',lambda s:s['app_menu']['status']=='Available' and not s['app_menu']['nodes'][0]['kind']['Submenu'][0]['enabled'])
            prefs(label+'-disabled');row('Choose',label+'-disabled-choice');time.sleep(.15);assert callbacks(label)==expected;picture(label+'-disabled-visible')
            # Geometry preserves an open panel and retires its old choices.
            # Protection discards the menu and its foreground worker.
            geometry_before=status();rotated=rotate('normal');case['orientations'].append('normal')
            assert rotated['system_menu_open'] and rotated['top_menu']['revision']!=geometry_before['top_menu']['revision']
            touch(40,16);observe(label+'-rotated-menu-dismissed',lambda s:not s['system_menu_open'])
            old_source=status()['app_menu']['stamp']['source'];prefs(label+'-before-lock');request(control,'lock')
            protected=observe(label+'-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True);assert 'app_menu' not in protected
            authenticate('native-menu-'+label+'-pam')
            observe(label+'-unlocked',lambda s:s['view']=={'App':owner} and not s['system_menu_open'] and s['app_menu']['status']=='Available' and s['app_menu']['stamp']['source']!=old_source);assert callbacks(label)==expected
            prefs(label+'-close');row('Back',label+'-back');observe(label+'-root',lambda s:s['top_menu']['panel']=='Root');row('Close',label+'-close-card')
            observe(label+'-gone',lambda s:s['cards']==before['cards'] and not s['system_menu_open']);await_value(lambda:Path('/proc',str(candidates[0]['pid'])).exists(),lambda alive:not alive,seconds=8)
            assert callbacks(label)==expected;case.update(passed=True,callbacks=callbacks(label),held_flip_retired=True,actual_pam=True,cooperative_close=True,claim_removed=True)
            request(control,'activate',id=before['cards'][0]['id']);observe(label+'-restored',lambda s:s['view']==before['view'] and s['cards']==before['cards'])
        result['passed']=True
    finally:
        for slot in list(screen.active):screen.up(slot=slot)
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
