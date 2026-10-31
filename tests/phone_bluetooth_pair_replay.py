"""Real Bluetooth pairing by touch against an owner-provided test peer.

The peer (a host USB adapter running tests/bluetooth_peer.py) accepts numeric
comparison and records its code; this replay reads Tarot's displayed code by
OCR so the two can be compared afterwards. Trust, connect/disconnect and
two-step Forget follow, each read back independently from BlueZ.
"""
import csv
import io
import json
import os
import pwd
import re
import subprocess
import time
from pathlib import Path
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request

READ = '''import json,sys,gi
gi.require_version('Gio','2.0')
from gi.repository import Gio
b=Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
r=b.call_sync('org.bluez','/','org.freedesktop.DBus.ObjectManager','GetManagedObjects',None,None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
a=[v['org.bluez.Adapter1'] for v in r.values() if 'org.bluez.Adapter1' in v]
d=[v['org.bluez.Device1'] for v in r.values() if 'org.bluez.Device1' in v and v['org.bluez.Device1'].get('Address')==sys.argv[1]]
print(json.dumps(dict(adapter={k:a[0].get(k) for k in ['Powered','Discovering','Pairable','Discoverable']},
 device=None if not d else {k:d[0].get(k) for k in ['Alias','Paired','Bonded','Trusted','Connected','Blocked']})))
'''


def readback(peer):
    account = pwd.getpwnam('user')
    output = subprocess.check_output(['/usr/bin/python3', '-c', READ, peer], user=account.pw_uid, group=account.pw_gid,
                                     extra_groups=os.getgrouplist(account.pw_name, account.pw_gid),
                                     env={'PATH': '/usr/bin:/bin', 'HOME': account.pw_dir}, text=True, timeout=8)
    return json.loads(output)


def code_in(path):
    """Six-digit numeric comparison code as rendered (large, bold)."""
    out = subprocess.run(['tesseract', str(path), 'stdout', '--psm', '11', 'tsv'], capture_output=True, text=True,
                         check=True, timeout=30).stdout
    found = [row['text'] for row in csv.DictReader(io.StringIO(out), delimiter='\t')
             if re.fullmatch(r'\d{6}', row['text'].strip() or '') and float(row['conf']) >= 60]
    return found


def replay(socket, tap, state, capture, before, directory, peer, screen):
    r = dict(passed=False, peer_address=peer, real_peer=True, steps=[], targets=[])
    original = readback(peer)['adapter']
    r['original_adapter'] = original
    assert readback(peer)['device'] is None or not readback(peer)['device']['Paired'], 'Start unpaired'

    def bt(label, predicate, seconds=20):
        s = state('bt-' + label, lambda s: predicate(s['bluetooth']), seconds=seconds)
        return s['bluetooth']

    def word(label, text, scroll=True, vertical=(.06, .94)):
        last = None
        for attempt in range(8 if scroll else 1):
            time.sleep(.4)
            name = 'bt-' + label + '-' + str(attempt)
            capture(name)
            try:
                found = locate(directory / (name + '.png'), text, vertical=vertical, scale=1)
                r['targets'].append(dict(label=label, word=text, point=found['point']))
                tap(found['point'][0] / 2, found['point'][1] / 2)
                return name
            except AssertionError as error:
                last = error
            if scroll:
                screen.down(180, 560)
                for y in range(530, 260, -30):
                    screen.move(180, y, delay=.025)
                screen.up()
        raise last

    def device(label, predicate, seconds=60):
        value = await_value(lambda: readback(peer)['device'], lambda d: predicate(d), seconds=seconds)
        r['steps'].append(dict(step=label, device=value))
        return value

    try:
        bt('ready', lambda b: b['available'] and b['agent_available'] and b['default_agent'])
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Bluetooth')
        state('bt-page', lambda s: s['system_page'] == 'Bluetooth')
        word('find', 'Find', scroll=False)
        bt('searching', lambda b: b['scanning'])
        device('discovered', lambda d: d is not None, seconds=60)
        word('select-peer', 'TarotPeer')
        bt('selected', lambda b: b['selected_device'] is not None)
        word('pair', 'Pair')
        prompt = bt('prompt', lambda b: b['prompt'] is not None, seconds=40)['prompt']
        r['prompt_kind'] = prompt['kind']
        time.sleep(.5)
        capture('bt-code')
        r['tarot_codes_ocr'] = code_in(directory / 'bt-code.png')
        # The prompt is drawn first; never reach the incoming-pairing switch.
        word('allow', 'Allow', scroll=False, vertical=(.06, .55))
        assert not request(socket, 'status')['bluetooth']['visible']
        device('paired', lambda d: d and d['Paired'] and d['Bonded'])
        bt('paired-ui', lambda b: b['prompt'] is None and any(d['paired'] for d in b['devices']))
        word('trust', 'Trust')
        device('trusted', lambda d: d['Trusted'])
        word('connect', 'Connect')
        try:
            device('connected', lambda d: d['Connected'], seconds=60)
            r['connected'] = True
            word('disconnect', 'Disconnect')
            device('disconnected', lambda d: not d['Connected'], seconds=60)
        except AssertionError:
            r['connected'] = False
            r['connect_error'] = request(socket, 'status')['bluetooth']['action_error']
        word('forget', 'Forget')
        bt('forget-confirm', lambda b: b['pending'] is None)
        word('forget-confirmed', 'Forget', scroll=False)
        device('forgotten', lambda d: d is None or not d['Paired'], seconds=30)
        r['passed'] = True
        return r
    finally:
        value = request(socket, 'status')
        if value['lock']['phase'] == 'Unlocked':
            b = value['bluetooth']
            adapter = b['selected_adapter']
            if b['scanning']:
                request(socket, 'bluetooth_action', action={'Scan': {'adapter': adapter, 'enabled': False}})
                await_value(lambda: request(socket, 'status'), lambda s: not s['bluetooth']['scanning'])
            request(socket, 'system_menu', open=False)
        final = readback(peer)
        r['final_adapter'] = final['adapter']
        r['final_device'] = final['device']
        r['adapter_restored'] = final['adapter'] == original
        (directory / 'bluetooth-pair-result.json').write_text(json.dumps(r, indent=2) + '\n')
