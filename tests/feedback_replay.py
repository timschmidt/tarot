#!/usr/bin/env python3
"""Feedbackd integration on an owned display/private bus; no sound or motor I/O."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from dialog_button import locate
from hardware_replay import PrivateInput
from lock_replay import append, any_request
from notification_replay import Fixture, await_value
from phone_gesture_replay import request
from with_xvfb import read_display_number


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--binary', type=Path, default=Path('target/debug/tarot'))
    p.add_argument('--xvfb', default='Xvfb')
    a = p.parse_args()
    out = a.output.resolve()
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.binary.resolve()
    examples = binary.parent / 'examples'
    root = Path.cwd()
    children, streams = [], []
    peer = notifier = mouse = None
    record = dict(passed=False, hardware_feedback_tested=False, actions=[],
                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())

    def log(name):
        stream = (out / name).open('x')
        streams.append(stream)
        return stream

    with tempfile.TemporaryDirectory(prefix='tarot-feedback-') as temporary:
        work = Path(temporary)
        (work / 'empty').mkdir()
        (work / 'runtime').mkdir(mode=0o700)
        commands = work / 'lock.commands'
        commands.write_text('')
        control = work / 'control.sock'

        def state(label, condition=lambda s: True, seconds=10):
            value = await_value(lambda: request(control, 'status'), condition, seconds=seconds)
            record['actions'].append(dict(label=label, state=value))
            assert 'Private fixture' not in json.dumps(value)
            return value

        def feedback(label, condition, seconds=10):
            return state(label, lambda s: condition(s['feedback']), seconds)['feedback']

        def action(value):
            request(control, 'feedback_action', action=value)

        def profile(value):
            current = request(control, 'status')['feedback']
            action({'Profile': dict(epoch=current['epoch'], profile=value)})
            return feedback('profile-' + value, lambda f: f['profile'] == value and f['pending'] is None)

        def events(name):
            return [e for e in peer.events() if e['event'] == name]

        def trigger(before, name='message-new-instant'):
            values = await_value(lambda: events('trigger'), lambda es: len(es) > before)
            assert values[-1]['name'] == name
            return values[-1]

        def end(identifier):
            values = await_value(lambda: events('end'), lambda es: any(e['id'] == identifier for e in es))
            value = next(e for e in values if e['id'] == identifier)
            assert value['belongs_to_sender']
            return value

        def notify(**fields):
            return notifier.notify(summary='Private fixture title', body='Private fixture body', timeout=0, **fields)

        def dnd(value):
            request(control, 'notification_action', action={'Settings': dict(dnd=value, hide_previews=False)})
            state('dnd-' + str(value), lambda s: s['notifications']['dnd'] == value)

        def capture(name):
            path = out / (name + '.png')
            request(control, 'capture', path=str(path))
            state('capture-' + name, lambda s: not s['capture_pending'] and not s['capture_error'])
            return path

        def click(x, y):
            mouse.move(x, y)
            time.sleep(.04)
            mouse.button(True)
            time.sleep(.06)
            mouse.button(False)
            time.sleep(.12)

        try:
            read, write = os.pipe()
            try:
                display = subprocess.Popen([a.xvfb, '-displayfd', str(write), '-nolisten', 'tcp',
                          '-noreset', '-ac', '-screen', '0', '1000x1000x24'], pass_fds=(write,),
                          stdout=log('xvfb.log'), stderr=subprocess.STDOUT)
                children.append(display)
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
                       XDG_RUNTIME_DIR=str(work / 'runtime'), DBUS_SESSION_BUS_ADDRESS=address,
                       XDG_CONFIG_HOME=str(work / 'empty'), XDG_CONFIG_DIRS=str(work / 'empty'),
                       XDG_DATA_HOME=str(work / 'empty'), XDG_DATA_DIRS=str(work / 'empty'))
            env.pop('WAYLAND_DISPLAY', None)
            env.pop('WAYLAND_SOCKET', None)
            peer = Fixture(root / 'tests/feedback_fixture.py', out / 'feedback', env)
            client = subprocess.Popen([str(binary), '--reduced-motion', '--system-services',
                     '--platform-bus', address, '--backlight-root', str(work / 'empty'),
                     '--publish-session-environment', '--notifications', '--control-socket', str(control),
                     '--config-file', str(work / 'settings.toml'), '--lock-probe-unlocked-start',
                     '--lock-client', str(examples / 'lock-probe'), '--lock-client-arg=--role',
                     '--lock-client-arg=locker', '--lock-client-arg=--commands',
                     '--lock-client-arg=' + str(commands), '--lock-client-arg=--report',
                     '--lock-client-arg=' + str(out / 'locker.jsonl'), '--exit-after-ms', '220000'],
                     env=env, stdout=log('session.log'), stderr=subprocess.STDOUT)
            children.append(client)
            await_value(lambda: control.exists() or client.poll() is not None, bool)
            assert client.poll() is None
            state('ready', lambda s: s['feedback']['available'] and s['notifications']['available'])
            notifier = Fixture(examples / 'notification-probe', out / 'notifier', env,
                               extra_args=['--exit-after-seconds', '220'])
            mouse = PrivateInput(client.pid, display, env['DISPLAY'])
            await_value(mouse.find, bool)
            request(control, 'system_menu', open=True)
            request(control, 'system_page', page='Audio')
            time.sleep(.3)
            target = locate(capture('sound-controls'), 'Silent', vertical=(.08, .87), scale=2)
            record['rendered_target'] = target
            click(*target['point'])
            feedback('rendered-silent', lambda f: f['profile'] == 'Silent' and f['pending'] is None)
            profile('Quiet')
            peer.call('mode', value='ignore-set')
            current = request(control, 'status')['feedback']
            action({'Profile': dict(epoch=current['epoch'], profile='Full')})
            feedback('profile-pending-until-observed', lambda f: f['pending'] == 'Full' and f['profile'] == 'Quiet')
            feedback('profile-observation-timeout', lambda f: f['pending'] is None and bool(f['action_error']), seconds=6)
            assert len([e for e in events('set-profile') if e['value'] == 'full']) == 1
            peer.call('mode', value='normal')
            profile('Full')

            peer.call('profile', value='unsupported-fixture-profile')
            feedback('unsupported-profile-visible', lambda f: not f['available'] and bool(f['error']))
            count = len(events('trigger'))
            notify()
            peer.call('profile', value='full')
            feedback('supported-profile-recovers', lambda f: f['available'] and f['profile'] == 'Full')
            profile('Quiet')
            assert len(events('trigger')) == count

            count = len(events('trigger'))
            identifier = notify()
            first = trigger(count)
            feedback('notification-active', lambda f: f['active_notifications'] == 1)
            assert first['hints'] == {'profile': 'full'} and first['timeout'] == -1
            count += 1
            assert notify(replaces_id=identifier) == identifier
            second = trigger(count)
            end(first['id'])
            assert first['id'] != second['id']
            peer.call('complete', id=second['id'], spoof=True)
            assert request(control, 'status')['feedback']['active_notifications'] == 1
            peer.call('complete', id=second['id'])
            feedback('real-completion', lambda f: f['active_notifications'] == 0)

            count = len(events('trigger'))
            identifier = notify(suppress_sound=True, important=True, sound_file='/private/ignored.wav')
            quiet = trigger(count)
            assert quiet['hints'] == {'profile': 'quiet'}
            notifier.call('close', id=identifier)
            end(quiet['id'])
            feedback('close-cancels', lambda f: f['active_notifications'] == 0)
            count = len(events('trigger'))
            notify(urgency=0)
            notify(feedback_profile='none')
            notify(feedback_profile='unknown')
            profile('Quiet')
            assert len(events('trigger')) == count

            identifier = notify()
            current = trigger(count)
            dnd(True)
            end(current['id'])
            feedback('dnd-cancels-active', lambda f: f['active_notifications'] == 0)
            count = len(events('trigger'))
            notify(urgency=2, important=True)
            dnd(False)
            profile('Silent')
            assert len(events('trigger')) == count
            action({'Notifications': {'enabled': False}})
            feedback('notification-setting-saved', lambda f: not f['notifications_enabled'] and f['settings_saved'])
            notify()
            action({'Notifications': {'enabled': True}})
            feedback('notification-setting-enabled', lambda f: f['notifications_enabled'] and f['settings_saved'])
            profile('Quiet')
            assert len(events('trigger')) == count

            peer.call('mode', value='hold-trigger')
            notify()
            delayed = trigger(count)
            dnd(True)
            peer.call('release-held')
            end(delayed['id'])
            feedback('late-trigger-is-ended-after-dnd', lambda f: f['active_notifications'] == 0)
            peer.call('mode', value='normal')
            dnd(False)

            current = request(control, 'status')['feedback']
            count = len(events('trigger'))
            action({'Test': {'epoch': current['epoch']}})
            testing = trigger(count, 'button-pressed')
            assert testing['timeout'] == -1 and testing['hints'] == {'profile': 'quiet'}
            feedback('test-active', lambda f: f['test_active'])
            append(commands, 'lock')
            covered = state('locked', lambda s: s['lock']['phase'] == 'Locked')
            assert set(covered) == {'ok', 'pid', 'lock'}
            assert not any_request(control, 'feedback_action', action={'Test': {'epoch': current['epoch']}})['ok']
            end(testing['id'])
            count = len(events('trigger'))
            identifier = notify()
            locked = trigger(count)
            notifier.call('close', id=identifier)
            end(locked['id'])
            append(commands, 'unlock')
            state('unlocked', lambda s: s['lock']['phase'] == 'Unlocked' and not s['feedback']['test_active'])

            current = request(control, 'status')['feedback']
            count = len(events('trigger'))
            action({'Test': {'epoch': current['epoch']}})
            testing = trigger(count, 'button-pressed')
            peer.call('complete', id=testing['id'], reason=4294967295)
            feedback('daemon-reports-no-feedback', lambda f: not f['test_active'] and f['test_result'] == 'Unavailable')
            profile('Quiet')
            assert request(control, 'status')['feedback']['test_result'] is None
            count = len(events('trigger'))
            action({'Test': {'epoch': current['epoch']}})
            testing = trigger(count, 'button-pressed')
            peer.call('complete', id=testing['id'], reason=1234)
            feedback('unknown-result-is-not-success', lambda f: not f['test_active'] and
                     f['test_result'] is None and bool(f['action_error']))

            count = len(events('trigger'))
            notify()
            trigger(count)
            previous = request(control, 'status')['feedback']
            peer.call('replace-owner')
            renewed = feedback('owner-replacement', lambda f: f['available'] and f['epoch'] > previous['epoch'] and f['active_notifications'] == 0)
            before = len(events('set-profile'))
            action({'Profile': dict(epoch=previous['epoch'], profile='Full')})
            feedback('old-epoch-rejected', lambda f: bool(f['action_error']))
            assert len(events('set-profile')) == before
            count = len(events('trigger'))
            identifier = notify()
            fresh = trigger(count)
            assert fresh['id'] == 100
            feedback('fresh-owner-active', lambda f: f['active_notifications'] == 1)
            notifier.call('close', id=identifier)
            end(100)
            feedback('fresh-owner-ended', lambda f: f['active_notifications'] == 0)
            assert all(e['belongs_to_sender'] for e in events('end'))

            count = len(events('trigger'))
            notify()
            old = trigger(count)
            previous = request(control, 'status')['feedback']
            peer.call('rapid-owner', old_id=old['id'])
            feedback('same-exporter-release-reacquire', lambda f: f['available'] and
                     f['epoch'] > previous['epoch'] and f['active_notifications'] == 0)
            count = len(events('trigger'))
            identifier = notify()
            fresh = trigger(count)
            assert fresh['id'] == 100
            feedback('old-completions-do-not-retire-reused-id', lambda f: f['active_notifications'] == 1)
            profile('Full')
            assert request(control, 'status')['feedback']['active_notifications'] == 1
            notifier.call('close', id=identifier)
            await_value(lambda: events('end'), lambda es: es[-1]['id'] == 100)
            feedback('rapid-reacquire-ended', lambda f: f['active_notifications'] == 0)

            peer.call('mode', value='deny-trigger')
            count = len(events('trigger'))
            notify()
            trigger(count)
            feedback('trigger-error-visible', lambda f: bool(f['action_error']) and f['active_notifications'] == 0)
            profile('Quiet')
            assert len(events('trigger')) == count + 1
            peer.call('mode', value='normal')

            # Recreate the entire private bus without restarting the compositor.
            previous = request(control, 'status')['feedback']
            notifier.close()
            notifier = None
            peer.close()
            peer = None
            bus.terminate()
            bus.wait(timeout=5)
            feedback('broker-gone', lambda f: not f['available'])
            bus = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile',
                    '--address=' + address], stdout=log('bus-restarted.log'), stderr=subprocess.STDOUT)
            children.append(bus)
            await_value(lambda: (work / 'bus').exists() or bus.poll() is not None, bool)
            assert bus.poll() is None
            peer = Fixture(root / 'tests/feedback_fixture.py', out / 'feedback-restarted', env)
            state('broker-recovered', lambda s: s['feedback']['available'] and
                  s['feedback']['epoch'] > previous['epoch'] and s['notifications']['available'])
            assert not events('trigger') and not events('set-profile')
            notifier = Fixture(examples / 'notification-probe', out / 'notifier-restarted', env,
                               extra_args=['--exit-after-seconds', '220'])
            action({'Profile': dict(epoch=previous['epoch'], profile='Quiet')})
            feedback('old-broker-action-rejected', lambda f: bool(f['action_error']))
            assert not events('set-profile')
            count = len(events('trigger'))
            identifier = notify()
            owned = trigger(count)
            notifier.close()
            notifier = None
            end(owned['id'])
            feedback('notification-owner-loss-cancels', lambda f: f['active_notifications'] == 0)
            notifier = Fixture(examples / 'notification-probe', out / 'notifier-batch', env,
                               extra_args=['--exit-after-seconds', '220'])
            request(control, 'notification_action', action='Clear')
            state('history-cleared', lambda s: not s['notifications']['items'])
            count = len(events('trigger'))
            for _ in range(64):
                notify()
            await_value(lambda: events('trigger'), lambda es: len(es) == count + 64, seconds=8)
            feedback('bounded-active-batch', lambda f: f['active_notifications'] == 64)
            peer.call('mode', value='hang-end')
            started = time.monotonic()
            dnd(True)
            feedback('batch-stop-has-one-timeout', lambda f: f['active_notifications'] == 0 and
                     bool(f['action_error']), seconds=5)
            record['batch_stop_64_seconds'] = time.monotonic() - started
            assert record['batch_stop_64_seconds'] < 5
            peer.call('release-held')
            peer.call('mode', value='normal')
            action({'Notifications': {'enabled': False}})
            feedback('disabled-before-restart', lambda f: not f['notifications_enabled'] and f['settings_saved'])
            dnd(False)
            profile('Quiet')
            peer.call('mode', value='hang-end')
            current = request(control, 'status')['feedback']
            count = len(events('trigger'))
            action({'Test': {'epoch': current['epoch']}})
            trigger(count, 'button-pressed')
            feedback('exit-with-active-test', lambda f: f['test_active'])
            started = time.monotonic()
            request(control, 'exit')
            assert client.wait(timeout=6) == 0
            record['shutdown_seconds'] = time.monotonic() - started
            record['passed'] = True
            print('PASS: private feedback profiles, observations, notification policy, cancellation, lock, owner epochs and shutdown', out)
        finally:
            if mouse is not None:
                mouse.close()
            cleanup = []
            for fixture in [notifier, peer]:
                if fixture is not None:
                    try:
                        fixture.close()
                    except Exception as error:
                        cleanup.append(type(error).__name__)
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=6)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
            for stream in streams:
                stream.close()
            record['cleanup_errors'] = cleanup
            record['passed'] &= not cleanup
            (out / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
            if cleanup:
                raise AssertionError(cleanup)


if __name__ == '__main__':
    main()
