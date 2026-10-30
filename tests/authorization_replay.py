#!/usr/bin/env python3
"""Rendered Polkit agent protocol acceptance on an owned Xvfb/private bus.

Uses only a nonprivileged synthetic helper and synthetic credentials. The fake
authority's grant is not evidence of authorization by the system authority.
"""
import argparse,ast,hashlib,json,os,pwd,subprocess,tempfile,time
from pathlib import Path
from dialog_button import locate
from hardware_replay import PrivateInput
from lock_replay import append,any_request
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from with_xvfb import read_display_number


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--binary',type=Path,default=Path('target/debug/tarot'));p.add_argument('--xvfb',default='Xvfb');a=p.parse_args()
    out=a.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False);root=Path.cwd();children=[];logs=[];mouse=None
    records={'passed':False,'real_authority_tested':False,'real_credentials_used':False,'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'actions':[]}
    sources=out/'qa-sources';sources.mkdir();queue=[Path(__file__).resolve(),root/'tests/polkit_fixture.py'];hashes={}
    while queue:
        path=queue.pop()
        if path.name in hashes:continue
        payload=path.read_bytes();hashes[path.name]=hashlib.sha256(payload).hexdigest();(sources/path.name).write_bytes(payload)
        for node in ast.walk(ast.parse(payload)):
            modules=[x.name for x in node.names] if isinstance(node,ast.Import) else [node.module] if isinstance(node,ast.ImportFrom) and node.module else []
            for module in modules:
                q=root/'tests'/(module.split('.')[0]+'.py')
                if q.exists() and q.name not in hashes:queue.append(q)
    records['sources_sha256']=hashes
    with tempfile.TemporaryDirectory(prefix='tarot-polkit-') as temporary:
        work=Path(temporary);control=work/'control.sock';commands=work/'commands.jsonl';commands.write_text('');report=out/'authority.jsonl';helper=work/'helper.py';locker_commands=work/'locker.commands';locker_commands.write_text('');sequence=0
        (work/'empty').mkdir();(work/'runtime').mkdir(mode=0o700)
        def log(name):s=(out/name).open('x');logs.append(s);return s
        def events(name=None):
            all_events=[json.loads(line) for line in report.read_bytes().split(b'\n')[:-1]] if report.exists() else []
            return [e for e in all_events if name is None or e['event']==name]
        def command(name,**fields):
            nonlocal sequence
            sequence+=1
            with commands.open('a') as stream:stream.write(json.dumps(dict(command=name,request=sequence,**fields))+'\n')
            await_value(lambda:events('command'),lambda es:any(e['request']==sequence for e in es))
        def state(label,predicate=lambda s:True,seconds=10):
            s=await_value(lambda:request(control,'status'),predicate,seconds=seconds);records['actions'].append(dict(label=label,state=s))
            encoded=json.dumps(s)
            assert not any(value in encoded for value in ['synthetic-secret','002345','Fixture action','Password:','Fixture authentication','qa-cookie'])
            return s
        def auth(label,predicate,seconds=10):return state(label,lambda s:predicate(s['authorization']),seconds)['authorization']
        def question(label):return auth(label,lambda a:a['request'] is not None and a['request']['phase']=='Question')['request']
        def start(name,mode='password',identities=None):
            command('mode',value=mode);fields={} if identities is None else {'identities':identities};command('start',case=name,**fields);return question(name)
        def result(name):
            found=await_value(lambda:events('result'),lambda es:any(e['case']==name for e in es));return next(e for e in found if e['case']==name)
        def capture(label):
            time.sleep(.2);path=out/(label+'.png');request(control,'capture',path=str(path));state('capture-'+label,lambda s:not s['capture_pending'] and not s['capture_error']);return path
        def click(x,y):mouse.move(x,y);time.sleep(.04);mouse.button(True);time.sleep(.06);mouse.button(False);time.sleep(.12)
        def word(label,text,vertical=(.08,.62)):
            found=locate(capture(label),text,vertical=vertical,scale=2);records.setdefault('rendered_targets',[]).append(dict(label=label,target=found));click(*found['point'])
        def respond(q,value):request(control,'authorization_action',action={'Respond':{'request':q['id'],'turn':q['turn'],'response':value}})
        def closed(label):return state(label,lambda s:s['authorization']['request'] is None and not s['system_menu_open'])
        def failed(label):
            state(label,lambda s:s['authorization']['request'] is None and bool(s['authorization']['error']) and s['system_menu_open'] and not s['keyboard_visible'])
            word(label+'-close','Close');closed(label+'-dismissed')
        try:
            read,write=os.pipe()
            try:
                xvfb=subprocess.Popen([a.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1000x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
            finally:
                os.close(read)
                if write is not None:os.close(write)
            address='unix:path='+str(work/'bus')
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+address],stdout=log('bus.log'),stderr=subprocess.STDOUT);children.append(bus);await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
            env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_RUNTIME_DIR=str(work/'runtime'),XDG_CONFIG_HOME=str(work/'empty'),XDG_CONFIG_DIRS=str(work/'empty'),XDG_DATA_HOME=str(work/'empty'),XDG_DATA_DIRS=str(work/'empty'),DBUS_SESSION_BUS_ADDRESS=address)
            env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
            fixture=subprocess.Popen(['/usr/bin/python3',str(root/'tests/polkit_fixture.py'),'--report',str(report),'--commands',str(commands),'--helper',str(helper)],env=env,stdout=log('fixture.log'),stderr=subprocess.STDOUT);children.append(fixture);await_value(lambda:events('ready'),bool)
            client=subprocess.Popen([str(a.binary.resolve()),'--width','360','--height','720','--reduced-motion','--system-services','--platform-bus',address,'--authorization-helper',str(helper),'--backlight-root',str(work/'empty'),'--control-socket',str(control),'--config-file',str(work/'settings.toml'),'--lock-probe-unlocked-start','--lock-client',str(root/'target/debug/examples/lock-probe'),'--lock-client-arg=--role','--lock-client-arg=locker','--lock-client-arg=--report','--lock-client-arg='+str(out/'locker.jsonl'),'--lock-client-arg=--commands','--lock-client-arg='+str(locker_commands),'--exit-after-ms','240000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT);children.append(client);await_value(lambda:control.exists() or client.poll() is not None,bool);assert client.poll() is None
            mouse=PrivateInput(client.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool);mouse.move(180,300)
            initial=auth('enrolled',lambda a:a['available']);assert len(events('registered'))==1
            q=start('rendered-success');state('trusted-modal',lambda s:s['system_menu_open'] and s['system_page']=='Authorization' and s['keyboard_visible'])
            Keyboard(click).type_ascii('synthetic-secret');capture('masked-entry');word('authenticate','Authenticate')
            assert result('rendered-success')['granted'];closed('success-restores-origin')
            q=start('wrong-password');mouse.tap(10);mouse.tap(36);r=result('wrong-password');assert r['agent_reply'] and not r['granted'];failed('failure-clears-entry')
            q=start('cancel');word('cancel','Cancel',vertical=(.08,.62));r=result('cancel');assert not r['granted'] and r['error'].endswith('Cancelled');closed('cancel-closes')
            q=start('two-turns','two');respond(q,'synthetic-secret');second=auth('second-question',lambda a:a['request'] is not None and a['request']['turn']!=q['turn'] and a['request']['phase']=='Question')['request'];respond(q,'stale-secret')
            assert question('stale-turn-keeps-current')['turn']==second['turn'];Keyboard(click).type_ascii('002345');word('second-submit','Authenticate');assert result('two-turns')['granted'];closed('two-turns-closed')
            other_uid=65534 if os.geteuid()!=65534 else 0;other_name=pwd.getpwuid(other_uid).pw_name
            command('mode',value='password');command('start',case='choice',identities=[os.geteuid(),other_uid])
            auth('choose-offered-account',lambda a:a['request'] is not None and a['request']['phase']=='Choose')
            word('choose-other-account',other_name);q=question('chosen-account-question');respond(q,'synthetic-secret');assert result('choice')['granted'];closed('choice-closed')
            command('start',case='spoof',spoof=True);r=result('spoof');assert not r['agent_reply'] and not r['granted'];closed('spoof-no-modal')
            q=start('first-overlap');command('start',case='overlap');assert not result('overlap')['agent_reply'];assert question('same-original')['id']==q['id'];click(325,64);assert not result('first-overlap')['granted'];closed('overlap-cleanup')
            q=start('inactive');command('rapid-inactive');assert not result('inactive')['granted'];closed('rapid-inactive-cancels')
            q=start('session-lock-hint');command('locked-hint',value=True);assert not result('session-lock-hint')['granted'];closed('lock-hint-cancels')
            command('start',case='locked-hint-denied');assert not result('locked-hint-denied')['granted'];command('locked-hint',value=False)
            q=start('late-cancel');command('cancel',previous=True);assert question('late-cancel-keeps-current')['id']==q['id'];click(325,64);assert not result('late-cancel')['granted'];closed('late-cancel-cleanup')
            q=start('session-change');epoch=request(control,'status')['authorization']['epoch'];command('session-replace');assert not result('session-change')['granted'];auth('session-reenrolled',lambda a:a['available'] and a['epoch']>epoch and a['request'] is None)
            q=start('owner-change');epoch=request(control,'status')['authorization']['epoch'];command('replace-owner');auth('owner-reenrolled',lambda a:a['available'] and a['epoch']>epoch and a['request'] is None);closed('owner-cleanup')
            for mode in ['oversized','malformed','hang']:
                command('mode',value=mode);command('start',case=mode)
                if mode=='hang':auth('hung-helper-pending',lambda a:a['request'] is not None);click(325,64)
                r=result(mode);assert not r['granted']
                if mode=='hang':closed(mode+'-closed')
                else:failed(mode+'-failed')
            q=start('lock');append(locker_commands,'lock');locked=state('protected',lambda s:s['lock']['phase']=='Locked');assert set(locked)=={'ok','pid','lock'};assert not result('lock')['granted']
            command('start',case='while-locked');assert not result('while-locked')['granted'];assert not any_request(control,'authorization_action',action={'Respond':{'request':q['id'],'turn':q['turn'],'response':'synthetic-secret'}})['ok']
            append(locker_commands,'unlock');state('unlocked',lambda s:s['lock']['phase']=='Unlocked');closed('unlock-keeps-input-retired')
            q=start('after-unlock');respond(q,'synthetic-secret');assert result('after-unlock')['granted'];closed('after-unlock-success')
            # The whole broker can disappear without restarting the compositor.
            epoch=request(control,'status')['authorization']['epoch'];fixture.terminate();fixture.wait(timeout=5);bus.terminate();bus.wait(timeout=5);auth('broker-gone',lambda a:not a['available'])
            report=out/'authority-restarted.jsonl';commands.write_text('');helper.unlink()
            records['helper_events_before_restart']=[json.loads(line) for line in helper.with_suffix('.jsonl').read_text().splitlines()]
            helper.with_suffix('.jsonl').unlink()
            bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--nopidfile','--address='+address],stdout=log('bus-restarted.log'),stderr=subprocess.STDOUT);children.append(bus);await_value(lambda:(work/'bus').exists() or bus.poll() is not None,bool);assert bus.poll() is None
            fixture=subprocess.Popen(['/usr/bin/python3',str(root/'tests/polkit_fixture.py'),'--report',str(report),'--commands',str(commands),'--helper',str(helper)],env=env,stdout=log('fixture-restarted.log'),stderr=subprocess.STDOUT);children.append(fixture);await_value(lambda:events('ready'),bool)
            auth('broker-reenrolled',lambda a:a['available'] and a['epoch']>epoch);q=start('after-broker');respond(q,'synthetic-secret');assert result('after-broker')['granted'];closed('broker-complete')
            request(control,'exit');assert client.wait(timeout=6)==0;records['passed']=True;print('PASS: private authority enrollment, rendered entry, two-turn framing, rejection, caller/cookie/session/owner validation and lock cancellation',out)
        except BaseException:
            if 'client' in locals() and client.poll() is None:
                try:subprocess.run(['gdb','-q','-batch','-ex','thread apply all bt','-p',str(client.pid)],stdout=log('failure-stacks.log'),stderr=subprocess.STDOUT,timeout=15)
                except (OSError,subprocess.TimeoutExpired):pass
            raise
        finally:
            if helper.with_suffix('.jsonl').exists():records['helper_events']=[json.loads(line) for line in helper.with_suffix('.jsonl').read_text().splitlines()]
            if mouse is not None:mouse.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:child.wait(timeout=6)
                    except subprocess.TimeoutExpired:child.kill();child.wait()
            for stream in logs:stream.close()
            (out/'result.json').write_text(json.dumps(records,indent=2)+'\n')
            if (out/'session.log').exists():assert 'synthetic-secret' not in (out/'session.log').read_text()


if __name__=='__main__':main()
