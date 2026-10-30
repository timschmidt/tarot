#!/usr/bin/env python3
"""Synthetic authority/login1 and nonprivileged helper on one private D-Bus.

A synthetic grant proves only the fixture protocol, never real system privilege.
"""
import argparse,json,os,uuid
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--report',type=Path,required=True);p.add_argument('--commands',type=Path,required=True);p.add_argument('--helper',type=Path,required=True);a=p.parse_args()
address=os.environ['DBUS_SESSION_BUS_ADDRESS'];assert address.startswith('unix:') and '/run/dbus/system_bus_socket' not in address
report=a.report.open('x',buffering=1)
def emit(event,**fields):report.write(json.dumps(dict(event=event,**fields))+'\n')
AUTH='org.freedesktop.PolicyKit1';AUTH_PATH='/org/freedesktop/PolicyKit1/Authority';AUTH_IFACE=AUTH+'.Authority';AGENT=AUTH+'.AuthenticationAgent';LOGIN='org.freedesktop.login1';SESSION='/org/freedesktop/login1/session/fixture';SI=LOGIN+'.Session';uid=os.geteuid()
xml='''<node>
<interface name="org.freedesktop.PolicyKit1.Authority"><method name="CheckAuthorization"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/><arg type="a{ss}" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="(bba{ss})" direction="out"/></method><method name="CancelCheckAuthorization"><arg type="s" direction="in"/></method><method name="RegisterAuthenticationAgent"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/></method><method name="UnregisterAuthenticationAgent"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/></method><method name="AuthenticationAgentResponse2"><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="(sa{sv})" direction="in"/></method></interface>
<interface name="org.freedesktop.login1.Manager"><method name="GetSessionByPID"><arg type="u" direction="in"/><arg type="o" direction="out"/></method></interface>
<interface name="org.freedesktop.login1.Session"><property name="Id" type="s" access="read"/><property name="User" type="(uo)" access="read"/><property name="Active" type="b" access="read"/><property name="Remote" type="b" access="read"/><property name="Type" type="s" access="read"/><property name="LockedHint" type="b" access="read"/></interface></node>'''
info={i.name:i for i in Gio.DBusNodeInfo.new_for_xml(xml).interfaces};props={'Id':GLib.Variant('s','fixture-1'),'User':GLib.Variant('(uo)',(uid,'/org/freedesktop/login1/user/_'+str(uid))),'Active':GLib.Variant('b',True),'Remote':GLib.Variant('b',False),'Type':GLib.Variant('s','wayland'),'LockedHint':GLib.Variant('b',False)}
loop=GLib.MainLoop();bus=None;registered=None;pending={};last_cookie=None;previous_cookie=None;consumed=0;check_count=0;checks={};mode='password';mode_path=a.helper.with_suffix('.mode');helper_report=a.helper.with_suffix('.jsonl')
helper_code='''#!/usr/bin/python3
import json,os,sys,time,pwd
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
address=ADDRESS
mode=json.loads(Path(__file__).with_suffix('.mode').read_text())
report=Path(__file__).with_suffix('.jsonl')
def emit(event,**fields):
 with report.open('a') as f:f.write(json.dumps(dict(event=event,**fields))+'\\n')
cookie=sys.stdin.readline().removesuffix('\\n');emit('started',cookie_supplied=bool(cookie),argv_count=len(sys.argv),euid=os.geteuid())
if mode=='oversized':print('X'*9000,flush=True);time.sleep(30);raise SystemExit(1)
if mode=='malformed':print('SUCCESS extra',flush=True);raise SystemExit(1)
if mode=='hang':time.sleep(30);raise SystemExit(1)
print('PAM_TEXT_INFO Fixture authentication',flush=True)
print(('PAM_PROMPT_ECHO_ON Code:' if mode=='echo' else 'PAM_PROMPT_ECHO_OFF Password:'),flush=True)
reply=sys.stdin.readline();ok=reply=='synthetic-secret\\n';emit('response',turn=1,matches=ok)
if mode=='two' and ok:
 print('PAM_PROMPT_ECHO_ON Token:',flush=True);reply=sys.stdin.readline();ok=reply=='002345\\n';emit('response',turn=2,matches=ok)
if ok:
 try:
  b=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
  b.call_sync('org.freedesktop.PolicyKit1','/org/freedesktop/PolicyKit1/Authority','org.freedesktop.PolicyKit1.Authority','AuthenticationAgentResponse2',GLib.Variant('(us(sa{sv}))',(os.getuid(),cookie,('unix-user',{'uid':GLib.Variant('u',IDENTITY_UID)}))),None,Gio.DBusCallFlags.NONE,5000,None)
  b.close_sync(None)
 except GLib.Error:ok=False
if not ok:print('PAM_ERROR_MSG Authentication rejected',flush=True)
print('SUCCESS' if ok else 'FAILURE',flush=True);raise SystemExit(0 if ok else 1)
'''.replace('ADDRESS',repr(address)).replace('IDENTITY_UID','pwd.getpwnam(sys.argv[1]).pw_uid')
a.helper.write_text(helper_code);a.helper.chmod(0o700);mode_path.write_text(json.dumps(mode))
def success(inv):inv.return_value(GLib.Variant('()',()))
def fail(inv,name=AUTH+'.Error.Failed'):inv.return_dbus_error(name,'Synthetic fixture result')
def getprop(c,sender,path,interface,name):return props.get(name)
def method(c,sender,path,interface,name,parameters,inv):
 global registered,check_count
 values=parameters.unpack()
 if name=='CheckAuthorization':
  subject,action,details,flags,cancellation=values
  if registered is None or not flags&1:
   inv.return_value(GLib.Variant('((bba{ss}))',((False,True,{}),)));return
  expected=bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetConnectionUnixProcessID',GLib.Variant('(s)',(registered[0],)),None,Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
  if subject[0]=='unix-process':
   process=subject[1].get('pid');process_uid=subject[1].get('uid');start_time=subject[1].get('start-time')
   valid=process==expected and process_uid==uid and start_time==int(Path('/proc',str(expected),'stat').read_text().rsplit(')',1)[1].split()[19])
  else:valid=False
  if not valid:fail(inv);return
  check_count+=1;case='mechanism-'+str(check_count);key=(sender,cancellation)
  emit('check',case=case,action=action,subject_pid=expected,subject_kind=subject[0],interaction=bool(flags&1))
  def complete(ok,granted,error):
   checks.pop(key,None)
   authorized=ok and granted and props['Active'].unpack() and not props['LockedHint'].unpack()
   inv.return_value(GLib.Variant('((bba{ss}))',((authorized,False,{}),)))
  checks[key]=start(case,action=action,completed=complete);return
 if name=='CancelCheckAuthorization':
  cookie=checks.get((sender,values[0]))
  if cookie:call_agent(bus,'CancelAuthentication',GLib.Variant('(s)',(cookie,)),lambda ok,error:None)
  success(inv);return
 if name=='GetSessionByPID':emit('session-query',pid=values[0]);inv.return_value(GLib.Variant('(o)',(SESSION,)));return
 if name=='RegisterAuthenticationAgent':
  subject,locale,path=values
  assert subject==('unix-session',{'session-id':props['Id'].unpack()}) and locale=='C.UTF-8'
  if registered is not None:fail(inv);return
  registered=(sender,path);emit('registered',owner=sender,subject=subject);success(inv);return
 if name=='UnregisterAuthenticationAgent':
  if registered is not None and sender==registered[0]:registered=None
  emit('unregistered');success(inv);return
 if name=='AuthenticationAgentResponse2':
  client,cookie,identity=values;entry=pending.get(cookie)
  if client!=uid or entry is None or entry.get('revoked',False) or identity[0]!='unix-user' or identity[1].get('uid') not in entry['uids']:fail(inv);return
  entry['granted']=True;emit('helper-grant',case=entry['case']);success(inv);return
 fail(inv)
def owner_lost(c,sender,path,interface,signal,parameters,*unused):
 global registered
 name,old,new=parameters.unpack()
 if registered and name==registered[0] and not new:registered=None;emit('agent-gone')
 if not new:
  for key,cookie in list(checks.items()):
   if key[0]==name:
    if cookie in pending:pending[cookie]['revoked']=True
    if registered is not None:call_agent(bus,'CancelAuthentication',GLib.Variant('(s)',(cookie,)),lambda ok,error:None)
    emit('mechanism-gone')

def connect():
 global bus,registered
 bus=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None);bus.set_exit_on_close(False);registered=None
 for name in [AUTH,LOGIN]:
  r=bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',GLib.Variant('(su)',(name,4)),None,Gio.DBusCallFlags.NONE,5000,None);assert r.unpack()==(1,)
 for path,interface in [(AUTH_PATH,AUTH_IFACE),('/org/freedesktop/login1',LOGIN+'.Manager'),(SESSION,SI)]:bus.register_object(path,info[interface],method,getprop if interface==SI else None,None)
 bus.signal_subscribe('org.freedesktop.DBus','org.freedesktop.DBus','NameOwnerChanged','/org/freedesktop/DBus',None,Gio.DBusSignalFlags.NONE,owner_lost,None)
 emit('ready',owner=bus.get_unique_name())
def call_agent(connection,name,arguments,callback):
 assert registered is not None
 def finished(c,result,*unused):
  try:c.call_finish(result);callback(True,None)
  except GLib.Error as error:callback(False,Gio.DBusError.get_remote_error(error))
 connection.call(registered[0],registered[1],AGENT,name,arguments,None,Gio.DBusCallFlags.NONE,190000,None,finished,None)
def start(case,spoof=False,identities=None,action=None,completed=None):
 global last_cookie,previous_cookie
 previous_cookie=last_cookie;last_cookie=uuid.uuid4().hex;cookie=last_cookie;uids=identities if identities is not None else [uid]
 pending[cookie]={'case':case,'granted':False,'uids':uids}
 connection=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None) if spoof else bus
 def finished(ok,error):
  entry=pending.pop(cookie,{'granted':False});emit('result',case=case,agent_reply=ok,granted=entry['granted'],error=error)
  if completed is not None:completed(ok,entry['granted'],error)
  if spoof:connection.close_sync(None)
 arguments=GLib.Variant('(sssa{ss}sa(sa{sv}))',(action or 'org.tarot.Fixture','Fixture action needs authentication','',{},cookie,[('unix-user',{'uid':GLib.Variant('u',u)}) for u in uids]))
 call_agent(connection,'BeginAuthentication',arguments,finished)
 return cookie
def changed(values):
 props.update(values);bus.emit_signal(None,SESSION,'org.freedesktop.DBus.Properties','PropertiesChanged',GLib.Variant('(sa{sv}as)',(SI,values,[])))
def command(value):
 global mode
 name=value['command']
 if name=='start':start(value['case'],value.get('spoof',False),value.get('identities'))
 elif name=='mode':mode=value['value'];mode_path.write_text(json.dumps(mode))
 elif name=='cancel':
  cookie=previous_cookie if value.get('previous') else last_cookie
  call_agent(bus,'CancelAuthentication',GLib.Variant('(s)',(cookie,)),lambda ok,error:emit('canceled',accepted=ok))
 elif name=='locked-hint':changed({'LockedHint':GLib.Variant('b',value['value'])})
 elif name=='active':changed({'Active':GLib.Variant('b',value['value'])})
 elif name=='rapid-inactive':changed({'Active':GLib.Variant('b',False)});changed({'Active':GLib.Variant('b',True)})
 elif name=='session-replace':changed({'Id':GLib.Variant('s','fixture-'+uuid.uuid4().hex)})
 elif name=='replace-owner':pending.clear();checks.clear();bus.close_sync(None);connect()
 elif name=='quit':loop.quit()
 else:raise AssertionError(name)
 emit('command',name=name,request=value['request'])
def poll():
 global consumed
 for line in a.commands.read_text().splitlines(keepends=True)[consumed:]:
  if not line.endswith('\n'):break
  command(json.loads(line));consumed+=1
 return True
connect();GLib.timeout_add(30,poll);GLib.timeout_add_seconds(300,lambda:(loop.quit(),False)[1]);loop.run();bus.close_sync(None);report.close()
