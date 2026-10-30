"""Actual installed keyboard layout/input after consecutive text-input enables."""
import hashlib
import json
import os
from pathlib import Path
import stat
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(control,tap,state,capture,before,out,cgroup,authenticate):
    folder=out/'text-enable-reset';report=folder/'events.jsonl';commands=folder/'commands.jsonl'
    audit=json.loads((folder/'fixture-audit.json').read_text())
    result={'passed':False,'physical_finger_tested':False,'cases':[],
            'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    def observe(label,condition=lambda s:True):return state('text-reset-'+label,condition)
    def send(command,**fields):
        fd=os.open(commands,os.O_WRONLY|os.O_APPEND|os.O_CLOEXEC|os.O_NOFOLLOW)
        try:
            info=os.fstat(fd);assert stat.S_ISREG(info.st_mode) and info.st_uid==10000 and stat.S_IMODE(info.st_mode)==0o600
            payload=(json.dumps(dict(command=command,**fields))+'\n').encode();assert os.write(fd,payload)==len(payload)
        finally:os.close(fd)
    def layout(label,height):
        return observe(label,lambda s:s['keyboard_visible'] and s['keyboard_owner']==owner and s['keyboard_rect']['height']==height and s['output']['orientation']=='normal' and s['output']['presented_revision']==s['output']['revision'])
    try:
        request(control,'launch',id='test.tarot.TextInputReset.desktop')
        mapped=observe('mapped',lambda s:len(s['cards'])==len(before['cards'])+1 and s['keyboard_visible'])
        owner=next(c['id'] for c in mapped['cards'] if c['id'] not in [v['id'] for v in before['cards']])
        candidates=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                if str(report).encode() not in (p/'cmdline').read_bytes().split(b'\0'):continue
                assert p.stat().st_uid==10000 and (p/'cgroup').read_text()==cgroup
                actual=hashlib.sha256((p/'exe').read_bytes()).hexdigest();assert actual==audit['binary_sha256']
                candidates.append({'pid':int(p.name),'uid':10000,'start':(p/'stat').read_text().rsplit(')',1)[1].split()[19],'sha256':actual,'cgroup':cgroup})
            except (FileNotFoundError,ProcessLookupError):pass
        assert len(candidates)==1;result['fixture']=candidates[0];layout('initial-terminal',288)
        for index,replacement in enumerate([False,True]):
            if index:send('purpose',value='terminal');layout('terminal-again',288)
            start=len(events(report));send('enable-reset',secure=False,replacement=replacement)
            await_value(lambda:events(report)[start:],lambda es:any(e['event']=='enable-reset-synced' for e in es))
            layout('normal-'+str(index),240);capture('text-reset-normal-'+str(index))
            Keyboard(tap).type_ascii('q')
            expected='hello '+'q'*(index+1)
            await_value(lambda:events(report)[start:],lambda es:any(e['event']=='applied' and e['text']==expected and e['preedit']=='' for e in es))
            result['cases'].append({'replacement':replacement,'passed':True,'height_before':288,'height_after':240,'text':expected})
        request(control,'lock');protected=observe('protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True)
        assert set(protected)=={'ok','pid','lock'};authenticate('text-reset-pam')
        layout('after-pam',240);Keyboard(tap).type_ascii('q')
        await_value(lambda:events(report),lambda es:any(e['event']=='applied' and e['text']=='hello qqq' for e in es))
        request(control,'close',id=owner);observe('fixture-closed',lambda s:s['cards']==before['cards'] and not s['keyboard_visible'])
        await_value(lambda:Path('/proc',str(candidates[0]['pid'])).exists(),lambda live:not live,seconds=8)
        request(control,'activate',id=before['cards'][0]['id']);observe('original-restored',lambda s:s['cards']==before['cards'] and s['view']==before['view'])
        result.update(passed=True,actual_pam=True,cooperative_close=True,process_gone=True,original_restored=True)
    finally:
        result['events']=events(report);(folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
