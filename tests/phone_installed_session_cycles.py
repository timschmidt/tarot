#!/usr/bin/env python3
"""Repeat normal greetd/PAM logins, native input/app checks and clean exits.

This root-only spare-phone controller keeps the supplied password in memory.
Each compositor and application runs in its actual ordinary-user PAM session.
The terminal result leaves Phrog ready for an independently verified login.
"""
import argparse
import getpass
import json
import os
from pathlib import Path
import pwd
import signal
import time
from types import SimpleNamespace

from phone_installed_session_replay import run
from phone_stock_login import login


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cycles', type=int, default=10)
    parser.add_argument('--detach', action='store_true')
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    assert 1 <= args.cycles <= 10
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    account = pwd.getpwnam('user')
    os.chown(args.output, account.pw_uid, account.pw_gid)
    password = getpass.getpass('Device password for repeated normal session/PAM replay: ')
    assert password and all(' ' <= c <= '~' for c in password)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    if args.detach:
        child = os.fork()
        if child:
            password = None
            print('Detached installed-session launch-cycle PID', child, flush=True)
            return
        os.setsid()
        null = os.open('/dev/null', os.O_RDONLY)
        os.dup2(null, 0)
        os.close(null)
        log = os.open(args.output/'controller.log', os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.chown(args.output/'controller.log', account.pw_uid, account.pw_gid)
        os.dup2(log, 1)
        os.dup2(log, 2)
        os.close(log)
    records = {'cycles': []}

    def save(name):
        (args.output/name).write_text(json.dumps(records, indent=2)+'\n')

    try:
        for number in range(1, args.cycles+1):
            folder = args.output/f'cycle-{number:02}'
            folder.mkdir(mode=0o700)
            os.chown(folder, account.pw_uid, account.pw_gid)
            # Let the newly returned greeter finish its initial PAM/socket
            # setup before this independent QA client starts a conversation.
            time.sleep(1)
            login(SimpleNamespace(output=folder/'login', session='tarot', diagnostics=True), password)
            replay = folder/'replay'
            replay.mkdir(mode=0o700)
            os.chown(replay, account.pw_uid, account.pw_gid)
            run(SimpleNamespace(output=replay, cycles=0, require_audio=True, crash=False), password)
            result = json.loads((replay/'status.json').read_text())
            assert result['validation']['passed']
            records['cycles'].append({'number': number, 'package_owner': result['package_owner'],
                                      'binaries_sha256': result['binaries_sha256'],
                                      'validation': result['validation']})
            if number > 1:
                assert records['cycles'][0]['binaries_sha256'] == result['binaries_sha256']
            save('running.json')
        records['validation'] = {'passed': True, 'normal_launch_exit_cycles': args.cycles,
                                 'real_greetd_and_locker_pam_each_cycle': True,
                                 'native_app_audio_and_cgroup_cleanup_each_cycle': True,
                                 'manual_sensor_tested': False, 'password_logged': False}
        print('PASS: repeated normal installed login/input/app/audio/exit cycles', flush=True)
    except BaseException as error:
        records['validation'] = {'passed': False, 'exception_type': type(error).__name__}
        raise
    finally:
        password = None
        save('status.json')
        (args.output/'running.json').unlink(missing_ok=True)


if __name__ == '__main__':
    main()
