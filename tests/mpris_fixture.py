#!/usr/bin/python3
"""Owned private MPRIS player fixture; no audio output or host session use."""
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
p.add_argument('--name', default='org.mpris.MediaPlayer2.TarotFixture')
a = p.parse_args()
address = os.environ['DBUS_SESSION_BUS_ADDRESS']
assert address.startswith('unix:') and '/run/dbus/system_bus_socket' not in address
report = a.report.open('x', buffering=1)
PATH = '/org/mpris/MediaPlayer2'
ROOT = 'org.mpris.MediaPlayer2'
PLAYER = 'org.mpris.MediaPlayer2.Player'
xml = '''<node>
<interface name="org.mpris.MediaPlayer2">
<property name="Identity" type="s" access="read"/>
<method name="Raise"/><method name="Quit"/>
</interface>
<interface name="org.mpris.MediaPlayer2.Player">
<property name="PlaybackStatus" type="s" access="read"/>
<property name="Metadata" type="a{sv}" access="read"/>
<property name="CanPlay" type="b" access="read"/>
<property name="CanPause" type="b" access="read"/>
<property name="CanGoNext" type="b" access="read"/>
<property name="CanGoPrevious" type="b" access="read"/>
<property name="CanControl" type="b" access="read"/>
<method name="PlayPause"/><method name="Next"/><method name="Previous"/>
</interface></node>'''
info = Gio.DBusNodeInfo.new_for_xml(xml)
loop = GLib.MainLoop()
bus = None
registrations = []
mode = 'normal'
held = []
reads = 0
state = dict(identity='Fixture Player', status='Paused', title='Morning Song', artist=['Example Artist'],
             padding=0, can_play=True, can_pause=True, can_next=True, can_previous=False, can_control=True)


def emit(event, **fields):
    report.write(json.dumps(dict(event=event, **fields)) + '\n')


def metadata():
    value = {'mpris:trackid': GLib.Variant('o', '/org/tarot/fixture/track'),
             'xesam:title': GLib.Variant('s', state['title']),
             'xesam:artist': GLib.Variant('as', state['artist'])}
    if state['padding']:
        value['xesam:comment'] = GLib.Variant('as', ['x' * state['padding']])
    return value


def player_props():
    return {'PlaybackStatus': GLib.Variant('s', state['status']),
            'Metadata': GLib.Variant('a{sv}', metadata()),
            'CanPlay': GLib.Variant('b', state['can_play']),
            'CanPause': GLib.Variant('b', state['can_pause']),
            'CanGoNext': GLib.Variant('b', state['can_next']),
            'CanGoPrevious': GLib.Variant('b', state['can_previous']),
            'CanControl': GLib.Variant('b', state['can_control'])}


def changed():
    bus.emit_signal(None, PATH, 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
                    GLib.Variant('(sa{sv}as)', (PLAYER, player_props(), [])))


def getprop(c, sender, path, iface, name):
    global reads
    reads += 1
    if iface == ROOT:
        return GLib.Variant('s', state['identity'])
    return player_props()[name]


def method(c, sender, path, iface, name, parameters, inv):
    emit('call', method=name, sender=sender, interface=iface)
    if mode == 'hang':
        held.append(inv)
        return
    if name == 'PlayPause':
        state['status'] = 'Paused' if state['status'] == 'Playing' else 'Playing'
    elif name == 'Next':
        state['title'] = state['title'] + ' (next)'
    inv.return_value(GLib.Variant('()', ()))
    changed()


def connect():
    global bus
    bus = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    bus.set_exit_on_close(False)
    for interface in info.interfaces:
        registrations.append(bus.register_object(PATH, interface, method, getprop, None))
    result = bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName',
                           GLib.Variant('(su)', (a.name, 4)), None, Gio.DBusCallFlags.NONE, 3000, None)
    assert result.unpack() == (1,)
    emit('ready', owner=bus.get_unique_name())


def command(value):
    global mode, reads
    name = value['command']
    result = None
    if name == 'set':
        state.update(value['values'])
        changed()
    elif name == 'mode':
        mode = value['value']
    elif name == 'release':
        for inv in held:
            inv.return_value(GLib.Variant('()', ()))
        held.clear()
    elif name == 'reads':
        result = dict(reads=reads)
    elif name == 'replace-owner':
        bus.close_sync(None)
        registrations.clear()
        connect()
        result = dict(owner=bus.get_unique_name())
    elif name == 'vanish':
        bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'ReleaseName',
                      GLib.Variant('(s)', (a.name,)), None, Gio.DBusCallFlags.NONE, 3000, None)
    elif name == 'flood':
        for _ in range(value['count']):
            changed()
    emit('reply', request=value['request'], ok=True, result=result)


offset = 0


def poll():
    global offset
    data = a.commands.read_bytes()
    lines = data[offset:].split(b'\n')
    for line in lines[:-1]:
        offset += len(line) + 1
        if line:
            command(json.loads(line))
    return True


connect()
GLib.timeout_add(50, poll)
loop.run()
