"""System sleep policy by finger: Power page -> pkexec -> Tarot's Polkit agent.

Starts with the policy off. Turns it on with the actual administrator password
(the drop-in appears and the page asks for a restart, while elogind keeps
IdleAction=ignore), refuses a canceled request without changing anything, then
turns it off again: the drop-in is removed and elogind's reload restores
IdleAction=ignore at once. No reboot happens here; the restart path has its
own check. Taps use OCR of the rendered page.
"""
import subprocess
import time
from pathlib import Path
from dialog_button import locate
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

MANAGED = Path('/etc/elogind/logind.conf.d/50-tarot-automatic-sleep-setting.conf')
PACKAGE = Path('/etc/elogind/logind.conf.d/50-tarot-automatic-sleep.conf')


def idle_action():
    raw = subprocess.check_output(['gdbus', 'call', '--system', '--dest', 'org.freedesktop.login1', '--object-path',
                                   '/org/freedesktop/login1', '--method', 'org.freedesktop.DBus.Properties.Get',
                                   'org.freedesktop.login1.Manager', 'IdleAction'], text=True, timeout=8)
    return raw.strip().split("'")[1]


def replay(socket, screen, tap, state, capture, before, directory, password):
    r = dict(passed=False, targets=[])
    assert not MANAGED.exists() and not PACKAGE.exists()
    r['idle_action_before'] = idle_action()

    def policy(s):
        return s['display_settings']['sleep_policy']

    def page(label):
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Power')
        return state('sleep-policy-' + label + '-page', lambda s: s['lock']['phase'] == 'Unlocked' and s['system_menu_open']
                     and s['system_page'] == 'Power' and policy(s)['available'] and policy(s)['enabled'] is not None
                     and policy(s)['pending'] is None)

    def word(label, text, scroll=True):
        last = None
        for attempt in range(8 if scroll else 1):
            time.sleep(.3)
            name = 'sleep-policy-' + label + '-' + str(attempt)
            capture(name)
            try:
                target = locate(directory/(name + '.png'), text, vertical=(.07, .94), scale=1)
                r['targets'].append(dict(label=label, target=target))
                tap(target['point'][0]/2, target['point'][1]/2)
                return
            except AssertionError as error:
                last = error
            if scroll:
                screen.down(180, 580)
                for y in range(550, 250, -30):
                    screen.move(180, y, delay=.025)
                screen.up()
        raise last

    def question(label):
        s = state('sleep-policy-' + label + '-agent', lambda s: s['authorization']['request'] is not None
                  and s['authorization']['request']['phase'] in ['Choose', 'Question'], seconds=40)
        if s['authorization']['request']['phase'] == 'Choose':
            word(label + '-account', 'user', scroll=False)
            s = state('sleep-policy-' + label + '-question', lambda s: s['authorization']['request'] is not None
                      and s['authorization']['request']['phase'] == 'Question', seconds=30)
        q = s['authorization']['request']
        assert q['echo'] is False and s['keyboard_visible'] and policy(s)['pending'] is not None
        return q

    assert policy(page('initial'))['enabled'] is False
    word('enable', 'Let')
    question('enable')
    Keyboard(tap).type_ascii(password)
    word('enable-submit', 'Authenticate', scroll=False)
    s = state('sleep-policy-enabled', lambda s: s['lock']['phase'] == 'Unlocked' and policy(s)['enabled'] is True
              and policy(s)['pending'] is None, seconds=60)
    assert policy(s)['error'] is None and policy(s)['restart_required'] and not policy(s)['package']
    assert MANAGED.is_file() and 'IdleAction=suspend' in MANAGED.read_text()
    r['idle_action_after_enable'] = idle_action()
    capture('sleep-policy-restart-note')
    r['enabled_by_finger'] = True
    enabled_text = MANAGED.read_text()

    page('cancel')
    word('disable-cancel', 'Let')
    question('cancel')
    word('cancel', 'Cancel', scroll=False)
    s = state('sleep-policy-cancel-reported', lambda s: policy(s)['pending'] is None and policy(s)['error'] is not None,
              seconds=60)
    assert policy(s)['enabled'] is True and 'Not changed' in policy(s)['error']
    assert MANAGED.read_text() == enabled_text
    r['cancel_left_file_unchanged'] = True

    page('disable')
    word('disable', 'Let')
    question('disable')
    Keyboard(tap).type_ascii(password)
    word('disable-submit', 'Authenticate', scroll=False)
    s = state('sleep-policy-disabled', lambda s: policy(s)['enabled'] is False and policy(s)['pending'] is None, seconds=60)
    assert not MANAGED.exists() and not policy(s)['restart_required']
    r['idle_action_after_disable'] = idle_action()
    assert r['idle_action_after_disable'] == 'ignore'
    r['disabled_by_finger'] = True
    request(socket, 'system_menu', open=False)
    state('sleep-policy-closed', lambda s: not s['system_menu_open'] and s['cards'] == before['cards'])
    r['passed'] = True
    return r
