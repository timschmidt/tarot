#!/usr/bin/env python3
"""Native touch volume/mute/default-output controls with independent libpulse readback."""
import argparse,hashlib,json,os,pwd,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'));p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    owner=args.socket.stat();assert owner.st_uid==10000;os.chown(args.output,owner.st_uid,owner.st_gid)
    account=pwd.getpwuid(owner.st_uid);groups=os.getgrouplist(account.pw_name,owner.st_gid)
    initial=await_value(lambda:request(args.socket,'status'),lambda s:s['audio']['available'] and len(s['audio']['outputs'])==1)
    assert Path(f'/proc/{initial["pid"]}').stat().st_uid==owner.st_uid
    env=dict(os.environ,HOME=account.pw_dir,XDG_RUNTIME_DIR=str(args.socket.parent),WAYLAND_DISPLAY=initial['wayland_display'])
    env.pop('WAYLAND_SOCKET',None);env.pop('DISPLAY',None);env.pop('PULSE_SERVER',None)
    def pactl(*command):
        return subprocess.check_output(['pactl',*map(str,command)],env=env,text=True,user=owner.st_uid,group=owner.st_gid,extra_groups=groups,stderr=subprocess.STDOUT)
    original_default=pactl('get-default-sink').strip()
    def sink(name):return next(s for s in json.loads(pactl('-f','json','list','sinks')) if s['name']==name)
    original=sink(original_default);values=[v['value'] for v in original['volume'].values()];muted=original['mute']
    original_id=initial['audio']['outputs'][0]['id'];records={'initial':initial,'original_audio':{'values':values,'muted':muted,'default_matches':True,'default_hash':hashlib.sha256(original_default.encode()).hexdigest()}};screen=client=log=None;module=None;admin_recovery=False
    def status(label,condition=lambda s:True):
        state=await_value(lambda:request(args.socket,'status'),condition,seconds=10);assert state['pid']==initial['pid'];records[label]=state;return state
    def audio(label,condition):return status(label,lambda s:condition(s['audio']))['audio']
    def output(a,id=original_id):return next(o for o in a['outputs'] if o['id']==id)
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(args.socket,'status')['pid']==initial['pid']
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.15)
    def capture(name):
        request(args.socket,'capture',path=str(args.output/(name+'.png')));status('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
    try:
        screen=Touchscreen(guard);request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
        tap(310,14);status('menu-open',lambda s:s['system_menu_open']);time.sleep(.25)
        capture('menu-before-audio');tap(180,315);status('audio-page',lambda s:s['system_page']=='Audio');time.sleep(.25);capture('native-audio-page')
        before=output(initial['audio'])['percent'];tap(59,258)
        audio('touch-volume-down',lambda a:a['pending'] is None and output(a)['percent']==max(before-10,0))
        actual=sink(original_default);assert abs(max(v['value'] for v in actual['volume'].values())*100/65536-max(before-10,0))<=1
        tap(131,258);audio('touch-volume-up',lambda a:a['pending'] is None and output(a)['percent']==min(before,100))
        tap(225,258);audio('touch-mute',lambda a:a['pending'] is None and output(a)['muted']!=muted)
        assert sink(original_default)['mute']!=muted;capture('native-mute-confirmed')
        tap(225,258);audio('touch-mute-restored',lambda a:a['pending'] is None and output(a)['muted']==muted)
        # A second standard playback sink exercises actual default selection without
        # changing hardware/card profiles or touching other clients' stream routes.
        module=int(pactl('load-module','module-null-sink','sink_name=tarot_audio_qa','sink_properties=device.description=Audio_test_output').strip())
        second=audio('hotplug-output',lambda a:len(a['outputs'])==2);second_id=next(o['id'] for o in second['outputs'] if o['id']!=original_id)
        time.sleep(.3);tap(170,426)
        audio('touch-default-selected',lambda a:a['pending'] is None and output(a,second_id)['default'])
        assert pactl('get-default-sink').strip()=='tarot_audio_qa';capture('native-default-selected')
        # Sorting puts the new default first; the original sink's same second-row
        # button now restores the actual speaker as default.
        tap(170,426);audio('touch-default-restored',lambda a:a['pending'] is None and output(a)['default'])
        assert pactl('get-default-sink').strip()==original_default
        pactl('unload-module',module);module=None;audio('hotplug-removed',lambda a:len(a['outputs'])==1)
        pactl('set-sink-volume',original_default,'0%')
        zero=audio('zero-volume-observed',lambda a:output(a)['percent']==0)
        time.sleep(.25);tap(59,258)
        audio('touch-zero-already-current',lambda a:a['generation']>zero['generation'] and a['pending'] is None and output(a)['percent']==0)
        # External changes must arrive through subscriptions even with the menu shut.
        tap(310,14);status('menu-closed',lambda s:not s['system_menu_open'])
        pactl('set-sink-volume',original_default,'20%');audio('external-volume',lambda a:output(a)['percent']==20)
        # Known raw volume values restore exact channel balance, not a rounded percent.
        pactl('set-sink-volume',original_default,*values);pactl('set-sink-mute',original_default,int(muted))
        audio('original-audio-restored',lambda a:output(a)['percent']==before and output(a)['muted']==muted)
        actual=sink(original_default);assert [v['value'] for v in actual['volume'].values()]==values
        # A native text field proves the audio modal cannot type through to the app.
        commands=args.output/'text-commands.jsonl';events_file=args.output/'text-events.jsonl';commands.write_text('');os.chown(commands,owner.st_uid,owner.st_gid)
        log=(args.output/'client.log').open('w');client=subprocess.Popen(['/home/user/tarot-dev/bin/text-probe','--role','app','--report',str(events_file),'--commands',str(commands)],env=env,user=owner.st_uid,group=owner.st_gid,extra_groups=groups,stdin=subprocess.DEVNULL,stdout=log,stderr=log,cwd=account.pw_dir)
        opened=status('field-opened',lambda s:s['keyboard_visible']);text_view=opened['view'];time.sleep(.3)
        def events():return [json.loads(line) for line in events_file.read_bytes().split(b'\n')[:-1] if line] if events_file.exists() else []
        tap(21,504);await_value(events,lambda es:any(e['event']=='applied' and e['text']=='hello q' for e in es))
        tap(310,14);status('audio-hides-keyboard',lambda s:s['system_menu_open'] and not s['keyboard_visible'] and s['system_page']=='Audio');time.sleep(.25)
        start=len(events());tap(21,504);time.sleep(.15);assert not any(e['event']=='applied' for e in events()[start:])
        status('same-field-restored',lambda s:not s['system_menu_open'] and s['keyboard_visible'] and s['view']==text_view);time.sleep(.3)
        tap(21,504);await_value(events,lambda es:any(e['event']=='applied' and e['text']=='hello qq' for e in es));capture('field-after-audio-menu')
        records['validation']={'passed':True,'real_touch_controls':True,'independent_server_readback':True,'exact_original_audio_restored':True,'default_selection_on_temporary_null_sink':True,'acoustic_output_verified':False}
        print('PASS: native volume/mute touches, real server readback, default selection/hotplug, external updates, exact restore and same-field typing')
    finally:
        if screen is not None:screen.close()
        if client is not None and client.poll() is None:
            client.terminate()
            try:client.wait(timeout=3)
            except subprocess.TimeoutExpired:client.kill();client.wait()
        if log is not None:log.close()
        # Always recover the pre-test server state; note any failure of the UI path.
        if pactl('get-default-sink').strip()!=original_default:admin_recovery=True;pactl('set-default-sink',original_default)
        if module is not None:admin_recovery=True;pactl('unload-module',module)
        pactl('set-sink-volume',original_default,*values);pactl('set-sink-mute',original_default,int(muted))
        actual=sink(original_default);assert [v['value'] for v in actual['volume'].values()]==values and actual['mute']==muted
        request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
        records['admin_route_recovery_used']=admin_recovery
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
