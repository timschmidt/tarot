#!/usr/bin/env python3
"""Independently inspect restoration after an actual-suspend replay.

Read-only apart from the requested private report. A successful cleanup does not
turn an interrupted replay into a successful suspend reliability run.
"""
import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--allow-failed-evidence', action='store_true')
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    base = Path('/home/user/tarot-dev')
    root = base/'artifacts'
    record = json.loads(args.evidence.read_text())
    validation = record.get('validation', {})
    assert args.allow_failed_evidence or validation.get('passed') is True
    assert not Path(f"/proc/{record['pid']}").exists(), 'Replay controller must have exited'
    assert record['boot_id'] == Path('/proc/sys/kernel/random/boot_id').read_text().strip()

    def command(*argv):
        return subprocess.check_output(argv, text=True, timeout=10).strip()

    result = {
        'evidence': str(args.evidence),
        'native_validation': validation,
        'validated_cycles': sum(c.get('validated', False) for c in record['cycles']),
        'harness_exit': record['harness_exit'],
        'exits': {name: int((root/name).read_text()) for name in
                  ['drm-client.exit', 'local-session.exit', 'drm-launcher.exit']},
        'processes': [], 'stock_processes': [], 'daemons': [],
        'boot_id': record['boot_id'],
        'active_vt': Path('/sys/class/tty/tty0/active').read_text().strip(),
        'vt_allocations': {name: Path('/sys/class/vc', name).exists() for name in ['vcs7', 'vcs8']},
        'rtc_alarm': Path('/sys/class/rtc/rtc0/wakealarm').read_text().strip(),
        'kernel_stats': {p.name: p.read_text().strip() for p in Path('/sys/power/suspend_stats').iterdir()},
        'battery_percent': int(Path('/sys/class/power_supply/rk818-battery/capacity').read_text()),
    }
    for proc in Path('/proc').iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            exe = os.readlink(proc/'exe')
            comm = (proc/'comm').read_text().strip()
            if exe.startswith(str(base/'bin')+'/') or exe in [
                '/usr/bin/seatd', '/usr/sbin/unix_chkpwd', '/sbin/unix_chkpwd',
                '/usr/bin/gtk3-demo', '/usr/bin/gtk4-demo',
            ]:
                result['processes'].append({'pid': int(proc.name), 'exe': exe})
            if comm in ['phoc', 'phosh', 'phrog']:
                result['stock_processes'].append({'pid': int(proc.name), 'uid': proc.stat().st_uid, 'name': comm})
            if exe in ['/usr/lib/polkit-1/polkitd', '/usr/sbin/ModemManager']:
                parent = int((proc/'stat').read_text().rsplit(')', 1)[1].split()[1])
                result['daemons'].append({'exe': exe, 'parent_args':
                    (Path('/proc')/str(parent)/'cmdline').read_bytes().replace(b'\0', b' ').decode()})
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    result['temporary_sockets'] = [str(p) for p in [Path('/run/user/10000/tarot-drm.sock'), Path('/run/seatd.sock')] if p.exists()]
    result['replay_devices'] = [p.read_text().strip() for p in Path('/sys/class/input').glob('input*/name') if 'Tarot development' in p.read_text()]
    result['sha256'] = {n: hashlib.sha256((base/'bin'/n).read_bytes()).hexdigest() for n in [
        'tarot', 'tarot-keyboard', 'tarot-lock', 'lock-probe', 'local-session', 'recovery-client', 'suspend-request']}
    result['backlight'] = {n: int((Path('/sys/class/backlight/backlight')/n).read_text()) for n in ['brightness', 'actual_brightness']}
    result['network_devices'] = command('nmcli', '-t', '-f', 'DEVICE,STATE', 'device')
    result['radio'] = command('nmcli', 'radio', 'wifi')
    profile = command('nmcli', '-g', 'GENERAL.CON-UUID', 'device', 'show', 'wlan0')
    result['profile_hash'] = hashlib.sha256(profile.encode()).hexdigest()
    result['original_profile_retained'] = result['profile_hash'] == json.loads((root/'persistence-restore-final/status.json').read_text())['original_profile_hash']
    result['remaining_tarot_inhibitors'] = [line for line in command('gdbus', 'call', '--system', '--dest', 'org.freedesktop.login1', '--object-path', '/org/freedesktop/login1', '--method', 'org.freedesktop.login1.Manager.ListInhibitors').splitlines() if "'Tarot'" in line or '"Tarot"' in line or 'Tarot suspend replay' in line]
    result['services'] = {}
    for service in ['greetd', 'polkit', 'modemmanager', 'networkmanager', 'sshd']:
        checked = subprocess.run(['rc-service', service, 'status'], capture_output=True, text=True, timeout=10)
        result['services'][service] = {'exit': checked.returncode, 'text': checked.stdout+checked.stderr}
    result['graphics_errors'] = [line for line in (root/'drm.log').read_text().splitlines() if any(e in line for e in ['GL_INVALID', 'GL_OUT_OF_MEMORY', 'GL error'])]
    result['coordination_errors'] = [line for line in (root/'drm.log').read_text().splitlines() if 'sleep coordination unavailable' in line]
    result['restored_level'] = 'authenticated Phosh' if any(p['name'] == 'phosh' and p['uid'] == 10000 for p in result['stock_processes']) else 'greetd/Phrog login, not authenticated user desktop'
    policy = Path('/etc/pam.d/tarot-lock')
    result['pam_policy'] = {'sha256': hashlib.sha256(policy.read_bytes()).hexdigest(), 'uid': policy.stat().st_uid, 'mode': oct(policy.stat().st_mode & 0o777)}
    checks = {
        'harness_helpers_zero': result['harness_exit'] == 0 and result['exits']['drm-client.exit'] == result['exits']['local-session.exit'] == 0,
        'launcher': result['exits']['drm-launcher.exit'] == 0 or (result['exits']['drm-launcher.exit'] == 8 and 'openvt deferred VT deallocation; compositor status: 0' in (args.evidence.parent/'harness.log').read_text()),
        'no_test_resources': not any(result[k] for k in ['processes', 'temporary_sockets', 'replay_devices', 'remaining_tarot_inhibitors', 'rtc_alarm', 'graphics_errors']),
        'stock_vt': result['active_vt'] == 'tty7' and result['vt_allocations'] == {'vcs7': True, 'vcs8': False} and any(p['name'] in ['phosh', 'phrog'] for p in result['stock_processes']),
        'brightness': result['backlight']['brightness'] == result['backlight']['actual_brightness'] == 15,
        'network': result['original_profile_retained'] and result['radio'] == 'enabled' and 'usb0:connected' in result['network_devices'],
        'services': all(s['exit'] == 0 for s in result['services'].values()) and len(result['daemons']) == 2 and all('supervise-daemon' in d['parent_args'] for d in result['daemons']),
        'pam_policy': result['pam_policy']['uid'] == 0 and result['pam_policy']['mode'] == '0o644',
    }
    result['cleanup_validation'] = dict(checks, passed=all(checks.values()))
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'w') as stream:
        stream.write(json.dumps(result, indent=2)+'\n')
    assert all(checks.values()), checks
    print('PASS: independently restored stock session/services/network/brightness; test resources removed; raw launcher status retained')


if __name__ == '__main__':
    main()
