#!/usr/bin/env python3
"""Real pointer hover handoff over an owned native client; private X server only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from PIL import Image
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--output',type=Path,required=True)
p.add_argument('--compositor',type=Path,required=True)
p.add_argument('--client',type=Path,default=Path('target/debug/examples/input-probe'))
p.add_argument('--xvfb',default='Xvfb')
p.add_argument('--scale',type=int,choices=[1,2],default=2)
a=p.parse_args();root=Path.cwd();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
r={'passed':False,'phone_tested':False,'scale':a.scale,'compositor_sha256':hashlib.sha256(a.compositor.read_bytes()).hexdigest(),'client_sha256':hashlib.sha256(a.client.read_bytes()).hexdigest(),'source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((root/'tests').glob('*.py'))},'observations':{},'captures':{}}
children=[];logs=[];mouse=None
with tempfile.TemporaryDirectory(prefix='tarot-hover-') as temporary:
 work=Path(temporary);control=work/'control.sock'
 for name in ['runtime','data','config','empty']:(work/name).mkdir(mode=0o700)
 def log(name):
  f=(out/name).open('x');logs.append(f);return f
 def state(label,predicate=lambda s:True):
  value=await_value(lambda:request(control,'status'),predicate,seconds=10);r['observations'][label]=value;return value
 def events():return [json.loads(l) for l in (out/'client.jsonl').read_text().splitlines()]
 def capture(label):
  path=out/(label+'.png');request(control,'capture',path=str(path));s=state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
  with Image.open(path) as image:
   assert image.size==(360*a.scale,720*a.scale);pixel=list(image.getpixel((30*a.scale,55*a.scale)))
  r['captures'][label]={'pixel':pixel,'sample_physical':[30*a.scale,55*a.scale],'rendered_frames':s['render_metrics']['rendered_frames']};return pixel
 def move(x,y):mouse.move(x*a.scale,y*a.scale);time.sleep(.08)
 def click(x,y):move(x,y);mouse.button(True);time.sleep(.05);mouse.button(False);time.sleep(.12)
 try:
  read,write=os.pipe()
  try:
   xvfb=subprocess.Popen([a.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;number=read_display_number(read)
  finally:
   os.close(read)
   if write is not None:os.close(write)
  env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work/'runtime'),XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty'),XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
  env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
  compositor=subprocess.Popen([str(a.compositor.resolve()),'--width',str(360*a.scale),'--height',str(720*a.scale),'--scale',str(a.scale),'--control-socket',str(control),'--exit-after-ms','60000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(compositor)
  await_value(lambda:control.exists() or compositor.poll() is not None,bool);assert compositor.poll() is None
  s=state('ready');mouse=PrivateInput(compositor.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
  native=dict(env,WAYLAND_DISPLAY=s['wayland_display']);native.pop('DISPLAY',None)
  client=subprocess.Popen([str(a.client.resolve()),'--report',str(out/'client.jsonl'),'--no-child','--fit-output','--opaque','--solid-color','208048','--exit-after-seconds','50'],env=native,stdout=log('client.log'),stderr=subprocess.STDOUT);children.append(client)
  state('mapped',lambda s:len(s['cards'])==1 and s['presentation']['windows'] and s['presentation']['windows'][0]['opacity']==1 and s['presentation']['windows'][0]['rect']==s['app_area'])
  # Establish a real visible tooltip, then leave the title for native content.
  move(40,16);time.sleep(1.2);assert capture('tooltip-visible')!=[32,128,72,255]
  move(180,350);time.sleep(.3)
  assert capture('native-hover-cleared')==[32,128,72,255], 'Old shell hover obscures the native app after pointer handoff'
  await_value(events,lambda es:any(e['event']=='pointer_enter' and e['target']=='root' for e in es))
  time.sleep(1.2);assert capture('native-hover-stays-clear')==[32,128,72,255]
  before=len([e for e in events() if e['event']=='pointer_button'])
  # A shell-owned press dragged outside the panel must keep its physical tail.
  click(40,16);state('menu-open',lambda s:s['system_menu_open'])
  move(60,110);mouse.button(True);time.sleep(.06)
  for step in range(1,6):
   mouse.move((60+50*step)*a.scale,(110+40*step)*a.scale);time.sleep(.02)
  mouse.button(False);time.sleep(.2)
  assert len([e for e in events() if e['event']=='pointer_button'])==before
  state('shell-drag-retained',lambda s:s['system_menu_open'] and isinstance(s['view'],dict))
  click(320,400);state('outside-dismissed',lambda s:not s['system_menu_open'])
  assert len([e for e in events() if e['event']=='pointer_button'])==before
  click(180,350);await_value(events,lambda es:len([e for e in es if e['event']=='pointer_button'])==before+2)
  pair=[e for e in events() if e['event']=='pointer_button'][-2:];assert [e['pressed'] for e in pair]==[True,False]
  r['native_fresh_pair']=pair;r['passed']=True;print('PASS native pointer handoff clears tooltip and preserves shell press/release ownership')
 finally:
  if mouse is not None:mouse.close()
  for child in reversed(children):
   if child.poll() is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait()
  for f in logs:f.close()
  (out/'validation.json').write_text(json.dumps(r,indent=2)+'\n')
