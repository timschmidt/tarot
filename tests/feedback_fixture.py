#!/usr/bin/python3
"""Owned private feedbackd protocol fixture; no hardware or host session use."""
import argparse
import json
import os
from pathlib import Path
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--report', type=Path, required=True)
p.add_argument('--commands', type=Path, required=True)
a = p.parse_args()
address = os.environ['DBUS_SESSION_BUS_ADDRESS']
assert address.startswith('unix:') and '/run/dbus/system_bus_socket' not in address
report = a.report.open('x', buffering=1)
NAME = 'org.sigxcpu.Feedback'
PATH = '/org/sigxcpu/Feedback'
xml = '''<node><interface name="org.sigxcpu.Feedback">
<property name="Profile" type="s" access="readwrite"/>
<method name="TriggerFeedback"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="EndFeedback"><arg type="u" direction="in"/></method>
<signal name="FeedbackEnded"><arg type="u"/><arg type="u"/></signal>
</interface></node>'''
interface = Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0]
loop = GLib.MainLoop()
profile = 'full'
mode = 'normal'
next_id = 100
active = {}
held = []
consumed = 0
bus = None


def emit(event, **fields):
    report.write(json.dumps(dict(event=event, **fields)) + '\n')


def success(inv):
    inv.return_value(GLib.Variant('()', ()))


def changed(value):
    global profile
    profile = value
    bus.emit_signal(None, PATH, 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
                    GLib.Variant('(sa{sv}as)', (NAME, {'Profile': GLib.Variant('s', profile)}, [])))


def getprop(c, sender, path, iface, name):
    return GLib.Variant('s', profile)


def setprop(c, sender, path, iface, name, value):
    emit('set-profile', value=value.unpack(), sender=sender)
    if mode == 'deny-set':
        return False
    if mode != 'ignore-set':
        changed(value.unpack())
    return True


def complete(identifier, spoof=False, reason=0):
    connection = bus
    if spoof:
        connection = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    else:
        active.pop(identifier, None)
    connection.emit_signal(None, PATH, NAME, 'FeedbackEnded', GLib.Variant('(uu)', (identifier, reason)))
    connection.flush_sync(None)
    if spoof:
        connection.close_sync(None)


def method(c, sender, path, iface, name, parameters, inv):
    global next_id
    values = parameters.unpack()
    if name == 'TriggerFeedback':
        app_id, event, hints, timeout = values
        identifier = next_id
        next_id += 1
        assert timeout == -1
        assert app_id == 'org.tarot.Shell' and event in ['button-pressed', 'message-new-instant']
        assert set(hints) == {'profile'} and hints['profile'] in ['full', 'quiet', 'silent']
        emit('trigger', id=identifier, app_id=app_id, name=event, hints=hints, timeout=timeout, sender=sender)
        if mode == 'deny-trigger':
            inv.return_dbus_error('org.sigxcpu.Feedback.Error.Failed', 'Synthetic rejection')
        elif mode == 'hold-trigger':
            held.append((inv, identifier, sender))
        else:
            active[identifier] = sender
            inv.return_value(GLib.Variant('(u)', (identifier,)))
    elif name == 'EndFeedback':
        identifier, = values
        emit('end', id=identifier, sender=sender, belongs_to_sender=active.get(identifier) == sender)
        if mode != 'hang-end':
            complete(identifier)
            success(inv)
        else:
            held.append((inv, None, sender))


def connect():
    global bus
    bus = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    bus.set_exit_on_close(False)
    result = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName', GLib.Variant('(su)', (NAME, 4)), None, Gio.DBusCallFlags.NONE, 3000, None)
    assert result.unpack() == (1,)
    bus.register_object(PATH, interface, method, getprop, setprop)
    emit('ready', owner=bus.get_unique_name())


def command(value):
    global mode, next_id
    name = value['command']
    if name == 'profile':
        changed(value['value'])
    elif name == 'mode':
        mode = value['value']
    elif name == 'complete':
        complete(value['id'], value.get('spoof', False), value.get('reason', 0))
    elif name == 'release-held':
        for inv, identifier, sender in held:
            if identifier is None:
                success(inv)
            else:
                active[identifier] = sender
                inv.return_value(GLib.Variant('(u)', (identifier,)))
        held.clear()
    elif name == 'replace-owner':
        bus.close_sync(None)
        active.clear()
        held.clear()
        next_id = 100
        connect()
    elif name == 'rapid-owner':
        if value.get('old_id') is not None:
            for _ in range(65):
                complete(value['old_id'])
        for method_name, signature, values in [
                ('ReleaseName', '(s)', (NAME,)), ('RequestName', '(su)', (NAME, 4))]:
            bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                          'org.freedesktop.DBus', method_name, GLib.Variant(signature, values),
                          None, Gio.DBusCallFlags.NONE, 3000, None)
        active.clear()
        held.clear()
        next_id = 100
    elif name == 'quit':
        loop.quit()
    else:
        raise AssertionError(name)
    emit('reply', request=value.get('request'), ok=True)


def poll():
    global consumed
    for line in a.commands.read_text().splitlines(keepends=True)[consumed:]:
        if not line.endswith('\n'):
            break
        command(json.loads(line))
        consumed += 1
    return True


connect()
GLib.timeout_add(30, poll)
GLib.timeout_add_seconds(240, lambda: (loop.quit(), False)[1])
loop.run()
bus.close_sync(None)
report.close()
