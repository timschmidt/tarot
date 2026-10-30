#!/usr/bin/env python3
"""Rendered NetworkManager/Polkit integration on an isolated bus and display.

Uses a nonprivileged synthetic helper. A fixture grant is never evidence of
system authorization, radio operation, or successful hardware networking.
"""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from dialog_button import locate
from hardware_replay import PrivateInput
from lock_replay import append
from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from with_xvfb import read_display_number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/debug/tarot'))
    parser.add_argument('--xvfb', default='Xvfb')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    root = Path.cwd()
    binary = args.binary.resolve()
    examples = binary.parent / 'examples'
    children, logs = [], []
    mouse = network = None
    records = dict(passed=False, real_authority_tested=False,
                   real_credentials_used=False, hardware_networking_tested=False,
                   actions=[], binaries_sha256={})
    for path in [binary, examples / 'network-probe', examples / 'lock-probe']:
        records['binaries_sha256'][path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    sources = out / 'qa-sources'
    sources.mkdir()
    queue = [Path(__file__).resolve(), root / 'tests/polkit_fixture.py']
    hashes = {}
    while queue:
        path = queue.pop()
        if path.name in hashes:
            continue
        payload = path.read_bytes()
        hashes[path.name] = hashlib.sha256(payload).hexdigest()
        (sources / path.name).write_bytes(payload)
        for node in ast.walk(ast.parse(payload)):
            modules = ([x.name for x in node.names] if isinstance(node, ast.Import)
                       else [node.module] if isinstance(node, ast.ImportFrom) and node.module else [])
            for module in modules:
                dependency = root / 'tests' / (module.split('.')[0] + '.py')
                if dependency.exists() and dependency.name not in hashes:
                    queue.append(dependency)
    records['sources_sha256'] = hashes
    with tempfile.TemporaryDirectory(prefix='tarot-network-authorization-') as temporary:
        work = Path(temporary)
        control = work / 'control.sock'
        commands = work / 'authority.commands'
        commands.write_text('')
        report = out / 'authority.jsonl'
        helper = work / 'helper.py'
        locker_commands = work / 'locker.commands'
        locker_commands.write_text('')
        (work / 'empty').mkdir()
        (work / 'runtime').mkdir(mode=0o700)
        sequence = 0

        def log(name):
            stream = (out / name).open('x')
            logs.append(stream)
            return stream

        def events(name=None):
            values = ([json.loads(line) for line in report.read_bytes().split(b'\n')[:-1]]
                      if report.exists() else [])
            return [e for e in values if name is None or e['event'] == name]

        def authority(command, **fields):
            nonlocal sequence
            sequence += 1
            with commands.open('a') as stream:
                stream.write(json.dumps(dict(command=command, request=sequence, **fields)) + '\n')
            await_value(lambda: events('command'), lambda es: any(e['request'] == sequence for e in es))

        def state(label, predicate=lambda s: True, seconds=10):
            value = await_value(lambda: request(control, 'status'), predicate, seconds=seconds)
            records['actions'].append(dict(label=label, state=value))
            assert not any(secret in json.dumps(value) for secret in
                           ['synthetic-secret', 'tarot-test-psk', 'Fixture action', 'qa-cookie'])
            return value

        def question(label):
            return state(label, lambda s: s['authorization']['request'] is not None and
                         s['authorization']['request']['phase'] == 'Question')['authorization']['request']

        def respond(q, value='synthetic-secret'):
            request(control, 'authorization_action', action={'Respond':
                    dict(request=q['id'], turn=q['turn'], response=value)})

        def action(value):
            request(control, 'network_action', action=value)

        def open_wifi():
            request(control, 'system_menu', open=True)
            request(control, 'system_page', page='Wifi')

        def returned(label):
            return state(label, lambda s: s['authorization']['request'] is None and
                         s['system_menu_open'] and s['system_page'] == 'Wifi' and
                         not s['keyboard_visible'] and s['credential_input'] is None)

        def idle(label):
            return state(label, lambda s: not s['network']['busy'])

        def capture(label):
            path = out / (label + '.png')
            time.sleep(.2)
            request(control, 'capture', path=str(path))
            state('capture-' + label, lambda s: not s['capture_pending'] and not s['capture_error'])
            return path

        def click(x, y):
            mouse.move(x, y)
            time.sleep(.04)
            mouse.button(True)
            time.sleep(.06)
            mouse.button(False)
            time.sleep(.12)

        def word(label, text):
            target = locate(capture(label), text, vertical=(.08, .62), scale=2)
            records.setdefault('rendered_targets', []).append(dict(label=label, target=target))
            click(*target['point'])

        def authorized_count():
            return len([e for e in network.events() if e['event'] == 'scan-authorized'])

        try:
            read, write = os.pipe()
            try:
                xvfb = subprocess.Popen([args.xvfb, '-displayfd', str(write), '-nolisten', 'tcp',
                        '-noreset', '-ac', '-screen', '0', '1000x1000x24'], pass_fds=(write,),
                        stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
                children.append(xvfb)
                os.close(write)
                write = None
                number = read_display_number(read)
            finally:
                os.close(read)
                if write is not None:
                    os.close(write)
            address = 'unix:path=' + str(work / 'bus')
            bus = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile',
                    '--address=' + address], stdout=log('bus.log'), stderr=subprocess.STDOUT)
            children.append(bus)
            await_value(lambda: (work / 'bus').exists() or bus.poll() is not None, bool)
            assert bus.poll() is None
            env = dict(os.environ, DISPLAY=':' + number, WINIT_UNIX_BACKEND='x11',
                       XDG_SESSION_TYPE='x11', LIBGL_ALWAYS_SOFTWARE='1',
                       __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json',
                       XDG_RUNTIME_DIR=str(work / 'runtime'), XDG_CONFIG_HOME=str(work / 'empty'),
                       XDG_CONFIG_DIRS=str(work / 'empty'), XDG_DATA_HOME=str(work / 'empty'),
                       XDG_DATA_DIRS=str(work / 'empty'), DBUS_SESSION_BUS_ADDRESS=address)
            env.pop('WAYLAND_DISPLAY', None)
            env.pop('WAYLAND_SOCKET', None)
            fixture = subprocess.Popen(['/usr/bin/python3', str(root / 'tests/polkit_fixture.py'),
                      '--report', str(report), '--commands', str(commands), '--helper', str(helper)],
                      env=env, stdout=log('authority.log'), stderr=subprocess.STDOUT)
            children.append(fixture)
            await_value(lambda: events('ready'), bool)
            network = Fixture(examples / 'network-probe', out / 'network', env,
                              extra_args=['--credentials', '--authorization'])
            network.call('set', permission='auth')
            client = subprocess.Popen([str(binary), '--width', '360', '--height', '720',
                     '--reduced-motion', '--system-services', '--platform-bus', address,
                     '--authorization-helper', str(helper), '--backlight-root', str(work / 'empty'),
                     '--control-socket', str(control), '--config-file', str(work / 'settings.toml'),
                     '--lock-probe-unlocked-start', '--lock-client', str(examples / 'lock-probe'),
                     '--lock-client-arg=--role', '--lock-client-arg=locker',
                     '--lock-client-arg=--report', '--lock-client-arg=' + str(out / 'locker.jsonl'),
                     '--lock-client-arg=--commands', '--lock-client-arg=' + str(locker_commands),
                     '--exit-after-ms', '240000'], env=env, stdout=log('session.log'),
                     stderr=subprocess.STDOUT)
            children.append(client)
            await_value(lambda: control.exists() or client.poll() is not None, bool)
            assert client.poll() is None
            mouse = PrivateInput(client.pid, xvfb, env['DISPLAY'])
            await_value(mouse.find, bool)
            mouse.move(180, 300)
            initial = state('ready', lambda s: s['authorization']['can_prompt'] and
                            s['network']['agent_available'] and len(s['network']['networks']) == 4)
            assert initial['network']['scan_permission'] == 'Authentication'
            open_wifi()
            before = authorized_count()
            action('Scan')
            q = question('scan-password')
            time.sleep(6.2)
            state('beyond-old-five-second-timeout', lambda s: s['network']['busy'] and
                  s['network']['pending'] == 'Scan' and s['authorization']['request']['id'] == q['id'])
            network.call('set', second=False)
            state('inventory-live-during-pam', lambda s: len(s['network']['networks']) == 3 and
                  s['authorization']['request'] is not None and s['network']['busy'])
            Keyboard(click).type_ascii('synthetic-secret')
            word('submit-scan', 'Authenticate')
            returned('scan-returns-to-wifi')
            state('reply-is-not-scan-completion', lambda s: s['network']['busy'] and
                  s['network']['pending'] == 'Scan')
            assert authorized_count() == before + 1
            network.call('complete-scan')
            idle('scan-observed')
            network.call('set', second=True)
            state('inventory-restored', lambda s: len(s['network']['networks']) == 4)

            before = authorized_count()
            action('Scan')
            q = question('wrong-password')
            respond(q, 'wrong-synthetic-response')
            state('static-failure', lambda s: s['authorization']['request'] is None and
                  s['authorization']['error'] and not s['network']['busy'] and
                  s['system_page'] == 'Authorization' and not s['keyboard_visible'])
            assert authorized_count() == before
            assert events('result')[-1]['agent_reply'] and not events('result')[-1]['granted']
            word('dismiss-failure', 'Close')
            returned('failure-close-restores-wifi')

            for mode in ['cancel', 'home', 'lock']:
                open_wifi()
                before = authorized_count()
                action('Scan')
                q = question(mode + '-question')
                if mode == 'cancel':
                    word('cancel-scan', 'Cancel')
                    returned('cancel-restores-wifi')
                elif mode == 'home':
                    request(control, 'home')
                    state('home-keeps-menu-closed', lambda s: not s['system_menu_open'] and
                          s['authorization']['request'] is None)
                else:
                    append(locker_commands, 'lock')
                    protected = state('locked', lambda s: s['lock']['phase'] == 'Locked')
                    assert set(protected) == {'ok', 'pid', 'lock'}
                    append(locker_commands, 'unlock')
                    state('unlocked-without-reopening', lambda s: s['lock']['phase'] == 'Unlocked' and
                          s['authorization']['request'] is None and not s['system_menu_open'])
                idle(mode + '-mechanism-denied')
                assert authorized_count() == before
                respond(q)
                state(mode + '-old-response-retired', lambda s: s['authorization']['request'] is None)

            open_wifi()
            for enabled in [False, True]:
                action({'Radio': {'enabled': enabled}})
                respond(question('radio-' + str(enabled)))
                returned('radio-return-' + str(enabled))
                state('radio-observed-' + str(enabled), lambda s: not s['network']['busy'] and
                      s['network']['radio'] == enabled)
            action('Disconnect')
            respond(question('disconnect'))
            returned('disconnect-return')
            idle('disconnect-observed')
            snapshot = request(control, 'status')['network']
            saved = next(r for r in snapshot['networks'] if r['profiles'] and r['strength'] == 90)
            action({'Connect': {'network': saved['id'], 'profile': saved['profiles'][0]}})
            respond(question('saved-connect'))
            returned('saved-connect-return')
            state('saved-reply-still-pending', lambda s: s['network']['busy'])
            network.call('complete-connect')
            idle('saved-connect-observed')
            new = next(r for r in request(control, 'status')['network']['networks']
                       if r['security'] == 'Personal' and not r['profiles'])
            request(control, 'network_setup', network=new['id'])
            state('new-profile-input', lambda s: s['credential_input'] is not None)
            request(control, 'credential_text', value='tarot-test-psk')
            request(control, 'credential_remember', remember=False)
            request(control, 'credential_submit')
            first = question('new-profile-control')
            assert request(control, 'status')['credential_input'] is None
            respond(first)
            second = state('new-profile-create', lambda s: s['authorization']['request'] is not None and
                           s['authorization']['request']['id'] != first['id'] and
                           s['authorization']['request']['phase'] == 'Question')['authorization']['request']
            respond(first)
            assert question('old-control-response-cannot-answer-create')['id'] == second['id']
            respond(second)
            returned('new-profile-return')
            state('new-profile-reply-still-pending', lambda s: s['network']['busy'])
            made = await_value(network.events, lambda es: any(e['event'] == 'create' for e in es))
            made = next(e for e in made if e['event'] == 'create')
            assert made['password_matches'] and made['persist'] == 'volatile' and made['private_user']
            checks = events('check')[-2:]
            assert [e['action'] for e in checks] == [
                'org.freedesktop.NetworkManager.network-control',
                'org.freedesktop.NetworkManager.settings.modify.own']
            network.call('complete-connect')
            idle('new-profile-observed')

            network.call('set', permission='no')
            state('permission-no', lambda s: s['network']['scan_permission'] == 'No')
            count = len(events('check'))
            action('Scan')
            state('no-cannot-be-upgraded-by-agent', lambda s: not s['network']['busy'] and
                  bool(s['network']['action_error']))
            assert len(events('check')) == count
            network.call('set', permission='auth')
            authority('active', value=False)
            state('inactive-agent', lambda s: not s['authorization']['can_prompt'] and
                  s['network']['scan_permission'] == 'Authentication')
            action('Scan')
            state('inactive-denies-before-mechanism', lambda s: not s['network']['busy'] and
                  bool(s['network']['action_error']))
            assert len(events('check')) == count
            network.call('set', permission='yes')
            state('yes-without-agent', lambda s: s['network']['scan_permission'] == 'Yes')
            action('Scan')
            state('yes-awaits-observation', lambda s: s['network']['pending'] == 'Scan')
            network.call('complete-scan')
            idle('yes-completed-without-agent')
            assert len(events('check')) == count
            authority('active', value=True)
            network.call('set', permission='auth')
            state('agent-active-again', lambda s: s['authorization']['can_prompt'] and
                  s['network']['scan_permission'] == 'Authentication')

            # A daemon crash retires opaque rows and its authority conversation.
            # Its replacement must not receive an automatic replay of the write.
            old_epoch = request(control, 'status')['network']['epoch']
            old_saved = saved
            action('Scan')
            retired = question('daemon-loss-pending')
            network.process.terminate()
            network.process.wait(timeout=5)
            network.close()
            network = None
            state('daemon-loss-retires-call', lambda s: not s['network']['available'] and
                  not s['network']['busy'] and s['authorization']['request'] is None)
            returned('daemon-loss-returns-to-wifi')
            assert events('mechanism-gone')
            network = Fixture(examples / 'network-probe', out / 'network-restarted', env,
                              extra_args=['--credentials', '--authorization'])
            network.call('set', permission='auth')
            state('new-daemon-new-epoch', lambda s: s['network']['available'] and
                  s['network']['agent_available'] and s['network']['epoch'] > old_epoch)
            count = len(events('check'))
            action({'Connect': {'network': old_saved['id'], 'profile': old_saved['profiles'][0]}})
            state('old-row-cannot-target-replacement', lambda s: not s['network']['busy'] and
                  bool(s['network']['action_error']))
            respond(retired)
            assert len(events('check')) == count and not authorized_count()
            action('Scan')
            respond(question('new-daemon-fresh-request'))
            returned('new-daemon-return')
            state('new-daemon-awaits-scan', lambda s: s['network']['pending'] == 'Scan')
            network.call('complete-scan')
            idle('new-daemon-observed')

            # Losing the authority also ends the blocked mechanism call, without
            # restarting Tarot or interpreting an agent reply as a policy grant.
            action('Scan')
            retired = question('authority-loss-pending')
            old_epoch = request(control, 'status')['authorization']['epoch']
            fixture.terminate()
            fixture.wait(timeout=5)
            state('authority-loss-denied', lambda s: not s['authorization']['available'] and
                  not s['authorization']['can_prompt'] and not s['network']['busy'])
            returned('authority-loss-returns-to-wifi')
            assert authorized_count() == 1
            records['helper_events_before_authority_restart'] = [json.loads(line) for line in
                helper.with_suffix('.jsonl').read_text().splitlines()]
            helper.with_suffix('.jsonl').unlink()
            report = out / 'authority-restarted.jsonl'
            commands.write_text('')
            fixture = subprocess.Popen(['/usr/bin/python3', str(root / 'tests/polkit_fixture.py'),
                      '--report', str(report), '--commands', str(commands), '--helper', str(helper)],
                      env=env, stdout=log('authority-restarted.log'), stderr=subprocess.STDOUT)
            children.append(fixture)
            state('authority-reenrolled', lambda s: s['authorization']['can_prompt'] and
                  s['authorization']['epoch'] > old_epoch)
            respond(retired)
            assert not events('check')
            action('Scan')
            respond(question('new-authority-fresh-request'))
            returned('new-authority-return')
            state('new-authority-awaits-scan', lambda s: s['network']['pending'] == 'Scan')
            network.call('complete-scan')
            idle('new-authority-observed')

            action('Scan')
            retired = question('broker-loss-pending')
            previous = request(control, 'status')
            network.process.terminate()
            network.process.wait(timeout=5)
            network.close()
            network = None
            fixture.terminate()
            fixture.wait(timeout=5)
            bus.terminate()
            bus.wait(timeout=5)
            state('broker-loss-retires-platform', lambda s: not s['authorization']['available'] and
                  not s['network']['available'] and not s['network']['busy'])
            returned('broker-loss-returns-to-wifi')
            records['helper_events_before_broker_restart'] = [json.loads(line) for line in
                helper.with_suffix('.jsonl').read_text().splitlines()]
            helper.with_suffix('.jsonl').unlink()
            report = out / 'authority-after-broker.jsonl'
            commands.write_text('')
            bus = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile',
                    '--address=' + address], stdout=log('bus-restarted.log'), stderr=subprocess.STDOUT)
            children.append(bus)
            await_value(lambda: (work / 'bus').exists() or bus.poll() is not None, bool)
            assert bus.poll() is None
            fixture = subprocess.Popen(['/usr/bin/python3', str(root / 'tests/polkit_fixture.py'),
                      '--report', str(report), '--commands', str(commands), '--helper', str(helper)],
                      env=env, stdout=log('authority-after-broker.log'), stderr=subprocess.STDOUT)
            children.append(fixture)
            await_value(lambda: events('ready'), bool)
            network = Fixture(examples / 'network-probe', out / 'network-after-broker', env,
                              extra_args=['--credentials', '--authorization'])
            network.call('set', permission='auth')
            state('broker-recovered-with-fresh-identities', lambda s:
                  s['authorization']['can_prompt'] and s['network']['agent_available'] and
                  s['network']['scan_permission'] == 'Authentication' and
                  s['authorization']['epoch'] > previous['authorization']['epoch'] and
                  s['network']['epoch'] > previous['network']['epoch'])
            respond(retired)
            assert not events('check') and not authorized_count()
            action('Scan')
            respond(question('after-broker-fresh-request'))
            returned('after-broker-return')
            state('after-broker-awaits-scan', lambda s: s['network']['pending'] == 'Scan')
            network.call('complete-scan')
            idle('after-broker-observed')
            action('Scan')
            question('shutdown-pending')
            stopped = time.monotonic()
            request(control, 'exit')
            assert client.wait(timeout=6) == 0
            records['shutdown_with_pending_authorization_seconds'] = time.monotonic() - stopped
            records['passed'] = True
            print('PASS: private Wi-Fi authorization, live inventory, settings return, denials, '
                  'lock cancellation, sequential profile prompts and bounded shutdown', out)
        finally:
            failed = sys.exc_info()[0] is not None
            cleanup_errors = []
            if helper.with_suffix('.jsonl').exists():
                records['helper_events'] = [json.loads(line) for line in
                                            helper.with_suffix('.jsonl').read_text().splitlines()]
            if mouse is not None:
                mouse.close()
            if network is not None:
                try:
                    network.close()
                except Exception as error:
                    cleanup_errors.append(type(error).__name__)
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=6)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
            for stream in logs:
                stream.close()
            records['cleanup_errors'] = cleanup_errors
            records['passed'] &= not cleanup_errors
            (out / 'result.json').write_text(json.dumps(records, indent=2) + '\n')
            for path in out.rglob('*.log'):
                assert 'synthetic-secret' not in path.read_text() and 'tarot-test-psk' not in path.read_text()
            if cleanup_errors and not failed:
                raise RuntimeError('Fixture cleanup failed: ' + ', '.join(cleanup_errors))


if __name__ == '__main__':
    main()
