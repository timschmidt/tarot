#!/usr/bin/env python3
"""Mapped export authority, held input and bus epoch checks on private services."""
import argparse, hashlib, json, os, re, subprocess, tempfile, time
from pathlib import Path
from PIL import Image
from dialog_button import locate
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--output',type=Path,required=True)
p.add_argument('--compositor',type=Path,required=True)
p.add_argument('--producer',type=Path,required=True)
p.add_argument('--xvfb',default='Xvfb')
p.add_argument('--landscape',action='store_true')
a=p.parse_args();root=Path.cwd();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
width,height=(720,360) if a.landscape else (360,720)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
r={'passed':False,'phone_tested':False,'landscape':a.landscape,'compositor_sha256':sha(a.compositor),'producer_sha256':sha(a.producer),'sources':{str(p):sha(p) for p in [*Path('tests').glob('*.py'),Path('tests/native_menu_session.conf'),Path('examples/native-menu-egui.rs'),*Path('examples/native_menu_egui').glob('*.rs')]},'observations':{},'targets':[]}
children=[];logs=[];mouse=None
with tempfile.TemporaryDirectory(prefix='tarot-menu-claims-') as temporary:
 work=Path(temporary);control=work/'control.sock'
 for n in ['runtime','data','config','empty']:(work/n).mkdir(mode=0o700)
 def log(name):
  f=(out/name).open('x');logs.append(f);return f
 def events(name='fixture'):return [json.loads(l) for l in (out/(name+'.jsonl')).read_text().splitlines()]
 def actions(name='fixture'):return [e for e in events(name) if e['event']=='action']
 def state(label,predicate=lambda s:True):
  s=await_value(lambda:request(control,'status'),predicate,seconds=12);r['observations'][label]=s;return s
 def count(n):assert len(actions())==n and not actions('foreign'),(actions(),actions('foreign'))
 def command(text):
  before=len([e for e in events() if e['event']=='claim-mutation'])
  fixture.stdin.write(text+'\n');fixture.stdin.flush()
  es=await_value(events,lambda es:len([e for e in es if e['event']=='claim-mutation'])==before+1)
  assert [e for e in es if e['event']=='claim-mutation'][-1]['command']==text
 def capture(label):
  state(label+'-settled',lambda s:not s['system_menu_open'] or not s['top_menu']['motion']['moving'])
  path=out/(label+'.png');request(control,'capture',path=str(path));state(label+'-captured',lambda s:not s['capture_pending'] and not s['capture_error']);return path
 def click(x,y):mouse.move(x,y);mouse.button(True);time.sleep(.04);mouse.button(False);time.sleep(.09)
 def point(word,label):
  with Image.open(capture(label)) as im:
   rgb=im.convert('RGB');bottom=height-36
   for y in range(48,height-36):
    red,green,blue=rgb.getpixel((8,y))
    if not (30<=red<=85 and 0<=green-red<=16 and 0<=blue-red<=16):bottom=y;break
   assert bottom>80,(label,bottom);source=im.copy()
  rejected=[]
  for left in [12,40]:
   bounds=(left,40,232,bottom-4);path=out/(label+'-panel-'+str(left)+'.png');source.crop(bounds).save(path)
   try:target=locate(path,word,vertical=(0,1),scale=2);break
   except AssertionError as error:rejected.append(str(error))
  else:raise AssertionError((word,rejected))
  xy=(target['point'][0]+bounds[0],target['point'][1]+bounds[1]);r['targets'].append({'label':label,'ocr':target,'point':xy,'rejected_crops':rejected});return xy
 def row(word,label):click(*point(word,label))
 def close_menu():
  if state('before-dismiss')['system_menu_open']:click(40,16);state('dismissed',lambda s:not s['system_menu_open'])
 def prefs(label):
  close_menu();click(40,16);state(label+'-root',lambda s:s['system_menu_open']);row('Preferences',label+'-preferences');state(label+'-submenu',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Native' in s['top_menu']['panel'])
 def choose(label,n):
  prefs(label);row('Choose',label+'-choose');await_value(actions,lambda es:len(es)==n);state(label+'-closed',lambda s:not s['system_menu_open']);count(n)
 def bus(name):
  child=subprocess.Popen(['dbus-daemon','--config-file='+str(root/'tests/native_menu_session.conf'),'--address='+address,'--nofork'],stdout=log(name+'.log'),stderr=subprocess.STDOUT);children.append(child)
  await_value(lambda:(work/'bus').exists() or child.poll() is not None,bool);assert child.poll() is None;return child
 def bus_id():
  text=subprocess.check_output(['gdbus','call','--address',address,'--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.GetId'],text=True,timeout=5);return re.fullmatch(r"\('([0-9a-f]{32})',\)\n",text)[1]
 def start_session(name,trusted=False):
  args=[str(a.compositor.resolve()),'--width',str(width),'--height',str(height),'--control-socket',str(control),'--exit-after-ms','240000']
  if trusted:args += ['--input-method',str(a.producer.resolve()),'--input-method-arg=--report','--input-method-arg='+str(out/'trusted.jsonl')]
  child=subprocess.Popen(args,env=env,stdout=log(name+'.log'),stderr=subprocess.STDOUT);children.append(child)
  await_value(lambda:control.exists() or child.poll() is not None,bool);assert child.poll() is None and sha(Path('/proc',str(child.pid),'exe'))==r['compositor_sha256'];return child
 try:
  read,write=os.pipe()
  try:
   xvfb=subprocess.Popen([a.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
  finally:
   os.close(read)
   if write is not None:os.close(write)
  address='unix:path='+str(work/'bus');first_bus=bus('bus-first');original_guid=bus_id()
  env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work/'runtime'),XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'),XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty'),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',DBUS_SESSION_BUS_ADDRESS=address)
  env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
  compositor=start_session('compositor');ready=state('ready');mouse=PrivateInput(compositor.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
  native=dict(env,WAYLAND_DISPLAY=ready['wayland_display']);native.pop('DISPLAY',None)
  foreign=subprocess.Popen([str(a.producer.resolve()),'--export-only','--qa-stdin','--report',str(out/'foreign.jsonl')],env=native,stdin=subprocess.PIPE,text=True,stdout=log('foreign.log'),stderr=subprocess.STDOUT);children.append(foreign)
  await_value(lambda:(out/'foreign.jsonl').exists() and bool(events('foreign')),bool);foreign_owner=next(e['owner'] for e in events('foreign') if e['event']=='export')
  fixture=subprocess.Popen([str(a.producer.resolve()),'--qa-stdin','--no-claim','--report',str(out/'fixture.jsonl')],env=native,stdin=subprocess.PIPE,text=True,stdout=log('fixture.log'),stderr=subprocess.STDOUT);children.append(fixture)
  assert sha(Path('/proc',str(fixture.pid),'exe'))==sha(Path('/proc',str(foreign.pid),'exe'))==r['producer_sha256']
  r['processes']={}
  for name,child in [('fixture',fixture),('foreign',foreign)]:
   path=Path('/proc',str(child.pid));r['processes'][name]={'pid':child.pid,'uid':path.stat().st_uid,'cgroup':(path/'cgroup').read_text(),'sha256':sha(path/'exe')}
  assert r['processes']['fixture']['uid']==r['processes']['foreign']['uid'] and r['processes']['fixture']['cgroup']==r['processes']['foreign']['cgroup'] and fixture.pid!=foreign.pid
  state('unclaimed-mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and s['app_menu']['status']=='Absent' and s['presentation']['windows'][0]['opacity']==1)
  command('claim-self');state('own-ready',lambda s:s['app_menu']['status']=='Available');choose('own-command',1)
  prefs('held-foreign');xy=point('Choose','held-foreign-row');mouse.move(*xy);mouse.button(True);time.sleep(.06);old=state('held-before')
  command('claim-other '+foreign_owner);blocked=state('foreign-rejected',lambda s:s['app_menu']['status']=='Unavailable' and not s['app_menu']['nodes']);assert blocked['app_menu']['stamp']!=old['app_menu']['stamp'] and blocked['top_menu']['revision']!=old['top_menu']['revision'];mouse.button(False);time.sleep(.15);count(1);point('Show','foreign-fallback')
  for text,status_name in [('clear-claim','Absent'),('claim-other org.tarot.MenuEgui','Absent'),('claim-other :9999.9999','Unavailable'),('release-claim','Absent')]:
   command(text);state(text.replace(' ','-'),lambda s:s['app_menu']['status']==status_name and not s['app_menu']['nodes']);count(1)
  command('claim-self');state('reclaimed',lambda s:s['app_menu']['status']=='Available');prefs('held-key')
  mouse.key(116,True);state('key-selected',lambda s:s['top_menu']['selected'] is not None)
  before=state('before-key-mutation');command('flip');state('key-revision-retired',lambda s:s['app_menu']['stamp']!=before['app_menu']['stamp'] and s['app_menu']['status']=='Available' and s['top_menu']['selected'] is None)
  time.sleep(.7);assert state('held-key-no-repeat')['top_menu']['selected'] is None;mouse.key(116,False);mouse.tap(36);time.sleep(.15);count(1);choose('fresh-after-key',2)
  prefs('held-churn');mouse.move(*point('Choose','held-churn-row'));mouse.button(True);time.sleep(.06)
  old_churn=state('before-claim-churn');acks=len([e for e in events() if e['event']=='claim-mutation'])
  burst=['claim-self','release-claim']*24+['claim-self'];fixture.stdin.write('\n'.join(burst)+'\n');fixture.stdin.flush();samples=[]
  def completed():
   names=[p.read_text().strip() for p in Path('/proc',str(compositor.pid),'task').glob('*/comm')]
   sample={'workers':sum(n=='tarot-app-menu' for n in names),'signal_readers':sum(n.startswith('tarot-menu-sign') for n in names),'fds':len(list(Path('/proc',str(compositor.pid),'fd').iterdir()))};samples.append(sample)
   assert sample['workers']<=2 and sample['signal_readers']<=2,sample
   return len([e for e in events() if e['event']=='claim-mutation'])==acks+len(burst)
  await_value(completed,bool,seconds=12);state('churn-final-claim',lambda s:s['app_menu']['status']=='Available' and s['app_menu']['stamp']['source']!=old_churn['app_menu']['stamp']['source']);mouse.button(False);time.sleep(.15);count(2)
  r['claim_churn']={'commands':len(burst),'samples':samples,'held_release_did_not_activate':True}
  command('close-export');state('export-disconnected',lambda s:s['app_menu']['status']=='Unavailable' and len(s['cards'])==1);count(2)
  command('reconnect-export');new_owner=next(e['owner'] for e in reversed(events()) if e['event']=='export');assert new_owner!=next(e['owner'] for e in events() if e['event']=='export')
  time.sleep(.15);assert state('same-bus-unclaimed-reconnect')['app_menu']['status']=='Unavailable';command('claim-self');state('new-owner-claimed',lambda s:s['app_menu']['status']=='Available');choose('fresh-after-owner',3)
  prefs('held-bus');mouse.move(*point('Choose','held-bus-row'));mouse.button(True);time.sleep(.06);before_bus=state('before-bus-exit')
  first_bus.terminate();assert first_bus.wait(timeout=6)==0;await_value(lambda:not (work/'bus').exists(),bool);state('bus-lost',lambda s:s['app_menu']['status']=='Unavailable' and not s['app_menu']['nodes']);mouse.button(False);time.sleep(.15);count(3)
  second_bus=bus('bus-replacement');new_guid=bus_id();assert new_guid!=original_guid
  # Reuse the exact claimed unique name on a new bus. A name/process-only
  # comparison must fail this test; the original claim retains its bus GUID.
  index=int(re.fullmatch(r':1\.(\d+)',new_owner)[1]);assert 1<=index<=100
  for _ in range(index-1):assert bus_id()==new_guid
  command('reconnect-export');reused_owner=next(e['owner'] for e in reversed(events()) if e['event']=='export');assert reused_owner==new_owner,(reused_owner,new_owner)
  close_menu();click(width/2,height-18);state('bus-cards',lambda s:s['view']=='Overview' and s['presentation']['overview']==1)
  click(width/2,height/2);state('old-epoch-rejected',lambda s:isinstance(s['view'],dict) and s['app_menu']['stamp']['source']!=before_bus['app_menu']['stamp']['source'] and s['app_menu']['status']=='Unavailable' and not s['app_menu']['nodes']);count(3)
  r['bus_epochs']={'old_guid':original_guid,'new_guid':new_guid,'reused_owner':reused_owner}
  command('claim-self');state('fresh-epoch-claimed',lambda s:s['app_menu']['status']=='Available');choose('fresh-after-bus',4)
  click(40,16);state('close-root',lambda s:s['system_menu_open']);row('Close','cooperative-close');state('client-gone',lambda s:not s['cards'] and s['app_menu']['status']=='Absent');assert fixture.wait(timeout=5)==0;count(4)
  foreign.stdin.write('exit\n');foreign.stdin.flush();assert foreign.wait(timeout=5)==0
  r['callbacks']=actions();assert r['callbacks']==[{'event':'action','name':'choose','target':'night'}]*4
  mouse.close();mouse=None;request(control,'exit');assert compositor.wait(timeout=6)==0;await_value(lambda:not control.exists(),bool)
  compositor=start_session('trusted-compositor',trusted=True);mouse=PrivateInput(compositor.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
  trusted=state('trusted-mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and s['input_method']['pid'] is not None)
  assert trusted['app_menu']['status']=='Absent' and trusted['native_export_peers']==[];pid=trusted['input_method']['pid'];assert sha(Path('/proc',str(pid),'exe'))==r['producer_sha256']
  await_value(lambda:(out/'trusted.jsonl').exists() and any(e['event']=='mapped' for e in events('trusted')),bool);time.sleep(.5);assert state('trusted-stays-excluded')['app_menu']['status']=='Absent'
  click(40,16);state('trusted-card-menu',lambda s:s['system_menu_open']);point('Show','trusted-fallback');assert not actions('trusted');r['trusted_input_method_excluded']=True
  r['passed']=True;print('PASS mapped claim authority, release/reclaim, held input, exact-name bus replacement and trusted-input exclusion')
 finally:
  if mouse is not None:mouse.close()
  for child in reversed(children):
   if child.poll() is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait()
  for f in logs:f.close()
  (out/'validation.json').write_text(json.dumps(r,indent=2)+'\n')
