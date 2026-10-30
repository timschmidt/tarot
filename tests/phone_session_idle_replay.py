"""Real session idle hints with protected panel off; IdleAction must stay ignore.

Covers the persistent sleep settings: disabled, delayed and external-power policy.

No Suspend, system policy change, physical audio or Bluetooth operation.
"""
import hashlib
import json
import os
from pathlib import Path
import pwd
import select
import signal
import subprocess
import sys
import threading
import time

from notification_replay import await_value
from phone_audio_state import devices, pactl, restore, snapshot
from phone_gesture_replay import request
from phone_lock_replay import Keyboard
from phone_suspend_probe import stats

NAME = 'org.freedesktop.login1'
ROOT = '/org/freedesktop/login1'
MANAGER = NAME + '.Manager'
SESSION = NAME + '.Session'


class Observer:
    def __init__(self, pid):
        assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
        import gi
        gi.require_version('Gio', '2.0')
        from gi.repository import Gio, GLib
        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
        self.owner = self.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                               'GetNameOwner', '(s)', (NAME,))[0]
        assert self.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                         'GetConnectionUnixUser', '(s)', (self.owner,))[0] == 0
        self.path = self.call(self.owner, ROOT, MANAGER, 'GetSessionByPID', '(u)', (pid,))[0]
        self.pid = pid
        self.session_id = self.prop(self.path, SESSION, 'Id')
        assert self.prop(self.path, SESSION, 'User')[0] == 10000
        assert self.prop(self.path, SESSION, 'Type') == 'wayland'
        assert self.prop(self.path, SESSION, 'Class') == 'user'
        assert self.prop(self.path, SESSION, 'Active') and not self.prop(self.path, SESSION, 'Remote')
        assert self.prop(self.path, SESSION, 'VTNr') == 7
        assert self.prop(self.path, SESSION, 'Seat')[0] == 'seat0'
        self.events = []
        self.mutex = threading.Lock()
        self.stop = threading.Event()
        self.ready = threading.Event()
        self.failure = None
        self.context = GLib.MainContext.new()
        self.thread = threading.Thread(target=self.watch)
        self.thread.start()
        assert self.ready.wait(5)
        assert self.failure is None
        self.exit_index = None
        try:
            self.initial = self.read()
            assert self.initial['idle'] is False
        except BaseException:
            self.finish()
            raise

    def call(self, dest, path, interface, member, signature=None, values=()):
        argument = None if signature is None else self.GLib.Variant(signature, values)
        return self.bus.call_sync(dest, path, interface, member, argument, None,
                                 self.Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def prop(self, path, interface, field):
        return self.call(self.owner, path, 'org.freedesktop.DBus.Properties', 'Get',
                         '(ss)', (interface, field))[0]

    def read(self):
        assert self.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                         'GetNameOwner', '(s)', (NAME,))[0] == self.owner
        action = self.prop(ROOT, MANAGER, 'IdleAction')
        assert action == 'ignore', 'Native qualification must not enable a manager idle action'
        return dict(idle=self.prop(self.path, SESSION, 'IdleHint'), action=action,
                    session=self.path, compositor_pid=self.pid, uid=10000)

    def append(self, value):
        with self.mutex:
            self.events.append(dict(monotonic=time.monotonic(), **value))

    def watch(self):
        subscriptions = []
        self.context.push_thread_default()
        try:
            def changed(*args):
                try:
                    interface, changed, invalid = args[5].unpack()
                    if interface == SESSION:
                        if 'IdleHint' in changed:
                            self.append(dict(event='hint', value=changed['IdleHint']))
                        elif 'IdleHint' in invalid:
                            self.append(dict(event='hint-invalidated'))
                except BaseException as error:
                    self.failure = type(error).__name__
            def removed(*args):
                identifier, path = args[5].unpack()
                if identifier == self.session_id and path == self.path:
                    self.append(dict(event='session-removed'))
            subscriptions.append(self.bus.signal_subscribe(self.owner, 'org.freedesktop.DBus.Properties',
                'PropertiesChanged', self.path, SESSION, self.Gio.DBusSignalFlags.NONE, changed))
            subscriptions.append(self.bus.signal_subscribe(self.owner, MANAGER, 'SessionRemoved', ROOT,
                self.session_id, self.Gio.DBusSignalFlags.NONE, removed))
            self.ready.set()
            while not self.stop.is_set():
                self.context.iteration(True)
        except BaseException as error:
            self.failure = type(error).__name__
            self.ready.set()
        finally:
            for subscription in subscriptions:
                self.bus.signal_unsubscribe(subscription)
            self.context.pop_thread_default()

    def recorded(self):
        with self.mutex:
            return list(self.events)

    def before_exit(self, expected):
        assert self.read()['idle'] is expected
        self.exit_index = len(self.recorded())

    def finish(self, require_teardown=False, expect_true=False):
        try:
            if require_teardown:
                events = await_value(self.recorded, lambda es:any(e['event']=='session-removed' for e in es), seconds=15)
                if expect_true:
                    assert self.exit_index is not None
                    tail = events[self.exit_index:]
                    cleared = next(i for i,e in enumerate(tail) if e['event']=='hint' and e['value'] is False)
                    removed = next(i for i,e in enumerate(tail) if e['event']=='session-removed')
                    assert cleared < removed, 'Explicit false hint must precede session removal'
            assert self.failure is None, self.failure
            return dict(passed=True, events=self.recorded(), session_removed=require_teardown,
                        false_before_removal_verified=require_teardown and expect_true,
                        session=self.path, manager_owner=self.owner)
        finally:
            self.stop.set()
            self.context.wakeup()
            self.thread.join(timeout=3)
            assert not self.thread.is_alive()


def replay(socket, state, before, directory, guard, authenticate, mode, observer):
    expected = mode == 'external'
    assert mode in ['battery', 'external']
    assert not devices('sink-inputs') and not devices('source-outputs')
    baseline = state('session-idle-enabled', lambda s:s['lock']['session_idle']['publisher']['available']
                     and s['audio']['available'] and s['audio_inhibition']['available']
                     and s['power']['battery']['available'])
    assert baseline['lock']['session_idle']['enabled']
    assert baseline['lock']['session_idle']['allow_external_power'] is expected
    assert baseline['power']['battery']['external_power'] is True, 'Connected external-power policy needs actual observation'
    original_idle = baseline['display_settings']['idle']
    assert baseline['display_settings']['sleep_available'] is True
    assert baseline['display_settings']['sleep'] == dict(automatic=False, after_off_seconds=60, on_external_power=False)
    original_audio = snapshot()
    kernel_before = stats()
    panels = [p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip()=='connected']
    assert len(panels) == 1
    def panel():
        return dict(dpms=(panels[0]/'dpms').read_text().strip(),
                    backlight_power=int(Path('/sys/class/backlight/backlight/bl_power').read_text()))
    result = dict(passed=False, mode=mode, expected_idle=expected, suspend_called=False,
                  physical_audio_used=False, actual_pam=False, kernel_before=kernel_before)
    module = peer = keyboard = None
    log = None
    sink = 'tarot_qa_idle_' + str(os.getpid())
    try:
        account = pwd.getpwuid(10000)
        proc = Path('/proc',str(before['pid']))
        env = dict(field.decode().split('=',1) for field in (proc/'environ').read_bytes().split(b'\0') if b'=' in field)
        env.update(HOME=account.pw_dir, USER=account.pw_name, LOGNAME=account.pw_name, XDG_RUNTIME_DIR='/run/user/10000')
        module = int(pactl('load-module','module-null-sink','sink_name='+sink))
        assert next(d for d in devices('sinks') if d['name']==sink)['driver']=='module-null-sink.c'
        log = (directory/'session-idle-stream.log').open('x')
        peer = subprocess.Popen(['python3',str(Path(__file__).with_name('pulse_stream_fixture.py')),
            '--server','unix:/run/user/10000/pulse/native','--device',sink], env=env,
            user=10000, group=account.pw_gid, extra_groups=os.getgrouplist(account.pw_name,account.pw_gid),
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True)
        def reply():
            assert select.select([peer.stdout],[],[],5)[0]
            line=peer.stdout.readline();assert line,peer.poll();return json.loads(line)
        def cork(value):
            peer.stdin.write(json.dumps(dict(command='cork',value=value))+'\n');peer.stdin.flush()
            assert reply()==dict(event='corked',value=value)
        assert reply()['corked']; cork(False)
        state('session-idle-audio-block-held',lambda s:s['audio_inhibition']['held'] and s['audio']['activity']['playback']==1)
        def grants():
            return [r for r in observer.call(observer.owner,ROOT,MANAGER,'ListInhibitors')[0]
                    if r[1]=='Tarot' and r[2]=='Audio playback or recording is active']
        grant=await_value(grants,bool);assert len(grant)==1 and grant[0][3:] == ('block',10000,before['pid'])
        def sleep_settings(label,**changes):
            current=state(label+'-before',lambda s:s['lock']['phase']=='Unlocked')['display_settings']['sleep']
            wanted=dict(current,**changes)
            reply=request(socket,'sleep_settings',settings=wanted)
            assert reply.get('ok',True),reply
            state(label,lambda s:s['display_settings']['sleep']==wanted and s['display_settings']['settings_saved'])
            result.setdefault('sleep_settings_applied',[]).append(dict(label=label,settings=wanted))
        sleep_settings('session-idle-setting-enabled',automatic=True,after_off_seconds=0)
        request(socket,'idle_timeouts',settings=dict(lock_after_seconds=15,off_after_seconds=20,screen_off_enabled=True))
        def off(label,hint=expected,readiness=None):
            readiness=readiness or ('Ready' if hint else 'ExternalPower')
            value = state(label,lambda s:s['lock']['phase']!='Unlocked' and s['lock']['idle']['output_off']
                and s['lock']['session_idle']['readiness']==readiness
                and s['lock']['session_idle']['publisher']['observed_idle'] is hint,seconds=45)
            assert observer.read()['idle'] is hint
            result[label]=dict(status=value,independent=observer.read(),driver=await_value(panel,lambda p:p==dict(dpms='Off',backlight_power=4)))
            return value
        off('session-idle-automatic-off-with-audio'); assert grants()==grant
        result['audio_block_and_idle_coexist']=True
        cork(True);await_value(grants,lambda rs:not rs)
        assert observer.read()['idle'] is expected
        result['last_audio_release_with_panel_off']=True
        restored=authenticate('session-idle-automatic-wake');result['actual_pam']=True
        assert restored['cards']==before['cards'] and restored['view']==before['view']
        state('session-idle-wake-cleared',lambda s:s['lock']['session_idle']['publisher']['observed_idle'] is False
              and s['lock']['session_idle']['readiness']=='Active')
        assert observer.read()['idle'] is False
        request(socket,'idle_timeouts',settings=original_idle)
        state('session-idle-timeouts-restored',lambda s:s['display_settings']['idle']==original_idle and s['display_settings']['settings_saved'])
        result['idle_preferences_restored']=True
        keyboard=Keyboard(guard);keyboard.tap(116)
        manual=off('session-idle-manual-power-off')
        if expected:
            locker=manual['lock']['locker']['pid'];target=Path('/proc',str(locker))
            descriptor=os.pidfd_open(locker)
            try:
                assert target.stat().st_uid==10000 and (target/'cgroup').read_text()==(proc/'cgroup').read_text()
                assert os.readlink(target/'exe')=='/usr/bin/tarot-lock'
                assert hashlib.sha256((target/'exe').read_bytes()).digest()==hashlib.sha256(Path('/usr/bin/tarot-lock').read_bytes()).digest()
                assert request(socket,'status')['lock']['locker']['pid']==locker
                signal.pidfd_send_signal(descriptor,0)
                signal.pidfd_send_signal(descriptor,signal.SIGKILL)
            finally:os.close(descriptor)
            changed=state('session-idle-new-owner-while-off',lambda s:s['lock']['locker']['pid'] not in [None,locker]
                and s['lock']['generation']!=manual['lock']['generation'] and s['lock']['idle']['output_off']
                and s['lock']['session_idle']['publisher']['observed_idle'] is True
                and s['lock']['session_idle']['readiness']=='Ready')
            assert changed['lock']['idle']['off_count']==manual['lock']['idle']['off_count']
            assert changed['lock']['idle']['off_presentation']==manual['lock']['idle']['off_presentation']
            assert changed['lock']['unprotected_presentations']==manual['lock']['unprotected_presentations']
            assert observer.read()['idle'] is True
            result['historical_proof_survives_owner_recovery']=True
            recovered=authenticate('session-idle-owner-recovery-pam')
            assert recovered['cards']==before['cards'] and recovered['view']==before['view']
            state('session-idle-recovery-wake-cleared',lambda s:s['lock']['session_idle']['publisher']['observed_idle'] is False)
            # A persistent delay starts at the proven protected off, then a timer
            # publishes without any further input or service event.
            sleep_settings('session-idle-setting-delay',after_off_seconds=10)
            start=len(observer.recorded())
            waiting_started=time.monotonic()
            keyboard.tap(116)
            waiting=off('session-idle-delay-pending',hint=False,readiness='Delay')
            proven=waiting['lock']['session_idle']['proven_off']
            assert proven is not None and proven[0]==waiting['lock']['idle']['off_count']
            # Signal timestamps are dispatch times, an upper bound only. Direct
            # property reads bound the transition from both sides.
            reads=[]
            while not reads or not reads[-1][1]:
                assert time.monotonic()-waiting_started<40,reads[-3:]
                if reads:time.sleep(.2)
                reads.append((time.monotonic(),observer.read()['idle']))
            last_false=max(t for t,v in reads if not v) if any(not v for _,v in reads) else None
            assert last_false is not None and last_false-waiting_started>=9.5,(waiting_started,reads[:3],reads[-3:])
            assert reads[-1][0]-waiting_started<=25
            events=await_value(observer.recorded,lambda es:any(e['event']=='hint' and e['value'] is True for e in es[start:]),seconds=40)
            published=next(e for e in events[start:] if e['event']=='hint' and e['value'] is True)
            ready=state('session-idle-delay-elapsed',lambda s:s['lock']['session_idle']['readiness']=='Ready'
                and s['lock']['session_idle']['publisher']['observed_idle'] is True)
            assert ready['lock']['session_idle']['proven_off']==proven and observer.read()['idle'] is True
            # Session-relative proof time to the independent signal's monotonic time.
            result['delay']=dict(configured_seconds=10,pending_status_seen=True,hint_signal_monotonic=published['monotonic'],
                                 last_false_read_after_request=last_false-waiting_started,
                                 first_true_read_after_request=reads[-1][0]-waiting_started,property_reads=len(reads),
                                 proven_off_session_seconds=proven[1]['secs']+proven[1]['nanos']/1e9,
                                 off_requested_monotonic=waiting_started)
            assert published['monotonic']-waiting_started <= 25, result['delay']
        else:
            restored=authenticate('session-idle-battery-policy-wake')
            assert restored['cards']==before['cards'] and restored['view']==before['view']
            sleep_settings('session-idle-setting-disabled',automatic=False)
            keyboard.tap(116);off('session-idle-disabled-power-off',hint=False,readiness='Disabled')
            authenticate('session-idle-disabled-wake')
            sleep_settings('session-idle-setting-external-power',automatic=True,on_external_power=True)
            keyboard.tap(116);off('session-idle-persistent-external-power',hint=True)
            result['persistent_external_power_allows_idle']=True
        result['left_protected_for_shutdown']=True
        result['kernel_after']=stats();assert result['kernel_after']==kernel_before
        result['passed']=True
        return result
    finally:
        if sys.exc_info()[0] is not None:result['passed']=False
        try:
            if keyboard is not None:keyboard.close()
            if peer is not None and peer.poll() is None:
                try:
                    peer.stdin.write('{"command":"exit"}\n');peer.stdin.flush();assert peer.wait(timeout=5)==0
                finally:
                    if peer.poll() is None:peer.terminate();peer.wait(timeout=5)
            if log is not None:log.close()
            if module is not None:
                rows=[line.split('\t') for line in pactl('list','short','modules').splitlines() if line.split('\t')[0]==str(module)]
                assert len(rows)==1 and rows[0][1]=='module-null-sink' and rows[0][2].split()==['sink_name='+sink]
                pactl('unload-module',module)
            if snapshot()!=original_audio:restore(original_audio)
            assert snapshot()==original_audio and not devices('sink-inputs') and not devices('source-outputs')
            result['exact_audio_restored']=True
        except BaseException:
            result['passed']=False
            raise
        finally:
            (directory/'session-idle-result.json').write_text(json.dumps(result,indent=2)+'\n')
