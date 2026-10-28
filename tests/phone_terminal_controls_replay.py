"""Touch-only terminal tools exercised against real foot, ash and PTY children."""
import hashlib,os,signal
from pathlib import Path
from notification_replay import await_value
from phone_touch_keyboard import Keyboard
from phone_gesture_replay import request


def replay(tap,folder,shell_pid,capture,control,state,terminal):
    result={'passed':False,'physical_keyboard_used':False,'files':{}}
    keyboard=Keyboard(tap)
    def received(name,expected):
        p=folder/'shell'/name
        data=await_value(lambda:p.read_bytes() if p.exists() else None,lambda b:b==expected,seconds=15)
        assert p.stat().st_uid==10000 and not p.is_symlink()
        result['files'][name]=data.hex()
    def line(text):keyboard.type_ascii(text);tap(250,663)
    def child(name):
        found=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                fields=(p/'stat').read_text().rsplit(')',1)[1].split()
                if int(fields[1])==shell_pid and (p/'comm').read_text().strip()==name:
                    found.append((int(p.name),fields[19]))
            except (FileNotFoundError,ProcessLookupError):pass
        return found
    def gone(identity):
        p=Path('/proc',str(identity[0]),'stat')
        try:return p.read_text().rsplit(')',1)[1].split()[19]!=identity[1]
        except (FileNotFoundError,ProcessLookupError):return True
    # Arrow insertion inside a quoted argument must reach ash's editor.
    keyboard.type_ascii("printf '%s' 'ab")
    tap(332,423);tap(151,423)
    keyboard.type_ascii('X');tap(210,423)
    keyboard.type_ascii("' > navigation.txt");tap(250,663)
    received('navigation.txt',b'aXb')
    keyboard.type_ascii("printf '%s' 'nav' > home-end.txt")
    tap(33,423);keyboard.type_ascii('true; ');tap(92,423)
    keyboard.type_ascii('.ok');tap(250,663)
    received('home-end.txt.ok',b'nav')
    tap(328,423)
    # Up recalls the last command; Down must restore an empty new line.
    tap(231,423);keyboard.type_ascii('.history');tap(250,663)
    received('home-end.txt.ok.history',b'nav')
    tap(231,423);tap(281,423)
    line("printf '%s' 'down' > down.txt");received('down.txt',b'down')
    keyboard.type_ascii("printf '%s' 'control' > control.txt")
    tap(28,423);keyboard.type_ascii('a');keyboard.type_ascii('true; ')
    tap(28,423);keyboard.type_ascii('e');keyboard.type_ascii('.ok');tap(250,663)
    received('control.txt.ok',b'control')
    # Ctrl+C must terminate an actual long-running foreground child.
    line('/bin/sleep 30')
    sleeping=await_value(lambda:child('sleep'),bool,seconds=8);assert len(sleeping)==1
    tap(28,423);keyboard.type_ascii('c')
    await_value(lambda:gone(sleeping[0]),bool,seconds=6)
    line("printf '%s' 'interrupt' > interrupt.txt");received('interrupt.txt',b'interrupt')
    result['interrupted_child']={'pid':sleeping[0][0],'start':sleeping[0][1]}
    # Standard raw PTY reads independently observe Alt and Escape bytes.
    for name,count,action,expected in [
        ('alt.bin',2,lambda:(tap(79,423),keyboard.type_ascii('x')),b'\x1bx'),
        ('escape.bin',1,lambda:tap(180,423),b'\x1b')]:
        line('stty raw -echo; /bin/dd bs=1 count='+str(count)+' of='+name+'; stty sane')
        reader=await_value(lambda:child('dd'),bool,seconds=8);assert len(reader)==1
        action();received(name,expected)
        await_value(lambda:gone(reader[0]),bool,seconds=6)
    line("printf '%s' 'tab' > tarot-completion-only")
    keyboard.type_ascii('cat tarot-completion-o');tap(129,423)
    keyboard.type_ascii('> completed.txt');tap(250,663)
    received('completed.txt',b'tab')
    capture('terminal-touch-controls')
    # Kill only the authenticated session's managed helper. The replacement
    # must rediscover terminal purpose and discard the old local selections.
    tap(28,423);tap(332,423)
    current=request(control,'status');pid=current['input_method']['pid'];assert pid
    proc=Path('/proc',str(pid));exe=Path('/usr/bin/tarot-keyboard')
    assert proc.stat().st_uid==10000 and (proc/'exe').resolve()==exe
    assert (proc/'cgroup').read_text()==Path('/proc',str(shell_pid),'cgroup').read_text()
    identity=proc.joinpath('stat').read_text().rsplit(')',1)[1].split()[19]
    digest=hashlib.sha256((proc/'exe').read_bytes()).hexdigest()
    assert digest==hashlib.sha256(exe.read_bytes()).hexdigest()
    fd=os.pidfd_open(pid)
    try:
        assert proc.joinpath('stat').read_text().rsplit(')',1)[1].split()[19]==identity
        signal.pidfd_send_signal(fd,signal.SIGKILL)
    finally:os.close(fd)
    recovered=state('terminal-helper-recovered',lambda s:
        s['input_method']['pid'] and s['input_method']['pid']!=pid and
        s['keyboard_visible'] and s['keyboard_rect']['height']==288 and
        s['keyboard_owner']==terminal and s['view']=={'App':terminal})
    capture('terminal-helper-recovered')
    keyboard=Keyboard(tap)
    keyboard.type_ascii('cat tarot-completion-o');tap(129,423)
    keyboard.type_ascii('> after-helper-crash.txt');tap(250,663)
    received('after-helper-crash.txt',b'tab')
    result['helper_recovery']={'passed':True,'killed_pid':pid,'killed_start':identity,
        'replacement_pid':recovered['input_method']['pid'],'keyboard_sha256':digest,
        'pending_modifiers_and_page_cleared':True,'terminal_panel_restored':True}
    result.update(passed=True,arrows_edit_actual_line=True,home_end=True,history_up_down=True,
        control_one_shot=True,foreground_interrupt=True,alt_escape_exact_pty_bytes=True,tab_completion=True)
    return result
