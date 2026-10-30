"""Owner-identified JLab Studio pairing and A2DP playback on the real phone.

Selects only the exact owner-provided alias, requiring one matching device.
Leaves its new pairing saved, disconnected and untrusted; restores playback
routing/volume and discovery state. No unrelated device is modified.
"""
import csv,hashlib,io,json,math,os,pwd,struct,subprocess,time,wave
from pathlib import Path
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request

PEER='JLab Studio'

def as_user(argv):
 u=pwd.getpwnam('user')
 return subprocess.check_output(argv,user=u.pw_uid,group=u.pw_gid,extra_groups=os.getgrouplist(u.pw_name,u.pw_gid),env={'PATH':'/usr/bin:/bin','HOME':u.pw_dir,'XDG_RUNTIME_DIR':'/run/user/10000'},text=True,timeout=10)

def peers():
 code='''import json,gi
gi.require_version('Gio','2.0')
from gi.repository import Gio
b=Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
o=b.call_sync('org.bluez','/','org.freedesktop.DBus.ObjectManager','GetManagedObjects',None,None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
r=[]
for path,v in o.items():
 d=v.get('org.bluez.Device1',{})
 if d.get('Name')=='JLab Studio' or d.get('Alias')=='JLab Studio':
  r.append({'path':path,'properties':{k:d.get(k) for k in ['Name','Alias','Paired','Trusted','Connected','Blocked','ServicesResolved','UUIDs']},'transports':[{'path':p,**{k:i['org.bluez.MediaTransport1'].get(k) for k in ['State','UUID','Codec']}} for p,i in o.items() if p.startswith(path+'/') and 'org.bluez.MediaTransport1' in i]})
print(json.dumps(r))
'''
 return json.loads(as_user(['/usr/bin/python3','-c',code]))

def locate_peer(path):
 # Tesseract emits one TSV token per word; require the complete adjacent
 # owner-provided name on one line, rather than matching another Studio device.
 for mode in [3,6,11]:
  result=subprocess.run(['tesseract',str(path),'stdout','--psm',str(mode),'tsv'],capture_output=True,text=True,check=True,timeout=30)
  rows=[r for r in csv.DictReader(io.StringIO(result.stdout),delimiter='\t') if r['text'].strip()]
  matches=[]
  for first,second in zip(rows,rows[1:]):
   line=lambda r:(r['block_num'],r['par_num'],r['line_num'])
   if first['text'].casefold()=='jlab' and second['text'].casefold()=='studio' and line(first)==line(second) and min(float(first['conf']),float(second['conf']))>=80:
    x=(int(first['left'])+int(second['left'])+int(second['width']))/2;y=int(first['top'])+int(first['height'])/2
    if 100<y<1350:matches.append({'word':PEER,'point':(x,y),'mode':mode,'recognized':[first,second]})
  assert len(matches)<=1,'Ambiguous rendered owner alias'
  if matches:return matches[0]
 raise AssertionError('Complete owner-provided alias is not visible')


def replay(socket,screen,tap,state,capture,before,directory,authenticate):
 folder=directory/'headphones';folder.mkdir(mode=0o700);u=pwd.getpwnam('user');os.chown(folder,u.pw_uid,u.pw_gid)
 r={'passed':False,'owner_identified_alias':PEER,'targets':[],'physical_peer_tested':True,'pairing_tested':False,'audible_output_owner_confirmed':False,'recording_tested':False};player=None;monitor=None;selected=False;original_sink=as_user(['pactl','get-default-sink']).strip();r['original_default_sink_sha256']=hashlib.sha256(original_sink.encode()).hexdigest()
 def trace(event):
  v=request(socket,'status')
  uart=Path('/sys/class/bluetooth/hci0').resolve().parents[1]/'power'
  power={k:(uart/k).read_text().strip() for k in ['runtime_status','runtime_active_time','runtime_suspended_time'] if (uart/k).exists()}
  with (folder/'events.jsonl').open('a') as f:f.write(json.dumps({'event':event,'monotonic':time.monotonic(),'bluetooth':v.get('bluetooth'),'lock':v['lock']['phase'],'uart_power':power})+'\n')
 def bt(label,predicate=lambda b:True):return state('headphones-'+label,lambda s:s['lock']['phase']=='Unlocked' and predicate(s['bluetooth']),seconds=30)
 def current():
  ps=peers();assert len(ps)<=1,'Ambiguous owner alias; do not select any device';return ps[0] if ps else None
 def read(label,predicate):
  v=await_value(current,lambda p:p is not None and predicate(p['properties']),seconds=30);r[label]=v;return v
 def word(label,text,scroll=False):
  last=None
  for attempt in range(7 if scroll else 1):
   time.sleep(.2);name='headphones-'+label+'-'+str(attempt);capture(name)
   try:
    point=locate_peer(directory/(name+'.png')) if text==PEER else locate(directory/(name+'.png'),text,vertical=(.07,.94),scale=1);r['targets'].append({'label':label,'target':point});trace(label+'-before-tap');tap(point['point'][0]/2,point['point'][1]/2);trace(label+'-after-tap');return
   except AssertionError as error:last=error
   if scroll:
    screen.down(180,580)
    for y in range(550,220,-30):screen.move(180,y,delay=.025)
    screen.up();time.sleep(.2)
  raise last
 def finish_connection(label,paired):
  end=time.monotonic()+45;settled=None
  while time.monotonic()<end:
   s=request(socket,'status');assert s['lock']['phase']=='Unlocked' and s['system_page']=='Bluetooth' and s['system_menu_open']
   prompt=s['bluetooth']['prompt']
   if prompt is not None:
    assert prompt['device']==s['bluetooth']['selected_device'] and prompt['kind'] in ['Confirm','AuthorizePairing','AuthorizeService'],'Unexpected code request; do not guess a PIN'
    word(label+'-allow','Allow')
   p=current();trace(label+'-waiting')
   if p is not None and p['properties']['Paired'] and (not paired or p['properties']['Connected'] and p['properties']['ServicesResolved']) and s['bluetooth']['pending'] is None and not s['bluetooth']['busy'] and s['bluetooth']['prompt'] is None:
    settled=settled or time.monotonic()
    if time.monotonic()-settled>=.6:return p
   else:settled=None
   assert s['bluetooth']['action_error'] is None,s['bluetooth']['action_error'];time.sleep(.15)
  raise AssertionError('Pair/connect did not reach observed state')
 try:
  s=bt('ready',lambda b:b['available'] and b['agent_available'] and b['default_agent']);assert not s['bluetooth']['scanning'] and not s['bluetooth']['visible']
  request(socket,'system_page',page='Bluetooth');request(socket,'system_menu',open=True);bt('page')
  word('find','Find');bt('searching',lambda b:b['scanning']);candidate=read('discovered',lambda p:not p['Blocked']);r['peer_object_sha256']=hashlib.sha256(candidate['path'].encode()).hexdigest();r['paired_before']=candidate['properties']['Paired'];r['trusted_before']=candidate['properties']['Trusted'];assert not r['trusted_before'],'Preserve an already trusted peer; this test expects the recorded untrusted JLab pairing'
  monitor=subprocess.Popen(['/usr/bin/python3',str(Path(__file__).with_name('phone_headphone_monitor.py')),'--peer-path',candidate['path'],'--output',str(folder/'wire.jsonl')],stdout=(folder/'monitor.log').open('x'),stderr=subprocess.STDOUT)
  await_value(lambda:(folder/'wire.jsonl').exists() and 'ready' in (folder/'wire.jsonl').read_text(),bool,seconds=8)
  word('stop','Stop');bt('search-stopped',lambda b:not b['scanning']);word('select',PEER,scroll=True);s=bt('selected',lambda b:b['selected_device'] is not None);selected=True;r['device_token']=s['bluetooth']['selected_device']
  if not r['paired_before']:
   word('pair','Pair');r['paired']=finish_connection('pair',False);r['pairing_tested']=True
  if current()['properties']['Connected']:
   word('initial-disconnect','Disconnect');read('initial-disconnected',lambda p:not p['Connected'])
  bt('before-trust',lambda b:b['pending'] is None and not b['busy'] and b['prompt'] is None)
  word('trust-on','Trust');read('trusted',lambda p:p['Trusted']);bt('trust-finished',lambda b:b['pending'] is None and not b['busy'])
  (folder/'awaiting-power-cycle.json').write_text(json.dumps({'alias':PEER,'ready':True,'requires_owner_reply':True})+'\n')
  await_value(lambda:(folder/'reconnect-ready').exists(),bool,seconds=300)
  bt('owner-reconnect-ready',lambda b:b['pending'] is None and not b['busy'] and b['prompt'] is None)
  if not current()['properties']['Connected']:word('connect','Connect');r['connect_requested']=True
  try:r['connected']=finish_connection('connect',True)
  except AssertionError:
   # Separate lower-layer diagnosis, after the GUI operation has returned an
   # observed error. This is not substituted for passing GUI-control evidence.
   p=current();s=request(socket,'status')
   if p is not None and not p['properties']['Connected'] and s['bluetooth']['pending'] is None and not s['bluetooth']['busy']:
    code="import sys,json,gi\ngi.require_version('Gio','2.0')\nfrom gi.repository import Gio,GLib\nb=Gio.bus_get_sync(Gio.BusType.SYSTEM,None)\ntry:\n b.call_sync('org.bluez',sys.argv[1],'org.bluez.Device1','Connect',None,None,Gio.DBusCallFlags.NONE,30000,None);print(json.dumps({'success':True}))\nexcept GLib.Error as e:print(json.dumps({'success':False,'error':Gio.DBusError.get_remote_error(e),'message':e.message}))\n"
    try:r['independent_connect_diagnosis']=json.loads(subprocess.check_output(['/usr/bin/python3','-c',code,p['path']],user=u.pw_uid,group=u.pw_gid,extra_groups=os.getgrouplist(u.pw_name,u.pw_gid),env={'PATH':'/usr/bin:/bin','HOME':u.pw_dir,'XDG_RUNTIME_DIR':'/run/user/10000'},text=True,timeout=35))
    except subprocess.TimeoutExpired:r['independent_connect_diagnosis']={'timed_out':True}
   raise

  sink_prefix='bluez_sink.'+candidate['path'].rsplit('/',1)[-1].removeprefix('dev_')+'.'
  sinks=await_value(lambda:json.loads(as_user(['pactl','-f','json','list','sinks'])),lambda ss:any(s['name'].startswith(sink_prefix) for s in ss),seconds=20)
  matches=[s for s in sinks if s['name'].startswith(sink_prefix)];assert len(matches)==1,'Ambiguous Bluetooth audio outputs';sink=matches[0];r['audio_profile']=sink['properties'].get('bluetooth.protocol');r['bluetooth_output_sha256']=hashlib.sha256(sink['name'].encode()).hexdigest()
  assert r['audio_profile'] in ['a2dp_sink','a2dp'];r['a2dp_output_observed']=True
  # A quiet synthetic tone on this specific output. Stream volume is 10%; do
  # not change the user's per-output gain or launch any microphone recording.
  tone=folder/'tone.wav'
  with wave.open(str(tone),'wb') as w:
   w.setnchannels(2);w.setsampwidth(2);w.setframerate(48000)
   frames=bytearray()
   for i in range(48000*12):
    t=i/48000;envelope=min(1,t/.1,(12-t)/.1);value=int(0.25*32767*envelope*math.sin(2*math.pi*(440 if int(t)%2==0 else 660)*t));frames.extend(struct.pack('<hh',value,value))
   w.writeframes(frames)
  os.chown(tone,u.pw_uid,u.pw_gid)
  player=subprocess.Popen(['paplay','--device='+sink['name'],'--volume=6554',str(tone)],user=u.pw_uid,group=u.pw_gid,extra_groups=os.getgrouplist(u.pw_name,u.pw_gid),env={'PATH':'/usr/bin:/bin','HOME':u.pw_dir,'XDG_RUNTIME_DIR':'/run/user/10000'},stdout=(folder/'playback.log').open('x'),stderr=subprocess.STDOUT)
  active=await_value(current,lambda p:p is not None and any(t['State']=='active' for t in p['transports']),seconds=12);r['active_transport']=active['transports'];(folder/'playing.json').write_text(json.dumps({'playing':True,'alias':PEER})+'\n')
  assert player.wait(timeout=25)==0;r['playback_completed']=True;r['passed']=True;return r
 finally:
  try:
   if monitor is not None and monitor.poll() is None:monitor.terminate();monitor.wait(timeout=5)
   if player is not None and player.poll() is None:player.terminate();player.wait(timeout=6)
   s=request(socket,'status')
   if s['lock']['phase']=='Unlocked':
    request(socket,'system_page',page='Bluetooth');request(socket,'system_menu',open=True)
    if selected:
     p=current()
     if p is not None and p['properties']['Connected']:word('disconnect','Disconnect');read('disconnected',lambda p:not p['Connected'])
    p=current()
    if selected and p is not None and p['properties']['Trusted']!=r.get('trusted_before',p['properties']['Trusted']):
     bt('before-trust-restore',lambda b:b['pending'] is None and not b['busy'] and b['prompt'] is None);word('trust-restore','Trust');read('trust-restored',lambda v:v['Trusted']==r['trusted_before'])
    s=request(socket,'status')
    if s['bluetooth']['scanning']:
     request(socket,'bluetooth_action',action={'Scan':{'adapter':s['bluetooth']['selected_adapter'],'enabled':False}});bt('cleanup-stopped',lambda b:not b['scanning'])
    if as_user(['pactl','get-default-sink']).strip()!=original_sink:as_user(['pactl','set-default-sink',original_sink])
    request(socket,'system_menu',open=False)
    after=request(socket,'status');assert after['cards']==before['cards'] and after['view']==before['view'];r['original_default_restored']=as_user(['pactl','get-default-sink']).strip()==original_sink
    r['final_peer']=current()
  except BaseException as error:
   r['passed']=False;r['cleanup_error_type']=type(error).__name__
   raise
  finally:
   (folder/'result.json').write_text(json.dumps(r,indent=2)+'\n')
