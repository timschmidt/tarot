#!/usr/bin/env python3
"""Measure the already-authenticated installed Tarot login while locked/off.

No credential entry, policy changes or synthetic input. Waits for normal idle
off, samples resources, then ends only the verified test session. Root-only QA
on the authorized spare; restore authenticated Phosh independently afterward.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess

from lock_replay import any_request
from notification_replay import await_value
from phone_gesture_replay import request
from phone_resource_sample import effective_uid, proc_stat
from phone_stock_login import processes


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--sample-output', type=Path, required=True)
    p.add_argument('--detach', action='store_true')
    args = p.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    os.umask(0o077)
    account = pwd.getpwnam('user')
    args.output = args.output.resolve(); args.sample_output = args.sample_output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chown(args.output, account.pw_uid, account.pw_gid)
    assert not args.sample_output.exists()
    socket = Path('/run/user/10000/tarot-session/control.sock')
    first = request(socket, 'status')
    pid = first['pid']; proc = Path('/proc', str(pid))
    pidfd = os.pidfd_open(pid)
    start = proc_stat((proc/'stat').read_text())['start']
    assert effective_uid((proc/'status').read_text()) == 10000 and (proc/'exe').resolve() == Path('/usr/bin/tarot')
    assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty7'
    assert first['lock']['first_presented_protected'] and first['lock']['unprotected_presentations'] == 0
    assert first['lock']['idle']['policy']['lock_after']['secs'] == 120 and first['lock']['idle']['policy']['off_after']['secs'] == 180
    record = {'pid': pid, 'start': start, 'compositor_cgroup': (proc/'cgroup').read_text(),
              'binaries_sha256': {n: hashlib.sha256(Path('/usr/bin', n).read_bytes()).hexdigest() for n in ['tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard']},
              'first': first, 'phase_source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'sampler_sha256': hashlib.sha256(Path(__file__).with_name('phone_resource_sample.py').read_bytes()).hexdigest()}
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    if args.detach:
        child = os.fork()
        if child:
            print('Detached Tarot resource-phase PID', child, flush=True); return
        os.setsid()
        null = os.open('/dev/null', os.O_RDONLY); os.dup2(null, 0); os.close(null)
        log = os.open(args.output/'controller.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.fchown(log, account.pw_uid, account.pw_gid)
        os.dup2(log, 1); os.dup2(log, 2); os.close(log)

    def save():
        path = args.output/'status.json'
        path.write_text(json.dumps(record, indent=2)+'\n')
        os.chown(path, account.pw_uid, account.pw_gid)

    def alive():
        try:
            return ((proc/'exe').resolve() == Path('/usr/bin/tarot')
                    and proc_stat((proc/'stat').read_text())['start'] == start
                    and effective_uid((proc/'status').read_text()) == 10000)
        except (FileNotFoundError, ProcessLookupError):
            return False

    try:
        record['stage'] = 'waiting-for-normal-idle-off'; save()
        off = await_value(lambda: request(socket, 'status'), lambda s: s['pid'] == pid and s['lock']['phase'] == 'Locked' and s['lock']['idle']['output_off'], seconds=200)
        assert off['lock']['unprotected_presentations'] == 0 and off['lock']['last_presented_surface'] is True
        assert any_request(socket, 'capture', path=str(args.output/'denied.png')) == {'ok': False, 'error': 'Session is locked'}
        panels = list(Path('/sys/class/drm').glob('card*-DSI-*/dpms'))
        assert len(panels) == 1 and panels[0].read_text().strip() == 'Off'
        assert Path('/sys/class/backlight/backlight/bl_power').read_text().strip() == '4'
        record.update(stage='sampling-locked-panel-off', off=off); save()
        subprocess.run(['python3', str(Path(__file__).with_name('phone_resource_sample.py')), '--session', 'tarot', '--output', str(args.sample_output), '--label', 'locked-panel-off', '--seconds', '300', '--interval', '5'], check=True, timeout=360)
        sample = json.loads((args.sample_output/'status.json').read_text()); assert sample['collection_complete']
        after = request(socket, 'status')
        assert alive() and after['pid'] == pid and after['lock']['phase'] == 'Locked' and after['lock']['idle']['output_off']
        assert after['lock']['unprotected_presentations'] == 0
        record.update(after=after, validation={'passed': True, 'normal_idle_policy_unchanged': True, 'no_ordinary_presentations': True, 'credentials_entered': False, 'kernel_suspend_test': False})
    except BaseException as error:
        record['validation'] = {'passed': False, 'exception_type': type(error).__name__}
        raise
    finally:
        try:
            if alive(): signal.pidfd_send_signal(pidfd, signal.SIGTERM)
        finally:
            os.close(pidfd)
        try:
            await_value(processes, lambda ps: not alive() and any(p['name'] == 'phrog' and p['uid'] == 109 for p in ps), seconds=35)
            remaining = []
            for q in Path('/proc').iterdir():
                if not q.name.isdecimal(): continue
                try:
                    if (q/'cgroup').read_text() == record['compositor_cgroup']: remaining.append(int(q.name))
                except (FileNotFoundError, ProcessLookupError): pass
            assert not remaining and not socket.exists()
            record['cgroup_empty_and_socket_removed'] = True
        except BaseException as error:
            record['cleanup_error'] = type(error).__name__
            record.setdefault('validation', {})['passed'] = False
        record['stage'] = 'terminal'; save()


if __name__ == '__main__':
    main()
