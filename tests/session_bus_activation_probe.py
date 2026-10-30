#!/usr/bin/env python3
"""Check an ordinary Tarot session bus using a temporary activation service.

The caller stages org.tarot.SessionBusProbe.service in the session user's D-Bus
service directory, pointing to this script with --service, and removes it after
the test. The client checks inherited desktop identity, actual ProcessFDs and
the legacy PID method, then asks its bounded helper to exit. It never changes
the activation environment. Run it as the ordinary session user.
"""
import json
import os
from pathlib import Path
import sys

import gi

gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

NAME = 'org.tarot.SessionBusProbe'
OBJECT_PATH = '/org/tarot/SessionBusProbe'
TIMEOUT_MS = 3000


def serve(connection):
    loop = GLib.MainLoop()
    node = Gio.DBusNodeInfo.new_for_xml('''<node>
      <interface name="org.tarot.SessionBusProbe">
        <method name="Probe">
          <arg type="s" direction="out"/>
          <arg type="u" direction="out"/>
          <arg type="u" direction="out"/>
        </method>
        <method name="Stop"/>
      </interface>
    </node>''')

    def call(_connection, _sender, _path, _interface, method, _args, invocation):
        if method == 'Stop':
            invocation.return_value(GLib.Variant('()', ()))
            connection.flush_sync(None)
            GLib.idle_add(lambda: loop.quit() or False)
        else:
            invocation.return_value(GLib.Variant('(suu)', (
                os.environ.get('XDG_CURRENT_DESKTOP', 'absent'),
                os.getuid(), os.getpid())))

    registration = connection.register_object(
        OBJECT_PATH, node.interfaces[0], call, None, None)
    assert registration
    owner = Gio.bus_own_name_on_connection(
        connection, NAME, Gio.BusNameOwnerFlags.NONE, None, None)
    connection.connect('closed', lambda *_args: loop.quit())
    GLib.timeout_add_seconds(10, lambda: loop.quit() or False)
    loop.run()
    Gio.bus_unown_name(owner)
    connection.unregister_object(registration)


def probe(connection):
    def bus(method, args, reply_type):
        return connection.call_sync(
            'org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', method, args,
            GLib.VariantType.new(reply_type), Gio.DBusCallFlags.NONE,
            TIMEOUT_MS, None).unpack()

    def credentials(unique_name, expected_pid):
        result, fds = connection.call_with_unix_fd_list_sync(
            'org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionCredentials',
            GLib.Variant('(s)', (unique_name,)),
            GLib.VariantType.new('(a{sv})'), Gio.DBusCallFlags.NONE,
            TIMEOUT_MS, None, None)
        values = result.unpack()[0]
        assert values['UnixUserID'] == os.getuid()
        assert values['ProcessID'] == expected_pid
        index = values['ProcessFD']
        assert fds is not None and 0 <= index < fds.get_length()
        fd = fds.get(index)
        try:
            info = Path('/proc/self/fdinfo', str(fd)).read_text()
            fields = dict(line.split(':', 1) for line in info.splitlines())
            assert int(fields['Pid']) == expected_pid
            assert os.readlink(f'/proc/self/fd/{fd}') == 'anon_inode:[pidfd]'
        finally:
            os.close(fd)
        return sorted(values)

    reply = connection.call_sync(
        NAME, OBJECT_PATH, NAME, 'Probe', None,
        GLib.VariantType.new('(suu)'), Gio.DBusCallFlags.NONE, 5000, None).unpack()
    assert reply[0] == 'Tarot' and reply[1] == os.getuid()
    owner = bus('GetNameOwner', GLib.Variant('(s)', (NAME,)), '(s)')[0]
    legacy_pid = bus('GetConnectionUnixProcessID',
                     GLib.Variant('(s)', (owner,)), '(u)')[0]
    assert legacy_pid == reply[2]
    own_credentials = credentials(connection.get_unique_name(), os.getpid())
    service_credentials = credentials(owner, reply[2])
    connection.call_sync(
        NAME, OBJECT_PATH, NAME, 'Stop', None, GLib.VariantType.new('()'),
        Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, None)
    print(json.dumps({
        'passed': True, 'service_activation': True,
        'activation_environment': True, 'legacy_pid_api': True,
        'own_credentials': own_credentials,
        'service_credentials': service_credentials,
        'same_uid': reply[1], 'service_pid': reply[2],
    }))


if __name__ == '__main__':
    assert os.getuid() != 0
    assert sys.argv[1:] in ([], ['--service'])
    connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    if sys.argv[1:] == ['--service']:
        serve(connection)
    else:
        probe(connection)
