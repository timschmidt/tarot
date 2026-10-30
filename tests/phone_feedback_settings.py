"""Snapshot and restore logical feedbackd GSettings values and user overrides.

Run as the actual graphical user with that login's private bus environment.
No raw dconf database is copied while its service is running.
"""
import json
import os
import subprocess
import sys
import time

import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib


def snapshot():
    settings = Gio.Settings.new('org.sigxcpu.feedbackd')
    schema = settings.props.settings_schema
    result = {}
    for key in schema.list_keys():
        value = settings.get_value(key)
        user = settings.get_user_value(key)
        result[key] = dict(type=value.get_type_string(), effective=value.unpack(),
                           user=None if user is None else user.unpack())
    return result


def restore(original):
    assert os.geteuid() == 10000
    current = snapshot()
    assert {k: v for k, v in current.items() if k != 'profile'} == {
        k: v for k, v in original.items() if k != 'profile'}
    settings = Gio.Settings.new('org.sigxcpu.feedbackd')
    expected = original['profile']
    assert expected['type'] == 's' and expected['effective'] in ['full', 'quiet', 'silent']
    assert settings.set_value('profile', GLib.Variant('s', expected['effective']))
    Gio.Settings.sync()
    deadline = time.monotonic() + 10
    while True:
        raw = subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.sigxcpu.Feedback',
              '--object-path', '/org/sigxcpu/Feedback', '--method', 'org.freedesktop.DBus.Properties.Get',
              'org.sigxcpu.Feedback', 'Profile'], text=True, timeout=5).strip()
        if raw == "(<'" + expected['effective'] + "'>,)":
            break
        assert time.monotonic() < deadline, raw
        time.sleep(.1)
    if expected['user'] is None:
        settings.reset('profile')
    else:
        assert settings.set_value('profile', GLib.Variant('s', expected['user']))
    Gio.Settings.sync()
    assert snapshot() == original
    return dict(passed=True, original_user_override_restored=True, values=snapshot())


if __name__ == '__main__':
    assert os.geteuid() == 10000 and os.environ.get('DBUS_SESSION_BUS_ADDRESS')
    if sys.argv[1:] == ['snapshot']:
        print(json.dumps(snapshot(), sort_keys=True))
    elif sys.argv[1:] == ['restore']:
        print(json.dumps(restore(json.load(sys.stdin)), sort_keys=True))
    else:
        raise SystemExit('snapshot | restore (JSON on stdin)')
