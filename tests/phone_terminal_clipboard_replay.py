"""Native foot/ash touch input and real GTK clipboard exchange, synthetic data only."""
import base64
import hashlib
import json
from pathlib import Path
from dialog_button import locate
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from terminal_clipboard_fixture import GTK_TEXT, FOOT_TEXT


def replay(control, tap, guard, state, capture, out, cgroup, authenticate):
    folder=out/'terminal-clipboard'
    result={'passed':False,'physical_finger_tested':False,
            'clipboard_paste_uses_physical_modifiers':False,'mobile_modifier_controls_tested':True}
    report=folder/'clipboard-events.jsonl'
    def observe(event, condition=lambda e:True):
        return await_value(lambda:[e for e in events(report) if e['event']==event and condition(e)],bool,seconds=15)[-1]
    def activate(identifier,label,keyboard=False):
        request(control,'activate',id=identifier)
        return state('terminal-'+label,lambda s:s['view']=={'App':identifier}
            and s['presentation']['overview']==0 and s['presentation']['dock']==0
            and any(w['id']==identifier and w['rect']==s['app_area'] and w['opacity']==1 for w in s['presentation']['windows'])
            and (not keyboard or s['keyboard_visible']))
    def button(label):
        activate(peer,'peer-'+label.lower())
        name='terminal-peer-'+label.lower();capture(name)
        found=locate(out/(name+'.png'),label,vertical=(.08,.9))
        tap(found['point'][0]/2,found['point'][1]/2)
        result.setdefault('buttons',[]).append(found)
    def received(name,expected):
        path=folder/'shell'/name
        data=await_value(lambda:path.read_bytes() if path.exists() else None,lambda b:b==expected,seconds=15)
        assert path.stat().st_uid==10000 and not path.is_symlink()
        result.setdefault('received_files',{})[name]={'utf8':data.decode(),'hex':data.hex()}
    def enter():tap(250,663)
    try:
        assert not request(control,'status')['cards']
        request(control,'launch',id='test.tarot.Terminal.desktop')
        s=state('terminal-first-map',lambda s:len(s['cards'])==1 and s['keyboard_visible'])
        terminal=s['cards'][0]['id']
        activate(terminal,'first-settled',keyboard=True)
        assert s['keyboard_rect']['height']==288 and s['keyboard_owner']==terminal
        ready=await_value(lambda:json.loads((folder/'shell-ready.json').read_text()) if (folder/'shell-ready.json').exists() else None,bool,seconds=10)
        assert ready['uid']==10000 and ready['cgroup']==cgroup
        assert Path('/proc',str(ready['pid']),'cgroup').read_text()==cgroup
        await_value(lambda:Path('/proc',str(ready['pid']),'exe').resolve(),lambda p:p==Path('/bin/sh').resolve(),seconds=5)
        foot=Path('/proc',str(ready['foot_pid']))
        assert foot.stat().st_uid==10000 and (foot/'exe').resolve()==Path('/usr/bin/foot') and (foot/'cgroup').read_text()==cgroup
        result['foot_sha256']=hashlib.sha256((foot/'exe').read_bytes()).hexdigest()
        result['shell']=ready
        keyboard=Keyboard(tap)
        keyboard.type_ascii("printf '%s' 'terminalx")
        tap(330,609)
        keyboard.type_ascii("' > typed.txt");enter()
        received('typed.txt',b'terminal')
        keyboard.type_ascii("printf '%s' 'caf")
        tap(30,663);keyboard.page=1;tap(69.21429,609)
        keyboard.type_ascii("' > unicode.txt");enter()
        received('unicode.txt','café'.encode())
        capture('terminal-touch-input')
        from phone_terminal_controls_replay import replay as terminal_controls
        result['controls']=terminal_controls(tap,folder,ready['pid'],capture,control,state,terminal)
        tap(329,663)
        state('terminal-keyboard-hidden',lambda s:not s['keyboard_visible'])
        tap(329,702)
        state('terminal-keyboard-reopened',lambda s:s['keyboard_visible'] and s['keyboard_owner']==terminal)
        request(control,'launch',id='test.tarot.Clipboard.desktop')
        s=state('terminal-peer-map',lambda s:len(s['cards'])==2 and s['view']!={'App':terminal})
        peer=next(c['id'] for c in s['cards'] if c['id']!=terminal)
        event=observe('ready');assert event['uid']==10000
        assert Path('/proc',str(event['pid']),'cgroup').read_text()==cgroup
        result['gtk']=event
        button('Copy');observe('copy',lambda e:e['text']==GTK_TEXT)
        activate(terminal,'paste-target',keyboard=True);keyboard=Keyboard(tap)
        keyboard.type_ascii("printf '%s' '")
        tap(332,423);tap(269,423)
        keyboard.type_ascii("' > from-gtk.txt");enter()
        received('from-gtk.txt',GTK_TEXT.encode())
        encoded=base64.b64encode(FOOT_TEXT.encode()).decode()
        # Standard OSC52 reaches foot's real clipboard source. The shell
        # command is typed using the actual native keyboard, never injected.
        keyboard.type_ascii("printf '\\033]52;c;"+encoded+"\\007'");enter()
        button('Paste');observe('paste',lambda e:e['text']==FOOT_TEXT)
        capture('terminal-gtk-received')
        activate(terminal,'pending-modifier-before-lock',keyboard=True)
        tap(28,423);tap(332,423)
        request(control,'lock')
        protected=state('terminal-protected',lambda s:s['lock']['phase']=='Locked'
            and s['lock']['last_presented_surface'] and s['lock']['generation']==s['lock']['last_presented_generation'])
        assert set(protected)=={'ok','pid','lock'}
        authenticate('terminal-clipboard-actual-pam')
        state('terminal-restored-after-pam',lambda s:s['view']=={'App':terminal} and s['keyboard_visible'] and s['keyboard_rect']['height']==288)
        keyboard=Keyboard(tap)
        keyboard.type_ascii('cat tarot-completion-o');tap(129,423)
        keyboard.type_ascii('> after-pam-tab.txt');enter()
        received('after-pam-tab.txt',b'tab')
        keyboard.type_ascii("printf '%s' 'after-pam' > after-pam.txt");enter()
        received('after-pam.txt',b'after-pam');capture('terminal-after-pam')
        before=max(e['monotonic'] for e in events(report) if e['event']=='paste')
        button('Paste');observe('paste',lambda e:e['monotonic']>before and e['text']==FOOT_TEXT)
        activate(terminal,'after-pam',keyboard=True);keyboard=Keyboard(tap)
        keyboard.type_ascii('exit');enter()
        state('terminal-shell-exit',lambda s:all(c['id']!=terminal for c in s['cards']))
        request(control,'close',id=peer);observe('close');observe('exit')
        state('terminal-clients-gone',lambda s:not s['cards'])
        await_value(lambda:not Path('/proc',str(ready['pid'])).exists() and not Path('/proc',str(event['pid'])).exists(),bool,seconds=12)
        from phone_fullscreen_terminal_replay import replay as fullscreen_terminal
        result['fullscreen']=fullscreen_terminal(control,tap,state,capture,out,cgroup,authenticate)
        result.update(passed=True,actual_interactive_shell=True,native_touch_text_and_backspace=True,
            unicode_commit=True,hide_reopen=True,gtk_to_foot_exact=True,foot_to_gtk_exact=True,
            same_login_cgroup=True,actual_pam_restoration=True,cooperative_exit=True)
    finally:
        (folder/'validation.json').write_text(json.dumps(result,indent=2)+'\n')
    return result
