#!/usr/bin/env python3
"""Real GIO exports and bounded malicious menu producers for Rust adapter QA.

Private test fixture. No desktop files, global settings or platform services.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import sys
import time

import gi

gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--peer', required=True)
parser.add_argument('--report', type=Path, required=True)
parser.add_argument('--mode', default='normal', choices=[
    'normal', 'unsupported', 'many-actions', 'slow-load', 'short-link', 'wrong-link',
    'cycle', 'deep', 'groups', 'expanded', 'oversized', 'long-label',
    'many-items', 'attributes', 'total-bytes', 'missing-root', 'fd', 'silent'])
args = parser.parse_args()
peer = socket.socket(socket.AF_UNIX)
peer.connect(args.peer)
loop = GLib.MainLoop()
slow = False
connection = None
registrations = []
held = []
report = args.report.open('x', buffering=1)


def event(name, value=None):
    report.write(json.dumps({'action': name, 'value': value}) + '\n')


def callback(action, parameter):
    if slow:
        time.sleep(0.9)
    value = None if parameter is None else parameter.unpack()
    if action.get_name() == 'toggle':
        action.set_state(GLib.Variant('b', not action.get_state().unpack()))
    event(action.get_name(), value)


def build_exports():
    global connection, app, win, root
    connection = Gio.DBusConnection.new_for_address_sync(
        os.environ['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None, None)
    connection.set_exit_on_close(False)
    app = Gio.SimpleActionGroup()
    win = Gio.SimpleActionGroup()
    for name, parameter, state, group in [
            ('toggle', None, GLib.Variant('b', False), app),
            ('choose', GLib.VariantType.new('s'), None, app),
            ('disabled', None, None, app),
            ('window', GLib.VariantType.new('i'), None, win)]:
        action = Gio.SimpleAction.new_stateful(name, parameter, state) if state is not None else Gio.SimpleAction.new(name, parameter)
        action.connect('activate', callback)
        if name == 'disabled':
            action.set_enabled(False)
        group.add_action(action)
    root = Gio.Menu()
    preferences = Gio.Menu()
    preferences.append('Toggle', 'app.toggle')
    choose = Gio.MenuItem.new('Choose night', 'app.choose')
    choose.set_attribute_value('target', GLib.Variant('s', 'night'))
    preferences.append_item(choose)
    preferences.append('Disabled', 'app.disabled')
    window = Gio.MenuItem.new('Window seven', 'win.window')
    window.set_attribute_value('target', GLib.Variant('i', 7))
    preferences.append_item(window)
    root.append_submenu('Preferences', preferences)
    if args.mode == 'unsupported':
        action = Gio.SimpleAction.new('complex', GLib.VariantType.new('(ss)'))
        action.connect('activate', callback)
        app.add_action(action)
        item = Gio.MenuItem.new('Complex target', 'app.complex')
        item.set_attribute_value('target', GLib.Variant('(ss)', ('one', 'two')))
        preferences.append_item(item)
    elif args.mode == 'many-actions':
        for index in range(257):
            app.add_action(Gio.SimpleAction.new('extra' + str(index), None))
    section = Gio.Menu()
    section.append('Another toggle', 'app.toggle')
    root.append_section('Other', section)
    connection.export_action_group('/org/tarot/NativeMenu/app', app)
    connection.export_action_group('/org/tarot/NativeMenu/window', win)
    if args.mode in ['normal', 'unsupported', 'many-actions']:
        connection.export_menu_model('/org/tarot/NativeMenu/menu', root)
    else:
        node = Gio.DBusNodeInfo.new_for_xml('''<node><interface name="org.gtk.Menus">
          <method name="Start"><arg type="au" direction="in"/><arg type="a(uuaa{sv})" direction="out"/></method>
          <method name="End"><arg type="au" direction="in"/></method>
        </interface></node>''')
        def request(c, sender, path, interface, method, parameters, invocation):
            if method == 'End':
                invocation.return_value(GLib.Variant('()', ()))
                return
            if args.mode == 'silent':
                held.append(invocation)
                return
            group = parameters.unpack()[0][0]
            if args.mode == 'slow-load':
                report.write(json.dumps({'stage': 'Start entered'}) + '\n')
                time.sleep(0.25)
                content = [(0, 0, [{'label': GLib.Variant('s', 'Toggle'), 'action': GLib.Variant('s', 'app.toggle')}])]
            elif args.mode == 'short-link':
                content = [(0, 0, [{'label': GLib.Variant('s', 'Short'), ':submenu': GLib.Variant('(u)', (0,))}])]
            elif args.mode == 'wrong-link':
                content = [(0, 0, [{'label': GLib.Variant('s', 'Wrong'), ':submenu': GLib.Variant('(us)', (0, 'wrong'))}])]
            elif args.mode in ['cycle', 'deep', 'expanded']:
                count = 1 if args.mode == 'cycle' else 10 if args.mode == 'deep' else 8
                content = []
                for menu in range(count):
                    rows = [{'label': GLib.Variant('s', 'Next'), ':submenu': GLib.Variant('(uu)', (0, (menu+1) % count))}]
                    if menu == count-1 and args.mode != 'cycle':
                        rows = [{'label': GLib.Variant('s', 'Leaf'), 'action': GLib.Variant('s', 'app.toggle')}]
                    elif args.mode == 'expanded':
                        rows = rows * 2
                    content.append((0, menu, rows))
            elif args.mode == 'groups':
                content = [(group, 0, [{'label': GLib.Variant('s', 'Next group'), ':submenu': GLib.Variant('(uu)', (group+1, 0))}])]
            elif args.mode == 'oversized':
                content = [(0, 0, [{'label': GLib.Variant('s', 'x' * 70000)}])]
            elif args.mode == 'long-label':
                content = [(0, 0, [{'label': GLib.Variant('s', 'x' * 513)}])]
            elif args.mode == 'many-items':
                content = [(0, 0, [{'label': GLib.Variant('s', 'Item')}] * 257)]
            elif args.mode == 'attributes':
                content = [(0, 0, [{**{'label': GLib.Variant('s', 'Item')}, **{'extra' + str(i): GLib.Variant('u', i) for i in range(17)}}])]
            elif args.mode == 'total-bytes':
                content = [(group, 0, [{'label': GLib.Variant('s', 'Next'), ':submenu': GLib.Variant('(uu)', (group+1, 0)), 'extra': GLib.Variant('s', 'x' * 60000)}])]
            elif args.mode == 'missing-root':
                content = [(0, 1, [{'label': GLib.Variant('s', 'Not root')}])]
            else:
                fdlist = Gio.UnixFDList.new()
                fd = os.open('/dev/null', os.O_RDONLY)
                try:
                    index = fdlist.append(fd)
                finally:
                    os.close(fd)
                content = [(0, 0, [{'label': GLib.Variant('s', 'Descriptor'), 'extra': GLib.Variant('h', index)}])]
                invocation.return_value_with_unix_fd_list(GLib.Variant('(a(uuaa{sv}))', (content,)), fdlist)
                return
            invocation.return_value(GLib.Variant('(a(uuaa{sv}))', (content,)))
        registrations.append(connection.register_object('/org/tarot/NativeMenu/menu', node.interfaces[0], request, None, None))


def ready():
    print(json.dumps({'owner': connection.get_unique_name(), 'pid': os.getpid(), 'uid': os.getuid()}), flush=True)


def control(source, condition):
    global slow
    if condition & GLib.IO_HUP:
        loop.quit()
        return False
    command = sys.stdin.readline().strip()
    if command == 'disable':
        app.lookup_action('choose').set_enabled(False)
    elif command == 'slow':
        slow = True
    elif command == 'relabel':
        root.remove_all()
        root.append('Changed toggle', 'app.toggle')
    elif command == 'close':
        connection.close_sync(None)
    elif command == 'reconnect':
        connection.close_sync(None)
        build_exports()
    elif command == 'exit':
        loop.quit()
    else:
        raise AssertionError('unknown fixture command')
    ready()
    return True


build_exports()
ready()
GLib.io_add_watch(sys.stdin, GLib.IO_IN | GLib.IO_HUP, control)
GLib.timeout_add_seconds(60, lambda: loop.quit() or False)
loop.run()
if not connection.is_closed():
    connection.close_sync(None)
peer.close()
report.close()
