#!/usr/bin/env python3
"""Private NetworkManager creation/secret-agent regression; no system networking."""
import argparse,json,os,subprocess,tempfile,time
from pathlib import Path
from notification_replay import Fixture,await_value
from phone_gesture_replay import request


def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
 runtime=tempfile.TemporaryDirectory(prefix='tarot-credentials-');control=Path(runtime.name)/'control.sock';address='unix:path='+str(Path(runtime.name)/'bus')
 env=dict(os.environ,DBUS_SESSION_BUS_ADDRESS=address,LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
 root=Path.cwd();bus=client=fixture=None;logs=[];records={};password='tarot-test-psk'
 def log(name):f=(out/name).open('w');logs.append(f);return f
 def state(label,condition=lambda s:True,seconds=8):
  s=await_value(lambda:request(control,'status'),condition,seconds=seconds);records[label]=s
  assert password not in json.dumps(s) and 'Tarot Lab' not in json.dumps(s) and 'Tarot New' not in json.dumps(s)
  return s
 def wifi(label,condition,seconds=8):return state(label,lambda s:condition(s['network']),seconds)['network']
 def action(action):request(control,'network_action',action=action)
 def input(text):request(control,'credential_text',value=text)
 def capture(name):
  time.sleep(.3)
  request(control,'capture',path=str(out/(name+'.png')));state('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
 def events(name):return [e for e in fixture.events() if e['event']==name]
 def result_after(before):return await_value(lambda:events('secret-result'),lambda es:len(es)>before)[-1]
 try:
  bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--address',address,'--print-address=1'],stdout=subprocess.PIPE,stderr=log('bus.log'),text=True);assert bus.stdout.readline().startswith(address)
  fixture=Fixture(root/'target/debug/examples/network-probe',out/'service',env,extra_args=['--credentials'])
  client=subprocess.Popen([str(root/'target/debug/tarot'),'--system-services','--platform-bus',address,'--control-socket',str(control),'--exit-after-ms','240000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT)
  await_value(lambda:control.exists() or client.poll() is not None,bool);assert client.poll() is None
  initial=wifi('initial',lambda n:n['agent_available'] and n['create_permission']=='Yes' and len(n['networks'])==4)
  saved=next(r for r in initial['networks'] if r['connected']);new=next(r for r in initial['networks'] if r['security']=='Personal' and not r['profiles']);open_row=next(r for r in initial['networks'] if r['security']=='Open')
  request(control,'system_menu',open=True);request(control,'system_page',page='Wifi')
  request(control,'network_setup',network=new['id']);state('new-form',lambda s:s['credential_input'] is not None and s['keyboard_visible'])
  input('short');assert not state('short-invalid')['credential_input']['valid'];request(control,'credential_submit');assert not events('create')
  request(control,'credential_cancel');assert state('cancel-clears')['credential_input'] is None and not events('create')
  request(control,'network_setup',network=new['id']);input(password);state('masked-ready',lambda s:s['credential_input']['valid']);capture('masked-new-password')
  request(control,'credential_remember',remember=False);request(control,'credential_submit')
  wifi('new-pending',lambda n:n['busy'] and n['pending']=={'Connect':new['id']})
  made=events('create')[-1];assert made['password_matches'] and made['persist']=='volatile' and made['secret_flags']==0 and made['private_user']
  fixture.call('complete-connect');wifi('new-connected',lambda n:not n['busy'] and n['active_profile'] is not None and any(r['id']==new['id'] and r['connected'] for r in n['networks']))
  request(control,'network_setup',network=open_row['id']);state('open-confirmation',lambda s:s['credential_input'] is not None and not s['keyboard_visible']);request(control,'credential_submit')
  wifi('open-pending',lambda n:n['busy'] and n['pending']=={'Connect':open_row['id']});assert events('create')[-1]['persist']=='disk' and events('create')[-1]['secret_flags'] is None
  fixture.call('complete-connect');wifi('open-connected',lambda n:not n['busy'] and any(r['id']==open_row['id'] and r['connected'] for r in n['networks']))
  # Permission checks apply again on dispatch, including diagnostic requests.
  fixture.call('set',permission='auth');wifi('create-auth',lambda n:n['create_permission']=='Authentication');before=len(events('create'))
  action({'Create':{'network':new['id'],'password':password,'remember':True}});wifi('create-denied',lambda n:not n['busy'] and n['action_error']);assert len(events('create'))==before
  fixture.call('set',permission='yes');wifi('create-allowed',lambda n:n['create_permission']=='Yes')
  action({'Connect':{'network':saved['id'],'profile':saved['profiles'][0]}});wifi('saved-pending',lambda n:n['busy'] and n['pending']=={'Connect':saved['id']})
  fixture.call('auth-needed')
  # Wrong sender, noninteractive requests and another profile cannot prompt.
  for fields,suffix in [({'spoof':True},'PermissionDenied'),({'flags':0},'NoSecrets'),({'index':2},'NoSecrets')]:
   before=len(events('secret-result'));fixture.call('secret-request',**fields);reply=result_after(before);assert not reply['ok'] and reply['error'].endswith(suffix),reply
   assert state('rejected-agent-'+suffix)['credential_input'] is None
  before=len(events('secret-result'));fixture.call('secret-request')
  prompt=state('agent-prompt',lambda s:s['credential_input'] is not None and s['credential_input']['target']=='Agent');prompt_id=prompt['credential_input']['id'];capture('empty-agent-prompt')
  # Stale responses cannot fill the pending prompt.
  action({'Secrets':{'request':prompt_id+1,'password':password}});assert state('stale-response')['credential_input']['id']==prompt_id
  input(password);request(control,'credential_submit');reply=result_after(before);assert reply['ok'] and reply['password_matches'],reply
  fixture.call('complete-connect');wifi('agent-connected',lambda n:not n['busy'] and n['secret_prompt'] is None)
  assert state('submitted-form-gone')['credential_input'] is None
  # User cancel and daemon cancel return their distinct protocol errors.
  for mode,suffix in [('user','UserCanceled'),('daemon','AgentCanceled')]:
   fixture.call('set',state=30,connected=0)
   action({'Connect':{'network':saved['id'],'profile':saved['profiles'][0]}});wifi('pending-'+mode,lambda n:n['busy'] and n['pending']=={'Connect':saved['id']});fixture.call('auth-needed')
   before=len(events('secret-result'));fixture.call('secret-request');state('prompt-'+mode,lambda s:s['credential_input'] is not None)
   input(password)
   if mode=='user':request(control,'credential_cancel')
   else:fixture.call('secret-cancel')
   reply=result_after(before);assert not reply['ok'] and reply['error'].endswith(suffix),reply
   state('cleared-'+mode,lambda s:s['credential_input'] is None);fixture.call('set',state=120);wifi('failed-'+mode,lambda n:not n['busy'])
  # Losing the owner closes the populated prompt and retires its token.
  fixture.call('set',state=30,connected=0)
  action({'Connect':{'network':saved['id'],'profile':saved['profiles'][0]}});wifi('owner-loss-pending',lambda n:n['busy'] and n['pending']=={'Connect':saved['id']});fixture.call('auth-needed')
  before=len(events('secret-result'));fixture.call('secret-request');prompt=state('owner-loss-prompt',lambda s:s['credential_input'] is not None);old=prompt['credential_input']['id'];input(password)
  fixture.call('release');state('owner-loss-clears',lambda s:not s['network']['available'] and s['credential_input'] is None);assert not result_after(before)['ok']
  fixture.call('acquire');wifi('owner-returned',lambda n:n['available'] and n['agent_available'] and n['epoch']>initial['epoch'])
  action({'Secrets':{'request':old,'password':password}});assert state('old-prompt-stays-gone')['credential_input'] is None
  renewed=state('fresh-identities')['network'];saved=next(r for r in renewed['networks'] if r['profiles'] and r['security']=='Personal' and r['strength']==90)
  fixture.call('set',state=30,connected=0)
  action({'Connect':{'network':saved['id'],'profile':saved['profiles'][0]}});wifi('exit-pending',lambda n:n['busy'] and n['pending']=={'Connect':saved['id']});fixture.call('auth-needed')
  fixture.call('secret-request');state('exit-with-populated-prompt',lambda s:s['credential_input'] is not None);input(password)
  assert not events('secrets-requested')
  stopped=time.monotonic();request(control,'exit');code=client.wait(timeout=6);assert code==0;records['shutdown_with_pending_prompt_seconds']=time.monotonic()-stopped
  records['validation']={'passed':True,'compositor_exit':code};print('PASS: new-network setup, masked entry/cancel, private temporary/persistent profiles, secret-agent authorization/replies/cancellation and owner-loss cleanup',out)
 finally:
  if fixture is not None:fixture.close()
  for proc in [client,bus]:
   if proc is not None and proc.poll() is None:
    proc.terminate()
    try:proc.wait(timeout=6)
    except subprocess.TimeoutExpired:proc.kill();proc.wait()
  for f in logs:f.close()
  runtime.cleanup();(out/'status.json').write_text(json.dumps(records,indent=2)+'\n')
  for path in out.rglob('*.log'):assert password not in path.read_text(),path

if __name__=='__main__':main()
