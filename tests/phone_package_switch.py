#!/usr/bin/env python3
"""Hash-checked local Tarot package switch on the authorized spare phone.

Requires no live Tarot processes. Simulates and requires a Tarot-only change
before applying it. Uses a private root-owned input copy, no network, and no
global signing-key or greeter-policy changes. Authentication/session behavior
must be tested independently after the transaction.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import shutil
import signal
import subprocess
import tempfile


POLICIES = ['/etc/phrog/greetd-config.toml', '/etc/pam.d/greetd',
            '/etc/pam.d/tarot-lock', '/etc/elogind/logind.conf']
BINARIES = ['tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard']


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def state():
    return {'binaries_sha256': {n: sha(Path('/usr/bin', n)) for n in BINARIES},
            'policies_sha256': {p: sha(p) for p in POLICIES},
            'package_owner': subprocess.check_output(['apk', 'info', '--who-owns', '/usr/bin/tarot-session'], text=True).strip()}


def no_tarot_processes():
    found = []
    for p in Path('/proc').iterdir():
        if not p.name.isdecimal(): continue
        try:
            if (p/'comm').read_text().strip() in BINARIES:
                found.append(int(p.name))
        except (FileNotFoundError, ProcessLookupError): pass
    assert not found, 'End the Tarot session before replacing its installed programs'
    assert not Path('/run/user/10000/tarot-session/control.sock').exists()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apk', type=Path, required=True)
    parser.add_argument('--audit', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    audit = json.loads(args.audit.read_text())
    assert audit['validation']['passed'] and audit['validation']['install_scripts_absent']
    assert set(audit['binaries_sha256']) == set(BINARIES)
    version = re.search(r'^pkgver = (0\.1\.0-r\d+)$', audit['pkginfo'], re.M).group(1)
    assert re.search(r'^pkgname = tarot$', audit['pkginfo'], re.M)
    assert args.apk.is_file() and not args.apk.is_symlink() and args.apk.stat().st_size < 32*1024**2
    no_tarot_processes()
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    os.umask(0o077)
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    account = pwd.getpwnam('user')
    os.chown(args.output, account.pw_uid, account.pw_gid)
    config = Path('/home/user/.config/tarot/settings.toml')
    assert not config.is_symlink()
    record = {'before': state(), 'target_version': version,
              'apk_sha256': audit['package_sha256'], 'audit_sha256': sha(args.audit),
              'controller_sha256': sha(__file__), 'settings_sha256_before': sha(config) if config.exists() else None,
              'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip()}

    def log_command(name, command):
        path = args.output/name
        with path.open('x') as stream:
            os.chown(path, account.pw_uid, account.pw_gid)
            result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=180)
        return result.returncode

    try:
        with tempfile.TemporaryDirectory(prefix='tarot-package-switch-') as folder:
            # Verify the private copy that APK will open, not just the mutable
            # source path in the development user's directory.
            copied = Path(folder, f'tarot-{version}.apk')
            shutil.copyfile(args.apk, copied)
            assert sha(copied) == audit['package_sha256']
            command = ['apk', 'add', '--network=no', '--allow-untrusted', str(copied)]
            record['simulation_exit'] = log_command('simulation.log', command[:2]+['--simulate']+command[2:])
            assert record['simulation_exit'] == 0
            plan = (args.output/'simulation.log').read_text()
            changes = re.findall(r'^\(\d+/\d+\) (?:Installing|Upgrading|Downgrading|Reinstalling|Purging) (\S+)', plan, re.M)
            assert changes == ['tarot'], 'Simulation includes another package or no version change'
            no_tarot_processes()
            record['transaction_exit'] = log_command('transaction.log', command)
            assert record['transaction_exit'] == 0
        record['after'] = state()
        assert record['after']['binaries_sha256'] == audit['binaries_sha256']
        assert record['after']['policies_sha256'] == record['before']['policies_sha256']
        assert ('tarot-'+version) in record['after']['package_owner']
        for name in BINARIES:
            metadata = Path('/usr/bin', name).stat()
            assert metadata.st_uid == 0 and metadata.st_gid == 0 and metadata.st_mode & 0o7777 == 0o755
        record['settings_sha256_after'] = sha(config) if config.exists() else None
        assert record['settings_sha256_before'] == record['settings_sha256_after']
        record['validation'] = {'passed': True, 'only_tarot_package_changed': True,
                                'audited_payloads_installed': True, 'policies_and_settings_unchanged': True,
                                'session_behavior_tested': False}
        print('PASS: local Tarot package switch, exact payloads and unchanged policies/settings')
    except BaseException as error:
        record['validation'] = {'passed': False, 'exception_type': type(error).__name__}
        raise
    finally:
        path = args.output/'status.json'
        path.write_text(json.dumps(record, indent=2)+'\n')
        os.chown(path, account.pw_uid, account.pw_gid)


if __name__ == '__main__':
    main()
