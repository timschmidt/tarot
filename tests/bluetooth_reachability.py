#!/usr/bin/env python3
"""Host-side check that a visible phone answers inquiry and page.

Each round, every given host adapter runs a short BR/EDR discovery and reports
the phone's RSSI, or null when the phone never answered. With --page-adapter,
that adapter then opens a connection to the phone: a page timeout means the
phone's page scan never answered, while a BlueZ error from later in the
connection (such as the authentication refusal an unpaired phone gives) means
the page reached it. A temporary agent rejects every request meanwhile, so no
pairing prompt reaches the desktop; BlueZ restores the previous default agent
when it unregisters. The host's device entry for the phone is removed again
after each round, and a phone already paired on a page adapter is refused.

Make the phone visible first, for example with tests/phone_bluetooth_visible.py.
Results go to stdout as JSON lines.
"""
import argparse
import json
import time
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

AGENT = '/org/tarot/qa/reachability_agent'
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
    p.add_argument('--phone-address', required=True)
    p.add_argument('--adapter-address', action='append', required=True,
                   help='Host adapter used for discovery (repeatable)')
    p.add_argument('--page-adapter', help='Host adapter that pages the phone after discovery')
    p.add_argument('--rounds', type=int, default=3)
    p.add_argument('--seconds', type=int, default=10)
    a = p.parse_args()
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)

    def call(path, interface, method, signature=None, values=(), timeout=10000):
        return bus.call_sync('org.bluez', path, interface, method,
                             None if signature is None else GLib.Variant(signature, values),
                             None, Gio.DBusCallFlags.NONE, timeout, None).unpack()

    def objects():
        return call('/', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects')[0]

    def adapter_path(address):
        found = [path for path, i in objects().items() if i.get('org.bluez.Adapter1', {}).get('Address') == address]
        assert len(found) == 1, 'Adapter %s must exist exactly once' % address
        return found[0]

    def phone(adapter):
        for path, i in objects().items():
            d = i.get('org.bluez.Device1')
            if d and d.get('Adapter') == adapter and d.get('Address') == a.phone_address:
                return path, d
        return None, None

    def forget(adapter):
        path, device = phone(adapter)
        if path is not None and not device.get('Paired'):
            call(adapter, 'org.bluez.Adapter1', 'RemoveDevice', '(o)', (path,))

    def discover(adapter):
        call(adapter, 'org.bluez.Adapter1', 'SetDiscoveryFilter', '(a{sv})',
             ({'Transport': GLib.Variant('s', 'bredr'), 'DuplicateData': GLib.Variant('b', True)},))
        call(adapter, 'org.bluez.Adapter1', 'StartDiscovery')
        rssi = []
        try:
            end = time.monotonic() + a.seconds
            while time.monotonic() < end:
                device = phone(adapter)[1]
                if device and device.get('RSSI') is not None:
                    rssi.append(device['RSSI'])
                time.sleep(1)
        finally:
            call(adapter, 'org.bluez.Adapter1', 'StopDiscovery')
            call(adapter, 'org.bluez.Adapter1', 'SetDiscoveryFilter', '(a{sv})', ({},))
        return max(rssi) if rssi else None

    context = GLib.MainContext.default()

    def page(path):
        done = {}

        def finished(source, result):
            try:
                bus.call_finish(result)
                done['result'] = 'connected'
            except GLib.Error as error:
                done['result'] = error.message.split(': ')[-1]
        bus.call('org.bluez', path, 'org.bluez.Device1', 'Connect', None, None,
                 Gio.DBusCallFlags.NONE, 30000, None, finished)
        while not done:
            context.iteration(True)
        return done['result']

    def reject(connection, sender, path, interface, name, parameters, invocation):
        if name in ['Release', 'Cancel', 'DisplayPasskey', 'DisplayPinCode']:
            invocation.return_value(None)
        else:
            invocation.return_dbus_error('org.bluez.Error.Rejected', 'Reachability check only')

    pager = adapter_path(a.page_adapter) if a.page_adapter else None
    registration = None
    if pager:
        assert not (phone(pager)[1] or {}).get('Paired'), 'Unpair the phone from the page adapter first'
        registration = bus.register_object(AGENT, Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0], reject, None, None)
        call('/org/bluez', 'org.bluez.AgentManager1', 'RegisterAgent', '(os)', (AGENT, 'NoInputNoOutput'))
        call('/org/bluez', 'org.bluez.AgentManager1', 'RequestDefaultAgent', '(o)', (AGENT,))
    try:
        for number in range(1, a.rounds + 1):
            row = dict(round=number, rssi={})
            for address in a.adapter_address:
                path = adapter_path(address)
                forget(path)
                row['rssi'][address] = discover(path)
            if pager:
                path = phone(pager)[0]
                if path is None:
                    row['page'] = 'not-discovered'
                else:
                    started = time.monotonic()
                    row['page'] = page(path)
                    row['page_seconds'] = round(time.monotonic() - started, 1)
                    # A D-Bus timeout says nothing about the radio, so it stays unknown.
                    row['page_reached_phone'] = (None if row['page'] == 'Timeout was reached'
                                                 else 'page-timeout' not in row['page'])
                    try:
                        call(path, 'org.bluez.Device1', 'Disconnect')
                    except GLib.Error:
                        pass
                    forget(pager)
            print(json.dumps(row), flush=True)
    finally:
        if registration is not None:
            call('/org/bluez', 'org.bluez.AgentManager1', 'UnregisterAgent', '(o)', (AGENT,))
            bus.unregister_object(registration)


if __name__ == '__main__':
    main()
