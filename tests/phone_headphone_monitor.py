#!/usr/bin/env python3
"""Root, read-only D-Bus trace of one identified headphone's Connect result.

Never records authentication/agent input bodies or unrelated device replies.
No system bus policy or daemon configuration is changed.
"""
import argparse,json,os,time
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--peer-path',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();assert os.geteuid()==0 and a.peer_path.startswith('/org/bluez/hci0/dev_')
f=a.output.open('x',buffering=1);loop=GLib.MainLoop();tracked=set()
def emit(event,**values):f.write(json.dumps(dict(event=event,monotonic=time.monotonic(),**values))+'\n')
b=Gio.bus_get_sync(Gio.BusType.SYSTEM,None);owner=b.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetNameOwner',GLib.Variant('(s)',('org.bluez',)),None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
def inspect(connection,message,incoming,*unused):
 if not incoming:return message
 kind=message.get_message_type();body=message.get_body()
 if kind==Gio.DBusMessageType.METHOD_CALL and message.get_path()==a.peer_path and message.get_interface()=='org.bluez.Device1' and message.get_member()=='Connect':
  tracked.add((message.get_sender(),message.get_serial()));emit('connect-call',caller=message.get_sender(),serial=message.get_serial())
 elif kind in [Gio.DBusMessageType.ERROR,Gio.DBusMessageType.METHOD_RETURN]:
  # BlueZ Connect errors carry a transport failure reason, never PIN input.
  matched=(message.get_destination(),message.get_reply_serial()) in tracked
  emit('reply-header',sender=message.get_sender(),destination=message.get_destination(),reply_serial=message.get_reply_serial(),error=message.get_error_name(),tracked=matched)
  if matched:
   details=body.unpack() if body is not None else ()
   emit('connect-result',error=message.get_error_name(),reason=str(details)[:512] if kind==Gio.DBusMessageType.ERROR else None)
 elif kind==Gio.DBusMessageType.METHOD_CALL and message.get_interface()=='org.bluez.Agent1' and message.get_member()=='AuthorizeService':
  values=body.unpack()
  if values[0]==a.peer_path:emit('service-authorization',uuid=values[1])
 return message
b.add_filter(inspect,None)
rules=["type='method_call',destination='org.bluez',path='"+a.peer_path+"',interface='org.bluez.Device1',member='Connect'","type='error',sender='"+owner+"'","type='method_return',sender='"+owner+"'","type='method_call',sender='"+owner+"',interface='org.bluez.Agent1',member='AuthorizeService'"]
b.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus.Monitoring','BecomeMonitor',GLib.Variant('(asu)',(rules,0)),None,Gio.DBusCallFlags.NONE,5000,None);emit('ready');GLib.timeout_add_seconds(180,lambda:(loop.quit(),False)[1]);loop.run();b.close_sync(None);f.close()
