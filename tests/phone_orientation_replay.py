#!/usr/bin/env python3
"""Installed PinePhone Pro rotation/input/keyboard replay with real PAM.

Prepare temporary desktop entries before logging into diagnostic Tarot. Fixtures
then launch through its actual catalog and inherit the authenticated PAM cgroup.
Only synthetic fixture text/pixels are captured. Physical sensor observations are
optional and separate; host pixel review is required to complete the result.
"""
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import signal
import subprocess
import time

from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import Touchscreen, request
from phone_stock_login import processes
from phone_touch_keyboard import Keyboard

BASE = Path('/home/user/tarot-dev')
CONTROL = Path('/run/user/10000/tarot-session/control.sock')
ENTRIES = {'input':'org.tarot.OrientationInput.desktop', 'text':'org.tarot.OrientationText.desktop'}


def owned(path, data):
    path.write_text(data)
    os.chmod(path, 0o600)
    os.chown(path, 10000, pwd.getpwnam('user').pw_gid)


def prepare(out):
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chown(out, 10000, pwd.getpwnam('user').pw_gid)
    apps = Path('/home/user/.local/share/applications')
    subprocess.run(["mkdir","-p",str(apps)], user=10000, group=pwd.getpwnam("user").pw_gid, check=True)
    assert apps.stat().st_uid == 10000
    manifest = {'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(), 'entries':{}}
    assert all(not (apps/name).exists() for name in ENTRIES.values())
    owned(out/'text.commands', '')
    try:
        for kind, name in ENTRIES.items():
            exe = BASE/'bin'/('input-probe' if kind=='input' else 'text-probe')
            assert exe.is_file()
            command = f'{exe} --report {out}/{kind}.jsonl --exit-after-seconds 600'
            if kind=='text':
                command += f' --role app --commands {out}/text.commands'
            content = f'[Desktop Entry]\nType=Application\nName=Tarot QA {kind} fixture\nExec={command}\nTryExec={exe}\n'
            path = apps/name
            owned(path, content)
            manifest['entries'][str(path)] = hashlib.sha256(content.encode()).hexdigest()
        owned(out/'prepared.json', json.dumps(manifest, indent=2)+'\n')
    except BaseException:
        for path in manifest['entries']:
            Path(path).unlink()
        raise
    print('Prepared two temporary catalog entries; log into diagnostic Tarot before replay')


def run(args, password):
    out = args.output
    prepared = json.loads((out/'prepared.json').read_text())
    assert prepared['boot_id'] == Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    initial = request(CONTROL, 'status')
    pid = initial['pid']
    proc = Path('/proc', str(pid))
    assert proc.stat().st_uid==10000 and (proc/'exe').resolve()==Path('/usr/bin/tarot')
    identity = (proc/'stat').read_text().rsplit(')',1)[1].split()[19]
    cgroup = (proc/'cgroup').read_text()
    record = {'pid':pid, 'boot_id':prepared['boot_id'], 'compositor_cgroup':cgroup, 'orientations':[], 'keyboard':[],
              'binaries_sha256':{n:hashlib.sha256(Path('/usr/bin',n).read_bytes()).hexdigest() for n in ['tarot','tarot-session','tarot-lock','tarot-keyboard']},
              'replay_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    screen = None
    typing = False

    def save():
        owned(out/'running.json', json.dumps(record, indent=2)+'\n')

    def alive():
        try:
            return (proc/'exe').resolve()==Path('/usr/bin/tarot') and (proc/'stat').read_text().rsplit(')',1)[1].split()[19]==identity
        except (FileNotFoundError, ProcessLookupError):
            return False

    def state(label, condition=lambda s:True):
        value = await_value(lambda:request(CONTROL,'status'),condition,seconds=25)
        assert value['pid']==pid
        record[label]=value
        save()
        return value

    def guard():
        assert alive() and Path('/sys/class/tty/tty0/active').read_text().strip()=='tty7'
        value=request(CONTROL,'status')
        assert value['pid']==pid
        if typing:
            assert value['lock']['phase']=='Locked' and value['lock']['output']['orientation']=='normal'

    def tap(x,y):
        screen.down(x,y)
        screen.up()
        time.sleep(.08)

    def coordinates(point, geometry):
        scale=geometry['scale']
        w,h=geometry['physical_width']/scale,geometry['physical_height']/scale
        lw,lh=(h,w) if geometry['orientation'] in ['left-up','right-up'] else (w,h)
        x,y=point[0]/lw,point[1]/lh
        x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[geometry['orientation']]
        return x*w,y*h

    def rotate(orientation):
        request(CONTROL,'rotate',orientation=orientation)
        return state('pose-'+orientation,lambda s:s['output']['orientation']==orientation and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'])

    def capture(label):
        request(CONTROL,'capture',path=str(out/(label+'.png')))
        return state(label,lambda s:not s['capture_pending'] and not s['capture_error'])

    def fixture_in_session(executable):
        found=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                if p.stat().st_uid==10000 and (p/'exe').resolve()==executable:
                    found.append({'pid':int(p.name),'cgroup':(p/'cgroup').read_text()})
            except (FileNotFoundError, ProcessLookupError):pass
        assert found and all(p['cgroup']==cgroup for p in found)
        return found

    try:
        ready=state('protected-start',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['output']['orientation']=='normal')
        assert ready['lock']['first_presented_protected'] and ready['lock']['unprotected_presentations']==0
        screen=Touchscreen(guard)
        if ready['lock']['idle']['output_off']:
            tap(180,350)
            state('panel-awake',lambda s:not s['lock']['idle']['output_off'])
        typing=True
        tap(180,608)
        state('private-entry-ready',lambda s:s['lock']['presented_frames']>ready['lock']['presented_frames'])
        Keyboard(lambda x,y:tap(x,y+28)).type_ascii(password)
        typing=False
        tap(180,330)
        unlocked=state('authenticated',lambda s:s['lock']['phase']=='Unlocked' and s['lock']['unprotected_presentations']>0)
        assert unlocked['output']['physical_width']==720 and unlocked['output']['physical_height']==1440 and unlocked['output']['scale']==2
        record['initial_sensor']=unlocked['sensors']
        request(CONTROL,'rotation_lock',locked=True)
        state('rotation-frozen',lambda s:s['rotation_locked'] and not s['sensors']['claimed'])
        request(CONTROL,'launch',id=ENTRIES['input'])
        state('input-mapped',lambda s:len(s['cards'])==1)
        record['input_processes']=fixture_in_session(BASE/'bin/input-probe')
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            value=rotate(orientation)
            time.sleep(.25)
            value=state('input-pose-'+orientation)
            rect=value['presentation']['windows'][0]['rect']
            factor=min(rect['width']/600,rect['height']/1000)
            samples=[]
            for cx,cy in [(423,437),(383,887),(563,677)]:
                point=(rect['x']+(rect['width']-600*factor)/2+(cx-20)*factor,rect['y']+(rect['height']-1000*factor)/2+(cy-30)*factor)
                px,py=coordinates(point,value['output'])
                start=len(events(out/'input.jsonl'))
                screen.down(px,py)
                received=await_value(lambda:events(out/'input.jsonl')[start:],lambda es:any(e['event']=='touch_down' for e in es))
                event=next(e for e in received if e['event']=='touch_down')
                screen.up()
                await_value(lambda:events(out/'input.jsonl')[start:],lambda es:any(e['event']=='touch_up' for e in es))
                assert event['target']=='root' and abs(event['x']-cx)<3 and abs(event['y']-cy)<3,(orientation,event,(cx,cy))
                samples.append({'physical':[px*2,py*2],'expected_client':[cx,cy],'received':event})
            label=f'input-{orientation}-{len(record["orientations"]):02}'
            capture(label)
            record['orientations'].append({'orientation':orientation,'geometry':value['output'],'samples':samples,'capture':label+'.png'})
            save()
        start=len(events(out/'input.jsonl'))
        screen.down(px,py)
        await_value(lambda:events(out/'input.jsonl')[start:],lambda es:any(e['event']=='touch_down' for e in es))
        rotate('left-up')
        await_value(lambda:events(out/'input.jsonl')[start:],lambda es:any(e['event']=='touch_cancel' for e in es))
        screen.up()
        assert not any(e['event']=='touch_up' for e in events(out/'input.jsonl')[start:])
        record['held_touch_canceled']=True
        value=request(CONTROL,'status')
        request(CONTROL,'close',id=value['cards'][0]['id'])
        state('input-closed',lambda s:not s['cards'])
        request(CONTROL,'launch',id=ENTRIES['text'])
        text=state('text-mapped',lambda s:len(s['cards'])==1 and s['keyboard_visible'])
        record['text_processes']=fixture_in_session(BASE/'bin/text-probe')
        helper=text['input_method']['pid']
        expected='hello '
        for orientation in ['normal','left-up','bottom-up','right-up','normal']:
            rotate(orientation)
            time.sleep(.25)
            value=state('keyboard-'+orientation,lambda s:s['keyboard_visible'])
            assert value['input_method']['pid']==helper
            rect=value['keyboard_rect']
            point=(rect['x']+rect['width']/20+3,rect['y']+rect['height']*.2+4)
            tap(*coordinates(point,value['output']))
            expected+='q'
            await_value(lambda:events(out/'text.jsonl'),lambda es:any(e['event']=='applied' and e['text']==expected for e in es))
            capture('keyboard-'+orientation)
            record['keyboard'].append({'orientation':orientation,'rect':rect,'expected_text':expected})
        record['phase']='waiting-for-physical-sensor' if args.physical_seconds else 'software-complete'
        request(CONTROL,'rotation_lock',locked=False)
        sensor=state('sensor-claimed',lambda s:s['sensors']['available'] and s['sensors']['claimed'])
        seen=set()
        record['physical_observations']=[]
        save()
        deadline=time.monotonic()+args.physical_seconds
        while time.monotonic()<deadline:
            value=request(CONTROL,'status')
            if value['lock']['phase']!='Unlocked':break
            sensor=value['sensors']
            pose=sensor['orientation']
            if sensor['claimed'] and pose and pose==value['output']['orientation'] and value['output']['presented_revision']==value['output']['revision']:
                if pose not in seen:
                    seen.add(pose)
                    record['physical_observations'].append({'orientation':pose,'output':value['output'],'sensor':sensor,'monotonic':time.monotonic()})
                    save()
                if pose=='normal' and {'normal','left-up','right-up'}<=seen:
                    record['physical_completion']={'orientation':pose,'output':value['output'],'sensor':sensor,'monotonic':time.monotonic()}
                    break
            time.sleep(.2)
        record['physical_sensor_completed']='physical_completion' in record
        record['phase']='ending'
        screen.close()
        screen=None
        request(CONTROL,'exit')
        await_value(processes,lambda ps:not alive() and any(p['uid']==109 and p['name']=='phrog' for p in ps),seconds=35)
        assert not CONTROL.exists()
        record['validation']={'native_replay_passed':True,'all_orientations_touch_and_keyboard':True,'actual_pam':True,'fixtures_in_login_cgroup':True,'returned_to_phrog':True,'pixel_review_complete':False,'physical_sensor_completed':record['physical_sensor_completed']}
        print('Native rotation replay complete; host pixel review required',flush=True)
    except BaseException as error:
        record['validation']={'native_replay_passed':False,'exception_type':type(error).__name__}
        raise
    finally:
        password=None
        if screen:screen.close()
        if alive():os.kill(pid,signal.SIGTERM)
        removed=[]
        for name,digest in prepared['entries'].items():
            path=Path(name)
            if path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest()==digest:
                path.unlink()
                removed.append(name)
        record['removed_temporary_entries']=removed
        owned(out/'status.json',json.dumps(record,indent=2)+'\n')
        (out/'running.json').unlink(missing_ok=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--prepare',action='store_true')
    parser.add_argument('--detach',action='store_true')
    parser.add_argument('--physical-seconds',type=int,default=0)
    args=parser.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output=args.output.resolve()
    assert args.output.is_relative_to(BASE/'artifacts') and re.fullmatch(r'[A-Za-z0-9_./-]+',str(args.output))
    assert 0<=args.physical_seconds<=120
    if args.prepare:
        prepare(args.output)
        return
    password=getpass.getpass('Device password for native rotation PAM replay: ')
    assert password and all(' '<=c<='~' for c in password)
    signal.signal(signal.SIGHUP,signal.SIG_IGN)
    if args.detach:
        child=os.fork()
        if child:
            password=None
            print('Detached native rotation replay PID',child,flush=True)
            return
        os.setsid()
        null=os.open('/dev/null',os.O_RDONLY)
        os.dup2(null,0)
        os.close(null)
        log=os.open(args.output/'controller.log',os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600)
        os.fchown(log,10000,pwd.getpwnam('user').pw_gid)
        os.dup2(log,1)
        os.dup2(log,2)
        os.close(log)
    run(args,password)


if __name__=='__main__':main()
