#!/usr/bin/env python3
"""Real private audio streams and login1 block-FD lifetime; no system bus/suspend."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time

from lock_replay import append
from notification_replay import Fixture, await_value
from phone_gesture_replay import request


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--binary', type=Path, default=Path('target/debug/tarot'))
    p.add_argument('--pulse-daemon', required=True, type=Path)
    p.add_argument('--pulse-modules', required=True, type=Path)
    p.add_argument('--pulse-libraries', required=True, type=Path)
    a = p.parse_args(); out = a.output.resolve(); out.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.binary.resolve(); examples = binary.parent/'examples'
    result = dict(passed=False, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), actions=[])
    logs = []; streams = []; bus = pulse = gui = authority = None
    def log(name):
        f = (out/name).open('x'); logs.append(f); return f
    class Stream:
        def __init__(self, recording=False):
            self.kind = 'recording' if recording else 'playback'
            cmd = [sys.executable, str(Path('tests/pulse_stream_fixture.py').resolve()), '--server', server,
                   '--device', 'tarot_qa_output.monitor' if recording else 'tarot_qa_output']
            if recording: cmd.append('--record')
            self.proc = subprocess.Popen(cmd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                         stderr=log('stream-'+str(len(streams))+'.log'), text=True)
            streams.append(self); assert self.read()['corked']
        def read(self):
            assert select.select([self.proc.stdout], [], [], 5)[0], 'Stream fixture timeout'
            line = self.proc.stdout.readline(); assert line, self.proc.poll(); return json.loads(line)
        def cork(self, value):
            self.proc.stdin.write(json.dumps(dict(command='cork', value=value))+'\n'); self.proc.stdin.flush()
            assert self.read() == dict(event='corked', value=value)
        def close(self):
            if self.proc.poll() is None:
                self.proc.stdin.write('{"command":"exit"}\n'); self.proc.stdin.flush()
                assert self.proc.wait(timeout=4) == 0
    with tempfile.TemporaryDirectory(prefix='tarot-audio-awake-') as tmp:
        root = Path(tmp); address = 'unix:path='+str(root/'bus'); server = 'unix:'+str(root/'pulse')
        control = root/'control'; commands = root/'locker.commands'; report = out/'locker.jsonl'
        env = dict(os.environ, DBUS_SESSION_BUS_ADDRESS=address, LIBGL_ALWAYS_SOFTWARE='1',
                   __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_SOCKET', None)
        for name in ['runtime', 'pulse-state', 'config']: (root/name).mkdir(mode=0o700)
        env['XDG_CONFIG_HOME'] = str(root/'config')
        pulse_env = dict(env, PULSE_RUNTIME_PATH=str(root/'runtime'), PULSE_STATE_PATH=str(root/'pulse-state'),
                         LD_LIBRARY_PATH=str(a.pulse_modules.resolve())+':'+str(a.pulse_libraries.resolve()))
        config = root/'default.pa'; config.write_text('load-module module-native-protocol-unix socket='+str(root/'pulse')+' auth-anonymous=yes\nload-module module-null-sink sink_name=tarot_qa_output\n')
        def start_bus(label):
            proc = subprocess.Popen(['dbus-daemon', '--session', '--nofork', '--nopidfile', '--address='+address],
                                    stdout=log(label+'.log'), stderr=subprocess.STDOUT)
            await_value(lambda: (root/'bus').exists() or proc.poll() is not None, bool); assert proc.poll() is None
            return proc
        def start_pulse(label):
            proc = subprocess.Popen([str(a.pulse_daemon.resolve()), '-n', '--daemonize=no', '--use-pid-file=no',
                '--exit-idle-time=-1', '--disable-shm=yes', '--log-target=stderr', '--file='+str(config),
                '--dl-search-path='+str(a.pulse_modules.resolve())], env=pulse_env,
                stdout=log(label+'.log'), stderr=subprocess.STDOUT)
            await_value(lambda: subprocess.run(['pactl', '--server', server, 'info'], env=env,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0 or proc.poll() is not None, bool)
            assert proc.poll() is None; return proc
        def status(label, condition=lambda s: True, seconds=12):
            s = await_value(lambda: request(control, 'status'), condition, seconds=seconds)
            result['actions'].append(dict(label=label, status=s)); return s
        def inhibition(label, held, playback=None, recording=None):
            return status(label, lambda s: s.get('audio_inhibition', {}).get('held') == held and
                (playback is None or s['audio']['activity']['playback'] == playback) and
                (recording is None or s['audio']['activity']['recording'] == recording))
        def blocks(): return [e['id'] for e in authority.events() if e['event']=='inhibit' and e['mode']=='block']
        def closed(identifier, fixture=None):
            return any(e['event']=='closed' and e['id']==identifier and e['eof'] for e in (fixture or authority).events())
        def unlock(label):
            status(label+'-owner', lambda s: s['lock']['locker']['pid'] is not None)
            append(commands, 'lock')
            status(label+'-locked', lambda s: s['lock']['phase']=='Locked')
            append(commands, 'unlock')
            return status(label, lambda s: s['lock']['phase']=='Unlocked')
        def start_gui(label, short_idle=False):
            commands.write_text('')
            cmd = [str(binary), '--system-services', '--platform-bus', address, '--audio-server', server,
                   '--sleep-coordination', '--lock-client', str(examples/'lock-probe'), '--control-socket', str(control),
                   '--exit-after-ms', '180000', '--idle-lock-ms', '1500' if short_idle else '86400000']
            for v in ['--role','locker','--commands',str(commands),'--report',str(report)]: cmd.append('--lock-client-arg='+v)
            proc = subprocess.Popen(cmd, env=env, stdout=log(label+'.log'), stderr=subprocess.STDOUT)
            await_value(lambda: control.exists() or proc.poll() is not None, bool); assert proc.poll() is None
            try:
                status('protected-'+label, lambda s: s['lock']['sleep']['armed'] and not s['lock']['sleep']['gate']['barrier'])
                unlock('unlocked-'+label)
                return proc
            except BaseException:
                if proc.poll() is None: proc.terminate(); proc.wait(timeout=5)
                raise
        try:
            bus = start_bus('bus1'); pulse = start_pulse('pulse1')
            authority = Fixture(examples/'sleep-probe', out/'manager1', env)
            gui = start_gui('gui1')
            status('inactive', lambda s: s['audio']['available'] and s['audio_inhibition']['available'] and not s['audio_inhibition']['held'])
            assert not blocks()
            playback = Stream(); inhibition('corked-playback', False, 0, 0); assert not blocks()
            playback.cork(False); inhibition('playback-held', True, 1, 0); first = blocks()[-1]; assert not closed(first)
            recording = Stream(True); recording.cork(False); inhibition('both-held', True, 1, 1)
            assert blocks() == [first], 'One block FD must cover both streams'
            playback.cork(True); inhibition('recording-retains', True, 0, 1); assert not closed(first)
            recording.cork(True); inhibition('all-corked', False, 0, 0); await_value(lambda: closed(first), bool)
            # A valid but late grant must be closed after the observed stop.
            authority.call('block-delay', milliseconds=250); count = len(blocks())
            playback.cork(False); await_value(lambda: authority.events(), lambda es: sum(e['event']=='block-request' for e in es)>count)
            playback.cork(True); inhibition('late-grant-retired', False, 0, 0)
            await_value(lambda: len(blocks())>count and closed(blocks()[-1]), bool)
            authority.call('block-delay', milliseconds=0)
            authority.call('deny-block', deny=True); playback.cork(False)
            status('denied-visible', lambda s: bool(s['audio_inhibition']['error']) and not s['audio_inhibition']['held'])
            assert request(control,'status')['lock']['sleep']['armed'], 'Block denial cannot retire the protection delay FD'
            authority.call('deny-block', deny=False); inhibition('denial-recovered', True, 1, 0)
            current = blocks()[-1]; authority.call('release')
            await_value(lambda: closed(current), bool)
            authority.call('acquire')
            # Existing sleep coordination protects on manager replacement.
            status('manager-returned-protected', lambda s: s['lock']['sleep']['armed'] and not s['lock']['sleep']['gate']['barrier'])
            unlock('manager-unlocked'); inhibition('manager-returned', True, 1, 0)
            current = blocks()[-1]; old_authority = authority
            bus.terminate(); assert bus.wait(timeout=4)==0
            await_value(lambda: closed(current, old_authority), bool)
            old_authority.close(); authority = None
            bus = start_bus('bus2'); authority = Fixture(examples/'sleep-probe', out/'manager2', env)
            status('broker-returned-protected', lambda s: s['lock']['sleep']['armed'] and not s['lock']['sleep']['gate']['barrier'])
            unlock('broker-unlocked'); inhibition('broker-returned', True, 1, 0)
            current = blocks()[-1]
            playback.close(); recording.close(); inhibition('clients-exit', False, 0, 0); await_value(lambda: closed(current), bool)
            # Timeout closes the whole uncertain connection, including a grant
            # that reaches the client after its method wait has expired.
            authority.call('block-delay', milliseconds=1200); playback = Stream(); playback.cork(False)
            status('timeout-visible', lambda s: bool(s['audio_inhibition']['error']) and not s['audio_inhibition']['held'])
            playback.cork(True); count = len(blocks())
            await_value(lambda: len(blocks())>count and closed(blocks()[-1]), bool)
            authority.call('block-delay', milliseconds=0)
            inhibition('timeout-settled', False, 0, 0)
            playback.cork(False); inhibition('after-timeout-held', True, 1, 0)
            current = blocks()[-1]; pulse.terminate(); assert pulse.wait(timeout=4)==0
            inhibition('audio-lost', False); await_value(lambda: closed(current), bool); playback.close()
            pulse = start_pulse('pulse2'); inhibition('audio-returned-idle', False, 0, 0)
            playback = Stream(); playback.cork(False); inhibition('fresh-stream-held', True, 1, 0)
            current = blocks()[-1]; request(control,'exit'); assert gui.wait(timeout=4)==0
            await_value(lambda: closed(current), bool)
            # Background audio does not inhibit automatic locking. Physical
            # panel-off is DRM-only and must be verified on the phone.
            gui = start_gui('gui-idle', True); await_value(lambda: len(blocks())>0 and not closed(blocks()[-1]), bool)
            current = blocks()[-1]
            status('idle-requests-protection', lambda s: s['lock']['phase']!='Unlocked' and s['lock']['locker']['pid'] is not None)
            append(commands, 'lock')
            protected = status('audio-does-not-prevent-auto-lock', lambda s: s['lock']['phase']=='Locked' and not s['lock']['idle']['policy']['inhibited'])
            assert set(protected)=={'ok','pid','lock'} and not closed(current)
            playback.cork(True); await_value(lambda: closed(current), bool)
            # Exit while a grant is pending; even its late descriptor must close.
            authority.call('block-delay', milliseconds=1200)
            before_requests = sum(e['event']=='block-request' for e in authority.events())
            playback.cork(False)
            await_value(lambda: sum(e['event']=='block-request' for e in authority.events())>before_requests, bool)
            start = time.monotonic(); request(control,'exit'); assert gui.wait(timeout=4)==0
            result['shutdown_seconds'] = time.monotonic()-start
            assert result['shutdown_seconds'] < 2
            await_value(lambda: blocks()[-1]!=current and closed(blocks()[-1]), bool)
            playback.close(); result['passed']=True; result['suspend_called']=False; result['physical_audio_used']=False
            print('PASS: real private streams/corking, one sleep-block FD, late/denied/timeout grants, manager/broker/audio recovery, automatic lock, redaction and pending logout cleanup')
        finally:
            for stream in streams:
                try: stream.close()
                except (BrokenPipeError, AssertionError):
                    if stream.proc.poll() is None: stream.proc.terminate(); stream.proc.wait(timeout=4)
            if gui is not None and gui.poll() is None: gui.terminate(); gui.wait(timeout=5)
            if authority is not None: authority.close()
            for child in [pulse,bus]:
                if child is not None and child.poll() is None: child.terminate(); child.wait(timeout=5)
            for f in logs: f.close()
            (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')


if __name__=='__main__': main()
