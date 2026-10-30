#!/usr/bin/env python3
"""Real mapped GTK surface claims and visible exported-menu interactions."""
import argparse,json,os,subprocess,tempfile,time,hashlib
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import request
from hardware_replay import PrivateInput
from dialog_button import locate
from with_xvfb import read_display_number
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--landscape',action='store_true');p.add_argument('--compositor',type=Path);a=p.parse_args()
out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);root=Path.cwd();width,height=(720,360) if a.landscape else (360,720)
compositor_binary=(a.compositor or root/'target/debug/tarot').resolve()
r={'passed':False,'phone_kms_tested':False,'landscape':a.landscape,'compositor_sha256':hashlib.sha256(compositor_binary.read_bytes()).hexdigest(),'observations':{}}
children=[];logs=[];mouse=None
with tempfile.TemporaryDirectory(prefix='tarot-native-menu-') as temporary:
 work=Path(temporary);control=work/'control.sock'
 for name in ['runtime','data','empty','config']:(work/name).mkdir(mode=0o700)
 (work/'settings.toml').write_text('version = 3\n[notifications]\ndnd = true\n')
 def log(name):
  f=(out/name).open('x');logs.append(f);return f
 def state(label,predicate=lambda s:True):
  value=await_value(lambda:request(control,'status'),predicate,seconds=12);r['observations'][label]=value;return value
 def events():return [json.loads(l) for l in (out/'fixture.jsonl').read_text().splitlines()]
 def capture(label):
  state(label+'-settled',lambda s:not s['system_menu_open'] or not s['top_menu']['motion']['moving'])
  image=out/(label+'.png');request(control,'capture',path=str(image));state(label+'-captured',lambda s:not s['capture_pending'] and not s['capture_error']);return image
 def click(x,y):mouse.move(x,y);mouse.button(True);time.sleep(.05);mouse.button(False);time.sleep(.1)
 def point(word,label):
  image=capture(label)
  with Image.open(image) as im:
   im.crop((0,28,260,height-36)).save(out/(label+'-menu.png'))
  found=locate(out/(label+'-menu.png'),word,vertical=(0,1));return (found['point'][0],found['point'][1]+28)
 def row(word,label):click(*point(word,label))
 def prefs(label):
  click(40,16);state(label+'-open',lambda s:s['system_menu_open']);row('Preferences',label+'-preferences');state(label+'-submenu',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Native' in s['top_menu']['panel'])
 try:
  read,write=os.pipe()
  try:
   xvfb=subprocess.Popen([str(root/'target/qa-pulse-r9/root/usr/bin/Xvfb'),'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
  finally:
   os.close(read)
   if write is not None:os.close(write)
  env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',XDG_RUNTIME_DIR=str(work/'runtime'),XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS='/usr/local/share:/usr/share',GTK_USE_PORTAL='0',NO_AT_BRIDGE='1',GSETTINGS_BACKEND='memory',XDG_CURRENT_DESKTOP='Tarot',XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'))
  env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
  compositor=subprocess.Popen([str(compositor_binary),'--width',str(width),'--height',str(height),'--config-file',str(work/'settings.toml'),'--control-socket',str(control),'--exit-after-ms','120000'],env=env,stdout=log('session.log'),stderr=subprocess.STDOUT);children.append(compositor)
  await_value(lambda:control.exists() or compositor.poll() is not None,bool);assert compositor.poll() is None
  ready=state('ready');mouse=PrivateInput(compositor.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
  native=dict(env,WAYLAND_DISPLAY=ready['wayland_display'],GDK_BACKEND='wayland');native.pop('DISPLAY',None)
  fixture=subprocess.Popen(['python3',str(root/'tests/native_app_menu_fixture.py'),'--report',str(out/'fixture.jsonl')],env=native,stdin=subprocess.PIPE,text=True,stdout=log('fixture.log'),stderr=subprocess.STDOUT);children.append(fixture)
  available=state('export-ready',lambda s:s.get('app_menu',{}).get('status')=='Available')
  assert available['app_menu']['nodes'] and available['app_menu']['nodes'][0]['label']=='Preferences'
  prefs('choose');row('Choose','choose-night');await_value(lambda:[e for e in events() if e['event']=='action'],lambda es:len(es)==1);state('choose-closed',lambda s:not s['system_menu_open'])
  assert [e for e in events() if e['event']=='action']==[{'event':'action','name':'choose','target':'night'}]
  previous_source=state('before-cards')['app_menu']['stamp']['source']
  click(width/2,height-18);state('cards',lambda s:s['view']=='Overview' and s['presentation']['overview']==1.0)
  click(width/2,height/2);state('returned-app',lambda s:isinstance(s['view'],dict) and 'App' in s['view'] and s['app_menu']['status']=='Available' and s['app_menu']['stamp']['source']!=previous_source and s['presentation']['overview']==0.0)
  prefs('toggle');row('Toggle','toggle-flag');await_value(lambda:[e for e in events() if e['event']=='action'],lambda es:len(es)==2);state('toggle-closed',lambda s:not s['system_menu_open'])
  checked=state('checked',lambda s:s['app_menu']['status']=='Available' and s['app_menu']['nodes'][0]['kind']['Submenu'][1]['checked'])
  prefs('held');x,y=point('Choose','held-choice');mouse.move(x,y);mouse.button(True);time.sleep(.08)
  before=state('held-before');fixture.stdin.write('flip\n');fixture.stdin.flush()
  after=state('held-invalidated',lambda s:s['app_menu']['stamp']!=before['app_menu']['stamp'] and s['app_menu']['status']=='Available')
  assert after['top_menu']['revision']!=before['top_menu']['revision'];mouse.button(False);time.sleep(.15)
  assert len([e for e in events() if e['event']=='action'])==2
  # Menu remains at its root after a source revision; re-enter with fresh input.
  row('Preferences','after-flip-preferences');state('fresh-submenu',lambda s:isinstance(s['top_menu']['panel'],dict) and 'Native' in s['top_menu']['panel'])
  row('Choose','after-flip-choice');await_value(lambda:[e for e in events() if e['event']=='action'],lambda es:len(es)==3);state('fresh-closed',lambda s:not s['system_menu_open'])
  fixture.stdin.write('disable\n');fixture.stdin.flush();state('disabled-update',lambda s:s['app_menu']['status']=='Available' and not s['app_menu']['nodes'][0]['kind']['Submenu'][0]['enabled'])
  prefs('disabled');row('Choose','disabled-choice');time.sleep(.15);assert len([e for e in events() if e['event']=='action'])==3
  capture('disabled-visible');fixture.terminate();fixture.wait(timeout=5);state('owner-dead',lambda s:not s['cards'] and s['app_menu']['status']=='Absent')
  r['actual_callbacks']=[e for e in events() if e['event']=='action'];r['passed']=True
  print('PASS real mapped GTK exports, visible commands/state, held-row flip retirement, fresh choice, disabled and owner-death behavior')
 finally:
  if mouse is not None:mouse.close()
  for child in reversed(children):
   if child.poll() is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait()
  for f in logs:f.close()
  (out/'validation.json').write_text(json.dumps(r,indent=2)+'\n')
