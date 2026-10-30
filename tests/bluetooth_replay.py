#!/usr/bin/env python3
"""BlueZ agent and rendered controls on an owned Xvfb and private D-Bus.

Uses synthetic device IDs/codes only; never connects to the system Bluetooth bus.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from PIL import Image
from dialog_button import locate
from hardware_replay import PrivateInput
from lock_replay import append,any_request
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from with_xvfb import read_display_number


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--xvfb',default='Xvfb')
    p.add_argument('--binary',type=Path,default=Path('target/debug/tarot'))
    a=p.parse_args();out=a.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();children=[];logs=[];mouse=None
    records={'passed':False,'phone_kms_tested':False,'real_radio_tested':False,'actions':[],
             'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest()}
    sources=out/'qa-sources';sources.mkdir();pending=[Path(__file__).resolve(),root/'tests/bluez_fixture.py'];hashes={}
    while pending:
        path=pending.pop()
        if path.name in hashes:continue
        payload=path.read_bytes();hashes[path.name]=hashlib.sha256(payload).hexdigest();(sources/path.name).write_bytes(payload)
        for node in ast.walk(ast.parse(payload)):
            modules=[n.name for n in node.names] if isinstance(node,ast.Import) else [node.module] if isinstance(node,ast.ImportFrom) and node.module else []
            for module in modules:
                candidate=root/'tests'/(module.split('.')[0]+'.py')
                if candidate.exists() and candidate.name not in hashes:pending.append(candidate)
    records['source_sha256']=hashes
    records['locker_probe_sha256']=hashlib.sha256((root/'target/debug/examples/lock-probe').read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix='tarot-bluez-') as temporary:
        work=Path(temporary);control=work/'control.sock';commands=work/'commands.jsonl';commands.write_text('');locker_commands=work/'locker.commands';locker_commands.write_text('');report=out/'fixture.jsonl';sequence=0
        (work/'empty').mkdir();(work/'config').mkdir();(work/'runtime').mkdir(mode=0o700)
        def log(name):
            stream=(out/name).open('x');logs.append(stream);return stream
        def events(name=None):
            es=[json.loads(line) for line in report.read_bytes().split(b'\n')[:-1]] if report.exists() else []
            return [e for e in es if name is None or e['event']==name]
        def command(name,**fields):
            nonlocal sequence
            sequence+=1
            with commands.open('a') as stream:stream.write(json.dumps(dict(command=name,request=sequence,**fields))+'\n')
            await_value(lambda:events('command'),lambda es:any(e['request']==sequence for e in es))
        def state(label,predicate=lambda s:True,seconds=10):
            value=await_value(lambda:request(control,'status'),predicate,seconds=seconds)
            records['actions'].append(dict(label=label,state=value))
            encoded=json.dumps(value)
            assert 'Fixture keyboard' not in encoded and 'Fixture adapter' not in encoded
            assert '000123' not in encoded and '004321' not in encoded
            return value
        def bt(label,predicate,seconds=10):return state(label,lambda s:predicate(s['bluetooth']),seconds)['bluetooth']
        def action(value):request(control,'bluetooth_action',action=value)
        def reply(identifier,answer):action({'Reply':{'request':identifier,'answer':answer}})
        def capture(label):
            time.sleep(.2);path=out/(label+'.png');request(control,'capture',path=str(path))
            state('capture-'+label,lambda s:not s['capture_pending'] and not s['capture_error'])
            return path
        def click(x,y):
            mouse.move(x,y);time.sleep(.04);mouse.button(True);time.sleep(.06);mouse.button(False);time.sleep(.12)
        def word(label,text,vertical=(.05,.94)):
            path=capture(label);found=locate(path,text,vertical=vertical,scale=2)
            records.setdefault('rendered_targets',[]).append(dict(label=label,target=found));click(*found['point'])
        def opened():
            request(control,'system_menu',open=True);request(control,'system_page',page='Bluetooth')
            state('page-open',lambda s:s['system_menu_open'] and s['system_page']=='Bluetooth')
        def idle(label):return bt(label,lambda b:b['prompt'] is None and not b['busy'])
        def pair(kind='Confirm',rendered=False):
            current=idle('before-pair');device=current['devices'][0]['id']
            if rendered:
                request(control,'bluetooth_select',device=device);word('pair-button-'+str(sequence),'Pair')
            else:action({'Pair':{'device':device}})
            prompt=bt('prompt-'+kind,lambda b:b['prompt'] is not None and b['prompt']['kind']==kind)['prompt']
            return device,prompt['id']
        try:
            read,write=os.pipe()
            try:
                xvfb=subprocess.Popen([a.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1000x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
            finally:
                os.close(read)
                if write is not None:os.close(write)
            address='unix:path='+str(work/'bus')
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+address],stdout=log('bus.log'),stderr=subprocess.STDOUT);children.append(bus)
            await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_DATA_HOME=str(work/'empty'),XDG_DATA_DIRS=str(work/'empty'),XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'),XDG_RUNTIME_DIR=str(work/'runtime'),DBUS_SESSION_BUS_ADDRESS=address)
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            fixture=subprocess.Popen(['/usr/bin/python3',str(root/'tests/bluez_fixture.py'),'--report',str(report),'--commands',str(commands)],env=env,stdout=log('fixture.log'),stderr=subprocess.STDOUT);children.append(fixture)
            await_value(lambda:events('ready'),bool)
            client=subprocess.Popen([str(a.binary.resolve()),'--width','360','--height','720','--reduced-motion','--system-services','--platform-bus',address,'--backlight-root',str(work/'empty'),'--control-socket',str(control),'--config-file',str(work/'settings.toml'),'--lock-probe-unlocked-start','--lock-client',str(root/'target/debug/examples/lock-probe'),'--lock-client-arg=--role','--lock-client-arg=locker','--lock-client-arg=--report','--lock-client-arg='+str(out/'locker.jsonl'),'--lock-client-arg=--commands','--lock-client-arg='+str(locker_commands),'--exit-after-ms','240000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT);children.append(client)
            await_value(lambda:control.exists() or client.poll() is not None,bool);assert client.poll() is None
            mouse=PrivateInput(client.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool);mouse.move(180,300)
            initial=bt('initial',lambda b:b['available'] and b['agent_available'] and b['default_agent'] and len(b['devices'])==1)
            # Closed-page healthy idle must not turn agent cleanup into a polling loop.
            command('summary');before=events('summary')[-1]['managed_calls'];time.sleep(1);command('summary');after=events('summary')[-1]['managed_calls'];assert after-before<=1,(before,after)
            records['closed_idle_managed_reads']=after-before
            click(315,14);word('root-menu','Bluetooth');state('rendered-page-open',lambda s:s['system_page']=='Bluetooth')
            adapter=initial['selected_adapter'];device=initial['devices'][0]['id']
            word('radio-disable','enabled');bt('radio-off',lambda b:not b['adapters'][0]['powered'])
            word('radio-enable','enabled');bt('radio-on',lambda b:b['adapters'][0]['powered'])
            command('foreign-scan');word('start-discovery','Find');bt('own-discovery',lambda b:b['scanning'])
            command('deny-stop',enabled=True);action({'Scan':{'adapter':adapter,'enabled':False}})
            bt('denied-stop-retains-ownership',lambda b:b['scanning'] and b['action_error'])
            command('deny-stop',enabled=False);word('stop-discovery','Stop');bt('own-discovery-stopped',lambda b:not b['scanning'] and b['adapters'][0]['discovering'])
            command('end-foreign-scan');bt('all-discovery-stopped',lambda b:not b['adapters'][0]['discovering'])
            device,identifier=pair(rendered=True);path=capture('confirm-six-digits')
            assert '000123' in subprocess.check_output(['tesseract',str(path),'stdout','--psm','6'],text=True,stderr=subprocess.DEVNULL)
            before=len(events('incoming-reply'));command('incoming-confirm');await_value(lambda:events('incoming-reply'),lambda es:len(es)>before);assert not events('incoming-reply')[-1]['accepted']
            assert state('concurrent-agent-request')['bluetooth']['prompt']['id']==identifier
            reply(identifier+1,'Accept');assert bt('stale-reply',lambda b:b['prompt'] is not None)['prompt']['id']==identifier
            command('foreign-confirm');await_value(lambda:events('foreign-rejected'),lambda es:es and es[-1]['rejected'])
            word('confirm-allow','Allow');bt('paired',lambda b:b['devices'][0]['paired'] and not b['busy']);assert events('pair-reply')[-1]['accepted']
            word('connect','Connect');bt('connected',lambda b:b['devices'][0]['connected'] and not b['busy'])
            word('disconnect','Disconnect');bt('disconnected',lambda b:not b['devices'][0]['connected'] and not b['busy'])
            word('trust','Trust');bt('trusted',lambda b:b['devices'][0]['trusted'] and not b['busy'])
            command('service');await_value(lambda:events('service-reply'),lambda es:es and es[-1]['accepted']);assert state('trusted-service')['bluetooth']['prompt'] is None
            # Forget requires the rendered two-step confirmation, even diagnostics cannot bypass it.
            before=len([e for e in events('method') if e['name']=='RemoveDevice']);action({'Forget':{'device':device}});time.sleep(.2);assert len([e for e in events('method') if e['name']=='RemoveDevice'])==before
            word('ask-forget','Forget');capture('forget-confirmation');word('keep-device','Keep')
            assert bt('kept-device',lambda b:len(b['devices'])==1)['devices'][0]['paired']
            word('ask-forget-again','Forget');word('confirm-forget','Forget');bt('forgotten',lambda b:len(b['devices'])==0 and not b['busy'])
            command('add-device');bt('device-returned',lambda b:len(b['devices'])==1)
            for mode,kind in [('pin','Pin'),('passkey','Passkey')]:
                command('mode',value=mode);device,identifier=pair(kind);state('keyboard-'+mode,lambda s:s['keyboard_visible']);capture('entry-'+mode)
                if mode=='pin':Keyboard(click).type_ascii('004321')
                else:
                    for digit in '004321':
                        # Numeric layout: three columns plus bottom 0/Enter row.
                        if digit=='0':click(135,663)
                        else:
                            n=int(digit)-1;click(5+(n%3+.5)*(350-8)/3+(n%3)*4,504+(n//3)*53)
                word('submit-'+mode,'Continue',vertical=(.1,.55))
                bt('paired-'+mode,lambda b:b['devices'][0]['paired'] and not b['busy'])
                answer=events('pair-reply')[-1];assert answer['accepted'] and answer['answer']==(['004321'] if mode=='pin' else [4321]),answer
                command('reset');bt('reset-'+mode,lambda b:not b['devices'][0]['paired'])
            # Populated input must disappear on close; delayed old answers cannot revive it.
            command('mode',value='pin');device,identifier=pair('Pin');Keyboard(click).type_ascii('004321')
            request(control,'system_menu',open=False);idle('close-cancels');assert not events('pair-reply')[-1]['accepted']
            opened();reply(identifier,{'Code':'004321'});assert state('late-answer-cleared')['bluetooth']['prompt'] is None
            # Repeated lock/unlock catches cross-thread cancellation and context races.
            for cycle in range(5):
                command('mode',value='pin');device,identifier=pair('Pin');Keyboard(click).type_ascii('004321')
                append(locker_commands,'lock');locked=state('locked-'+str(cycle),lambda s:s['lock']['phase']=='Locked');assert set(locked)=={'ok','pid','lock'}
                assert not any_request(control,'bluetooth_action',action={'Pair':{'device':device}})['ok']
                before=len(events('service-reply'));command('service');await_value(lambda:events('service-reply'),lambda es:len(es)>before);assert not events('service-reply')[-1]['accepted']
                append(locker_commands,'unlock');state('unlocked-'+str(cycle),lambda s:s['lock']['phase']=='Unlocked');idle('lock-canceled-pair');opened();reply(identifier,{'Code':'004321'});assert state('lock-retires-input')['bluetooth']['prompt'] is None
            retired=identifier
            command('mode',value='display');device,identifier=pair('DisplayPasskey');reply(retired,'Reject');assert state('stale-rejection-keeps-current')['bluetooth']['prompt']['id']==identifier;capture('display-passkey');command('display-progress',entered=4)
            assert bt('display-same-prompt',lambda b:b['prompt'] is not None)['prompt']['id']==identifier
            capture('display-progress');command('complete-display');bt('display-complete',lambda b:b['prompt'] is None and b['devices'][0]['paired'] and not b['busy'])
            command('reset');bt('reset-display',lambda b:not b['devices'][0]['paired'])
            # Same-path removal must retire its identity and any pending agent request.
            command('mode',value='confirm');device,identifier=pair();before=len([e for e in events('method') if e['name']=='Pair']);command('recreate')
            replaced=bt('same-path-replacement',lambda b:b['prompt'] is None and len(b['devices'])==1 and b['devices'][0]['id']!=device and not b['busy'])
            action({'Pair':{'device':device}});time.sleep(.2);assert len([e for e in events('method') if e['name']=='Pair'])==before
            # Owner replacement also retires adapter and device IDs and cancels the held prompt.
            device,identifier=pair();command('replace-owner')
            renewed=bt('owner-replacement',lambda b:b['epoch']>replaced['epoch'] and b['agent_available'] and b['default_agent'] and b['prompt'] is None and not b['busy'])
            assert renewed['selected_adapter']!=adapter and renewed['devices'][0]['id']!=device
            reply(identifier,'Accept');assert state('old-owner-answer')['bluetooth']['prompt'] is None
            # Incoming requests are refused without an explicit, page-scoped visibility lease.
            before=len(events('incoming-reply'));command('incoming-confirm');await_value(lambda:events('incoming-reply'),lambda es:len(es)>before);assert not events('incoming-reply')[-1]['accepted']
            adapter=renewed['selected_adapter'];action({'Visible':{'adapter':adapter,'enabled':True}});bt('incoming-enabled',lambda b:b['visible'])
            before=len(events('incoming-reply'));command('incoming-confirm');identifier=bt('incoming-prompt',lambda b:b['prompt'] is not None)['prompt']['id'];reply(identifier,'Accept');await_value(lambda:events('incoming-reply'),lambda es:len(es)>before);assert events('incoming-reply')[-1]['accepted']
            request(control,'system_menu',open=False);bt('visibility-released',lambda b:not b['visible'] and not b['adapters'][0]['discoverable']);command('summary')
            sets=events('set');assert any(e['property']=='Pairable' and e['value'] is False for e in sets)
            # Recover from loss of the whole private broker as well as owner replacement.
            old_epoch=state('before-bus-loss')['bluetooth']['epoch'];fixture.terminate();fixture.wait(timeout=5);bus.terminate();bus.wait(timeout=5)
            bt('bus-unavailable',lambda b:not b['available'])
            report=out/'fixture-restarted.jsonl';commands.write_text('')
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+address],stdout=log('bus-restarted.log'),stderr=subprocess.STDOUT);children.append(bus)
            await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
            fixture=subprocess.Popen(['/usr/bin/python3',str(root/'tests/bluez_fixture.py'),'--report',str(report),'--commands',str(commands)],env=env,stdout=log('fixture-restarted.log'),stderr=subprocess.STDOUT);children.append(fixture);await_value(lambda:events('ready'),bool)
            bt('bus-recovered',lambda b:b['available'] and b['epoch']>old_epoch and b['agent_available'] and b['default_agent']);opened();device,identifier=pair();reply(identifier,'Accept');bt('post-reconnect-paired',lambda b:b['devices'][0]['paired'] and not b['busy'])
            request(control,'exit');assert client.wait(timeout=6)==0;records['passed']=True
            print('PASS: private BlueZ discovery ownership, pairing UI/codes, trust/connect/forget, cancellation, spoof rejection, identity retirement and owner recovery',out)
        except BaseException:
            # Preserve thread stacks before cleanup when the compositor stops
            # answering, rather than losing the failure behind process teardown.
            if 'client' in locals() and client.poll() is None:
                try:
                    subprocess.run(['gdb','-q','-batch','-ex','set pagination off',
                                    '-ex','thread apply all bt','-p',str(client.pid)],
                                   stdout=log('failure-stacks.log'),stderr=subprocess.STDOUT,timeout=15)
                except (OSError,subprocess.TimeoutExpired):pass
            raise
        finally:
            if mouse is not None:mouse.close()
            for process in reversed(children):
                if process.poll() is None:
                    process.terminate()
                    try:process.wait(timeout=6)
                    except subprocess.TimeoutExpired:process.kill();process.wait()
            for stream in logs:stream.close()
            (out/'result.json').write_text(json.dumps(records,indent=2)+'\n')
            for name in ['session.log']:
                text=(out/name).read_text() if (out/name).exists() else ''
                assert '004321' not in text and '000123' not in text


if __name__=='__main__':main()
