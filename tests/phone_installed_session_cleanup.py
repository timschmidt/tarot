#!/usr/bin/env python3
"""Independently check stock recovery after installed-session replays.

Reads current processes, cgroups, policies, services and devices. It writes only
the requested private report; no previous VT8-harness exit files are consulted.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import subprocess

from phone_stock_login import processes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suspend', type=Path, required=True)
    parser.add_argument('--launch-cycles', type=Path)
    parser.add_argument('--extra-replay', type=Path, action='append', default=[])
    parser.add_argument('--allow-failed-evidence', action='store_true')
    parser.add_argument('--stock-check', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    suspend = json.loads(args.suspend.read_text())
    launches = json.loads(args.launch_cycles.read_text()) if args.launch_cycles else {'cycles': []}
    stock = json.loads(args.stock_check.read_text())
    assert stock['validation']['passed']
    if not args.allow_failed_evidence:
        assert suspend['validation']['passed'] and len(suspend['cycles']) == 20 and all(c['validated'] for c in suspend['cycles'])
        assert args.launch_cycles and launches['validation']['passed'] and len(launches['cycles']) == 10
    groups = {suspend['compositor_cgroup']}
    for path in args.extra_replay:
        extra = json.loads(path.read_text())
        assert extra['validation'].get('passed', extra['validation'].get('native_replay_passed', False))
        assert extra['binaries_sha256'] == suspend['binaries_sha256']
        if 'compositor_cgroup' in extra:
            groups.add(extra['compositor_cgroup'])
        else:
            assert extra['sessions']
            groups.update(session['compositor_cgroup'] for session in extra['sessions'])
    for c in launches['cycles']:
        replay = json.loads((args.launch_cycles.parent/f"cycle-{c['number']:02}"/'replay/status.json').read_text())
        assert replay['validation']['passed']
        groups.add(replay['compositor_cgroup'])
    result = {'native_validation': suspend['validation'], 'validated_suspend_cycles': sum(c.get('validated', False) for c in suspend['cycles']),
              'completed_launch_cycles': len(launches['cycles']),
              'old_session_cgroups': sorted(groups), 'remaining': [], 'stock_processes': processes()}
    watched = {'phone_installed_session_replay.py', 'phone_installed_session_cycles.py',
               'phone_greeter_choice_probe.py', 'phone_session_picker.py',
               'phone_orientation_replay.py', 'phone_display_preferences.py',
               'phone_resource_sample.py', 'phone_resource_tarot_phase.py',
               'phone_installed_session_resources_replay.py',
               'phone_package_switch.py', 'suspend-request'}
    for proc in Path('/proc').iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            exe = (proc/'exe').resolve()
            argv = (proc/'cmdline').read_bytes().split(b'\0')
            controlled = any(Path(os.fsdecode(a)).name in watched for a in argv[:2] if a)
            controlled |= exe.name == 'dbus-monitor' and any(b'net.hadess.SensorProxy' in a for a in argv)
            if ((proc/'cgroup').read_text() in groups or controlled or
                    exe.name in ['tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard', 'seatd']):
                result['remaining'].append({'pid': int(proc.name), 'exe': str(exe)})
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass

    def command(*argv):
        return subprocess.check_output(argv, text=True, timeout=10).strip()

    result['rtc_alarm'] = Path('/sys/class/rtc/rtc0/wakealarm').read_text().strip()
    result['replay_devices'] = [p.read_text().strip() for p in Path('/sys/class/input').glob('input*/name') if 'Tarot development' in p.read_text()]
    result['temporary_sockets'] = [str(p) for p in [Path('/run/seatd.sock'), Path('/run/user/10000/tarot-drm.sock'), Path('/run/user/10000/tarot-session/control.sock')] if p.exists()]
    result['active_vt'] = Path('/sys/class/tty/tty0/active').read_text().strip()
    result['backlight'] = {n: int((Path('/sys/class/backlight/backlight')/n).read_text()) for n in ['brightness', 'actual_brightness']}
    profile = command('nmcli', '-g', 'GENERAL.CON-UUID', 'device', 'show', 'wlan0')
    result['profile_hash'] = hashlib.sha256(profile.encode()).hexdigest()
    original = json.loads(Path('/home/user/tarot-dev/artifacts/persistence-restore-final/status.json').read_text())['original_profile_hash']
    result['network_devices'] = command('nmcli', '-t', '-f', 'DEVICE,STATE', 'device')
    result['radio'] = command('nmcli', 'radio', 'wifi')
    result['services'] = {s: subprocess.run(['rc-service', s, 'status'], capture_output=True, timeout=10).returncode for s in ['greetd', 'polkit', 'modemmanager', 'networkmanager', 'sshd']}
    result['remaining_tarot_inhibitors'] = [line for line in command('gdbus', 'call', '--system', '--dest', 'org.freedesktop.login1', '--object-path', '/org/freedesktop/login1', '--method', 'org.freedesktop.login1.Manager.ListInhibitors').splitlines() if 'Tarot' in line]
    policies = {'/etc/phrog/greetd-config.toml': '85dde819880207b66bf9f2b7b4a7ffe094fd5768b57ee372ca1ff2c252c3c8bb',
                '/etc/pam.d/greetd': 'c7fa524a268fa31921847d673bceecb131f1b83e612a4d689cd1b0706356597f',
                '/etc/pam.d/tarot-lock': 'f6d73ceeca2acf86ce428709a8ff21ddf5923efb26fe3f6614e61878b8ad2012'}
    result['policy_sha256'] = {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in policies}
    result['binaries_sha256'] = {n: hashlib.sha256(Path('/usr/bin', n).read_bytes()).hexdigest() for n in suspend['binaries_sha256']}
    result['kernel_stats'] = {p.name: p.read_text().strip() for p in Path('/sys/power/suspend_stats').iterdir()}
    result['battery_percent'] = int(Path('/sys/class/power_supply/rk818-battery/capacity').read_text())
    checks = {'no_test_resources': not any(result[k] for k in ['remaining', 'rtc_alarm', 'replay_devices', 'temporary_sockets', 'remaining_tarot_inhibitors']),
              'same_authenticated_stock_session': result['stock_processes'] == stock['after'] and any(p['uid'] == 10000 and p['name'] == 'phosh' for p in result['stock_processes']) and result['active_vt'] == 'tty7',
              'brightness_restored': result['backlight']['brightness'] == result['backlight']['actual_brightness'] == 15,
              'original_network': result['profile_hash'] == original and result['radio'] == 'enabled' and 'usb0:connected' in result['network_devices'],
              'services_running': all(code == 0 for code in result['services'].values()),
              'policies_unchanged': result['policy_sha256'] == policies,
              'tested_binaries_retained': result['binaries_sha256'] == suspend['binaries_sha256']}
    result['validation'] = dict(checks, passed=all(checks.values()))
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.fchown(fd, 10000, pwd.getpwnam('user').pw_gid)
    with os.fdopen(fd, 'w') as stream:
        stream.write(json.dumps(result, indent=2)+'\n')
    assert all(checks.values()), checks
    print('PASS: current authenticated Phosh, services/network/brightness/policies and empty old session cgroups')


if __name__ == '__main__':
    main()
