#!/usr/bin/env python3
"""Native boot-destination acceptance on the PinePhone Pro, in reboot-separated phases.

select:  record the original greetd selection, prove an inactive caller is
         refused by Polkit, then select direct boot through the root helper.
direct:  after a reboot, require greetd's initial session to be a valid local
         Wayland Tarot session with no greeter, then end it and require the
         greeter fallback; restore the greeter selection.
final:   after another reboot, require the original configuration and greeter.
Only process identities verified through pidfds receive signals. No password
is read here; normal login afterwards uses the existing greetd helper.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess
import time

HELPER = '/usr/libexec/tarot-boot-destination'
CONFD = Path('/etc/conf.d/greetd')
MANAGED = Path('/etc/tarot/greetd-direct.toml')
STATE = Path('/etc/tarot/boot-destination')
USER = 'user'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def processes():
    found = []
    for proc in Path('/proc').iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            found.append(dict(pid=int(proc.name), uid=proc.stat().st_uid, name=(proc/'comm').read_text().strip(),
                              start=(proc/'stat').read_text().rsplit(')', 1)[1].split()[19]))
        except (FileNotFoundError, ProcessLookupError):
            continue
    return found


def await_value(read, condition, seconds=30):
    deadline = time.monotonic() + seconds
    while True:
        value = read()
        if condition(value):
            return value
        assert time.monotonic() < deadline, value
        time.sleep(.5)


def gdbus(*args):
    return subprocess.check_output(['gdbus', 'call', '--system', '--dest', 'org.freedesktop.login1', *args], text=True, timeout=10).strip()


def session_of(pid):
    path = gdbus('--object-path', '/org/freedesktop/login1', '--method',
                 'org.freedesktop.login1.Manager.GetSessionByPID', str(pid)).split("'")[1]
    props = {}
    for key in ['User', 'Type', 'Class', 'Active', 'Remote', 'VTNr', 'Service', 'Seat']:
        props[key] = gdbus('--object-path', path, '--method', 'org.freedesktop.DBus.Properties.Get',
                           'org.freedesktop.login1.Session', key)
    return path, props


def helper(*args, uid=None):
    env = dict(PATH='/usr/bin:/bin')
    if uid is not None:
        env['PKEXEC_UID'] = str(uid)
    run = subprocess.run([HELPER, *args], env=env, capture_output=True, text=True, timeout=30)
    return dict(code=run.returncode, out=run.stdout.strip(), err=run.stderr.strip()[-300:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['select', 'direct', 'final'])
    parser.add_argument('--record', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    account = pwd.getpwnam(USER)
    record = json.loads(args.record.read_text()) if args.record.exists() else dict(phases={})
    phase = dict(boot_id=Path('/proc/sys/kernel/random/boot_id').read_text().strip(), passed=False)
    record['phases'][args.phase] = phase

    def save():
        args.record.write_text(json.dumps(record, indent=2)+'\n')
    try:
        if args.phase == 'select':
            assert not args.record.with_suffix('.started').exists()
            args.record.with_suffix('.started').write_text('')
            record['original'] = dict(confd_sha256=sha(CONFD), confd=CONFD.read_text(),
                                      mode=CONFD.stat().st_mode & 0o7777, uid=CONFD.stat().st_uid)
            assert not MANAGED.exists() and not STATE.exists()
            phase['status_before'] = json.loads(helper('status')['out'])
            assert phase['status_before'] == dict(destination='Greeter', user=None)
            # An ordinary process outside the active seat session must be refused.
            denied = subprocess.run(['pkexec', HELPER, 'set', 'tarot'], user=account.pw_uid, group=account.pw_gid,
                                    env=dict(PATH='/usr/bin:/bin', HOME=account.pw_dir), stdin=subprocess.DEVNULL,
                                    capture_output=True, text=True, timeout=60)
            phase['inactive_pkexec'] = dict(code=denied.returncode, err=denied.stderr.strip()[-200:])
            assert denied.returncode in (126, 127) and sha(CONFD) == record['original']['confd_sha256']
            assert helper('set', 'tarot')['code'] != 0, 'Root without PKEXEC_UID must be refused'
            assert helper('set', 'tarot', uid=0)['code'] != 0, 'Direct boot must not target root'
            phase['select'] = helper('set', 'tarot', uid=account.pw_uid)
            assert phase['select']['code'] == 0, phase['select']
            assert json.loads(phase['select']['out']) == dict(destination='Tarot', user=USER)
            changed = [line for line in CONFD.read_text().splitlines() if line not in record['original']['confd'].splitlines()]
            assert changed == ['cfgfile="%s"' % MANAGED], changed
            assert CONFD.stat().st_mode & 0o7777 == record['original']['mode'] and CONFD.stat().st_uid == record['original']['uid']
            phase['managed'] = MANAGED.read_text()
            phase['managed_sha256'] = sha(MANAGED)
            assert 'service = "tarot-greetd"' in phase['managed'] and 'user = "%s"' % USER in phase['managed']
            assert Path('/etc/pam.d/tarot-greetd').is_file() and Path('/usr/share/tarot/greetd-session.conf').is_file()
        elif args.phase == 'direct':
            assert record['phases']['select']['passed'] and phase['boot_id'] != record['phases']['select']['boot_id']
            ps = await_value(processes, lambda ps: any(p['uid'] == account.pw_uid and p['name'] == 'tarot' for p in ps), seconds=90)
            phase['greeter_absent'] = not any(p['name'] == 'phrog' for p in ps)
            assert phase['greeter_absent'], 'Direct boot must not show the greeter first'
            tarot = next(p for p in ps if p['uid'] == account.pw_uid and p['name'] == 'tarot')
            assert os.readlink('/proc/%d/exe' % tarot['pid']) == '/usr/bin/tarot'
            argv = Path('/proc/%d/cmdline' % tarot['pid']).read_bytes().split(b'\0')
            assert b'--lock-client' in argv and b'--control-socket' not in argv
            await_value(processes, lambda ps: any(p['uid'] == account.pw_uid and p['name'] == 'tarot-lock' for p in ps))
            phase['locker_running'] = True
            path, props = session_of(tarot['pid'])
            phase['session'] = dict(path=path, **props)
            assert "'wayland'" in props['Type'] and "'user'" in props['Class'] and 'true' in props['Active']
            assert 'false' in props['Remote'] and 'uint32 7' in props['VTNr'] and "'tarot-greetd'" in props['Service']
            assert 'uint32 %d' % account.pw_uid in props['User']
            # Ending the direct session must fall back to the greeter once.
            descriptor = os.pidfd_open(tarot['pid'])
            try:
                assert next(p for p in processes() if p['pid'] == tarot['pid']) == tarot
                signal.pidfd_send_signal(descriptor, signal.SIGTERM)
            finally:
                os.close(descriptor)
            ps = await_value(processes, lambda ps: any(p['name'] == 'phrog' for p in ps)
                             and not any(p['uid'] == account.pw_uid and p['name'] in ['tarot', 'tarot-session'] for p in ps), seconds=60)
            phase['greeter_fallback'] = True
            time.sleep(10)
            phase['initial_session_not_repeated'] = not any(p['uid'] == account.pw_uid and p['name'] == 'tarot' for p in processes())
            assert phase['initial_session_not_repeated']
            phase['restore'] = helper('set', 'greeter', uid=account.pw_uid)
            assert phase['restore']['code'] == 0 and json.loads(phase['restore']['out']) == dict(destination='Greeter', user=None)
            assert sha(CONFD) == record['original']['confd_sha256'] and not MANAGED.exists() and not STATE.exists()
        else:
            assert record['phases']['direct']['passed'] and phase['boot_id'] != record['phases']['direct']['boot_id']
            ps = await_value(processes, lambda ps: any(p['name'] == 'phrog' for p in ps), seconds=90)
            phase['greeter_at_boot'] = True
            phase['no_direct_session'] = not any(p['uid'] == account.pw_uid and p['name'] == 'tarot' for p in ps)
            assert phase['no_direct_session']
            assert sha(CONFD) == record['original']['confd_sha256'] and not MANAGED.exists() and not STATE.exists()
            assert json.loads(helper('status')['out']) == dict(destination='Greeter', user=None)
        phase['passed'] = True
        print('PASS', args.phase, flush=True)
    except BaseException as error:
        phase['error'] = '%s: %s' % (type(error).__name__, error)
        raise
    finally:
        save()


if __name__ == '__main__':
    main()
