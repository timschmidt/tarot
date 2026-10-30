#!/usr/bin/env python3
"""Fault-inject menu connection setup on owned Unix sockets and a private display."""
import array,argparse,hashlib,json,os,socket,struct,subprocess,tempfile,threading,time
from pathlib import Path
from PIL import Image
from dialog_button import locate
from hardware_replay import PrivateInput
from notification_replay import await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--output',type=Path,required=True);p.add_argument('--compositor',type=Path,required=True);p.add_argument('--producer',type=Path,required=True);p.add_argument('--xvfb',default='Xvfb')
p.add_argument('--fault',choices=['auth','hello','auth-flood','auth-fds','oversize','oversize-be','fields','backlog'],default='auth');p.add_argument('--cancel',action='store_true');p.add_argument('--fragment-header',action='store_true');a=p.parse_args()
out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);root=Path.cwd()
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
r={'passed':False,'phone_tested':False,'fault':a.fault,'cancel':a.cancel,'compositor_sha256':sha(a.compositor),'producer_sha256':sha(a.producer),'sources':{str(p):sha(p) for p in Path('tests').glob('*.py')},'observations':{},'server':{},'memory':[],'targets':[]}
children=[];logs=[];mouse=None;server=None;stop=threading.Event();injected=threading.Event();eof=threading.Event();listener=None;filler=None
with tempfile.TemporaryDirectory(prefix='tarot-menu-setup-') as temporary:
 work=Path(temporary);control=work/'control.sock'
 for n in ['runtime','data','config','empty']:(work/n).mkdir(mode=0o700)
 def log(n):f=(out/n).open('x');logs.append(f);return f
 def state(n,pred=lambda s:True,seconds=8):
  s=await_value(lambda:request(control,'status'),pred,seconds=seconds);r['observations'][n]=s;return s
 def events():return [json.loads(l) for l in (out/'fixture.jsonl').read_text().splitlines()]
 def click(x,y):mouse.move(x,y);mouse.button(True);time.sleep(.04);mouse.button(False);time.sleep(.09)
 def capture(n):
  state(n+'-settled',lambda s:not s['top_menu']['motion']['moving']);path=out/(n+'.png');request(control,'capture',path=str(path));state(n+'-captured',lambda s:not s['capture_pending'] and not s['capture_error']);return path
 def row(word,n):
  with Image.open(capture(n)) as im:
   rgb=im.convert('RGB');bottom=684
   for y in range(48,684):
    red,green,blue=rgb.getpixel((8,y))
    if not (30<=red<=85 and 0<=green-red<=16 and 0<=blue-red<=16):bottom=y;break
   bounds=(12,40,232,bottom-4);crop=out/(n+'-panel.png');im.crop(bounds).save(crop)
  target=locate(crop,word,vertical=(0,1),scale=2);r['targets'].append(target);click(target['point'][0]+bounds[0],target['point'][1]+bounds[1])
 def command(text):
  old=len([e for e in events() if e['event']=='claim-mutation']);fixture.stdin.write(text+'\n');fixture.stdin.flush();await_value(events,lambda es:len([e for e in es if e['event']=='claim-mutation'])==old+1)
 def memory():
  fields={line.split(':')[0]:line.split(':')[1].strip() for line in Path('/proc',str(compositor.pid),'status').read_text().splitlines() if ':' in line}
  value={'monotonic':time.monotonic(),'rss_kib':int(fields['VmRSS'].split()[0]),'threads':int(fields['Threads']),'fds':len(list(Path('/proc',str(compositor.pid),'fd').iterdir()))};r['memory'].append(value);return value
 def fake():
  try:
   listener.settimeout(.1)
   while not stop.is_set():
    try:conn,_=listener.accept();break
    except socket.timeout:continue
   else:return
   with conn:
    conn.settimeout(.05);data=b''
    while b'\r\n' not in data:
     try:part=conn.recv(1024)
     except socket.timeout:
      if stop.is_set():return
      continue
     assert part;data+=part
    assert data.startswith(b'\0AUTH EXTERNAL ');r['server']['accepted_monotonic']=time.monotonic();r['server']['received_auth']=True
    if a.fault in ['hello','oversize','oversize-be','fields']:
     conn.sendall(b'OK 0123456789abcdef0123456789abcdef\r\n');data=b''
     while b'Hello' not in data:
      try:part=conn.recv(4096)
      except socket.timeout:
       if stop.is_set():return
       continue
      assert part;data+=part
     assert b'BEGIN\r\n' in data and b'NEGOTIATE_UNIX_FD\r\n' in data;conn.sendall(b'AGREE_UNIX_FD\r\n');r['server']['received_hello']=True
     if a.fault!='hello':
      body,fields=(0,32*1024*1024) if a.fault=='fields' else (64*1024*1024,0)
      endian='>' if a.fault=='oversize-be' else '<';header=(b'B' if endian=='>' else b'l')+b'\x02\x01\x01'+struct.pack(endian+'III',body,1,fields)
      if a.fragment_header:
       for byte in header:conn.sendall(bytes([byte]));time.sleep(.003)
      else:conn.sendall(header)
      r['server'].update(announced_body_bytes=body,announced_fields_bytes=fields,fragmented_header=a.fragment_header)

    elif a.fault=='auth-fds':
     fd=os.open('/dev/null',os.O_RDONLY|os.O_CLOEXEC)
     try:conn.sendmsg([b'x'],[(socket.SOL_SOCKET,socket.SCM_RIGHTS,array.array('i',[fd]*64))])
     finally:os.close(fd)
     r['server']['auth_fds_offered']=64
    elif a.fault=='auth-flood':
     try:conn.sendall(b'x'*65536)
     except (BrokenPipeError,ConnectionResetError):pass
     r['server']['auth_bytes_offered']=65536
    r['server']['injected_monotonic']=time.monotonic();injected.set()
    while not stop.is_set():
     try:part=conn.recv(4096)
     except socket.timeout:continue
     except ConnectionResetError:part=b''
     if not part:r['server']['peer_closed_monotonic']=time.monotonic();eof.set();return
  except BaseException as error:r['server']['error']=repr(error);injected.set()
 try:
  listener=socket.socket(socket.AF_UNIX);listener.bind(str(work/'fake'));listener.listen(4)
  if a.fault=='backlog':
   listener.listen(0);filler=socket.socket(socket.AF_UNIX);filler.connect(str(work/'fake'))
  else:server=threading.Thread(target=fake);server.start()
  read,write=os.pipe()
  try:
   xvfb=subprocess.Popen([a.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1600x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT);children.append(xvfb);os.close(write);write=None;display=read_display_number(read)
  finally:
   os.close(read)
   if write is not None:os.close(write)
  real='unix:path='+str(work/'real');bus=subprocess.Popen(['dbus-daemon','--config-file='+str(root/'tests/native_menu_session.conf'),'--address='+real,'--nofork'],stdout=log('bus.log'),stderr=subprocess.STDOUT);children.append(bus);await_value(lambda:(work/'real').exists() or bus.poll() is not None,bool);assert bus.poll() is None
  env=dict(os.environ,DISPLAY=':'+display,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work/'runtime'),XDG_CONFIG_HOME=str(work/'config'),XDG_CONFIG_DIRS=str(work/'empty'),XDG_DATA_HOME=str(work/'data'),XDG_DATA_DIRS=str(work/'empty'),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',DBUS_SESSION_BUS_ADDRESS='unix:path='+str(work/'fake'))
  env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
  compositor=subprocess.Popen([str(a.compositor.resolve()),'--width','360','--height','720','--control-socket',str(control),'--exit-after-ms','60000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(compositor);await_value(lambda:control.exists() or compositor.poll() is not None,bool);assert compositor.poll() is None and sha(Path('/proc',str(compositor.pid),'exe'))==r['compositor_sha256']
  ready=state('ready');mouse=PrivateInput(compositor.pid,xvfb,env['DISPLAY']);await_value(mouse.find,bool)
  native=dict(env,DBUS_SESSION_BUS_ADDRESS=real,WAYLAND_DISPLAY=ready['wayland_display']);native.pop('DISPLAY',None)
  fixture=subprocess.Popen([str(a.producer.resolve()),'--qa-stdin','--no-claim','--report',str(out/'fixture.jsonl')],env=native,stdin=subprocess.PIPE,text=True,stdout=log('fixture.log'),stderr=subprocess.STDOUT);children.append(fixture)
  state('mapped',lambda s:len(s['cards'])==1 and isinstance(s['view'],dict) and s['presentation']['windows'][0]['opacity']==1);time.sleep(.4);baseline=memory();command('claim-self')
  if a.fault=='backlog':r['server']['injected_monotonic']=time.monotonic();injected.set()
  assert injected.wait(timeout=4) and 'error' not in r['server'],r['server']
  if a.cancel:
   state('loading',lambda s:s['app_menu']['status']=='Loading');started=time.monotonic();click(180,702);state('retired',lambda s:s['view']=='Overview' and s['app_menu']['status']=='Absent');limit=1.0
  else:started=r['server']['injected_monotonic'];limit=3.0 if a.fault in ['auth','hello','backlog'] else 1.0
  deadline=started+limit
  while not eof.is_set() and time.monotonic()<deadline:
   memory()
   if a.fault=='backlog' and not any(p.read_text().strip()=='tarot-app-menu' for p in Path('/proc',str(compositor.pid),'task').glob('*/comm')):
    r['server']['worker_retired_monotonic']=time.monotonic();eof.set()
   time.sleep(.01)
  assert eof.is_set(),'Menu setup retained the live stalled socket beyond the required bound'
  r['retirement_seconds']=r['server']['worker_retired_monotonic' if a.fault=='backlog' else 'peer_closed_monotonic']-started
  if not a.cancel:state('failed-unavailable',lambda s:s['app_menu']['status']=='Unavailable')
  memory();r['rss_growth_kib']=max(v['rss_kib'] for v in r['memory'])-baseline['rss_kib'];assert r['rss_growth_kib']<32*1024
  if filler is not None:filler.close();filler=None
  listener.close();listener=None;(work/'fake').unlink();os.rename(work/'real',work/'fake')
  if a.cancel:
   state('overview-settled',lambda s:s['presentation']['overview']==1);click(180,360);state('foreground-returned',lambda s:isinstance(s['view'],dict))
  command('claim-self');state('recovered',lambda s:s['app_menu']['status']=='Available');click(40,16);state('menu-root',lambda s:s['system_menu_open']);row('Preferences','preferences');state('submenu',lambda s:isinstance(s['top_menu']['panel'],dict));row('Choose','choose')
  await_value(events,lambda es:len([e for e in es if e['event']=='action'])==1);r['callbacks']=[e for e in events() if e['event']=='action'];assert r['callbacks']==[{'event':'action','name':'choose','target':'night'}]
  r['passed']=True;print('PASS',a.fault,'cancel' if a.cancel else 'deadline/bound','socket retired; fresh menu and real callback recover')
 finally:
  if mouse is not None:mouse.close()
  for child in reversed(children):
   if child.poll() is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait()
  stop.set()
  if server is not None:server.join(timeout=3);assert not server.is_alive()
  if listener is not None:listener.close()
  if filler is not None:filler.close()
  for f in logs:f.close()
  (out/'validation.json').write_text(json.dumps(r,indent=2)+'\n')
