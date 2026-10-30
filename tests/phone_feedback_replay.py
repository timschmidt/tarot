"""Native feedback profiles and one-shot quiet event on the authorized phone.

The selected system themes must have no silent button feedback and no sound or
light for the quiet button event. This does not prove felt vibration or acoustic
quality. The controller restores settings and independently audits the login.
"""
import ast
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import pwd
import re
import signal
import subprocess
import sys
import threading
import time
import tomllib

from dialog_button import locate
from lock_replay import any_request
from notification_replay import await_value, session_environment
from phone_gesture_replay import request
from phone_feedback_trace import DeviceTrace


def replay(socket, screen, tap, state, capture, before, directory, cgroup, authenticate):
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    uid = 10000
    account = pwd.getpwuid(uid)
    environment = session_environment(before['pid'])
    gui_environment = dict(field.split(b'=', 1) for field in
        Path('/proc', str(before['pid']), 'environ').read_bytes().split(b'\0') if b'=' in field)
    for name in ['XDG_CONFIG_HOME', 'XDG_CONFIG_DIRS', 'XDG_DATA_HOME', 'XDG_DATA_DIRS']:
        if name.encode() in gui_environment:
            environment[name] = gui_environment[name.encode()].decode()
        else:
            environment.pop(name, None)
    environment.update(HOME=account.pw_dir, USER=account.pw_name, LOGNAME=account.pw_name,
                       XDG_RUNTIME_DIR='/run/user/10000')
    address = environment['DBUS_SESSION_BUS_ADDRESS']
    result = dict(passed=False, actual_haptic_sensation_tested=False,
                  audio_playback_requested=False, targets=[], profiles=[], events=[])
    monitor = thread = trace = None
    events, errors = [], []
    mutex = threading.Lock()
    ready = threading.Event()

    def command(argv):
        return subprocess.check_output(argv, env=environment, user=uid, group=account.pw_gid,
                                       extra_groups=os.getgrouplist(account.pw_name, account.pw_gid),
                                       text=True, timeout=8).strip()

    def bus(method, *args, destination='org.freedesktop.DBus', path='/org/freedesktop/DBus'):
        return command(['gdbus', 'call', '--address', address, '--dest', destination,
                        '--object-path', path, '--method', method, *map(str, args)])

    def number(value):
        match = re.fullmatch(r'\(uint32 (\d+),\)', value)
        assert match, value
        return int(match[1])

    def daemon():
        owner = ast.literal_eval(bus('org.freedesktop.DBus.GetNameOwner', 'org.sigxcpu.Feedback'))[0]
        pid = number(bus('org.freedesktop.DBus.GetConnectionUnixProcessID', owner))
        assert number(bus('org.freedesktop.DBus.GetConnectionUnixUser', owner)) == uid
        proc = Path('/proc', str(pid))
        assert proc.stat().st_uid == uid and (proc / 'exe').resolve() == Path('/usr/libexec/feedbackd')
        assert (proc / 'cgroup').read_text() == cgroup
        return dict(owner=owner, pid=pid, uid=uid, start=(proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
                    sha256=hashlib.sha256((proc / 'exe').read_bytes()).hexdigest(), cgroup=cgroup)

    def observed_profile():
        value = command(['gsettings', 'get', 'org.sigxcpu.feedbackd', 'profile'])
        profile = ast.literal_eval(value)
        assert profile in ['full', 'quiet', 'silent']
        return profile

    def monitor_read():
        try:
            for line in monitor.stdout:
                if line.startswith('signal ') and 'member=NameLost' in line:
                    ready.set()
                if not line.startswith('method call '):
                    continue
                member = re.search(r'\bmember=(TriggerFeedback|EndFeedback|Set)\b', line)
                sender = re.search(r'\bsender=([^ ]+)', line)
                if not member or not sender:
                    continue
                try:
                    pid = number(bus('org.freedesktop.DBus.GetConnectionUnixProcessID', sender[1]))
                except (subprocess.SubprocessError, AssertionError):
                    continue
                if pid == before['pid']:
                    assert Path('/proc', str(pid), 'cgroup').read_text() == cgroup
                    with mutex:
                        events.append(dict(member=member[1], sender=sender[1], pid=pid))
        except BaseException as error:
            errors.append(type(error).__name__)

    def calls(member):
        with mutex:
            return [event for event in events if event['member'] == member]

    def capture_path(label):
        name = 'feedback-' + label
        capture(name)
        return directory / (name + '.png')

    def swipe():
        screen.down(180, 570)
        for step in range(1, 13):
            screen.move(180, 570 - step * 25)
            time.sleep(.025)
        screen.up()
        time.sleep(.3)

    def find_profile(label, text):
        # Match the whole rendered line so "Test vibration" can never satisfy
        # the Vibration profile target. No haptic test is tapped by a profile search.
        for attempt in range(5):
            image = capture_path(label + '-' + str(attempt))
            found = subprocess.run(['tesseract', str(image), 'stdout', '--psm', '11', 'tsv'],
                                   capture_output=True, text=True, check=True, timeout=20)
            groups = {}
            for row in csv.DictReader(io.StringIO(found.stdout), delimiter='\t'):
                if row['text'].strip() and float(row['conf']) >= 70:
                    key = tuple(row[k] for k in ['page_num', 'block_num', 'par_num', 'line_num'])
                    groups.setdefault(key, []).append(row)
            matches = []
            for rows in groups.values():
                if ' '.join(r['text'] for r in rows).casefold() != text.casefold():
                    continue
                left = min(int(r['left']) for r in rows)
                right = max(int(r['left']) + int(r['width']) for r in rows)
                top = min(int(r['top']) for r in rows)
                bottom = max(int(r['top']) + int(r['height']) for r in rows)
                if 180 < top < 1250:
                    matches.append(((left + right) / 4, (top + bottom) / 4))
            if matches:
                point = max(matches, key=lambda p: p[1])
                result['targets'].append(dict(label=label, text=text, point=point))
                tap(*point)
                return
            swipe()
        raise AssertionError('Rendered feedback profile not found: ' + text)

    def profile(name, text=None):
        if text is not None:
            find_profile('profile-' + name, text)
        else:
            current = request(socket, 'status')['feedback']
            request(socket, 'feedback_action', action={'Profile': dict(epoch=current['epoch'], profile=name)})
        state('feedback-profile-' + name, lambda s: s['feedback']['profile'] == name and
              s['feedback']['pending'] is None)
        actual = observed_profile()
        assert actual == name.lower()
        result['profiles'].append(dict(profile=name, independent_gsettings=actual))

    def test(label, expected):
        before_calls = len(calls('TriggerFeedback'))
        previous = trace.snapshot(label + '-before')
        for attempt in range(5):
            image = capture_path(label + '-' + str(attempt))
            try:
                target = locate(image, 'Test', vertical=(.15, .87), scale=1)
            except AssertionError:
                swipe()
                continue
            result['targets'].append(dict(label=label, target=target))
            tap(target['point'][0] / 2, target['point'][1] / 2)
            break
        else:
            raise AssertionError('Rendered vibration test not found')
        state('feedback-' + label, lambda s: not s['feedback']['test_active'] and
              s['feedback']['test_result'] == expected and not s['feedback']['action_error'])
        await_value(lambda: calls('TriggerFeedback'), lambda es: len(es) == before_calls + 1)
        observed = trace.snapshot(label + '-after')
        assert observed[:len(previous)] == previous, 'Trace ring overran or changed'
        device_calls = observed[len(previous):]
        uploads = [call for call in device_calls if call['kind'] == 'ioctl' and call['cmd'] & 0xffff == 0x4580]
        writes = [call for call in device_calls if call['kind'] == 'write']
        if expected == 'Unavailable':
            assert not uploads and not writes, device_calls
        else:
            assert uploads and writes, device_calls
            assert all(call['result'] == 0 for call in uploads), uploads
            assert all(call['count'] == 24 and call['result'] == 24 for call in writes), writes
        result['events'].append(dict(label=label, result=expected, gui_method_calls=1,
                                     kernel_uploads=uploads, kernel_event_writes=writes))

    try:
        initial = state('feedback-ready', lambda s: s['feedback']['available'] and s['notifications']['available'])
        assert initial['feedback']['profile'] in ['Full', 'Quiet', 'Silent']
        original_profile = initial['feedback']['profile']
        original_setting = initial['feedback']['notifications_enabled']
        assert initial['notifications']['dnd'], 'Native QA must suppress unsolicited alerts'
        assert observed_profile() == original_profile.lower()
        assert ast.literal_eval(command(['gsettings', 'get', 'org.sigxcpu.feedbackd', 'theme'])) == 'default'
        themes = [Path('/usr/share/feedbackd/themes', name) for name in ['default.json', 'pine64,pinephone-pro.json']]
        result['theme_sha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in themes}
        for path in themes:
            data = json.loads(path.read_text())
            for entry in data['profiles']:
                buttons = [f for f in entry['feedbacks'] if f.get('event-name') == 'button-pressed']
                if entry['name'] == 'silent':
                    assert not buttons
                if entry['name'] == 'quiet':
                    assert buttons and all(f['type'].startswith('Vibra') for f in buttons)
        assert not list(Path(account.pw_dir, '.config/feedbackd').glob('**/*.json'))
        assert not list(Path(account.pw_dir, '.local/share/feedbackd').glob('**/*.json'))
        result['original_daemon'] = daemon()
        inputs = []
        for fd in Path('/proc', str(result['original_daemon']['pid']), 'fd').iterdir():
            try:
                target = os.readlink(fd)
                if target.startswith('/dev/input/event'):
                    device = Path('/sys/class/input', Path(target).name, 'device')
                    bits = (device / 'capabilities/ff').read_text().strip()
                    inputs.append(dict(fd=int(fd.name), path=target, name=(device / 'name').read_text().strip(), ff=bits))
            except (FileNotFoundError, ProcessLookupError):
                pass
        result['provider_input_devices'] = inputs
        assert any(int(''.join(device['ff'].split()), 16) for device in inputs), 'Feedback daemon has no open force-feedback device'
        descriptors = {device['fd']: device['path'] for device in inputs
                       if device['name'] == 'gpio-vibrator' and int(''.join(device['ff'].split()), 16)}
        assert descriptors, 'Expected dedicated GPIO vibrator descriptor'
        trace = DeviceTrace(result['original_daemon'], descriptors, directory)
        rules = ["type='method_call',path='/org/sigxcpu/Feedback',interface='org.sigxcpu.Feedback'",
                 "type='method_call',path='/org/sigxcpu/Feedback',interface='org.freedesktop.DBus.Properties',member='Set'"]
        monitor = subprocess.Popen(['dbus-monitor', '--address', address, *rules], env=environment,
                  user=uid, group=account.pw_gid, extra_groups=os.getgrouplist(account.pw_name, account.pw_gid),
                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        thread = threading.Thread(target=monitor_read, daemon=True)
        thread.start()
        assert ready.wait(5), 'Feedback monitor did not acknowledge readiness'
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Audio')
        profile('Silent', 'Silent')
        test('silent-reports-no-output', 'Unavailable')
        profile('Quiet', 'Vibration')
        test('quiet-single-play', 'Ended')
        profile('Full', 'Sound & vibration')
        # The test itself always caps the event at Quiet; no Sound feedback is requested.
        test('full-profile-still-caps-test', 'Ended')
        profile('Silent')
        trace.close()
        assert not trace.path.exists()
        result['private_trace_removed_and_global_unchanged'] = True
        owned = result['original_daemon']
        current = request(socket, 'status')['feedback']
        assert not current['test_active'] and current['active_notifications'] == 0
        fd = os.pidfd_open(owned['pid'])
        try:
            assert daemon() == owned
            signal.pidfd_send_signal(fd, signal.SIGKILL)
        finally:
            os.close(fd)
        state('feedback-daemon-recovered', lambda s: s['feedback']['available'] and
              s['feedback']['epoch'] > current['epoch'] and s['feedback']['profile'] == 'Silent')
        result['replacement_daemon'] = daemon()
        assert result['replacement_daemon']['pid'] != owned['pid']
        assert result['replacement_daemon']['sha256'] == owned['sha256']
        before_sets = len(calls('Set'))
        request(socket, 'feedback_action', action={'Profile': dict(epoch=current['epoch'], profile='Full')})
        state('feedback-retired-epoch-rejected', lambda s: bool(s['feedback']['action_error']))
        assert observed_profile() == 'silent' and len(calls('Set')) == before_sets
        profile('Silent')
        config = Path(account.pw_dir, '.config/tarot/settings.toml')
        for enabled in [not original_setting, original_setting]:
            for attempt in range(5):
                image = capture_path('notification-switch-' + str(enabled) + '-' + str(attempt))
                try:
                    target = locate(image, 'notifications', vertical=(.15, .87), scale=1)
                except AssertionError:
                    swipe()
                    continue
                result['targets'].append(dict(label='notification-switch-' + str(enabled), target=target))
                tap(target['point'][0] / 2, target['point'][1] / 2)
                break
            else:
                raise AssertionError('Rendered notification feedback switch not found')
            state('feedback-setting-' + str(enabled), lambda s: s['feedback']['notifications_enabled'] == enabled and s['feedback']['settings_saved'])
            assert tomllib.loads(config.read_text())['feedback']['notifications'] is enabled
        profile(original_profile)
        request(socket, 'lock')
        covered = state('feedback-protected', lambda s: s['lock']['phase'] == 'Locked')
        assert set(covered) == {'ok', 'pid', 'lock'}
        assert not any_request(socket, 'feedback_action', action={'Profile': dict(epoch=current['epoch'], profile='Full')})['ok']
        assert not any_request(socket, 'feedback_action', action={'Test': dict(epoch=current['epoch'])})['ok']
        authenticate('feedback-pam')
        state('feedback-after-pam', lambda s: s['lock']['phase'] == 'Unlocked' and not s['system_menu_open'])
        assert observed_profile() == original_profile.lower()
        result['original_profile_restored'] = True
        result['actual_pam'] = True
        result['passed'] = True
        return result
    finally:
        failed = sys.exc_info()[0] is not None
        if trace is not None:
            try:
                trace.close()
            except Exception as error:
                errors.append('trace-cleanup-' + type(error).__name__)
        if monitor is not None:
            try:
                monitor.terminate()
                monitor.wait(timeout=5)
                if thread is not None:
                    thread.join(timeout=10)
                    assert not thread.is_alive()
            except Exception as error:
                errors.append(type(error).__name__)
        result['method_headers'] = events
        result['monitor_errors'] = errors
        if errors:
            result['passed'] = False
        (directory / 'feedback-result.json').write_text(json.dumps(result, indent=2) + '\n')
        if errors and not failed:
            raise AssertionError(errors)
