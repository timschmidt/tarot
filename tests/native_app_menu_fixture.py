#!/usr/bin/env python3
"""Mapped GTK/GIO app with real exported menus, plus controlled mutation QA."""
import argparse,json,sys
import gi
gi.require_version('Gtk','3.0')
from gi.repository import Gtk,Gio,GLib
p=argparse.ArgumentParser();p.add_argument('--report',required=True);a=p.parse_args()
report=open(a.report,'x',buffering=1)
def emit(event,**fields):report.write(json.dumps(dict(event=event,**fields))+'\n')
app=Gtk.Application(application_id='org.tarot.NativeMenuFixture')
def startup(app):
 def activate(action,target):
  if action.get_name()=='toggle':action.set_state(GLib.Variant('b',not action.get_state().unpack()))
  emit('action',name=action.get_name(),target=None if target is None else target.unpack())
 for name,typ,state in [('choose','s',None),('toggle',None,GLib.Variant('b',False)),('disabled',None,None)]:
  action=Gio.SimpleAction.new_stateful(name,None,state) if state is not None else Gio.SimpleAction.new(name,None if typ is None else GLib.VariantType.new(typ))
  action.connect('activate',activate)
  if name=='disabled':action.set_enabled(False)
  app.add_action(action)
 root=Gio.Menu();sub=Gio.Menu()
 item=Gio.MenuItem.new('Choose night','app.choose');item.set_attribute_value('target',GLib.Variant('s','night'));sub.append_item(item)
 sub.append('Toggle flag','app.toggle');sub.append('Disabled','app.disabled');root.append_submenu('Preferences',sub)
 app.set_app_menu(root)
 def control(source,condition):
  if condition&GLib.IO_HUP:return False
  command=sys.stdin.readline().strip()
  if command=='flip':app.lookup_action('choose').set_enabled(False);app.lookup_action('choose').set_enabled(True)
  elif command=='disable':app.lookup_action('choose').set_enabled(False)
  elif command=='rename':sub.remove(0);sub.insert(0,'Renamed command','app.toggle')
  elif command=='exit':app.quit()
  else:raise AssertionError(command)
  emit('mutation',command=command)
  return True
 GLib.io_add_watch(sys.stdin,GLib.IO_IN|GLib.IO_HUP,control)
def activated(app):
 window=Gtk.ApplicationWindow(application=app,title='Native menu fixture');window.set_default_size(320,500)
 window.add(Gtk.Label(label='Real GTK application'));window.show_all();emit('mapped')
app.connect('startup',startup);app.connect('activate',activated)
app.run([])
report.close()
