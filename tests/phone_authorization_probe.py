#!/usr/bin/env python3
"""Ordinary-user, real-system Polkit requester; never handles a password/cookie.

Launch from Tarot's normal application mechanism so the requesting connection
belongs to its actual graphical PAM session. The temporary action has auth_self
policy and no privileged mechanism; this probe only observes authority decisions.
"""
import argparse,json,os,time,uuid
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib

ACTION='org.tarot.Qa.Authentication'
AUTH='org.freedesktop.PolicyKit1';OBJECT='/org/freedesktop/PolicyKit1/Authority';IFACE=AUTH+'.Authority'

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--directory',type=Path,required=True);a=p.parse_args()
 assert os.getuid()==os.geteuid()==10000
 report=(a.directory/'authority.jsonl').open('x',buffering=1);commands=a.directory/'authority.commands';assert commands.is_file()
 def emit(event,**fields):report.write(json.dumps(dict(event=event,**fields))+'\n')
 bus=Gio.bus_get_sync(Gio.BusType.SYSTEM,None);bus.set_exit_on_close(False);loop=GLib.MainLoop();consumed=0;pending={}
 actions=bus.call_sync(AUTH,OBJECT,IFACE,'EnumerateActions',GLib.Variant('(s)',('C',)),None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
 action=next(v for v in actions if v[0]==ACTION)
 # No cached/retained authorization: only an active local user may authenticate.
 assert list(action[6:9])==[0,0,1],action[6:9]
 emit('ready',pid=os.getpid(),uid=os.geteuid(),cgroup=Path('/proc/self/cgroup').read_text(),unique=bus.get_unique_name(),action=ACTION,defaults=list(action[6:9]))
 def command(v):
  name=v['command']
  if name=='start':
   case=v['case'];assert case not in pending;token=uuid.uuid4().hex;pending[case]=token
   def finished(c,result,*unused):
    try:
     authorized,challenge,details=c.call_finish(result).unpack()[0]
     emit('result',case=case,authorized=authorized,challenge=challenge,error=None)
    except GLib.Error as error:emit('result',case=case,authorized=False,challenge=False,error=Gio.DBusError.get_remote_error(error))
    pending.pop(case,None)
   subject=('system-bus-name',{'name':GLib.Variant('s',bus.get_unique_name())})
   bus.call(AUTH,OBJECT,IFACE,'CheckAuthorization',GLib.Variant('((sa{sv})sa{ss}us)',(subject,ACTION,{},1,token)),None,Gio.DBusCallFlags.NONE,190000,None,finished,None)
  elif name=='cancel':
   token=pending.get(v['case'])
   if token:bus.call_sync(AUTH,OBJECT,IFACE,'CancelCheckAuthorization',GLib.Variant('(s)',(token,)),None,Gio.DBusCallFlags.NONE,5000,None)
  elif name=='quit':
   assert not pending;loop.quit()
  else:raise AssertionError(name)
  emit('command',name=name,request=v['request'])
 def poll():
  nonlocal consumed
  try:
   for line in commands.read_text().splitlines(keepends=True)[consumed:]:
    if not line.endswith('\n'):break
    command(json.loads(line));consumed+=1
   return True
  except BaseException as error:
   emit('failed',kind=type(error).__name__);loop.quit();return False
 GLib.timeout_add(40,poll);GLib.timeout_add_seconds(300,lambda:(emit('timeout'),loop.quit(),False)[-1]);loop.run();bus.close_sync(None);report.close()

if __name__=='__main__':main()
