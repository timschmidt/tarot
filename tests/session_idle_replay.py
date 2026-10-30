#!/usr/bin/env python3
"""Private manager/session idle-hint contract; never uses system bus or Suspend."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/debug/tarot'))
    a = parser.parse_args()
    assert os.geteuid() != 0
    out = a.output.resolve(); out.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.binary.resolve(); examples = binary.parent/'examples'
    result = dict(passed=False, cases=[], system_bus_used=False, suspend_called=False,
                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  probe_sha256=hashlib.sha256((examples/'session-idle-probe').read_bytes()).hexdigest(),
                  manager_sha256=hashlib.sha256((examples/'sleep-probe').read_bytes()).hexdigest())
    manager = client = gui = bus = None; logs = []
    def log(name):
        stream = (out/name).open('x'); logs.append(stream); return stream
    def passed(name):
        result['cases'].append(name)
        print('PASS', name, flush=True)
    with tempfile.TemporaryDirectory(prefix='tarot-session-idle-') as temp:
        root = Path(temp); address = 'unix:path=' + str(root/'bus')
        env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, LIBGL_ALWAYS_SOFTWARE='1',
                   __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_SOCKET', None)
        def start_bus(label):
            proc = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile', '--address='+address],
                                    env=env, stdout=log(label+'.log'), stderr=subprocess.STDOUT)
            await_value(lambda: (root/'bus').exists() or proc.poll() is not None, bool)
            assert proc.poll() is None
            return proc
        def start_client(label):
            return Fixture(examples/'session-idle-probe', out/label, env)
        def status(condition, instance=None, seconds=8):
            instance = instance or client
            def read():
                states = [e['status'] for e in instance.events() if e['event'] == 'status']
                return states[-1] if states else {}
            return await_value(read, condition, seconds=seconds)
        def confirmed(value):
            return status(lambda s: s.get('available') and s.get('observed_idle') == value)
        def hint():
            raw = subprocess.check_output(['gdbus', 'call', '--address', address, '--dest', 'org.freedesktop.login1',
                '--object-path', '/org/freedesktop/login1/session/tarot_test', '--method',
                'org.freedesktop.DBus.Properties.Get', 'org.freedesktop.login1.Session', 'IdleHint'],
                env=env, text=True, timeout=4).strip()
            assert raw in ('(<true>,)', '(<false>,)'), raw
            return raw == '(<true>,)'
        def commands_since(start, event):
            return [e for e in manager.events()[start:] if e['event'] == event]
        try:
            bus = start_bus('bus1')
            manager = Fixture(examples/'sleep-probe', out/'manager1', env)
            manager.call('session', idle=True)
            client = start_client('client1')
            confirmed(False); assert not hint()
            assert all(e['pid'] == client.process.pid for e in manager.events() if e['event'] == 'session-request')
            passed('actual-caller-pid-and-false-before-readiness')
            client.call('ready', ready=True); confirmed(True); assert hint()
            manager.call('session', active=False); confirmed(False); assert not hint()
            manager.call('session', active=True); confirmed(True); assert hint()
            client.call('ready', ready=False); confirmed(False); assert not hint()
            passed('readiness-and-session-active-transitions')
            time.sleep(.2); count = len(manager.events()); time.sleep(.4)
            assert len(manager.events()) == count
            passed('healthy-worker-does-not-poll')
            manager.call('session', idle=True); await_value(hint, lambda v: not v)
            client.call('ready', ready=True); confirmed(True)
            manager.call('session', idle=False); await_value(hint, bool)
            client.call('ready', ready=False); confirmed(False)
            passed('external-idle-property-corrected-from-fresh-readback')
            manager.call('hint-delay', milliseconds=250)
            count = len(manager.events()); client.call('ready', ready=True)
            await_value(manager.events, lambda _: any(e['idle'] for e in commands_since(count, 'hint-request')))
            client.call('ready', ready=False); mark = len(client.events())
            await_value(manager.events, lambda _: any(e['idle'] for e in commands_since(count, 'hint-applied')))
            await_value(hint, lambda v: not v); confirmed(False)
            assert not any(e['event']=='status' and e['status'].get('observed_idle') is True for e in client.events()[mark:])
            manager.call('hint-delay', milliseconds=0)
            passed('late-acknowledgement-retired-and-hint-cleared')
            manager.call('deny-hint', deny=True); client.call('ready', ready=True)
            status(lambda s: bool(s.get('error')) and s.get('observed_idle') is None)
            client.call('ready', ready=False); manager.call('deny-hint', deny=False)
            confirmed(False); assert not hint()
            passed('denial-reports-unknown-and-reconnect-clears-first')
            manager.call('hint-delay', milliseconds=1100)
            count = len(manager.events()); client.call('ready', ready=True)
            status(lambda s: bool(s.get('error')) and s.get('observed_idle') is None)
            client.call('ready', ready=False); manager.call('hint-delay', milliseconds=0)
            await_value(manager.events, lambda _: any(e['idle'] for e in commands_since(count, 'hint-applied')))
            confirmed(False); assert not hint()
            passed('timed-out-write-may-apply-late-and-is-cleared-on-reconnect')
            for action, delay in [('poweroff', 30_000_000), ('suspend', 1_000_000)]:
                client.call('ready', ready=True); confirmed(True)
                manager.call('idle-action', action=action, delay_us=delay)
                status(lambda s: bool(s.get('error'))); await_value(hint, lambda v: not v)
                client.call('ready', ready=False)
                manager.call('idle-action', action='ignore', delay_us=30_000_000)
                confirmed(False)
            manager.call('idle-action', action='suspend', delay_us=30_000_000)
            client.call('ready', ready=True)
            status(lambda s: s.get('available') and s.get('observed_idle') is True and s.get('manager_action')=='suspend')
            client.call('ready', ready=False); confirmed(False)
            manager.call('idle-action', action='ignore', delay_us=30_000_000)
            passed('manager-action-and-minimum-delay-validation-without-sleep-calls')
            client.close(); client = None; assert not hint()
            for i, invalid in enumerate([dict(uid=os.getuid()+1), dict(remote=True), dict(kind='tty'),
                                         dict(class_='greeter'), dict(seat='seat1'), dict(vt=0)]):
                invalid = {('class' if k=='class_' else k): v for k,v in invalid.items()}
                manager.call('session', idle=True, **invalid)
                count = len(manager.events()); client = start_client('invalid-'+str(i))
                status(lambda s: bool(s.get('error')))
                client.call('ready', ready=True)
                assert not commands_since(count, 'hint-request'), invalid
                client.close(); client = None
                manager.call('session', uid=os.getuid(), remote=False, kind='wayland', **{'class':'user'}, seat='seat0', vt=7, idle=False)
            passed('foreign-remote-nongraphical-greeter-seat-and-vt-rejected-before-write')
            client = start_client('client2'); confirmed(False)
            client.call('ready', ready=True); confirmed(True)
            manager.call('release'); status(lambda s: bool(s.get('error')))
            client.call('ready', ready=False); manager.call('acquire')
            confirmed(False); assert not hint()
            passed('same-unique-owner-release-reacquire-clears-persistent-hint')
            manager.close(); manager = None
            bus.terminate(); assert bus.wait(timeout=4)==0; bus = None
            status(lambda s: bool(s.get('error')))
            bus = start_bus('bus2'); manager = Fixture(examples/'sleep-probe', out/'manager2', env)
            manager.call('session', idle=True)
            confirmed(False); assert not hint()
            passed('whole-broker-replacement-without-fixed-guid')
            client.call('ready', ready=True); confirmed(True)
            client.close(); stopped = client.events()[-1]; assert stopped['event']=='stopped' and stopped['elapsed_us']<1_500_000
            assert not hint(); result['healthy_shutdown_us'] = stopped['elapsed_us']; client = None
            passed('healthy-shutdown-explicitly-clears-hint')
            client = start_client('pending-shutdown'); confirmed(False)
            manager.call('hint-delay', milliseconds=1100)
            count = len(manager.events()); client.call('ready', ready=True)
            await_value(manager.events, lambda _: any(e['idle'] for e in commands_since(count, 'hint-request')))
            client.close()
            final = [e['status'] for e in client.events() if e['event']=='status'][-1]
            assert final['observed_idle'] is None and 'shutdown' in final['error']
            await_value(manager.events, lambda _: any(e['idle'] for e in commands_since(count, 'hint-applied')))
            assert hint(), 'This async fixture deliberately demonstrates that idle hints are not leased'
            result['uncertain_shutdown'] = final; client = None
            manager.call('hint-delay', milliseconds=0)
            client = start_client('after-uncertain-shutdown'); confirmed(False); assert not hint()
            client.close(); client = None
            passed('uncertain-shutdown-reports-unknown-instead-of-promising-a-lease')
            # Actual nested compositor wiring must never infer physical panel
            # off from a synthetic lock. The normal launcher omits the opt-in.
            control = root/'control'; commands = root/'locker.commands'; commands.write_text('')
            args = [str(binary), '--system-services', '--platform-bus', address,
                    '--audio-server', 'unix:'+str(root/'missing-audio'), '--sleep-coordination', '--session-idle',
                    '--idle-lock-ms', '86400000', '--lock-client', str(examples/'lock-probe'),
                    '--control-socket', str(control), '--exit-after-ms', '15000']
            for arg in ['--role','locker','--commands',str(commands),'--report',str(out/'locker.jsonl')]:
                args.append('--lock-client-arg='+arg)
            gui = subprocess.Popen(args, env=env, stdout=log('compositor.log'), stderr=subprocess.STDOUT)
            await_value(lambda: control.exists() or gui.poll() is not None, bool)
            assert gui.poll() is None
            state = await_value(lambda: request(control,'status'), lambda s:s['lock']['session_idle']['publisher']['available'])
            assert state['lock']['session_idle']['enabled'] and state['lock']['session_idle']['readiness']=='Active'
            assert state['lock']['session_idle']['publisher']['observed_idle'] is False and not hint()
            request(control,'exit'); assert gui.wait(timeout=4)==0; gui = None; assert not hint()
            passed('nested-compositor-cannot-invent-physical-panel-off-readiness')
            result['passed'] = True
        finally:
            if gui is not None and gui.poll() is None: gui.terminate(); gui.wait(timeout=5)
            if client is not None: client.close()
            if manager is not None: manager.close()
            if bus is not None and bus.poll() is None: bus.terminate(); bus.wait(timeout=4)
            for stream in logs: stream.close()
            (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')


if __name__ == '__main__':
    main()
