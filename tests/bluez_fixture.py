#!/usr/bin/env python3
"""Synthetic BlueZ API on an explicitly supplied private session bus. No radio I/O."""
import argparse,json,os,time
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--report',type=Path,required=True);p.add_argument('--commands',type=Path,required=True);a=p.parse_args()
address=os.environ['DBUS_SESSION_BUS_ADDRESS'];assert '/run/dbus/system_bus_socket' not in address
report=a.report.open('x',buffering=1)
def emit(event,**fields):report.write(json.dumps(dict(event=event,**fields))+'\n')
ADAPTER='/org/bluez/hci0';DEVICE=ADAPTER+'/dev_00_00_00_00_00_01';AI='org.bluez.Adapter1';DI='org.bluez.Device1'
xml='''<node>
<interface name="org.freedesktop.DBus.ObjectManager"><method name="GetManagedObjects"><arg type="a{oa{sa{sv}}}" direction="out"/></method><signal name="InterfacesAdded"><arg type="o"/><arg type="a{sa{sv}}"/></signal><signal name="InterfacesRemoved"><arg type="o"/><arg type="as"/></signal></interface>
<interface name="org.bluez.AgentManager1"><method name="RegisterAgent"><arg type="o" direction="in"/><arg type="s" direction="in"/></method><method name="RequestDefaultAgent"><arg type="o" direction="in"/></method><method name="UnregisterAgent"><arg type="o" direction="in"/></method></interface>
<interface name="org.bluez.Adapter1"><method name="StartDiscovery"/><method name="StopDiscovery"/><method name="RemoveDevice"><arg type="o" direction="in"/></method><property name="Alias" type="s" access="read"/><property name="Powered" type="b" access="readwrite"/><property name="Discovering" type="b" access="read"/><property name="Pairable" type="b" access="readwrite"/><property name="Discoverable" type="b" access="readwrite"/><property name="PairableTimeout" type="u" access="readwrite"/><property name="DiscoverableTimeout" type="u" access="readwrite"/></interface>
<interface name="org.bluez.Device1"><method name="Pair"/><method name="CancelPairing"/><method name="Connect"/><method name="Disconnect"/><property name="Adapter" type="o" access="read"/><property name="Alias" type="s" access="read"/><property name="Paired" type="b" access="read"/><property name="Connected" type="b" access="read"/><property name="Trusted" type="b" access="readwrite"/><property name="Blocked" type="b" access="readwrite"/></interface>
</node>'''
info={i.name:i for i in Gio.DBusNodeInfo.new_for_xml(xml).interfaces}
props={ADAPTER:{'Alias':GLib.Variant('s','Fixture adapter'),'Powered':GLib.Variant('b',True),'Discovering':GLib.Variant('b',False),'Pairable':GLib.Variant('b',False),'Discoverable':GLib.Variant('b',False),'PairableTimeout':GLib.Variant('u',0),'DiscoverableTimeout':GLib.Variant('u',180)},DEVICE:{'Adapter':GLib.Variant('o',ADAPTER),'Alias':GLib.Variant('s','Fixture keyboard'),'Paired':GLib.Variant('b',False),'Connected':GLib.Variant('b',False),'Trusted':GLib.Variant('b',False),'Blocked':GLib.Variant('b',False)}}
loop=GLib.MainLoop();bus=None;registrations={};agents={};default_agent=None;pending=None;mode='confirm';sessions=set();consumed=0;managed=0;foreign=None;deny_stop=False

def changed(path,values):
 props[path].update(values);bus.emit_signal(None,path,'org.freedesktop.DBus.Properties','PropertiesChanged',GLib.Variant('(sa{sv}as)',(AI if path==ADAPTER else DI,values,[])))
def success(inv):inv.return_value(GLib.Variant('()',()))
def failure(inv,name='org.bluez.Error.Failed'):inv.return_dbus_error(name,'Synthetic fixture result')
def getprop(c,sender,path,interface,name):return props.get(path,{}).get(name)
def setprop(c,sender,path,interface,name,value):
 changed(path,{name:value});emit('set',path=path,property=name,value=value.unpack());return True

def agent_call(method,args,reply,callback,sender=None):
 destination,path=agents.get(sender) if sender in agents else default_agent
 def finished(connection,result,*unused):
  try:answer=connection.call_finish(result);callback(True,answer.unpack() if answer is not None else ())
  except GLib.Error:callback(False,())
 bus.call(destination,path,'org.bluez.Agent1',method,args,GLib.VariantType.new(reply),Gio.DBusCallFlags.NONE,65000,None,finished,None)
def pair_answer(invocation,connection,ok,value):
 global pending
 emit('pair-reply',accepted=ok,answer=value)
 if pending is not invocation or connection is not bus:return
 current=pending;pending=None
 if ok:changed(DEVICE,{'Paired':GLib.Variant('b',True)});success(current)
 else:failure(current,'org.bluez.Error.AuthenticationRejected')

def method(c,sender,path,interface,name,parameters,inv):
 global default_agent,pending,managed
 values=parameters.unpack()
 if interface=='org.freedesktop.DBus.ObjectManager':
  managed+=1
  objects={ADAPTER:{AI:props[ADAPTER]}}
  if DEVICE in props:objects[DEVICE]={DI:props[DEVICE]}
  inv.return_value(GLib.Variant('(a{oa{sa{sv}}})',(objects,)));return
 if interface=='org.bluez.AgentManager1':
  if name=='RegisterAgent':
   if sender in agents:failure(inv,'org.bluez.Error.AlreadyExists');return
   assert values[1]=='KeyboardDisplay';agents[sender]=(sender,values[0]);emit('agent-registered',owner=sender)
  elif name=='RequestDefaultAgent':default_agent=agents[sender];emit('default-agent',owner=sender)
  elif name=='UnregisterAgent':agents.pop(sender,None)
  success(inv);return
 emit('method',name=name,path=path,sender=sender)
 if name=='StartDiscovery':sessions.add(sender);changed(ADAPTER,{'Discovering':GLib.Variant('b',bool(sessions))});success(inv)
 elif name=='StopDiscovery':
  if deny_stop:failure(inv,'org.bluez.Error.NotAuthorized');return
  sessions.discard(sender);changed(ADAPTER,{'Discovering':GLib.Variant('b',bool(sessions))});success(inv)
 elif name=='RemoveDevice':
  if DEVICE in props:remove_device()
  success(inv)
 elif name in ['Connect','Disconnect']:
  changed(DEVICE,{'Connected':GLib.Variant('b',name=='Connect')});success(inv)
 elif name=='CancelPairing':
  current=pending;pending=None
  if current is not None:failure(current,'org.bluez.Error.AuthenticationCanceled')
  if default_agent is not None:agent_call('Cancel',None,'()',lambda ok,value:emit('agent-cancel',accepted=ok))
  success(inv)
 elif name=='Pair':
  if pending is not None:failure(inv,'org.bluez.Error.InProgress');return
  pending=inv
  connection=bus
  reply=lambda ok,value:pair_answer(inv,connection,ok,value)
  if mode=='confirm':agent_call('RequestConfirmation',GLib.Variant('(ou)',(DEVICE,123)),'()',reply,sender)
  elif mode=='pin':agent_call('RequestPinCode',GLib.Variant('(o)',(DEVICE,)),'(s)',reply,sender)
  elif mode=='passkey':agent_call('RequestPasskey',GLib.Variant('(o)',(DEVICE,)),'(u)',reply,sender)
  elif mode=='display':agent_call('DisplayPasskey',GLib.Variant('(ouq)',(DEVICE,123,0)),'()',lambda ok,value:emit('display-reply',accepted=ok),sender)
  else:failure(inv);pending=None
 else:failure(inv)

def register(path,interface):
 registrations[(path,interface)]=bus.register_object(path,info[interface],method,getprop if interface in [AI,DI] else None,setprop if interface in [AI,DI] else None)
def connect():
 global bus,registrations,agents,default_agent
 bus=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None);bus.set_exit_on_close(False)
 result=bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',GLib.Variant('(su)',('org.bluez',4)),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,5000,None);assert result.unpack()==(1,)
 registrations={};agents={};default_agent=None
 register('/','org.freedesktop.DBus.ObjectManager');register('/org/bluez','org.bluez.AgentManager1');register(ADAPTER,AI)
 if DEVICE in props:register(DEVICE,DI)
 emit('ready',owner=bus.get_unique_name())
def remove_device():
 props.pop(DEVICE,None);bus.unregister_object(registrations.pop((DEVICE,DI)));bus.emit_signal(None,'/','org.freedesktop.DBus.ObjectManager','InterfacesRemoved',GLib.Variant('(oas)',(DEVICE,[DI])))
def add_device():
 props[DEVICE]={'Adapter':GLib.Variant('o',ADAPTER),'Alias':GLib.Variant('s','Fixture keyboard'),'Paired':GLib.Variant('b',False),'Connected':GLib.Variant('b',False),'Trusted':GLib.Variant('b',False),'Blocked':GLib.Variant('b',False)}
 register(DEVICE,DI);bus.emit_signal(None,'/','org.freedesktop.DBus.ObjectManager','InterfacesAdded',GLib.Variant('(oa{sa{sv}})',(DEVICE,{DI:props[DEVICE]})))
def command(value):
 global mode,foreign,pending,deny_stop
 name=value['command']
 if name=='mode':mode=value['value']
 elif name=='deny-stop':deny_stop=value['enabled']
 elif name=='reset':changed(DEVICE,{'Paired':GLib.Variant('b',False),'Connected':GLib.Variant('b',False),'Trusted':GLib.Variant('b',False)})
 elif name=='recreate':remove_device();add_device()
 elif name=='add-device':add_device()
 elif name=='replace-owner':
  current=pending;pending=None
  if current is not None:failure(current,'org.bluez.Error.AuthenticationCanceled')
  bus.close_sync(None);sessions.clear();connect()
 elif name=='foreign-scan':sessions.add(':fixture-foreign');changed(ADAPTER,{'Discovering':GLib.Variant('b',True)})
 elif name=='end-foreign-scan':sessions.discard(':fixture-foreign');changed(ADAPTER,{'Discovering':GLib.Variant('b',bool(sessions))})
 elif name=='summary':emit('summary',managed_calls=managed,sessions=len(sessions))
 elif name=='foreign-confirm':
  foreign=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
  destination,path=default_agent
  def finished(c,result,*unused):
   try:c.call_finish(result);emit('foreign-rejected',rejected=False)
   except GLib.Error:emit('foreign-rejected',rejected=True)
   c.close_sync(None)
  foreign.call(destination,path,'org.bluez.Agent1','RequestConfirmation',GLib.Variant('(ou)',(DEVICE,123)),GLib.VariantType.new('()'),Gio.DBusCallFlags.NONE,5000,None,finished,None)
 elif name=='incoming-confirm':agent_call('RequestConfirmation',GLib.Variant('(ou)',(DEVICE,123)),'()',lambda ok,value:emit('incoming-reply',accepted=ok))
 elif name=='service':agent_call('AuthorizeService',GLib.Variant('(os)',(DEVICE,'00001124-0000-1000-8000-00805f9b34fb')),'()',lambda ok,value:emit('service-reply',accepted=ok))
 elif name=='display-progress':agent_call('DisplayPasskey',GLib.Variant('(ouq)',(DEVICE,123,value['entered'])),'()',lambda ok,value:emit('display-reply',accepted=ok))
 elif name=='complete-display':
  current=pending;pending=None;changed(DEVICE,{'Paired':GLib.Variant('b',True)})
  if current is not None:success(current)
  agent_call('Cancel',None,'()',lambda ok,value:emit('agent-cancel',accepted=ok))
 elif name=='quit':loop.quit()
 else:raise AssertionError(name)
 emit('command',name=name,request=value.get('request'))
def poll():
 global consumed
 values=a.commands.read_text().splitlines(keepends=True)
 for line in values[consumed:]:
  if not line.endswith('\n'):break
  command(json.loads(line));consumed+=1
 return True
connect();GLib.timeout_add(30,poll);GLib.timeout_add_seconds(300,lambda:(loop.quit(),False)[1]);loop.run();bus.close_sync(None);report.close()
