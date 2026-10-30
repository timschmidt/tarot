"""Installed mapped claim lifecycle using actual touch, key input and PAM.

No bus replacement is attempted on the device; the host case owns that bus.
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
from phone_lock_replay import Keyboard


def replay(control, screen, state, capture, before, out, cgroup, authenticate, guard):
    folder = out/'native-menu-claims'
    audit = json.loads((folder/'fixture-audit.json').read_text())
    result = {'passed': False, 'physical_finger_tested': False, 'processes': {},
              'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'targets': []}
    def status(): return request(control, 'status')
    def observe(label, predicate=lambda s: True): return state('native-claim-'+label, predicate)
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
        capture('native-claim-'+label);s=status();path=out/('native-claim-'+label+'.png')
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
    def mutate(command):
        prior=len([e for e in events('fixture') if e['event']=='claim-mutation'])
        path=folder/'commands.fifo';fd=os.open(path,os.O_WRONLY|os.O_NONBLOCK|os.O_NOFOLLOW)
        try:
            meta=os.fstat(fd);assert stat.S_ISFIFO(meta.st_mode) and meta.st_uid==10000 and stat.S_IMODE(meta.st_mode)==0o600
            assert command in ['claim-self','release-claim','clear-claim','flip','close-export','reconnect-export'] or command.startswith('claim-other ')
            assert len(command)<512 and os.write(fd,(command+'\n').encode())==len(command)+1
        finally:os.close(fd)
        values=await_value(lambda:events('fixture'),lambda es:len([e for e in es if e['event']=='claim-mutation'])==prior+1)
        assert [e for e in values if e['event']=='claim-mutation'][-1]['command']==command
    def counts(n):assert callbacks('fixture')==[{'event':'action','name':'choose','target':'night'}]*n and callbacks('foreign')==[]
    def dismiss():
        if status()['system_menu_open']:touch(40,16);observe('dismissed',lambda s:not s['system_menu_open'])
    def choose(label,n):
        dismiss();prefs(label);row('Choose',label+'-choose');await_value(lambda:callbacks('fixture'),lambda es:len(es)==n);observe(label+'-closed',lambda s:not s['system_menu_open']);counts(n)
    keyboard=None
    try:
        rotate('normal');request(control,'launch',id='test.tarot.NativeMenuClaims.desktop')
        mapped=observe('unclaimed-mapped',lambda s:len(s['cards'])==len(before['cards'])+1 and isinstance(s['view'],dict) and s['app_menu']['status']=='Absent')
        owner=next(c['id'] for c in mapped['cards'] if c['id'] not in [v['id'] for v in before['cards']])
        await_value(lambda:(folder/'processes.json').exists(),bool);processes=json.loads((folder/'processes.json').read_text())
        for name in ['runner','fixture','foreign']:
            actual=processes[name];p=Path('/proc',str(actual['pid']));assert p.stat().st_uid==actual['uid']==10000 and (p/'cgroup').read_text()==actual['cgroup']==cgroup
            assert (p/'stat').read_text().rsplit(')',1)[1].split()[19]==actual['start']
            assert hashlib.sha256((p/'exe').read_bytes()).hexdigest()==actual['executable_sha256']==audit['python_sha256' if name=='runner' else 'producer_sha256']
        assert processes['fixture']['pid']!=processes['foreign']['pid'];result['processes']=processes
        await_value(lambda:events('foreign'),lambda es:any(e['event']=='export' for e in es));foreign_owner=next(e['owner'] for e in events('foreign') if e['event']=='export')
        mutate('claim-self');observe('own-available',lambda s:s['app_menu']['status']=='Available' and s['app_menu']['owner']==owner);choose('first',1)
        rotate('left-up');prefs('foreign-held');down(*point('Choose','foreign-held-row'));time.sleep(.08);held=status()
        mutate('claim-other '+foreign_owner);changed=observe('foreign-rejected',lambda s:s['app_menu']['status']=='Unavailable' and not s['app_menu']['nodes'])
        assert changed['top_menu']['revision']!=held['top_menu']['revision'];screen.up();time.sleep(.15);counts(1);point('Show','foreign-fallback')
        for command,expected in [('clear-claim','Absent'),('claim-other org.tarot.MenuEgui','Absent'),('claim-other :9999.9999','Unavailable'),('release-claim','Absent')]:
            mutate(command);observe(command.replace(' ','-'),lambda s:s['app_menu']['status']==expected and not s['app_menu']['nodes']);counts(1)
        mutate('claim-self');observe('reclaimed',lambda s:s['app_menu']['status']=='Available');dismiss();prefs('key-held')
        keyboard=Keyboard(guard);keyboard.key(108,True);observe('key-selected',lambda s:s['top_menu']['selected'] is not None);held=status();mutate('flip')
        observe('key-selection-retired',lambda s:s['app_menu']['status']=='Available' and s['app_menu']['stamp']!=held['app_menu']['stamp'] and s['top_menu']['selected'] is None)
        time.sleep(.7);assert status()['top_menu']['selected'] is None;keyboard.key(108,False);keyboard.tap(28);time.sleep(.15);counts(1);keyboard.close();keyboard=None
        choose('fresh-after-key',2);rotate('normal')
        mutate('close-export');observe('export-disconnected',lambda s:s['app_menu']['status']=='Unavailable');counts(2)
        original=next(e['owner'] for e in events('fixture') if e['event']=='export');mutate('reconnect-export');replacement=next(e['owner'] for e in reversed(events('fixture')) if e['event']=='export');assert replacement!=original
        assert status()['app_menu']['status']=='Unavailable';mutate('claim-self');observe('replacement-claimed',lambda s:s['app_menu']['status']=='Available');choose('fresh-after-reconnect',3)
        old_source=status()['app_menu']['stamp']['source'];prefs('before-protection');request(control,'lock')
        protected=observe('protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True);assert set(protected)=={'ok','pid','lock'}
        mutate('release-claim');mutate('claim-self');counts(3);authenticate('native-claim-pam')
        observe('protected-claim-returned',lambda s:s['view']=={'App':owner} and not s['system_menu_open'] and s['app_menu']['status']=='Available' and s['app_menu']['stamp']['source']!=old_source);choose('fresh-after-pam',4)
        touch(40,16);observe('close-root',lambda s:s['system_menu_open']);row('Close','cooperative-close');observe('all-gone',lambda s:s['cards']==before['cards'] and not s['system_menu_open'])
        await_value(lambda:(folder/'result.json').exists(),bool);exits=json.loads((folder/'result.json').read_text());assert exits['passed'] and exits['fixture_exit']==exits['foreign_exit']==0
        await_value(lambda:all(not Path('/proc',str(processes[n]['pid'])).exists() for n in ['runner','fixture','foreign']),bool,seconds=10);counts(4)
        request(control,'activate',id=before['cards'][0]['id']);observe('original-card-restored',lambda s:s['view']==before['view'] and s['cards']==before['cards'])
        result.update(passed=True,callbacks=callbacks('fixture'),foreign_callbacks=callbacks('foreign'),actual_pam=True,held_key_retired=True,held_touch_retired=True,release_reclaim=True,same_bus_reconnect=True,processes_gone=True,orientations=['normal','left-up','normal'],bus_replacement_tested=False)
    finally:
        if keyboard is not None:keyboard.close()
        for slot in list(screen.active):screen.up(slot=slot)
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
