#!/usr/bin/env python3
"""Real UPower status, local-session brightness taps and modal input on the phone."""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'))
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    owner=args.socket.stat();os.chown(args.output,owner.st_uid,owner.st_gid)
    initial=await_value(lambda:request(args.socket,'status'),lambda s:s['power']['battery']['available'] and s['power']['brightness']['can_set'])
    records={'initial':initial};screen=client=None;log=None
    original=initial['power']['brightness']['actual'];maximum=initial['power']['brightness']['maximum']
    restore_percent=round(original*100/maximum)
    def status(label,condition=lambda s:True):
        value=await_value(lambda:request(args.socket,'status'),condition,seconds=8)
        assert value['pid']==initial['pid'];records[label]=value;return value
    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8'
        assert request(args.socket,'status')['pid']==initial['pid']
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.15)
    def capture(name):
        request(args.socket,'capture',path=str(args.output/(name+'.png')))
        status('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
    def menu(opened):
        tap(310,14);status('menu-'+str(opened),lambda s:s['system_menu_open']==opened);time.sleep(.25)
    try:
        # Compare the adapter's reading to fresh calls on the actual system bus.
        import re
        result=subprocess.check_output(['gdbus','call','--system','--dest','org.freedesktop.UPower','--object-path','/org/freedesktop/UPower/devices/DisplayDevice','--method','org.freedesktop.DBus.Properties.Get','org.freedesktop.UPower.Device','Percentage'],text=True)
        reported=float(re.search(r'[-+]?\d+(?:\.\d+)?',result).group())
        assert abs(initial['power']['battery']['percent']-reported)<=1
        assert Path(f'/proc/{initial["pid"]}').stat().st_uid==owner.st_uid==10000
        assert initial['power']['brightness']['actual']==int(Path('/sys/class/backlight/backlight/actual_brightness').read_text())
        records['upower_check']={'percentage':reported}
        screen=Touchscreen(guard);menu(True);capture('live-system-menu')
        # The inspected logical positions match the unit-tested status menu.
        tap(124,220)
        status('forty-confirmed',lambda s:s['power']['brightness']['actual']==(maximum*40+50)//100 and s['power']['brightness']['pending_percent'] is None)
        assert int(Path('/sys/class/backlight/backlight/actual_brightness').read_text())==(maximum*40+50)//100
        capture('brightness-forty')
        tap(196,220)
        status('sixty-confirmed',lambda s:s['power']['brightness']['actual']==(maximum*60+50)//100 and s['power']['brightness']['pending_percent'] is None)
        assert int(Path('/sys/class/backlight/backlight/actual_brightness').read_text())==(maximum*60+50)//100
        capture('brightness-sixty')
        assert status('native-view-retained')['view']==initial['view']
        menu(False)
        # Reuse a real native text client to verify the menu cannot type into it.
        account=pwd.getpwuid(owner.st_uid);events_file=args.output/'text-events.jsonl';commands=args.output/'text-commands.jsonl'
        commands.write_text('');os.chown(commands,owner.st_uid,owner.st_gid)
        env=dict(os.environ,WAYLAND_DISPLAY=initial['wayland_display'],XDG_RUNTIME_DIR=str(args.socket.parent),HOME=account.pw_dir)
        env.pop('WAYLAND_SOCKET',None);env.pop('DISPLAY',None)
        log=(args.output/'client.log').open('w')
        client=subprocess.Popen(['/home/user/tarot-dev/bin/text-probe','--role','app','--report',str(events_file),'--commands',str(commands)],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,user=owner.st_uid,group=owner.st_gid,extra_groups=os.getgrouplist(account.pw_name,owner.st_gid),cwd=account.pw_dir)
        opened=status('field-opened',lambda s:s['keyboard_visible']);text_view=opened['view'];time.sleep(.3)
        def events():return [json.loads(line) for line in events_file.read_bytes().split(b'\n')[:-1] if line] if events_file.exists() else []
        tap(21,504)
        await_value(events,lambda es:any(e['event']=='applied' and e['text']=='hello q' for e in es))
        menu(True)
        assert not status('menu-hides-keyboard')['keyboard_visible']
        before=len(events());tap(21,504);time.sleep(.1)
        assert not any(e['event']=='applied' for e in events()[before:])
        # The scrim tap closes the menu, restoring the same field without typing.
        status('scrim-restores-field',lambda s:not s['system_menu_open'] and s['keyboard_visible'] and s['view']==text_view)
        time.sleep(.3);tap(21,504)
        await_value(events,lambda es:any(e['event']=='applied' and e['text']=='hello qq' for e in es))
        capture('field-after-system-menu')
        print('PASS: real UPower comparison, unprivileged 40/60 percent brightness with sysfs readback, modal focus and same-field typing')
    finally:
        if screen is not None:screen.close()
        if client is not None and client.poll() is None:
            client.terminate()
            try:client.wait(timeout=3)
            except subprocess.TimeoutExpired:client.kill();client.wait()
        if log is not None:log.close()
        request(args.socket,'system_menu',open=False)
        request(args.socket,'power_action',action={'Brightness':{'percent':restore_percent}})
        status('brightness-restored',lambda s:s['power']['brightness']['actual']==original and s['power']['brightness']['pending_percent'] is None)
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
