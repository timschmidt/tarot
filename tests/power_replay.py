#!/usr/bin/env python3
"""Actual D-Bus power/control regression on an isolated bus and fake backlight."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    output=args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix='tarot-power-evidence-'))
    output.mkdir(parents=True,exist_ok=True)
    runtime=tempfile.TemporaryDirectory(prefix='tarot-power-runtime-')
    control=Path(runtime.name)/'control.sock'
    address='unix:path='+str(Path(runtime.name)/'bus')
    env=dict(os.environ,DBUS_SESSION_BUS_ADDRESS=address,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    root=Path.cwd()
    backlights=output/'backlights'
    panel=backlights/'panel'
    panel.mkdir(parents=True,exist_ok=False)
    for name,value in [('max_brightness',51),('brightness',15),('actual_brightness',15)]: (panel/name).write_text(str(value))
    bus=client=fixture=None
    records={}
    logs=[]
    def log(name):
        f=(output/name).open('w');logs.append(f);return f
    def start_bus(name):
        p=subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address,'--print-address=1'],stdout=subprocess.PIPE,stderr=log(name),text=True)
        assert p.stdout.readline().startswith(address)
        return p
    def status(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=8)
        records[label]=value
        return value
    def power(label,condition): return status(label,lambda s:condition(s['power']))
    def action(percent): request(control,'power_action',action={'Brightness':{'percent':percent}})
    def capture(name):
        request(control,'capture',path=str(output/(name+'.png')))
        status('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
    def new_fixture(name):
        return Fixture(root/'target/debug/examples/power-probe',output/name,env,extra_args=['--backlight-root',backlights])
    try:
        client=subprocess.Popen([str(root/'target/debug/tarot'),'--system-services','--platform-bus',address,'--backlight-root',str(backlights),'--control-socket',str(control),'--exit-after-ms','120000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or client.poll() is not None,bool)
        assert client.poll() is None
        power('initial-bus-absent',lambda p:not p['battery']['available'] and bool(p['battery']['error']))
        bus=start_bus('bus.log')
        fixture=new_fixture('first-service')
        initial=power('initial',lambda p:p['battery']['available'] and p['brightness']['can_set'])
        assert initial['power']['battery']['percent']==44 and initial['power']['battery']['state']=='Discharging' and initial['power']['battery']['external_power']
        assert initial['power']['brightness']['actual']==15
        request(control,'system_menu',open=True)
        time.sleep(.25)
        capture('initial-menu')
        fixture.call('set',percent=81,state=1,on_battery=False)
        power('charging-signal',lambda p:p['battery']['percent']==81 and p['battery']['state']=='Charging')
        fixture.call('set',level=3)
        power('coarse-signal',lambda p:p['battery']['coarse_level']=='Low')
        fixture.call('set',level=1,percent=101)
        power('invalid-percentage',lambda p:p['battery']['percent'] is None)
        fixture.call('set',percent=44,state=2,on_battery=True)
        power('discharging',lambda p:p['battery']['percent']==44 and p['battery']['state']=='Discharging' and p['battery']['external_power'] is False)
        fixture.call('set',deny=True)
        action(40)
        denied=power('denied',lambda p:p['brightness']['pending_percent'] is None and 'denied' in (p['brightness']['action_error'] or '').lower())
        assert denied['power']['brightness']['actual']==15 and (panel/'actual_brightness').read_text()=='15'
        time.sleep(1.1)
        assert status('denial-retained')['power']['brightness']['action_error']
        capture('denied-menu')
        fixture.call('set',deny=False,delay=700)
        action(40)
        pending=power('pending',lambda p:p['brightness']['pending_percent']==40)
        assert pending['power']['brightness']['actual']==15
        done=power('confirmed',lambda p:p['brightness']['pending_percent'] is None and p['brightness']['actual']==20)
        assert done['power']['brightness']['action_error'] is None
        fixture.call('set',confirm=False,delay=0)
        action(60)
        failed=power('readback-failed',lambda p:p['brightness']['pending_percent'] is None and 'confirm' in (p['brightness']['action_error'] or '').lower())
        assert failed['power']['brightness']['actual']==20 and failed['power']['brightness']['requested']==31
        fixture.call('set',confirm=True)
        action(60)
        power('readback-recovered',lambda p:p['brightness']['pending_percent'] is None and p['brightness']['actual']==31 and p['brightness']['action_error'] is None)
        fixture.call('set',remote=True)
        power('remote-disabled',lambda p:not p['brightness']['can_set'])
        fixture.call('set',remote=False,active=False)
        power('inactive-disabled',lambda p:not p['brightness']['can_set'])
        fixture.call('set',active=True)
        power('session-recovered',lambda p:p['brightness']['can_set'])
        fixture.call('release')
        power('upower-lost',lambda p:not p['battery']['available'] and p['brightness']['can_set'])
        fixture.call('acquire')
        power('upower-returned',lambda p:p['battery']['available'])
        fixture.close();fixture=None
        power('services-lost',lambda p:not p['battery']['available'] and not p['brightness']['can_set'])
        bus.terminate();bus.wait(timeout=4)
        power('bus-lost',lambda p:not p['battery']['available'] and 'system bus' in (p['battery']['error'] or ''))
        action(40)
        power('offline-action',lambda p:p['brightness']['pending_percent'] is None and 'unavailable' in (p['brightness']['action_error'] or ''))
        bus=start_bus('restarted-bus.log')
        fixture=new_fixture('second-service')
        power('bus-recovered',lambda p:p['battery']['available'] and p['brightness']['can_set'])
        action(40)
        power('action-after-reconnect',lambda p:p['brightness']['actual']==20 and p['brightness']['pending_percent'] is None)
        (panel/'max_brightness').write_text('0')
        power('invalid-backlight-range',lambda p:not p['brightness']['can_set'] and 'range' in (p['brightness']['error'] or '').lower())
        (panel/'max_brightness').write_text('51')
        power('backlight-range-recovered',lambda p:p['brightness']['can_set'])
        request(control,'system_menu',open=False)
        assert status('view-preserved')['view']==initial['view']
        fixture.close();fixture=None
        request(control,'exit');code=client.wait(timeout=5);assert code==0
        records['validation']={'passed':True,'compositor_exit':code}
        print('PASS: live battery signals, coarse/invalid data, pending/denied/readback controls, session authorization, service and bus recovery',output)
    finally:
        if fixture is not None:fixture.close()
        for p in [client,bus]:
            if p is not None and p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for f in logs:f.close()
        runtime.cleanup()
        (output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
