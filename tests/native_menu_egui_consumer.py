#!/usr/bin/env python3
"""Independent GIO reader for the owned Rust/egui example; never invokes actions."""
import argparse
import json
import os
from pathlib import Path
import time

import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--owner', required=True)
p.add_argument('--report', type=Path, required=True)
a = p.parse_args()
assert a.owner.startswith(':')
report = a.report.open('x', buffering=1)
def emit(event, **fields):
    report.write(json.dumps(dict(event=event, **fields)) + '\n')

connection = Gio.DBusConnection.new_for_address_sync(
    os.environ['DBUS_SESSION_BUS_ADDRESS'],
    Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
    None, None)
connection.set_exit_on_close(False)
base = '/org/tarot/MenuEgui/'
app = Gio.DBusActionGroup.get(connection, a.owner, base + 'app')
window = Gio.DBusActionGroup.get(connection, a.owner, base + 'window')
root = Gio.DBusMenuModel.get(connection, a.owner, base + 'menu')
states = {}
enabled = {}
gone = False
def state_changed(group, name, value):
    states[name] = value.unpack()
    emit('state', name=name, value=value.unpack())
def enabled_changed(group, name, value):
    enabled[name] = value
    emit('enabled', name=name, value=value)
app.connect('action-state-changed', state_changed)
app.connect('action-enabled-changed', enabled_changed)
def owner_changed(c, sender, path, interface, signal, parameters):
    global gone
    name, old, new = parameters.unpack()
    if name == a.owner and old and not new:
        gone = True
connection.signal_subscribe('org.freedesktop.DBus', 'org.freedesktop.DBus', 'NameOwnerChanged',
                            '/org/freedesktop/DBus', a.owner, Gio.DBusSignalFlags.NONE, owner_changed)
context = GLib.MainContext.default()
deadline = time.monotonic() + 100
ready = False
while not gone:
    for _ in range(64):
        if not context.pending():
            break
        context.iteration(False)
    assert time.monotonic() < deadline, 'GIO consumer deadline'
    if not ready:
        count = root.get_n_items()
        app_actions = set(app.list_actions())
        window_actions = set(window.list_actions())
        submenu = root.get_item_link(0, Gio.MENU_LINK_SUBMENU) if count else None
        if submenu is not None and submenu.get_n_items() == 4 and app_actions == {'choose', 'toggle', 'disabled'} and window_actions == {'window'}:
            assert root.get_item_attribute_value(0, 'label', None).unpack() == '_Preferences'
            rows = [{key: submenu.get_item_attribute_value(i, key, None).unpack()
                     for key in ['label', 'action']} for i in range(4)]
            assert [row['action'] for row in rows] == ['app.choose', 'app.toggle', 'app.disabled', 'win.window']
            assert submenu.get_item_attribute_value(0, 'target', None).unpack() == 'night'
            assert submenu.get_item_attribute_value(3, 'target', None).unpack() == 7
            assert app.get_action_state('choose').unpack() == 'day'
            assert app.get_action_state('toggle').unpack() is False
            assert not app.get_action_enabled('disabled')
            assert app.get_action_parameter_type('choose').dup_string() == 's'
            assert window.get_action_parameter_type('window').dup_string() == 'i'
            description = connection.call_sync(a.owner, base + 'app', 'org.gtk.Actions', 'Describe',
                GLib.Variant('(s)', ('choose',)), GLib.VariantType.new('((bgav))'), Gio.DBusCallFlags.NONE, 1000, None)
            assert description.unpack()[0] == (True, 's', ['day'])
            ready = True
            emit('ready', rows=rows, describe_signature=description.get_type_string(), never_activated=True)
    time.sleep(.01)
assert ready and states.get('choose') == 'night' and states.get('toggle') is True and enabled.get('choose') is False
emit('passed', owner_exited=True, never_activated=True)
connection.close_sync(None)
report.close()
