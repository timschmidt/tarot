#!/usr/bin/env python3
"""Host-side Bluetooth test peer for Tarot pairing acceptance.

Uses only the adapter with the given address, makes it visible under a test
alias, and answers BlueZ agent requests: numeric-comparison codes are
recorded and accepted. On exit it removes any pairing with the phone and
restores every adapter property it changed. Events go to a JSON-lines report.
"""
import argparse
import json
import signal
import time
from pathlib import Path
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

AGENT = '/org/tarot/qa/peer_agent'
XML = '''<node><interface name="org.bluez.Agent1">
<method name="Release"/>
<method name="RequestConfirmation"><arg type="o" direction="in"/><arg type="u" direction="in"/></method>
<method name="RequestAuthorization"><arg type="o" direction="in"/></method>
<method name="AuthorizeService"><arg type="o" direction="in"/><arg type="s" direction="in"/></method>
<method name="RequestPinCode"><arg type="o" direction="in"/><arg type="s" direction="out"/></method>
<method name="RequestPasskey"><arg type="o" direction="in"/><arg type="u" direction="out"/></method>
<method name="DisplayPasskey"><arg type="o" direction="in"/><arg type="u" direction="in"/><arg type="q" direction="in"/></method>
<method name="DisplayPinCode"><arg type="o" direction="in"/><arg type="s" direction="in"/></method>
<method name="Cancel"/>
</interface></node>'''


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--adapter-address', required=True)
    p.add_argument('--phone-address', required=True)
    p.add_argument('--alias', default='TarotPeer')
    p.add_argument('--report', type=Path, required=True)
    a = p.parse_args()
    report = a.report.open('x', buffering=1)
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)

    def emit(event, **fields):
        report.write(json.dumps(dict(event=event, monotonic=time.monotonic(), **fields)) + '\n')

    def call(path, interface, method, signature=None, values=(), reply=None):
        return bus.call_sync('org.bluez', path, interface, method,
                             None if signature is None else GLib.Variant(signature, values),
                             None if reply is None else GLib.VariantType.new(reply),
                             Gio.DBusCallFlags.NONE, 10000, None).unpack()

    def objects():
        return call('/', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects')[0]

    adapters = [path for path, ifaces in objects().items()
                if ifaces.get('org.bluez.Adapter1', {}).get('Address') == a.adapter_address]
    assert len(adapters) == 1, 'Exactly the requested adapter must exist'
    adapter = adapters[0]
    keys = ['Alias', 'Powered', 'Discoverable', 'DiscoverableTimeout', 'Pairable', 'PairableTimeout']
    original = {k: objects()[adapter]['org.bluez.Adapter1'][k] for k in keys}
    # An alias equal to the system name is BlueZ's default; restore it as default
    # (empty) so no explicit alias is left stored in BlueZ's settings.
    default_alias = original['Alias'] == objects()[adapter]['org.bluez.Adapter1']['Name']
    signatures = dict(Alias='s', Powered='b', Discoverable='b', DiscoverableTimeout='u', Pairable='b', PairableTimeout='u')
    emit('original', adapter=adapter, properties=original)

    def setprop(name, value):
        call(adapter, 'org.freedesktop.DBus.Properties', 'Set', '(ssv)',
             ('org.bluez.Adapter1', name, GLib.Variant(signatures[name], value)))

    def phone_path():
        for path, ifaces in objects().items():
            device = ifaces.get('org.bluez.Device1')
            if device and device.get('Adapter') == adapter and device.get('Address') == a.phone_address:
                return path, device
        return None, None

    path, device = phone_path()
    assert path is None or not device.get('Paired'), 'Start without an existing host pairing to the phone'

    def method(c, sender, path, iface, name, parameters, inv):
        values = parameters.unpack()
        emit('agent', method=name, device=values[0] if values else None,
             passkey=('%06d' % values[1]) if name in ['RequestConfirmation', 'DisplayPasskey'] else None,
             uuid=values[1] if name == 'AuthorizeService' else None)
        if name == 'RequestPinCode':
            inv.return_dbus_error('org.bluez.Error.Rejected', 'Numeric comparison only')
        elif name == 'RequestPasskey':
            inv.return_dbus_error('org.bluez.Error.Rejected', 'Numeric comparison only')
        else:
            inv.return_value(None)

    info = Gio.DBusNodeInfo.new_for_xml(XML)
    registration = bus.register_object(AGENT, info.interfaces[0], method, None, None)
    call('/org/bluez', 'org.bluez.AgentManager1', 'RegisterAgent', '(os)', (AGENT, 'DisplayYesNo'))
    call('/org/bluez', 'org.bluez.AgentManager1', 'RequestDefaultAgent', '(o)', (AGENT,))
    setprop('Alias', a.alias)
    setprop('Powered', True)
    setprop('PairableTimeout', 0)
    setprop('Pairable', True)
    setprop('DiscoverableTimeout', 0)
    setprop('Discoverable', True)
    emit('ready', alias=a.alias)

    loop = GLib.MainLoop()

    def watch():
        current, device = phone_path()
        state = None if current is None else {k: device.get(k) for k in ['Paired', 'Trusted', 'Connected', 'Bonded']}
        if state != getattr(watch, 'last', 'unset'):
            emit('phone', present=current is not None, state=state)
            watch.last = state
        return True
    GLib.timeout_add(500, watch)
    signal.signal(signal.SIGTERM, lambda *_: loop.quit())
    signal.signal(signal.SIGINT, lambda *_: loop.quit())
    try:
        loop.run()
    finally:
        current, device = phone_path()
        if current is not None:
            try:
                call(adapter, 'org.bluez.Adapter1', 'RemoveDevice', '(o)', (current,))
                emit('removed-phone', path=current)
            except GLib.Error as error:
                emit('remove-error', error=error.message)
        try:
            call('/org/bluez', 'org.bluez.AgentManager1', 'UnregisterAgent', '(o)', (AGENT,))
        except GLib.Error as error:
            emit('unregister-error', error=error.message)
        bus.unregister_object(registration)
        for key in ['Discoverable', 'DiscoverableTimeout', 'Pairable', 'PairableTimeout', 'Alias', 'Powered']:
            setprop(key, '' if key == 'Alias' and default_alias else original[key])
        deadline = time.monotonic() + 10
        while True:
            restored = {k: objects()[adapter]['org.bluez.Adapter1'][k] for k in keys}
            if restored == original or time.monotonic() > deadline:
                break
            time.sleep(.2)
        emit('restored', properties=restored, exact=restored == original, default_alias=default_alias)


if __name__ == '__main__':
    main()
