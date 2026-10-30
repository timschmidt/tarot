"""Real NetworkManager scan authorization on the verified spare PinePhone Pro.

Temporarily makes only this GUI process's explicit Wi-Fi scan require auth_self.
It never grants an action, retains a grant, changes profiles or toggles radios.
The root controller owns test input and removes its rule in all exit paths.
"""
import hashlib
import json
import os
from pathlib import Path
import time

from dialog_button import locate
from lock_replay import any_request
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard


def replay(socket, screen, tap, state, capture, before, directory, cgroup, authenticate, password):
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    pid = before['pid']
    proc = Path('/proc', str(pid))
    start = (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
    binary_sha = hashlib.sha256((proc / 'exe').read_bytes()).hexdigest()
    assert proc.stat().st_uid == 10000 and (proc / 'cgroup').read_text() == cgroup
    rule = Path('/etc/polkit-1/rules.d/00-tarot-qa-scan.rules')
    assert not rule.exists() and not rule.is_symlink()
    text = ('// Temporary restrictive Tarot scan acceptance rule. No retained authorization.\n'
            'polkit.addRule(function(action, subject) {\n'
            '    if (action.id === "org.freedesktop.NetworkManager.wifi.scan" &&\n'
            f'        subject.pid === {pid} && subject.user === "user" &&\n'
            '        subject.local && subject.active) {\n'
            '        return polkit.Result.AUTH_SELF;\n'
            '    }\n'
            '});\n')
    result = dict(passed=False, real_networkmanager=True, real_authority=True,
                  process_pid=pid, process_start=start, binary_sha256=binary_sha,
                  request_uid=10000, radio_or_profiles_changed=False,
                  rule_sha256=hashlib.sha256(text.encode()).hexdigest(), cases=[], targets=[])
    original_guard = screen.guard
    active = None
    created = False

    def identity_guard():
        original_guard()
        assert proc.stat().st_uid == 10000
        assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == start
        assert (proc / 'cgroup').read_text() == cgroup

    def entry_guard():
        identity_guard()
        value = request(socket, 'status')
        assert value['pid'] == pid and value['lock']['phase'] == 'Unlocked'
        q = value['authorization']['request']
        assert q is not None and (q['id'], q['turn']) == active
        assert q['phase'] == 'Question' and q['echo'] is False
        assert value['system_menu_open'] and value['system_page'] == 'Authorization'
        assert value['keyboard_visible']
        output = value['output']
        assert output['orientation'] == 'normal' and output['requested'] is None
        assert output['revision'] == output['presented_revision']

    def type_response(q, value):
        nonlocal active
        active = (q['id'], q['turn'])
        screen.guard = entry_guard
        try:
            Keyboard(tap).type_ascii(value)
        finally:
            screen.guard = original_guard
            active = None

    def word(label, text):
        time.sleep(.2)
        name = 'network-authorization-' + label
        capture(name)
        target = locate(directory / (name + '.png'), text, vertical=(.07, .65), scale=1)
        result['targets'].append(dict(label=label, target=target))
        tap(target['point'][0] / 2, target['point'][1] / 2)

    def returned(label):
        return state('network-authorization-' + label, lambda s:
                     s['lock']['phase'] == 'Unlocked' and
                     s['authorization']['request'] is None and
                     s['system_menu_open'] and s['system_page'] == 'Wifi' and
                     not s['keyboard_visible'] and s['credential_input'] is None)

    def question(label):
        word(label + '-scan', 'Scan')
        value = state('network-authorization-' + label + '-question', lambda s:
                      s['network']['busy'] and s['network']['pending'] == 'Scan' and
                      s['authorization']['request'] is not None and
                      s['authorization']['request']['phase'] == 'Question' and
                      s['keyboard_visible'])
        q = value['authorization']['request']
        assert not q['echo']
        return q

    def idle(label, error):
        value = state('network-authorization-' + label, lambda s:
                      not s['network']['busy'] and s['network']['pending'] is None and
                      bool(s['network']['action_error']) is error, seconds=45)
        assert value['network']['state'] == initial['network']['state']
        assert value['network']['active_profile'] == initial['network']['active_profile']
        assert value['cards'] == before['cards'] and value['view'] == before['view']
        result['cases'].append(dict(label=label, mechanism_error=error,
                                   last_scan=value['network']['last_scan']))
        return value

    try:
        initial = state('network-authorization-initial', lambda s:
                        s['authorization']['can_prompt'] and s['network']['available'] and
                        not s['network']['busy'] and s['network']['state'] == 'Connected')
        assert initial['network']['scan_permission'] == 'Yes'
        result['original_permissions'] = {key: initial['network'][key] for key in
            ['scan_permission', 'control_permission', 'radio_permission', 'create_permission']}
        identity_guard()
        with rule.open('x') as stream:
            created = True
            os.fchmod(stream.fileno(), 0o644)
            stream.write(text)
        assert rule.stat().st_uid == 0
        # The GUI's own GetPermissions must observe the changed policy before
        # a scan can be dispatched. The ordinary SSH session is a different subject.
        state('network-authorization-restrictive-rule-loaded', lambda s:
              s['network']['scan_permission'] == 'Authentication', seconds=25)
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Wifi')

        q = question('correct')
        baseline = request(socket, 'status')['network']['last_scan']
        time.sleep(6.2)
        state('network-authorization-human-delay', lambda s: s['network']['busy'] and
              s['authorization']['request'] is not None and
              s['authorization']['request']['id'] == q['id'])
        type_response(q, password)
        word('correct-submit', 'Authenticate')
        returned('correct-return')
        value = idle('correct-scan-observed', False)
        assert value['network']['last_scan'] > baseline

        q = question('wrong')
        type_response(q, 'wrong-qa-response')
        word('wrong-submit', 'Authenticate')
        state('network-authorization-wrong-cleared', lambda s:
              s['authorization']['request'] is None and s['authorization']['error'] and
              not s['keyboard_visible'] and s['system_page'] == 'Authorization')
        idle('wrong-denied-by-mechanism', True)
        word('wrong-close', 'Close')
        returned('wrong-return')

        q = question('cancel')
        type_response(q, 'unsubmitted')
        word('cancel', 'Cancel')
        returned('cancel-return')
        idle('cancel-denied-by-mechanism', True)

        q = question('lock')
        type_response(q, 'unsubmitted')
        request(socket, 'lock')
        covered = state('network-authorization-locked', lambda s: s['lock']['phase'] == 'Locked')
        assert set(covered) == {'ok', 'pid', 'lock'}
        assert not any_request(socket, 'authorization_action',
                               action={'Cancel': {'request': q['id']}})['ok']
        authenticate('network-authorization-pam')
        state('network-authorization-unlock-keeps-retired', lambda s:
              s['lock']['phase'] == 'Unlocked' and s['authorization']['request'] is None and
              not s['system_menu_open'])
        idle('lock-denied-by-mechanism', True)

        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Wifi')
        q = question('fresh-after-lock')
        type_response(q, password)
        word('fresh-submit', 'Authenticate')
        returned('fresh-return')
        idle('fresh-scan-observed', False)
        result['passed'] = True
        return result
    finally:
        screen.guard = original_guard
        active = None
        try:
            if created:
                assert rule.read_text() == text and rule.stat().st_uid == 0
                rule.unlink()
                result['temporary_rule_removed'] = not rule.exists()
                restored = state('network-authorization-original-permissions-restored', lambda s:
                                 s['network']['scan_permission'] == 'Yes', seconds=25)
                assert all(restored['network'][key] == value for key, value in result['original_permissions'].items())
                result['original_permissions_restored'] = True
        except BaseException:
            result['passed'] = False
            raise
        finally:
            (directory / 'network-authorization-result.json').write_text(json.dumps(result, indent=2) + '\n')
