"""At startup by finger: Power page -> pkexec -> Tarot's Polkit agent -> real password.

Selects direct boot with the actual administrator password, refuses a canceled
request without changing the file, then returns to the login screen. Taps use
OCR of the rendered page; credentials go through the guarded on-screen keyboard.
No reboot happens here; boot behavior has its own phased test.
"""
import hashlib
import time
from pathlib import Path
from dialog_button import locate
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard

CONFD = Path('/etc/conf.d/greetd')
MANAGED = Path('/etc/tarot/greetd-direct.toml')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def replay(socket, screen, tap, state, capture, before, directory, password):
    r = dict(passed=False, targets=[], original_confd_sha256=sha(CONFD))
    assert not MANAGED.exists()

    def boot(s):
        return s['display_settings']['boot']

    def page(label):
        request(socket, 'system_menu', open=True)
        request(socket, 'system_page', page='Power')
        return state('boot-' + label + '-page', lambda s: s['lock']['phase'] == 'Unlocked' and s['system_menu_open']
                     and s['system_page'] == 'Power' and boot(s)['available'] and boot(s)['current'] is not None
                     and boot(s)['pending'] is None)

    def word(label, text, scroll=True):
        last = None
        for attempt in range(8 if scroll else 1):
            time.sleep(.3)
            name = 'boot-' + label + '-' + str(attempt)
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
        s = state('boot-' + label + '-agent', lambda s: s['authorization']['request'] is not None
                  and s['authorization']['request']['phase'] in ['Choose', 'Question'], seconds=40)
        if s['authorization']['request']['phase'] == 'Choose':
            # wheel offers root and user; choose the owner's account by finger.
            word(label + '-account', 'user', scroll=False)
            s = state('boot-' + label + '-question', lambda s: s['authorization']['request'] is not None
                      and s['authorization']['request']['phase'] == 'Question', seconds=30)
        q = s['authorization']['request']
        assert q['echo'] is False and s['keyboard_visible'] and boot(s)['pending'] is not None
        return q

    page('initial')
    assert boot(state('boot-initial', lambda s: True))['current'] == 'Greeter'
    word('select-tarot', 'locked')
    question('select-tarot')
    Keyboard(tap).type_ascii(password)
    word('select-tarot-submit', 'Authenticate', scroll=False)
    s = state('boot-tarot-confirmed', lambda s: s['lock']['phase'] == 'Unlocked' and boot(s)['current'] == 'Tarot'
              and boot(s)['pending'] is None, seconds=60)
    assert boot(s)['error'] is None
    assert MANAGED.is_file() and 'cfgfile="%s"' % MANAGED in CONFD.read_text()
    r['tarot_selected_by_finger'] = True
    switched = sha(CONFD)

    page('cancel')
    word('select-greeter-cancel', 'login')
    question('cancel')
    word('cancel', 'Cancel', scroll=False)
    s = state('boot-cancel-reported', lambda s: boot(s)['pending'] is None and boot(s)['error'] is not None, seconds=60)
    assert boot(s)['current'] == 'Tarot' and 'Not changed' in boot(s)['error']
    assert sha(CONFD) == switched and MANAGED.is_file()
    r['cancel_left_file_unchanged'] = True

    page('restore')
    word('select-greeter', 'login')
    question('restore')
    Keyboard(tap).type_ascii(password)
    word('restore-submit', 'Authenticate', scroll=False)
    s = state('boot-greeter-confirmed', lambda s: boot(s)['current'] == 'Greeter' and boot(s)['pending'] is None, seconds=60)
    assert sha(CONFD) == r['original_confd_sha256'] and not MANAGED.exists()
    r['greeter_restored_by_finger'] = True
    request(socket, 'system_menu', open=False)
    state('boot-closed', lambda s: not s['system_menu_open'] and s['cards'] == before['cards'])
    r['passed'] = True
    return r
