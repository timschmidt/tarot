"""Does Tarot's lock-screen PAM authentication unlock the login keyring?

Reads the Secret Service `login` collection on Tarot's own session bus before
and after a real touch-typed unlock. Read-only apart from the session lock.
"""
import json
import pwd
import subprocess
from pathlib import Path
from phone_gesture_replay import request


def secrets(before, *arguments):
    account = pwd.getpwnam('user')
    environ = dict(f.decode().split('=', 1) for f in Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in f)
    env = dict(PATH='/usr/bin:/bin', HOME=account.pw_dir, XDG_RUNTIME_DIR='/run/user/10000',
               DBUS_SESSION_BUS_ADDRESS=environ['DBUS_SESSION_BUS_ADDRESS'])
    run = subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.freedesktop.secrets', *arguments],
                         env=env, user=account.pw_uid, group=account.pw_gid, capture_output=True, text=True, timeout=10)
    assert run.returncode == 0, run.stderr[-200:]
    return run.stdout.strip()


def locked(before):
    return secrets(before, '--object-path', '/org/freedesktop/secrets/collection/login', '--method',
                   'org.freedesktop.DBus.Properties.Get', 'org.freedesktop.Secret.Collection', 'Locked') == '(<true>,)'


def replay(socket, state, before, directory, authenticate):
    r = dict(passed=False)
    r['locked_at_start'] = locked(before)
    # Locking a collection needs no prompt; it models a keyring left locked by direct boot.
    r['lock_reply'] = secrets(before, '--object-path', '/org/freedesktop/secrets', '--method',
                              'org.freedesktop.Secret.Service.Lock', "['/org/freedesktop/secrets/collection/login']")
    r['locked_before_unlock'] = locked(before)
    assert r['locked_before_unlock']
    request(socket, 'lock')
    state('keyring-locked', lambda s: s['lock']['phase'] == 'Locked')
    restored = authenticate('keyring-unlock')
    assert restored['cards'] == before['cards']
    r['locked_after_unlock'] = locked(before)
    r['unlock_opened_keyring'] = r['locked_before_unlock'] and not r['locked_after_unlock']
    r['passed'] = True
    (directory/'keyring-result.json').write_text(json.dumps(r, indent=2)+'\n')
    return r
