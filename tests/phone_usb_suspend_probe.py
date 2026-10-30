#!/usr/bin/env python3
"""Qualify the dwc3 gadget suspend failure on the PinePhone Pro.

Runs RTC-woken manager suspends through login1 SuspendWithFlags(1), first with
the configfs gadget bound (control), then with it temporarily unbound while no
USB host is attached. The original binding is always restored. Detaches before
the first request because Wi-Fi SSH stops during sleep. No password is used.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time

STATS = Path('/sys/power/suspend_stats')
RTC = Path('/sys/class/rtc/rtc0')


def stats():
    return {p.name: (int(p.read_text()) if p.read_text().strip().lstrip('-').isdigit() else p.read_text().strip())
            for p in STATS.iterdir() if p.is_file()}


def udc_state(udc):
    base = Path('/sys/class/udc', udc)
    return dict(state=(base/'state').read_text().strip(), function=(base/'function').read_text().strip()
                if (base/'function').exists() else None)


def gdbus(*args):
    return subprocess.run(['gdbus', 'call', '--system', '--dest', 'org.freedesktop.login1',
                           '--object-path', '/org/freedesktop/login1', *args],
                          capture_output=True, text=True, timeout=10)


def attempt(label, seconds, log):
    before = stats()
    assert not (RTC/'wakealarm').read_text().strip(), 'Another wake alarm is armed'
    (RTC/'wakealarm').write_text('+%d' % seconds)
    started = time.monotonic()
    call = gdbus('--method', 'org.freedesktop.login1.Manager.SuspendWithFlags', '1')
    record = dict(label=label, before=before, call_returncode=call.returncode,
                  call_error=call.stderr.strip()[-300:] if call.returncode else None)
    try:
        deadline = started + seconds + 60
        while time.monotonic() < deadline:
            now = stats()
            if now['success'] != before['success'] or now['fail'] != before['fail']:
                break
            time.sleep(1)
        # Let a fallback s2idle attempt finish before the final counters.
        time.sleep(15)
        record['after'] = stats()
    finally:
        (RTC/'wakealarm').write_text('0')
    record['elapsed'] = time.monotonic() - started
    record['succeeded'] = record['after']['success'] > before['success'] and record['after']['fail'] == before['fail']
    record['kernel_log'] = [line for line in subprocess.run(['dmesg'], capture_output=True, text=True).stdout.splitlines()[-80:]
                            if any(k in line for k in ['PM:', 'dwc3', 'suspend', 'resume', 'Freezing'])][-30:]
    log(record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--gadget', default='g1')
    parser.add_argument('--seconds', type=int, default=20)
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    gadget = Path('/sys/kernel/config/usb_gadget', args.gadget, 'UDC')
    original = gadget.read_text().strip()
    assert original, 'Gadget must start bound so the control reproduces the default state'
    assert udc_state(original)['state'] == 'not attached', 'Only qualified without an attached USB host'
    args.output.mkdir(mode=0o700, exist_ok=False)
    if os.fork():
        print('Detached USB suspend probe', flush=True)
        return
    os.setsid()
    result = dict(passed=False, original_udc=original, udc_before=udc_state(original),
                  idle_action=gdbus('--method', 'org.freedesktop.DBus.Properties.Get',
                                    'org.freedesktop.login1.Manager', 'IdleAction').stdout.strip(),
                  inhibitors=gdbus('--method', 'org.freedesktop.login1.Manager.ListInhibitors').stdout.strip(),
                  attempts=[])
    path = args.output/'result.json'

    def save(_=None):
        path.with_suffix('.tmp').write_text(json.dumps(result, indent=2)+'\n')
        path.with_suffix('.tmp').replace(path)
    try:
        save()
        result['attempts'].append(attempt('gadget-bound-control', args.seconds, save))
        time.sleep(20)
        gadget.write_text('\n')
        result['unbound_state'] = gadget.read_text().strip()
        try:
            result['attempts'].append(attempt('gadget-unbound', args.seconds, save))
        finally:
            gadget.write_text(original+'\n')
        result['udc_after'] = udc_state(original)
        result['restored'] = gadget.read_text().strip() == original
        result['passed'] = result['restored']
    except BaseException as error:
        result['error'] = '%s: %s' % (type(error).__name__, error)
        if gadget.read_text().strip() != original:
            gadget.write_text(original+'\n')
        result['restored'] = gadget.read_text().strip() == original
    finally:
        save()


if __name__ == '__main__':
    main()
