#!/usr/bin/env python3
"""Real private-bus NetworkManager protocol/identity regression, no host networking changes."""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from notification_replay import Fixture,await_value
from phone_gesture_replay import request


def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path);args=p.parse_args()
 out=args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix='tarot-wifi-evidence-'));out.mkdir(mode=0o700,parents=True,exist_ok=True)
 runtime=tempfile.TemporaryDirectory(prefix='tarot-wifi-runtime-');control=Path(runtime.name)/'control.sock';address='unix:path='+str(Path(runtime.name)/'bus')
 env=dict(os.environ,DBUS_SESSION_BUS_ADDRESS=address,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
 root=Path.cwd();bus=client=fixture=None;logs=[];records={}
 def log(name):f=(out/name).open('w');logs.append(f);return f
 def start_bus(name):
  b=subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address,'--print-address=1'],stdout=subprocess.PIPE,stderr=log(name),text=True);assert b.stdout.readline().startswith(address);return b
 def fresh(name):return Fixture(root/'target/debug/examples/network-probe',out/name,env)
 def state(label,condition=lambda s:True,seconds=8):
  s=await_value(lambda:request(control,'status'),condition,seconds=seconds);records[label]=s
  assert 'Tarot Lab' not in json.dumps(s) and 'Saved 1' not in json.dumps(s),'Routine diagnostics must omit SSIDs/profile names'
  return s
 def wifi(label,condition,seconds=8):return state(label,lambda s:condition(s['network']),seconds=seconds)['network']
 def act(action):request(control,'network_action',action=action)
 def capture(name):
  request(control,'capture',path=str(out/(name+'.png')));state('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
 def replies(event):return [e for e in fixture.events() if e['event']==event]
 try:
  client=subprocess.Popen([str(root/'target/debug/tarot'),'--system-services','--platform-bus',address,'--control-socket',str(control),'--exit-after-ms','150000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT)
  await_value(lambda:control.exists() or client.poll() is not None,bool);assert client.poll() is None
  wifi('offline-start',lambda n:not n['available'] and n['error'])
  bus=start_bus('bus.log');fixture=fresh('first-service')
  initial=wifi('initial',lambda n:n['available'] and len(n['networks'])==3)
  connected=next(n for n in initial['networks'] if n['connected']);guest=next(n for n in initial['networks'] if not n['connected'] and n['profiles']);open_row=next(n for n in initial['networks'] if n['security']=='Open')
  assert connected['strength']==90 and len(connected['profiles'])==1 and open_row['profiles']==[]
  request(control,'system_menu',open=True);request(control,'system_page',page='Wifi');time.sleep(.2);capture('wifi-menu')
  act('Scan');wifi('scan-pending',lambda n:n['busy'] and n['pending']=='Scan');time.sleep(.15)
  assert state('scan-ack-not-completion')['network']['last_scan']==initial['last_scan']
  fixture.call('complete-scan');wifi('scan-complete',lambda n:not n['busy'] and n['last_scan']>initial['last_scan'])
  act('Scan');wifi('timeout-pending',lambda n:n['busy']);wifi('scan-timeout',lambda n:not n['busy'] and 'timed out' in (n['action_error'] or ''),seconds=25)
  fixture.call('set',deny=True);act('Scan');wifi('scan-denied',lambda n:not n['busy'] and n['action_error'])
  fixture.call('set',deny=False,permission='auth');wifi('authorization-needed',lambda n:n['scan_permission']=='Authentication')
  calls=len(replies('scan'));act('Scan');wifi('auth-not-bypassed',lambda n:not n['busy'] and n['action_error']);assert len(replies('scan'))==calls
  fixture.call('set',permission='yes');wifi('permission-restored',lambda n:n['scan_permission']=='Yes')
  activations=len(replies('activate'));act({'Connect':{'network':open_row['id'],'profile':connected['profiles'][0]}})
  wifi('security-mismatch-rejected',lambda n:not n['busy'] and n['action_error']);assert len(replies('activate'))==activations
  act({'Connect':{'network':guest['id'],'profile':guest['profiles'][0]}})
  pending=wifi('connection-pending',lambda n:n['busy'] and n['pending']=={'Connect':guest['id']})
  assert next(n for n in pending['networks'] if n['id']==guest['id'])['connected'] is False
  time.sleep(.15);assert state('activation-ack-not-completion')['network']['busy']
  fixture.call('complete-connect');wifi('connected',lambda n:not n['busy'] and n['active_profile']==guest['profiles'][0] and next(r for r in n['networks'] if r['id']==guest['id'])['connected'])
  assert all(e['device'].endswith('/Devices/1') for e in replies('activate'))
  act('Disconnect');wifi('disconnected',lambda n:not n['busy'] and n['state']=='Disconnected')
  assert all(not e['usb'] for e in replies('disconnect'))
  act({'Connect':{'network':connected['id'],'profile':connected['profiles'][0]}});wifi('auth-job-pending',lambda n:n['busy'] and n['pending']=={'Connect':connected['id']})
  fixture.call('auth-needed');wifi('credentials-needed',lambda n:not n['busy'] and n['state']=='CredentialsRequired' and n['action_error'])
  fixture.call('set',state=30,connected=0)
  act({'Radio':{'enabled':False}});wifi('radio-off',lambda n:not n['busy'] and not n['radio'])
  act({'Radio':{'enabled':True}});wifi('radio-on',lambda n:not n['busy'] and n['radio'])
  fixture.call('set',hardware=False);wifi('hardware-blocked',lambda n:not n['hardware'])
  calls=len(replies('radio'));act({'Radio':{'enabled':True}});wifi('hardware-not-bypassed',lambda n:not n['busy'] and n['action_error']);assert len(replies('radio'))==calls
  fixture.call('set',hardware=True,second=False);wifi('access-point-gone',lambda n:all(r['id']!=guest['id'] for r in n['networks']))
  calls=len(replies('activate'));act({'Connect':{'network':guest['id'],'profile':guest['profiles'][0]}});wifi('stale-row-rejected',lambda n:not n['busy'] and n['action_error']);assert len(replies('activate'))==calls
  fixture.call('set',second=True);wifi('access-point-returned',lambda n:any(r['profiles'] and r['id']!=connected['id'] for r in n['networks']))
  fixture.call('release');wifi('service-gone',lambda n:not n['available']);fixture.call('acquire')
  renewed=wifi('service-returned',lambda n:n['available'] and n['epoch']>initial['epoch']);assert all(r['id']!=connected['id'] for r in renewed['networks'])
  calls=len(replies('activate'));act({'Connect':{'network':connected['id'],'profile':connected['profiles'][0]}});wifi('old-owner-token-rejected',lambda n:not n['busy'] and n['action_error']);assert len(replies('activate'))==calls
  assert not replies('secrets-requested')
  fixture.close();fixture=None;wifi('owner-disconnected',lambda n:not n['available'])
  bus.terminate();bus.wait(timeout=4);bus=start_bus('restarted-bus.log');fixture=fresh('second-service')
  after=wifi('bus-recovered',lambda n:n['available']);assert after['epoch']>renewed['epoch']
  row=next(r for r in after['networks'] if not r['connected'] and r['profiles']);act({'Connect':{'network':row['id'],'profile':row['profiles'][0]}});wifi('after-reconnect-pending',lambda n:n['busy'] and n['pending']=={'Connect':row['id']});fixture.call('complete-connect');wifi('after-reconnect-connected',lambda n:not n['busy'] and n['active_profile']==row['profiles'][0])
  capture('wifi-connected');assert not replies('secrets-requested')
  request(control,'system_menu',open=False);assert state('view-retained')['view']=='Home'
  fixture.close();fixture=None;request(control,'exit');code=client.wait(timeout=6);assert code==0
  records['validation']={'passed':True,'compositor_exit':code};print('PASS: NetworkManager scan/activation confirmation, radio/disconnect, permissions, credential-needed state, security/stale tokens and service/bus recovery',out)
 finally:
  if fixture is not None:fixture.close()
  for process in [client,bus]:
   if process is not None and process.poll() is None:
    process.terminate()
    try:process.wait(timeout=6)
    except subprocess.TimeoutExpired:process.kill();process.wait()
  for f in logs:f.close()
  runtime.cleanup();(out/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
