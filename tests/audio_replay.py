#!/usr/bin/env python3
"""Exercise real libpulse control against a private PulseAudio daemon, never host audio."""
import argparse,json,os,signal,subprocess,tempfile,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--pulse-daemon',default='pulseaudio')
    parser.add_argument('--pulse-modules',type=Path)
    parser.add_argument('--pulse-libraries',type=Path)
    args=parser.parse_args();args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    runtime=tempfile.TemporaryDirectory(prefix='tarot-audio-');root=Path(runtime.name)
    control=root/'control.sock';server='unix:'+str(root/'pulse.sock')
    env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    pulse_env=dict(os.environ,PULSE_RUNTIME_PATH=str(root/'runtime'),PULSE_STATE_PATH=str(root/'state'),PULSE_CONFIG_PATH=str(root))
    if args.pulse_libraries:pulse_env['LD_LIBRARY_PATH']=str(args.pulse_libraries.resolve())
    if args.pulse_modules:pulse_env['LD_LIBRARY_PATH']=str(args.pulse_modules.resolve())+':'+pulse_env.get('LD_LIBRARY_PATH','')
    for name in ['runtime','state']:(root/name).mkdir(mode=0o700)
    config=root/'default.pa';config.write_text('load-module module-native-protocol-unix socket='+str(root/'pulse.sock')+' auth-anonymous=yes\n')
    logs=[];client=daemon=None;records={};validation={}
    def log(name):
        file=(args.output/name).open('w');logs.append(file);return file
    def pactl(*command):
        return subprocess.check_output(['pactl','--server',server,*map(str,command)],env=env,text=True,stderr=subprocess.STDOUT)
    def launch(label):
        nonlocal daemon
        command=[str(args.pulse_daemon),'-n','--daemonize=no','--use-pid-file=no','--exit-idle-time=-1','--disable-shm=yes','--log-target=stderr','--file='+str(config)]
        if args.pulse_modules:command+=['--dl-search-path='+str(args.pulse_modules.resolve())]
        daemon=subprocess.Popen(command,env=pulse_env,stdout=log(label+'.log'),stderr=subprocess.STDOUT)
        await_value(lambda:(root/'pulse.sock').exists() or daemon.poll() is not None,bool);assert daemon.poll() is None
        await_value(lambda:subprocess.run(['pactl','--server',server,'info'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode,lambda code:code==0)
    def audio(label,condition=lambda a:True,seconds=10):
        state=await_value(lambda:request(control,'status'),lambda s:condition(s['audio']),seconds=seconds)
        records[label]=state;return state['audio']
    def action(value):request(control,'audio_action',action=value)
    def complete(label,value,condition):
        action(value);return audio(label,lambda a:a['pending'] is None and not a['action_error'] and condition(a))
    def sink(name):return next(s for s in json.loads(pactl('-f','json','list','sinks')) if s['name']==name)
    def percent(s):return round(max(v['value'] for v in s['volume'].values())*100/65536)
    def capture(name):
        request(control,'capture',path=str(args.output/(name+'.png')))
        await_value(lambda:request(control,'status'),lambda s:not s['capture_pending'] and not s['capture_error'])
    try:
        client=subprocess.Popen([str(Path('target/debug/tarot').resolve()),'--system-services','--platform-bus','unix:path='+str(root/'absent-bus'),'--audio-server',server,'--control-socket',str(control),'--exit-after-ms','180000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT)
        await_value(lambda:control.exists() or client.poll() is not None,bool);assert client.poll() is None
        audio('initial-offline',lambda a:not a['available'] and bool(a['error']))
        launch('first-daemon');audio('empty-server',lambda a:a['available'] and not a['outputs'])
        first_module=int(pactl('load-module','module-null-sink','sink_name=tarot_first','sink_properties=device.description=First_output').strip())
        first=audio('first-output',lambda a:len(a['outputs'])==1);first_id=first['outputs'][0]['id'];epoch=first['epoch']
        second_module=int(pactl('load-module','module-null-sink','sink_name=tarot_second','sink_properties=device.description=Second_output').strip())
        outputs=audio('second-output',lambda a:len(a['outputs'])==2);second_id=next(o['id'] for o in outputs['outputs'] if o['id']!=first_id)
        pactl('set-sink-volume','tarot_first','50%','25%')
        audio('external-volume',lambda a:next(o for o in a['outputs'] if o['id']==first_id)['percent']==50)
        complete('balanced-volume',{'Volume':{'output':first_id,'percent':40}},lambda a:next(o for o in a['outputs'] if o['id']==first_id)['percent']==40)
        actual=list(sink('tarot_first')['volume'].values());assert abs(actual[0]['value']-26214)<=1 and abs(actual[1]['value']-13107)<=1
        complete('volume-already-current',{'Volume':{'output':first_id,'percent':40}},lambda a:next(o for o in a['outputs'] if o['id']==first_id)['percent']==40)
        complete('muted',{'Mute':{'output':first_id,'muted':True}},lambda a:next(o for o in a['outputs'] if o['id']==first_id)['muted'])
        assert sink('tarot_first')['mute'] and percent(sink('tarot_first'))==40
        pactl('set-sink-mute','tarot_first','0')
        audio('external-unmuted',lambda a:not next(o for o in a['outputs'] if o['id']==first_id)['muted'])
        complete('new-default',{'Default':{'output':second_id}},lambda a:next(o for o in a['outputs'] if o['id']==second_id)['default'])
        assert pactl('get-default-sink').strip()=='tarot_second'
        request(control,'system_menu',open=True);request(control,'system_page',page='Audio');time.sleep(.3);capture('outputs-and-controls')
        before=sink('tarot_first')['volume'];action({'Volume':{'output':first_id,'percent':101}})
        audio('invalid-volume-rejected',lambda a:a['pending'] is None and bool(a['action_error']));assert sink('tarot_first')['volume']==before
        pactl('unload-module',first_module);audio('removed-output',lambda a:len(a['outputs'])==1)
        action({'Mute':{'output':first_id,'muted':True}});audio('stale-output-rejected',lambda a:a['pending'] is None and 'no longer' in (a['action_error'] or ''))
        third_module=int(pactl('load-module','module-null-sink','sink_name=tarot_first','sink_properties=device.description=Replacement_output').strip())
        third=audio('replacement-output',lambda a:len(a['outputs'])==2);third_id=next(o['id'] for o in third['outputs'] if o['id']!=second_id);assert third_id!=first_id
        complete('zero-volume',{'Volume':{'output':third_id,'percent':0}},lambda a:next(o for o in a['outputs'] if o['id']==third_id)['percent']==0)
        assert percent(sink('tarot_first'))==0
        # Suspend only our private daemon to force delayed replies; compositor stays responsive.
        os.kill(daemon.pid,signal.SIGSTOP);action({'Volume':{'output':third_id,'percent':37}})
        pending=audio('delayed-reply',lambda a:a['pending'] is not None);assert next(o for o in pending['outputs'] if o['id']==third_id)['percent']==0
        request(control,'home');assert request(control,'status')['view']=='Home'
        os.kill(daemon.pid,signal.SIGCONT)
        audio('delayed-confirmed',lambda a:a['pending'] is None and not a['action_error'] and next(o for o in a['outputs'] if o['id']==third_id)['percent']==37)
        assert percent(sink('tarot_first'))==37
        os.kill(daemon.pid,signal.SIGSTOP);action({'Volume':{'output':third_id,'percent':41}})
        audio('before-timeout',lambda a:a['pending'] is not None)
        audio('timeout-unavailable',lambda a:not a['available'] and a['pending'] is None and bool(a['action_error']),seconds=12)
        os.kill(daemon.pid,signal.SIGCONT)
        audio('timeout-recovered',lambda a:a['available'] and a['epoch']>epoch,seconds=12)
        # Kill and replace only the private daemon. Old IDs must not cross epochs.
        daemon.terminate();daemon.wait(timeout=5)
        audio('daemon-lost',lambda a:not a['available'] and not a['outputs'])
        launch('second-daemon');audio('replacement-empty',lambda a:a['available'] and not a['outputs'])
        pactl('load-module','module-null-sink','sink_name=tarot_second','sink_properties=device.description=After_restart')
        final=audio('after-daemon-restart',lambda a:a['available'] and len(a['outputs'])==1);final_id=final['outputs'][0]['id'];assert final_id not in [first_id,second_id,third_id]
        action({'Volume':{'output':second_id,'percent':12}});audio('old-epoch-rejected',lambda a:a['pending'] is None and bool(a['action_error']))
        # A paused server also cannot hold compositor shutdown hostage.
        os.kill(daemon.pid,signal.SIGSTOP);action({'Mute':{'output':final_id,'muted':True}});audio('pending-at-shutdown',lambda a:a['pending'] is not None)
        start=time.monotonic();request(control,'exit');validation['compositor_exit']=client.wait(timeout=4);validation['shutdown_seconds']=time.monotonic()-start
        assert validation['compositor_exit']==0
        validation['passed']=True
        print('PASS: real private PulseAudio, empty/hotplug outputs, balance/zero/mute/default readback, stale IDs, delay/timeout, daemon restart and bounded pending shutdown')
    finally:
        if daemon is not None and daemon.poll() is None:
            os.kill(daemon.pid,signal.SIGCONT);daemon.terminate()
            try:daemon.wait(timeout=5)
            except subprocess.TimeoutExpired:daemon.kill();daemon.wait()
        if client is not None and client.poll() is None:
            client.terminate()
            try:client.wait(timeout=5)
            except subprocess.TimeoutExpired:client.kill();client.wait()
        for file in logs:file.close()
        (args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')
        (args.output/'validation.json').write_text(json.dumps(validation,indent=2)+'\n')
        runtime.cleanup()

if __name__=='__main__':main()
