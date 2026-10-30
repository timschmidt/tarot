"""Native BlueZ controls under the real ordinary-user Tarot login.

Does not pair, connect, trust, forget or select any nearby peripheral. The owner
must identify a test peer before physical pairing acceptance is attempted.
"""
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from PIL import Image
from dialog_button import locate
from phone_gesture_replay import request
from lock_replay import any_request


def readback():
    account=pwd.getpwnam('user')
    code='''import json,gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
b=Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
r=b.call_sync('org.bluez','/','org.freedesktop.DBus.ObjectManager','GetManagedObjects',None,None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
a=[v['org.bluez.Adapter1'] for v in r.values() if 'org.bluez.Adapter1' in v]
assert len(a)==1
print(json.dumps({k:a[0].get(k) for k in ['Powered','Discovering','Pairable','Discoverable','PairableTimeout','DiscoverableTimeout']}))
'''
    output=subprocess.check_output(['/usr/bin/python3','-c',code],user=account.pw_uid,group=account.pw_gid,extra_groups=os.getgrouplist(account.pw_name,account.pw_gid),env={'PATH':'/usr/bin:/bin','HOME':account.pw_dir},text=True,timeout=8)
    return json.loads(output)


def replay(socket,tap,state,capture,before,directory,authenticate):
    r={'passed':False,'real_radio':True,'physical_peer_tested':False,'pairing_tested':False,
       'audio_profiles_tested':False,'readback_uid':pwd.getpwnam('user').pw_uid,'targets':[]}
    original=readback();r['original']=original
    assert original['Powered'] and not original['Discovering'] and not original['Discoverable'],'Requires the recorded idle development adapter baseline'
    def observed(label,predicate):
        s=state('bluetooth-'+label,lambda s:'bluetooth' in s and predicate(s['bluetooth']))
        assert s['output']['orientation']=='normal' and s['cards']==before['cards']
        return s['bluetooth']
    def word(label,text):
        name='bluetooth-'+label;time.sleep(.25);capture(name);path=directory/(name+'.png')
        with Image.open(path) as image:assert image.size==(720,1440)
        found=locate(path,text,vertical=(.04,.94),scale=1);r['targets'].append({'label':label,'target':found});tap(found['point'][0]/2,found['point'][1]/2)
    def wait_readback(label,expected):
        from notification_replay import await_value
        result=await_value(readback,lambda v:all(v[k]==value for k,value in expected.items()),seconds=12);r[label]=result;return result
    try:
        observed('ready',lambda b:b['available'] and b['agent_available'] and b['default_agent'] and len(b['adapters'])==1)
        request(socket,'system_menu',open=False);tap(310,14);state('bluetooth-device-menu',lambda s:s['system_menu_open']);word('device-menu','Bluetooth');state('bluetooth-page',lambda s:s['system_page']=='Bluetooth')
        word('radio-off','enabled');observed('radio-off',lambda b:not b['adapters'][0]['powered']);wait_readback('off-readback',{'Powered':False})
        word('radio-on','enabled');observed('radio-on',lambda b:b['adapters'][0]['powered']);wait_readback('on-readback',{'Powered':True})
        word('find','Find');observed('searching',lambda b:b['scanning']);wait_readback('discovery-readback',{'Discovering':True})
        word('stop','Stop');observed('search-stopped',lambda b:not b['scanning']);wait_readback('discovery-restored',{'Discovering':False})
        word('incoming','Allow');observed('incoming',lambda b:b['visible']);wait_readback('incoming-readback',{'Discoverable':True,'Pairable':True,'DiscoverableTimeout':120,'PairableTimeout':120})
        # Protection cancels the page-scoped visibility allowance in the real PAM session.
        request(socket,'lock');locked=state('bluetooth-protected',lambda s:s['lock']['phase']=='Locked');assert set(locked)=={'ok','pid','lock'}
        assert not any_request(socket,'bluetooth_action',action={'Radio':{'adapter':1,'enabled':False}})['ok']
        wait_readback('lock-restores-adapter',original)
        authenticate('bluetooth-pam');observed('after-pam',lambda b:b['available'] and b['agent_available'] and not b['visible'] and not b['scanning'])
        assert request(socket,'status')['cards']==before['cards'] and request(socket,'status')['view']==before['view']
        r['after']=readback();assert r['after']==original;r['passed']=True;r['actual_pam']=True;r['protected_commands_denied']=True
        return r
    finally:
        value=request(socket,'status')
        if value['lock']['phase']=='Unlocked':
            current=readback()
            b=value['bluetooth']
            if current!=original or b['scanning'] or b['visible']:
                from notification_replay import await_value
                r['cleanup_controls_needed']=True
                request(socket,'system_menu',open=True)
                request(socket,'system_page',page='Bluetooth')
                adapter=b['selected_adapter']
                for kind in ['Scan','Visible']:
                    request(socket,'bluetooth_action',action={kind:{'adapter':adapter,'enabled':False}})
                await_value(lambda:request(socket,'status'),lambda s:not s['bluetooth']['scanning'] and not s['bluetooth']['visible'])
                if readback()['Powered']!=original['Powered']:
                    request(socket,'bluetooth_action',action={'Radio':{'adapter':adapter,'enabled':original['Powered']}})
                    await_value(readback,lambda v:v['Powered']==original['Powered'],seconds=12)
            request(socket,'system_menu',open=False)
        r['final_readback']=readback();r['adapter_restored']=r['final_readback']==original
        (directory/'bluetooth-result.json').write_text(json.dumps(r,indent=2)+'\n')
