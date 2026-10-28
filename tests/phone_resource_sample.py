#!/usr/bin/env python3
"""Read-only shell resource sampling on the authorized spare PinePhone Pro.

Does not keep the display awake, inject input, capture frames, change services,
or program alarms. CPU summaries require unchanged process membership and no
kernel sleep. Raw phase changes remain evidence, not a passing idle budget.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import signal
import time


NAMES = {'tarot': {'tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard'},
         'phosh': {'phoc', 'phosh'}}


def proc_stat(text):
    fields = text.rsplit(')', 1)[1].split()
    return {'start': int(fields[19]), 'cpu_ticks': int(fields[11])+int(fields[12])}


def effective_uid(text):
    line = next(line for line in text.splitlines() if line.startswith('Uid:'))
    return int(line.split()[2])


def memory(text):
    values = {}
    for line in text.splitlines():
        match = re.fullmatch(r'(Pss|Rss|SwapPss):\s+(\d+) kB', line)
        if match:
            values[match[1]+'_bytes'] = int(match[2])*1024
    if not {'Pss_bytes', 'Rss_bytes'} <= values.keys():
        raise ValueError('Incomplete smaps_rollup')
    return values


def gpu_client(text, device):
    fields = dict(line.split(':', 1) for line in text.splitlines() if ':' in line)
    if 'drm-client-id' not in fields or 'drm-driver' not in fields:
        return None
    counters = {}
    for name, value in fields.items():
        if not name.startswith('drm-'):
            continue
        match = re.fullmatch(r'\s*(\d+)\s*(KiB|bytes|ns|Hz)?\s*', value)
        if match and name != 'drm-client-id':
            number, unit = int(match[1]), match[2] or 'count'
            counters[name] = {'value': number*1024 if unit == 'KiB' else number,
                              'unit': 'bytes' if unit == 'KiB' else unit}
    return {'device': device, 'driver': fields['drm-driver'].strip(),
            'client_id': int(fields['drm-client-id']), 'counters': counters}


def deduplicate_gpu(clients):
    unique = {}
    for client in clients:
        key = (client['device'], client['driver'], client['client_id'])
        if key not in unique:
            unique[key] = dict(client, descriptors=1, counters=dict(client['counters']))
            continue
        old = unique[key]
        old['descriptors'] += 1
        # Different descriptor reads are sequential snapshots of one client.
        # Retain the maximum observed counter rather than summing aliases.
        for name, value in client['counters'].items():
            if name in old['counters'] and old['counters'][name]['unit'] != value['unit']:
                raise ValueError('Inconsistent GPU counter units')
            if name not in old['counters'] or old['counters'][name]['value'] < value['value']:
                old['counters'][name] = value
    return list(unique.values())


def clocks():
    return {'monotonic': time.monotonic(),
            'boottime': time.clock_gettime(time.CLOCK_BOOTTIME),
            'realtime': time.time()}


def sample(session, uid):
    members, gpu, raced = [], [], []
    for proc in sorted(Path('/proc').iterdir()):
        if not proc.name.isdecimal():
            continue
        included = False
        try:
            # Hardening can change procfs ownership. Read the actual effective
            # UID rather than treating directory ownership as process identity.
            if effective_uid((proc/'status').read_text()) != uid:
                continue
            exe = (proc/'exe').resolve(strict=True)
            if exe.name not in NAMES[session]:
                continue
            included = True
            process_gpu = []
            before = proc_stat((proc/'stat').read_text())
            p = {'pid': int(proc.name), 'exe': str(exe), **before,
                 **memory((proc/'smaps_rollup').read_text()),
                 'fd_count': len(list((proc/'fd').iterdir()))}
            for fd in (proc/'fdinfo').iterdir():
                try:
                    info = fd.read_text()
                    if 'drm-client-id:' not in info:
                        continue
                    dev = (proc/'fd'/fd.name).stat().st_rdev
                    device = str(Path(f'/sys/dev/char/{os.major(dev)}:{os.minor(dev)}/device').resolve(strict=True))
                    client = gpu_client(info, device)
                    if client:
                        process_gpu.append(client)
                except FileNotFoundError:
                    # Closing a descriptor is normal. The sample is explicitly
                    # sequential; the fd count and GPU readings are not atomic.
                    continue
            after = proc_stat((proc/'stat').read_text())
            if before['start'] != after['start'] or (proc/'exe').resolve(strict=True) != exe:
                raise ProcessLookupError('Process identity changed during sample')
            members.append(p)
            gpu.extend(process_gpu)
        except (FileNotFoundError, ProcessLookupError):
            if included:
                raced.append(int(proc.name))
    return {'clocks': clocks(), 'members': members, 'raced_pids': raced,
            'gpu_clients': deduplicate_gpu(gpu),
            'summed_pss_bytes': sum(p['Pss_bytes'] for p in members),
            'summed_fd_count': sum(p['fd_count'] for p in members),
            'battery': {name: (Path('/sys/class/power_supply/rk818-battery')/name).read_text().strip()
                        for name in ['capacity', 'status', 'current_now', 'voltage_now']},
            'backlight': {name: (Path('/sys/class/backlight/backlight')/name).read_text().strip()
                          for name in ['brightness', 'actual_brightness', 'bl_power']},
            'panel_dpms': {p.parent.name: p.read_text().strip()
                           for p in Path('/sys/class/drm').glob('card*-DSI-*/dpms')},
            'network_operstate': {p.parent.name: p.read_text().strip()
                                  for p in Path('/sys/class/net').glob('*/operstate')},
            'rfkill': [{name: (p/name).read_text().strip() for name in ['type', 'soft', 'hard']}
                       for p in Path('/sys/class/rfkill').glob('rfkill*')],
            'thermal_millicelsius': {p.parent.name: p.read_text().strip()
                                     for p in Path('/sys/class/thermal').glob('thermal_zone*/temp')}}


def cpu_summary(samples, ticks_per_second):
    def identities(s):
        return {(p['pid'], p['start'], p['exe']) for p in s['members']}
    stable = bool(samples and samples[0]['members']) and all(
        identities(s) == identities(samples[0]) and not s['raced_pids'] for s in samples)
    mono = samples[-1]['clocks']['monotonic']-samples[0]['clocks']['monotonic']
    boot = samples[-1]['clocks']['boottime']-samples[0]['clocks']['boottime']
    no_sleep = abs(boot-mono) < .25
    value = None
    if stable and no_sleep and mono > 0:
        ticks = sum(p['cpu_ticks'] for p in samples[-1]['members'])-sum(p['cpu_ticks'] for p in samples[0]['members'])
        if ticks < 0:
            raise ValueError('CPU counter regressed')
        value = 100*ticks/ticks_per_second/mono
    return {'unchanged_membership': stable, 'no_observed_kernel_sleep': no_sleep,
            'monotonic_seconds': mono, 'suspended_seconds_estimate': boot-mono,
            'percent_of_one_core': value}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--session', choices=NAMES, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=300)
    parser.add_argument('--interval', type=int, default=5)
    parser.add_argument('--label', required=True, help='Operator-defined workload/phase; no private app text')
    parser.add_argument('--detach', action='store_true')
    args = parser.parse_args()
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    assert 1 <= args.interval <= 30 and 10 <= args.seconds <= 600
    os.umask(0o077)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    account = pwd.getpwnam('user')
    os.chown(args.output, account.pw_uid, account.pw_gid)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    if args.detach:
        child = os.fork()
        if child:
            print('Detached read-only resource sampler PID', child, flush=True)
            return
        os.setsid()
        null = os.open('/dev/null', os.O_RDONLY)
        os.dup2(null, 0); os.close(null)
        log = os.open(args.output/'controller.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.fchown(log, account.pw_uid, account.pw_gid)
        os.dup2(log, 1); os.dup2(log, 2); os.close(log)
    record = {'session': args.session, 'label': args.label, 'samples': [],
              'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
              'sampler_pid': os.getpid(), 'clock_ticks_per_second': os.sysconf('SC_CLK_TCK'),
              'kernel_release': os.uname().release,
              'os_release': Path('/etc/os-release').read_text(),
              'sampler_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'scope': 'Named shell/helper processes only; excludes audio daemons, apps and system services. Sequential observations, no energy or presentation-deadline claim.'}
    started = time.monotonic()
    try:
        while True:
            value = sample(args.session, account.pw_uid)
            if not record['samples']:
                record['executable_sha256'] = {
                    name: hashlib.sha256(Path(name).read_bytes()).hexdigest()
                    for name in {p['exe'] for p in value['members']}}
            required = {'tarot'} if args.session == 'tarot' else {'phoc', 'phosh'}
            assert required <= {Path(p['exe']).name for p in value['members']}, 'Expected shell disappeared'
            record['samples'].append(value)
            path = args.output/'running.json'
            path.write_text(json.dumps(record, indent=2)+'\n')
            os.chown(path, account.pw_uid, account.pw_gid)
            elapsed = time.monotonic()-started
            if elapsed >= args.seconds:
                break
            time.sleep(min(args.interval, args.seconds-elapsed))
        assert Path('/proc/sys/kernel/random/boot_id').read_text().strip() == record['boot_id']
        record['cpu'] = cpu_summary(record['samples'], record['clock_ticks_per_second'])
        record['collection_complete'] = True
    except BaseException as error:
        record.update(collection_complete=False, exception_type=type(error).__name__)
        raise
    finally:
        path = args.output/'status.json'
        path.write_text(json.dumps(record, indent=2)+'\n')
        os.chown(path, account.pw_uid, account.pw_gid)
        (args.output/'running.json').unlink(missing_ok=True)


if __name__ == '__main__':
    main()
