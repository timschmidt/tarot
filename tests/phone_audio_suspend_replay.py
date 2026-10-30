#!/usr/bin/env python3
"""Spare PinePhone Pro: actual elogind audio-block refusal and optional RTC sleep.

Runs against an already-installed normal Tarot session without unlocking it,
changing its preferences, or using a physical audio device. This tests the
explicit root/check-inhibitors API, not automatic suspend or ordinary-user
Suspend semantics. In elogind 255 the caller's own inhibitors remain exempt.
No credentials, Bluetooth operations, physical microphone or audible PCM.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import select
import subprocess
import sys
import time
import traceback

from phone_audio_state import devices, pactl, peer_identity, restore, snapshot


LOGIN = 'org.freedesktop.login1'
OBJECT = '/org/freedesktop/login1'
MANAGER = LOGIN + '.Manager'
ROOT_CHECK_INHIBITORS = 1  # elogind v255.24 src/basic/login-util.h
AUDIO_REASON = 'Audio playback or recording is active'
ALARM = Path('/sys/class/rtc/rtc0/wakealarm')


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def identity(pid):
    proc = Path('/proc', str(pid))
    return dict(pid=pid, uid=proc.stat().st_uid,
                start=(proc/'stat').read_text().rsplit(')', 1)[1].split()[19],
                executable=os.readlink(proc/'exe'), sha256=digest(proc/'exe'),
                cgroup=(proc/'cgroup').read_text())


def clocks():
    return dict(boottime=time.clock_gettime(time.CLOCK_BOOTTIME),
                monotonic=time.monotonic(),
                boot=Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
                success=int(Path('/sys/power/suspend_stats/success').read_text()),
                fail=int(Path('/sys/power/suspend_stats/fail').read_text()))


def kernel_state():
    return dict(stats={p.name: p.read_text().strip() for p in Path('/sys/power/suspend_stats').iterdir()},
                mem_sleep=Path('/sys/power/mem_sleep').read_text().strip())


def wait(read, predicate, seconds=15):
    deadline = time.clock_gettime(time.CLOCK_BOOTTIME) + seconds
    while True:
        value = read()
        if predicate(value):
            return value
        assert time.clock_gettime(time.CLOCK_BOOTTIME) < deadline, 'Readiness deadline expired'
        time.sleep(.05)


def run(options):
    assert os.geteuid() == 0 and not sys.flags.optimize
    assert 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    installed = subprocess.check_output(['apk', 'list', '--installed', 'elogind'], text=True).splitlines()
    assert len(installed) == 1 and installed[0].split()[0] == 'elogind-255.24-r0'
    import gi
    gi.require_version('Gio', '2.0')
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
    def broker(method, argument):
        return bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                             'org.freedesktop.DBus', method, GLib.Variant('(s)', (argument,)),
                             None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    owner = broker('GetNameOwner', LOGIN)
    assert broker('GetConnectionUnixUser', owner) == 0
    manager_identity = identity(broker('GetConnectionUnixProcessID', owner))
    def call(method, argument=None, timeout=5000):
        assert broker('GetNameOwner', LOGIN) == owner
        return bus.call_sync(owner, OBJECT, MANAGER, method, argument, None,
                             Gio.DBusCallFlags.NONE, timeout, None).unpack()
    def preparing():
        return bus.call_sync(owner, OBJECT, 'org.freedesktop.DBus.Properties', 'Get',
                             GLib.Variant('(ss)', (MANAGER, 'PreparingForSleep')), None,
                             Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    def rows():
        return call('ListInhibitors')[0]
    def blocks():
        return [r for r in rows() if 'sleep' in r[0].split(':') and r[3] == 'block' and r[4] != 0]
    def audio():
        result = [r for r in rows() if r[1] == 'Tarot' and r[2] == AUDIO_REASON]
        for row in result:
            assert row[0] == 'sleep' and row[3] == 'block' and row[4:] == (10000, options.pid)
        assert len(result) <= 1
        return result
    expected = json.loads(options.package_audit.read_text())['binaries_sha256']
    assert set(expected) == {'tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard'}
    roles = {}
    for proc in Path('/proc').iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            name = (proc/'comm').read_text().strip()
            if proc.stat().st_uid == 10000 and name in expected:
                assert name not in roles
                role = identity(int(proc.name))
                assert role['executable'] == '/usr/bin/' + name and role['sha256'] == expected[name]
                roles[name] = role
        except (FileNotFoundError, ProcessLookupError):
            continue
    assert set(roles) == set(expected) and roles['tarot']['pid'] == options.pid
    assert len({r['cgroup'] for r in roles.values()}) == 1
    proc = Path('/proc', str(options.pid))
    argv = (proc/'cmdline').read_bytes().split(b'\0')
    assert b'--sleep-coordination' in argv and b'--hardware-keys' in argv
    assert b'--metadata' not in argv and b'--control-socket' not in argv
    assert not Path('/run/user/10000/tarot-session/control.sock').exists()
    assert peer_identity(required_cgroup=roles['tarot']['cgroup'])['uid'] == 10000
    assert not devices('sink-inputs') and not devices('source-outputs')
    assert not preparing() and not audio() and not blocks()
    assert any(r[0] == 'sleep' and r[1] == 'Tarot' and r[3] == 'delay'
               and r[4:] == (10000, options.pid) for r in rows())
    assert not ALARM.read_text().strip(), 'Preserve unrelated RTC alarm'
    watched = [Path(p) for p in ['/etc/phrog/greetd-config.toml', '/etc/pam.d/greetd',
        '/etc/pam.d/tarot-lock', '/etc/elogind/logind.conf', '/etc/sleep-inhibitor.conf',
        '/home/user/.config/tarot/settings.toml']]
    def policies():
        return {str(p): dict(sha256=digest(p), uid=p.stat().st_uid,
                            gid=p.stat().st_gid, mode=p.stat().st_mode & 0o7777)
                if p.exists() else None for p in watched}
    original = snapshot()
    before_policies = policies()
    original_peer = peer_identity()
    system_bus = identity(broker('GetConnectionUnixProcessID', 'org.freedesktop.DBus'))
    network_manager = identity(broker('GetConnectionUnixProcessID', 'org.freedesktop.NetworkManager'))
    result = dict(passed=False, controller_pid=os.getpid(), roles=roles,
                  scope='Explicit root SuspendWithFlags(1); root-owned blocks exempt on elogind255',
                  request_flags=ROOT_CHECK_INHIBITORS, ordinary_user_suspend_tested=False,
                  automatic_suspend_tested=False, physical_audio_used=False, bluetooth_used=False,
                  credentials_used=False, audio_before=original, policies_before=before_policies,
                  manager=manager_identity, manager_owner=owner, system_bus=system_bus,
                  network_manager=network_manager, audio_server=original_peer,
                  initial_inhibitors=rows(), kernel_control_requested=options.kernel_control,
                  kernel_control_passed=None, refusals_passed=False, kernel_before=kernel_state(),
                  inputs={str(p): digest(p) for p in [Path(__file__), options.package_audit,
                      Path(__file__).with_name('pulse_stream_fixture.py'),
                      Path(__file__).with_name('phone_audio_state.py')]})
    result['caller_uid_blocks_exempt'] = [r for r in rows() if r[3] == 'block' and r[4] == 0]
    def save():
        temporary = options.output/'result.tmp'
        temporary.write_text(json.dumps(result, indent=2) + '\n')
        temporary.replace(options.output/'result.json')
    save()
    account = pwd.getpwuid(10000)
    env = dict(field.decode().split('=', 1) for field in (proc/'environ').read_bytes().split(b'\0') if b'=' in field)
    env.update(HOME=account.pw_dir, USER=account.pw_name, LOGNAME=account.pw_name, XDG_RUNTIME_DIR='/run/user/10000')
    credentials = dict(user=10000, group=account.pw_gid,
                       extra_groups=os.getgrouplist(account.pw_name, account.pw_gid))
    module = None
    peers = []
    logs = []
    alarm_owned = None
    unsettled_request = False
    sink = 'tarot_qa_suspend_' + str(os.getpid())
    signals = []
    subscription = bus.signal_subscribe(owner, MANAGER, 'PrepareForSleep', OBJECT, None,
        Gio.DBusSignalFlags.NONE, lambda *args: signals.append(args[5].unpack()[0]))
    context = GLib.MainContext.default()
    def drain():
        while context.pending():
            context.iteration(False)
        return list(signals)
    def clear_alarm():
        nonlocal alarm_owned
        if alarm_owned is not None:
            current = ALARM.read_text().strip()
            assert current in ('', alarm_owned), 'Unrelated alarm replaced our alarm; preserve it'
            if current:
                ALARM.write_text('0\n')
            alarm_owned = None
    def arm():
        nonlocal alarm_owned
        clear_alarm()
        assert not ALARM.read_text().strip()
        rtc_before = dict(system=time.time(), rtc=int(ALARM.with_name('since_epoch').read_text()))
        assert abs(rtc_before['rtc'] - rtc_before['system']) < 3, 'RTC UTC mapping unverified'
        with (options.output/('rtc-' + str(len(result.get('requests', []))) + '.log')).open('x') as log:
            subprocess.run(['rtcwake', '-u', '-d', '/dev/rtc0', '-m', 'no', '-s', '15'],
                           check=True, stdout=log, stderr=log, timeout=5)
        alarm_owned = ALARM.read_text().strip()
        rtc_after = dict(system=time.time(), rtc=int(ALARM.with_name('since_epoch').read_text()))
        result.setdefault('rtc_arms', []).append(dict(before=rtc_before, after=rtc_after, alarm=alarm_owned))
        save()
        # rtcwake rounds against a whole-second RTC and adds a second; the
        # qualified phone's RTC is also ~0.75 s ahead of realtime. Keep a short
        # bounded future alarm while allowing the verified <3 s UTC mapping.
        assert abs(rtc_after['rtc'] - rtc_after['system']) < 3
        assert 3 < int(alarm_owned) - rtc_after['system'] <= 20
        assert 3 < int(alarm_owned) - rtc_after['rtc'] <= 18
    class Peer:
        def __init__(self, recording=False):
            args = ['python3', str(Path(__file__).with_name('pulse_stream_fixture.py')),
                    '--server', 'unix:/run/user/10000/pulse/native',
                    '--device', sink + '.monitor' if recording else sink]
            if recording:
                args.append('--record')
            log = (options.output/('stream-' + str(len(peers)) + '.log')).open('x')
            logs.append(log)
            self.child = subprocess.Popen(args, env=env, **credentials, stdin=subprocess.PIPE,
                                          stdout=subprocess.PIPE, stderr=log, text=True)
            peers.append(self)
            assert self.read()['corked']
        def read(self):
            assert select.select([self.child.stdout], [], [], 5)[0], 'Null stream reply timed out'
            line = self.child.stdout.readline()
            assert line, self.child.poll()
            return json.loads(line)
        def cork(self, value):
            self.child.stdin.write(json.dumps(dict(command='cork', value=value)) + '\n')
            self.child.stdin.flush()
            assert self.read() == dict(event='corked', value=value)
        def close(self):
            if self.child.poll() is None:
                try:
                    self.child.stdin.write('{"command":"exit"}\n')
                    self.child.stdin.flush()
                    assert self.child.wait(timeout=5) == 0
                finally:
                    if self.child.poll() is None:
                        self.child.terminate()
                        self.child.wait(timeout=5)
    def refused(label):
        nonlocal unsettled_request
        grant = wait(audio, bool)
        assert blocks() == grant, 'The Tarot audio block must be the sole non-root sleep block'
        assert not preparing() and not drain()
        descriptors = []
        for fd in (proc/'fd').iterdir():
            try:
                target = os.readlink(fd)
                if target.startswith('/run/systemd/inhibit/') and target.endswith('.ref'):
                    flags = dict(line.split(':', 1) for line in (proc/'fdinfo'/fd.name).read_text().splitlines() if ':' in line)
                    assert int(flags['flags'].strip(), 8) & os.O_CLOEXEC
                    descriptors.append(target)
            except FileNotFoundError:
                continue
        assert len(descriptors) >= 2  # separate Tarot delay and audio block
        arm()
        before = clocks()
        error = None
        unsettled_request = True
        try:
            call('SuspendWithFlags', GLib.Variant('(t)', (ROOT_CHECK_INHIBITORS,)), timeout=10000)
        except GLib.Error as exc:
            error = dict(name=Gio.DBusError.get_remote_error(exc), message=str(exc))
        if error and error['name'] == 'org.freedesktop.DBus.Error.AccessDenied' and 'Access denied to root due to active block inhibitor' in error['message']:
            unsettled_request = False
        after = clocks()
        entry = dict(label=label, before=before, after=after, error=error, audio_grant=grant,
                     all_inhibitors=rows(), descriptor_targets=descriptors, rtc_deadline=alarm_owned,
                     prepare_signals=drain())
        result.setdefault('requests', []).append(entry)
        save()
        assert error and error['name'] == 'org.freedesktop.DBus.Error.AccessDenied', error
        assert 'Access denied to root due to active block inhibitor' in error['message']
        assert after['boot'] == before['boot'] and after['success'] == before['success'] and after['fail'] == before['fail']
        assert (after['boottime'] - before['boottime']) - (after['monotonic'] - before['monotonic']) < .5
        assert not preparing() and not drain() and audio() == grant
        clear_alarm()
    try:
        assert not any(d['name'] == sink for d in devices('sinks'))
        module = int(pactl('load-module', 'module-null-sink', 'sink_name=' + sink))
        assert next(d for d in devices('sinks') if d['name'] == sink)['driver'] == 'module-null-sink.c'
        playback = Peer()
        recording = Peer(True)
        assert not audio()
        playback.cork(False)
        refused('playback')
        recording.cork(False)
        refused('playback-and-recording')
        playback.cork(True)
        refused('recording')
        recording.cork(True)
        wait(audio, lambda value: not value)
        assert not blocks()
        result['refusals_passed'] = True
        if options.kernel_control:
            # Identical requester, method and flags after release. Root-owned
            # SSH/APK blocks remain untouched and exempt in installed elogind255.
            arm()
            before = clocks()
            result['kernel_control_passed'] = False
            result['positive_control'] = dict(before=before, rtc_deadline=alarm_owned, inhibitors=rows())
            save()
            unsettled_request = True
            call('SuspendWithFlags', GLib.Variant('(t)', (ROOT_CHECK_INHIBITORS,)), timeout=10000)
            wait(clocks, lambda value: value['success'] != before['success'] or value['fail'] != before['fail'], seconds=40)
            # A deep-sleep failure can be followed by an s2idle fallback. Wait
            # for the entire manager operation before recording final counters.
            wait(preparing, lambda value: not value, seconds=40)
            unsettled_request = False
            after = clocks()
            suspended = (after['boottime'] - before['boottime']) - (after['monotonic'] - before['monotonic'])
            result['positive_control'].update(after=after, suspended_seconds=suspended, prepare_signals=drain(), kernel=kernel_state())
            save()
            assert after['boot'] == before['boot'] and after['success'] == before['success'] + 1 and after['fail'] == before['fail']
            assert suspended >= 3
            assert wait(drain, lambda value: value == [True, False]) == [True, False]
            clear_alarm()
            wait(rows, lambda value: any(r[0] == 'sleep' and r[1] == 'Tarot' and r[3] == 'delay'
                 and r[4:] == (10000, options.pid) for r in value))
            result['kernel_control_passed'] = True
        result['passed'] = True
    except BaseException as exc:
        result['error_type'] = type(exc).__name__
        traceback.print_exc()
        raise
    finally:
        if sys.exc_info()[0] is not None:
            result['passed'] = False
        try:
            # An unexpected success or RPC timeout can leave a suspend queued.
            # Keep its wake alarm through the full RTC window before cleanup.
            if unsettled_request and alarm_owned is not None:
                deadline = time.clock_gettime(time.CLOCK_BOOTTIME) + 22
                while time.clock_gettime(time.CLOCK_BOOTTIME) < deadline:
                    time.sleep(.1)
                assert not preparing(), 'Preserve wake alarm while a sleep request is unsettled'
            for peer in peers:
                peer.close()
            for log in logs:
                log.close()
            if module is not None:
                matching = [line.split('\t') for line in pactl('list', 'short', 'modules').splitlines()
                            if line.split('\t')[0] == str(module)]
                assert len(matching) == 1 and matching[0][1] == 'module-null-sink' and matching[0][2].split() == ['sink_name=' + sink]
                pactl('unload-module', module)
            clear_alarm()
            assert not devices('sink-inputs') and not devices('source-outputs')
            assert not any(d['name'] == sink for d in devices('sinks'))
            wait(audio, lambda value: not value)
            if snapshot() != original:
                restore(original)
            assert snapshot() == original
            assert policies() == before_policies
            assert all(identity(role['pid']) == role for role in roles.values())
            assert identity(manager_identity['pid']) == manager_identity
            assert identity(system_bus['pid']) == system_bus
            assert identity(network_manager['pid']) == network_manager
            assert peer_identity() == original_peer
            assert not ALARM.read_text().strip() and not blocks() and not preparing()
            result['cleanup_passed'] = True
            result['audio_after'] = snapshot()
            result['policies_after'] = policies()
            result['final_inhibitors'] = rows()
            result['roles_after'] = {name: identity(role['pid']) for name, role in roles.items()}
            result['final_clocks'] = clocks()
            result['kernel_after'] = kernel_state()
        except BaseException:
            result['passed'] = False
            result['cleanup_passed'] = False
            raise
        finally:
            bus.signal_unsubscribe(subscription)
            save()
    assert result['passed'] and result['cleanup_passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--package-audit', type=Path, required=True)
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--kernel-control', action='store_true', help='After refusal checks, request one real RTC-woken suspend with all streams corked')
    parser.add_argument('--detach', action='store_true')
    options = parser.parse_args()
    options.output = options.output.resolve()
    options.package_audit = options.package_audit.resolve(strict=True)
    assert os.geteuid() == 0 and not sys.flags.optimize
    os.umask(0o077)
    options.output.mkdir(mode=0o700)
    if options.detach:
        child = os.fork()
        if child:
            print('Detached audio suspend replay PID', child, flush=True)
            return
        os.setsid()
        fd = os.open('/dev/null', os.O_RDONLY)
        os.dup2(fd, 0)
        os.close(fd)
        fd = os.open(options.output/'controller.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.dup2(fd, 1)
        os.dup2(fd, 2)
        os.close(fd)
    run(options)
    print('PASS explicit elogind audio-block refusal, optional kernel control and cleanup', flush=True)


if __name__ == '__main__':
    main()
