#!/usr/bin/env python3
"""Independent GIO observation of bounded synthetic traffic; never activates."""
import argparse,json,os
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--report',type=Path,required=True);p.add_argument('--owners',nargs='+',required=True);p.add_argument('--count',type=int,required=True);a=p.parse_args();assert 1<=a.count<=32
report=os.fdopen(os.open(a.report,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_CLOEXEC,0o600),'w',buffering=1)
def emit(event,**fields):report.write(json.dumps(dict(event=event,**fields))+'\n')
loop=GLib.MainLoop();received=[];passed=False
bus=Gio.DBusConnection.new_for_address_sync(os.environ['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None);bus.set_exit_on_close(False)
def signal(connection,sender,path,interface,member,parameters,*unused):
 global passed
 if sender not in a.owners or parameters.get_type_string()!='(ay)' or parameters.get_child_value(0).n_children()!=96*1024:return
 record={'sender':sender,'path':path,'interface':interface,'member':member,'payload_bytes':parameters.get_child_value(0).n_children()};received.append(record);emit('signal',**record)
 if len(received)==a.count:passed=True;loop.quit()
bus.signal_subscribe(None,None,None,None,None,Gio.DBusSignalFlags.NONE,signal)
bus.flush_sync(None)
bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetId',None,GLib.VariantType.new('(s)'),Gio.DBusCallFlags.NONE,5000,None)
emit('ready',owner=bus.get_unique_name(),expected=a.count)
GLib.timeout_add_seconds(90,lambda:(loop.quit(),False)[1]);loop.run()
emit('passed' if passed else 'failed',count=len(received));bus.close_sync(None);report.close();assert passed
