#!/usr/bin/env python3
"""Make the phone's Bluetooth adapter visible for a bounded time, then restore it.

Run on the phone as the session user while tests/bluetooth_reachability.py
scans from the host. Only Discoverable and DiscoverableTimeout are changed, and
both are put back exactly; the result is printed as one JSON line.
"""
import argparse
import json
import time
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

KEYS = {'Discoverable': 'b', 'DiscoverableTimeout': 'u'}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('seconds', type=int)
    a = p.parse_args()
    assert 0 < a.seconds <= 600, 'Keep the visible window short'
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)

    def call(path, interface, method, signature=None, values=()):
        return bus.call_sync('org.bluez', path, interface, method,
                             None if signature is None else GLib.Variant(signature, values),
                             None, Gio.DBusCallFlags.NONE, 10000, None).unpack()

    def properties():
        objects = call('/', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects')[0]
        adapters = [(path, i['org.bluez.Adapter1']) for path, i in objects.items() if 'org.bluez.Adapter1' in i]
        assert len(adapters) == 1, 'Exactly one phone adapter'
        return adapters[0][0], {k: adapters[0][1][k] for k in KEYS}

    def setprop(name, value):
        call(adapter, 'org.freedesktop.DBus.Properties', 'Set', '(ssv)',
             ('org.bluez.Adapter1', name, GLib.Variant(KEYS[name], value)))

    adapter, original = properties()
    try:
        setprop('DiscoverableTimeout', a.seconds)
        setprop('Discoverable', True)
        print(json.dumps(dict(event='visible', seconds=a.seconds)), flush=True)
        time.sleep(a.seconds)
    finally:
        setprop('Discoverable', original['Discoverable'])
        setprop('DiscoverableTimeout', original['DiscoverableTimeout'])
        restored = properties()[1]
        print(json.dumps(dict(event='restored', exact=restored == original, properties=restored)), flush=True)


if __name__ == '__main__':
    main()
