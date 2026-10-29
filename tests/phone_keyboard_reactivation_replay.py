"""Keep native app contacts alive when its text input remaps the trusted OSK."""
import hashlib
import json
import os
from pathlib import Path
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_stock_login import processes
from phone_lock_replay import Keyboard


def replay(control,screen,tap,state,capture,out,cgroup,authenticate,require_remap=True):
    prefix='reactivation' if require_remap else 'context'
    folder=out/('keyboard-reactivation' if require_remap else 'keyboard-context');folder.mkdir(mode=0o700,exist_ok=True)
    os.chown(folder,10000,10000);report=folder/'events.jsonl'
    result={'passed':False,'physical_finger_tested':False,'cases':[],'require_surface_remap':require_remap}
    identifier=None;keys=None
    launchers=[p for p in processes() if p['uid']==10000 and p['name']=='tarot-session']
    assert len(launchers)==1
    logs=list(Path('/home/user/.local/state/tarot/sessions').glob('session-*-'+str(launchers[0]['pid'])+'.log'))
    assert len(logs)==1;session_log=logs[0]
    def metadata():
        return [json.loads(line) for line in session_log.read_text().splitlines() if line.startswith('{"event":"keyboard-metadata"')]
    def generation():return max((e['generation'] for e in metadata()),default=0)
    def serial():return max((e['serial'] for e in metadata() if e['stage']=='context-done'),default=0)
    def activation(before_generation,before_serial):
        es=await_value(metadata,lambda es:any(e['stage']=='context-done' and e['serial']>before_serial for e in es),seconds=5)
        if require_remap:
            return await_value(metadata,lambda es:any(e['stage']=='configure' and e['generation']>before_generation for e in es),seconds=5)
        assert max(e['generation'] for e in es)==before_generation,{'unchanged_context_remapped':True,'before':before_generation,'after':max(e['generation'] for e in es)}
        assert any(e['stage']=='context-reset' and e['serial']>before_serial for e in es)
        return es
    def fresh(start):return events(report)[start:]
    def remap_while_held(label,slot):
        start=len(events(report));before=generation();before_serial=serial()
        result['pending_case']={'label':label,'slot':slot,'before_generation':before}
        screen.down(180,180,slot=slot)
        await_value(lambda:fresh(start),lambda es:any(e['event']=='keyboard_enable' and e['reason']=='touch' for e in es),seconds=5)
        configured=activation(before,before_serial)
        assert slot in screen.active
        screen.move(185,185,slot=slot)
        observed=await_value(lambda:fresh(start),lambda es:any(e['event'] in ['touch_motion','touch_cancel'] for e in es),seconds=5)
        result['pending_case'].update(generation_after=max(e['generation'] for e in configured),events=observed)
        assert not any(e['event']=='touch_cancel' for e in observed),{'app_touch_canceled_by_keyboard_remap':label}
        return start
    def release_app(label,slot,start):
        screen.up(slot=slot)
        observed=await_value(lambda:fresh(start),lambda es:any(e['event'] in ['touch_up','touch_cancel'] for e in es),seconds=5)
        assert any(e['event']=='touch_up' for e in observed) and not any(e['event']=='touch_cancel' for e in observed),observed
        result['cases'].append({'label':label,'passed':True,'events':observed});result.pop('pending_case',None)
    try:
        assert not request(control,'status')['cards']
        request(control,'launch',id='test.tarot.'+('KeyboardReactivation' if require_remap else 'KeyboardContext')+'.desktop')
        s=state(prefix+'-ready',lambda s:len(s['cards'])==1 and s['keyboard_visible'])
        identifier=s['cards'][0]['id'];assert s['keyboard_owner']==identifier
        fixture='keyboard-reactivation-probe-r32-v3' if require_remap else 'keyboard-reactivation-probe-r29-v2'
        binary=Path('/home/user/tarot-dev/bin')/fixture
        audit=json.loads(Path(fixture+'-audit.json').read_text())
        assert hashlib.sha256(binary.read_bytes()).hexdigest()==audit['binary_sha256']
        found=[]
        for proc in Path('/proc').iterdir():
            if not proc.name.isdecimal():continue
            try:
                if proc.stat().st_uid==10000 and (proc/'exe').resolve()==binary:
                    assert (proc/'cgroup').read_text()==cgroup
                    assert hashlib.sha256((proc/'exe').read_bytes()).hexdigest()==audit['binary_sha256']
                    found.append({'pid':int(proc.name),'start':(proc/'stat').read_text().rsplit(')',1)[1].split()[19]})
            except (FileNotFoundError,ProcessLookupError,PermissionError):pass
        assert len(found)==1;result['fixture']={**found[0],'binary_sha256':audit['binary_sha256'],'same_login_cgroup':True}
        await_value(lambda:events(report),lambda es:any(e['event']=='keyboard_enable' and e['reason']=='enter' for e in es),seconds=5)
        capture(prefix+'-initial-keyboard')
        start=remap_while_held('app-contact',0);release_app('app-contact',0,start)
        # Both contacts remain with the app's implicit touch grab through remaps.
        start=remap_while_held('app-multitouch',0)
        second=len(events(report));before_generation=generation();before_serial=serial();screen.down(200,200,slot=1)
        await_value(lambda:fresh(second),lambda es:any(e['event']=='keyboard_enable' and e['reason']=='touch' for e in es),seconds=5)
        activation(before_generation,before_serial)
        screen.move(190,190,slot=0);screen.move(205,205,slot=1)
        screen.up(slot=1);screen.up(slot=0)
        observed=await_value(lambda:fresh(start),lambda es:sum(e['event']=='touch_up' for e in es)==2 or any(e['event']=='touch_cancel' for e in es),seconds=5)
        assert sum(e['event']=='touch_down' for e in observed)==2 and sum(e['event']=='touch_up' for e in observed)==2 and not any(e['event']=='touch_cancel' for e in observed),observed
        result['cases'].append({'label':'app-multitouch','passed':True,'events':observed});result.pop('pending_case',None)
        # Physical F12 asks the synthetic app to re-enable while the OSK owns
        # touch. Other contacts in this implicit grab would also belong to OSK.
        def key_guard():
            value=request(control,'status')
            assert value['pid']==s['pid'] and value['lock']['phase']=='Unlocked'
            assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty7'
        keys=Keyboard(key_guard)
        before=len(events(report));meta_start=len(metadata());screen.down(106,609,slot=0)
        await_value(metadata,lambda es:any(e['stage']=='touch-down' for e in es[meta_start:]),seconds=5)
        before_generation=generation();before_serial=serial();keys.tap(88)
        await_value(lambda:fresh(before),lambda es:any(e['event']=='keyboard_enable' and e['reason']=='key' for e in es),seconds=5)
        activation(before_generation,before_serial)
        assert any(e['stage']==('touch-cancel' if require_remap else 'context-reset') for e in metadata()[meta_start:])
        screen.up(slot=0)
        assert not any(e['event']=='text_commit' for e in fresh(before)),'Canceled held key committed into the new context'
        start=len(events(report));tap(22.66667,504)
        await_value(lambda:fresh(start),lambda es:any(e['event']=='text_commit' for e in es),seconds=5)
        assert sum(e['event']=='text_commit' for e in fresh(before))==1
        result['cases'].append({'label':'keyboard-owned-contact-canceled','passed':True,'no_stale_key_commit':True})
        result['fresh_key_after_remap']=True
        start=remap_while_held('protected-interruption',0)
        request(control,'lock')
        protected=state(prefix+'-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        await_value(lambda:fresh(start),lambda es:any(e['event']=='touch_cancel' for e in es),seconds=5)
        screen.up(slot=0);authenticate(prefix+'-actual-pam')
        result['cases'].append({'label':'protected-interruption','passed':True,'app_cancel_observed':True});result.pop('pending_case',None)
        state(prefix+'-after-pam',lambda s:s['view']=={'App':identifier} and s['keyboard_visible'])
        start=remap_while_held('after-pam-app-contact',0);release_app('after-pam-app-contact',0,start)
        request(control,'close',id=identifier);state(prefix+'-closed',lambda s:not s['cards']);identifier=None
        result['passed']=True
    finally:
        for slot in list(screen.active):screen.up(slot=slot)
        if keys is not None:keys.close()
        result['events']=events(report)
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
