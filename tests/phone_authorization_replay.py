"""Real authority acceptance in an ordinary-user Tarot graphical session.

The controller installs a temporary auth_self policy with no privileged
mechanism. Real credentials stay in memory and go through the guarded on-screen
keyboard, never the diagnostic input protocol. Root only owns QA input/process
observation and fault injection against an independently verified helper child.
"""
import hashlib,json,os,signal,time
from pathlib import Path
from PIL import Image
from dialog_button import locate
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from lock_replay import any_request


def replay(socket,screen,tap,state,capture,before,directory,cgroup,authenticate,password):
 folder=directory/'authorization';assert folder.is_dir();commands=folder/'authority.commands';report=folder/'authority.jsonl';sequence=0
 r={'passed':False,'real_authority':True,'request_uid':10000,'privileged_mechanism':False,'targets':[],'results':[]}
 pid=before['pid'];proc=Path('/proc',str(pid));start=(proc/'stat').read_text().rsplit(')',1)[1].split()[19];helper=Path('/usr/lib/polkit-1/polkit-agent-helper-1');helper_sha=hashlib.sha256(helper.read_bytes()).hexdigest();active=None;owned_helper=None
 original_guard=screen.guard
 def events(name):
  return [e for e in (json.loads(line) for line in report.read_bytes().split(b'\n')[:-1]) if e['event']==name] if report.exists() else []
 def command(name,**fields):
  nonlocal sequence
  sequence+=1
  with commands.open('a') as f:f.write(json.dumps(dict(command=name,request=sequence,**fields))+'\n')
  await_value(lambda:events('command'),lambda es:any(e['request']==sequence for e in es),seconds=12)
 def result(case,authorized):
  found=await_value(lambda:events('result'),lambda es:any(e['case']==case for e in es),seconds=25);v=next(e for e in found if e['case']==case);assert v['authorized'] is authorized,v;r['results'].append(v);return v
 def idle(label):
  s=state('authorization-'+label,lambda s:s['lock']['phase']=='Unlocked' and s['authorization']['request'] is None and not s['system_menu_open'])
  assert s['cards']==before['cards'] and s['view']==before['view'];return s
 def question(case):
  command('start',case=case)
  s=state('authorization-'+case+'-question',lambda s:s['lock']['phase']=='Unlocked' and s['system_menu_open'] and s['system_page']=='Authorization' and s['keyboard_visible'] and s['authorization']['request'] is not None and s['authorization']['request']['phase']=='Question',seconds=25)
  q=s['authorization']['request'];assert q['echo'] is False and s['output']['orientation']=='normal' and s['cards']==before['cards'];return q
 def entry_guard():
  original_guard()
  assert proc.stat().st_uid==10000 and (proc/'stat').read_text().rsplit(')',1)[1].split()[19]==start and (proc/'cgroup').read_text()==cgroup
  if active is not None:
   s=request(socket,'status');assert s['pid']==pid and s['lock']['phase']=='Unlocked'
   q=s['authorization']['request'];assert q is not None and (q['id'],q['turn'])==active and q['phase']=='Question' and q['echo'] is False
   assert s['system_menu_open'] and s['system_page']=='Authorization' and s['keyboard_visible']
   g=s['output'];assert g['orientation']=='normal' and g['requested'] is None and g['presented_revision']==g['revision']
 def type_response(q,value):
  nonlocal active
  active=(q['id'],q['turn']);screen.guard=entry_guard
  try:Keyboard(tap).type_ascii(value)
  finally:screen.guard=original_guard;active=None
 def word(label,text):
  time.sleep(.2);name='authorization-'+label;capture(name);path=directory/(name+'.png')
  with Image.open(path) as im:assert im.size==(720,1440)
  target=locate(path,text,vertical=(.07,.65),scale=1);r['targets'].append({'label':label,'target':target});tap(target['point'][0]/2,target['point'][1]/2)
 def live_helper():
  found=[]
  for p in Path('/proc').iterdir():
   if not p.name.isdigit():continue
   try:
    if os.readlink(p/'exe')!=str(helper):continue
    status=dict(line.split(':',1) for line in (p/'status').read_text().splitlines() if ':' in line)
    uids=list(map(int,status['Uid'].split()))
    if int(status['PPid'])==pid and uids[:2]==[10000,0] and (p/'cgroup').read_text()==cgroup:
     assert hashlib.sha256((p/'exe').read_bytes()).hexdigest()==helper_sha
     found.append((int(p.name),(p/'stat').read_text().rsplit(')',1)[1].split()[19]))
   except (FileNotFoundError,ProcessLookupError):pass
  assert len(found)==1,found
  ident=found[0];fd=os.pidfd_open(ident[0]);p=Path('/proc',str(ident[0]));assert (p/'stat').read_text().rsplit(')',1)[1].split()[19]==ident[1]
  return fd,ident
 def helper_gone(ident):
  p=Path('/proc',str(ident[0]));return not p.exists() or (p/'stat').read_text().rsplit(')',1)[1].split()[19]!=ident[1]
 try:
  state('authorization-agent-ready',lambda s:s['authorization']['available'])
  request(socket,'launch',id='test.tarot.Authorization.desktop')
  ready=await_value(lambda:events('ready'),bool,seconds=15)[0];assert ready['uid']==10000 and ready['cgroup']==cgroup and ready['defaults']==[0,0,1];r['requester']=ready
  q=question('correct');capture('authorization-reason');type_response(q,password);word('correct-submit','Authenticate');result('correct',True);idle('correct-restores-origin')
  q=question('wrong');type_response(q,'definitely-wrong-fixture-password');word('wrong-submit','Authenticate');result('wrong',False)
  state('authorization-rejection-visible',lambda s:s['authorization']['request'] is None and s['authorization']['error'] is not None and not s['keyboard_visible'] and s['system_menu_open']);word('rejection-close','Close');idle('wrong-cleared')
  q=question('cancel');type_response(q,'unsubmitted');word('cancel','Cancel');result('cancel',False);idle('cancel-cleared')
  q=question('requester-cancel');type_response(q,'unsubmitted');command('cancel',case='requester-cancel');result('requester-cancel',False);idle('requester-cancel-cleared')
  q=question('helper-killed');owned_helper=live_helper();signal.pidfd_send_signal(owned_helper[0],signal.SIGKILL);result('helper-killed',False);await_value(lambda:helper_gone(owned_helper[1]),bool);os.close(owned_helper[0]);owned_helper=None
  state('authorization-helper-failure-visible',lambda s:s['authorization']['request'] is None and s['authorization']['error'] is not None and not s['keyboard_visible']);word('helper-failure-close','Close');idle('helper-failure-cleared')
  q=question('lock-unsubmitted');type_response(q,'unsubmitted');request(socket,'lock');locked=state('authorization-protected',lambda s:s['lock']['phase']=='Locked');assert set(locked)=={'ok','pid','lock'};result('lock-unsubmitted',False)
  assert not any_request(socket,'authorization_action',action={'Cancel':{'request':q['id']}})['ok']
  command('start',case='while-locked');result('while-locked',False);authenticate('authorization-pam');idle('after-lock-retired')
  # Freeze the verified helper before valid entry is submitted. Locking must
  # cancel/kill it without allowing the queued response to reach PAM afterward.
  q=question('lock-checking');owned_helper=live_helper();signal.pidfd_send_signal(owned_helper[0],signal.SIGSTOP)
  helper_proc=Path('/proc',str(owned_helper[1][0]));await_value(lambda:(helper_proc/'status').read_text(),lambda t:'State:\tT' in t)
  type_response(q,password);word('paused-submit','Authenticate');state('authorization-checking',lambda s:s['authorization']['request'] is not None and s['authorization']['request']['phase']=='Checking')
  request(socket,'lock');state('authorization-checking-protected',lambda s:s['lock']['phase']=='Locked');result('lock-checking',False);await_value(lambda:helper_gone(owned_helper[1]),bool);os.close(owned_helper[0]);owned_helper=None
  authenticate('authorization-checking-pam');idle('checking-retired')
  q=question('fresh-after-lock');type_response(q,password);word('fresh-submit','Authenticate');result('fresh-after-lock',True);idle('fresh-restores-origin')
  command('quit');r['helper_sha256']=helper_sha;r['actual_pam']=True;r['protected_commands_denied']=True;r['passed']=True;return r
 finally:
  screen.guard=original_guard;active=None
  if owned_helper is not None:
   try:signal.pidfd_send_signal(owned_helper[0],signal.SIGKILL)
   except ProcessLookupError:pass
   os.close(owned_helper[0])
