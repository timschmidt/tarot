"""Root-only observation of one verified compositor's SensorProxy calls.

Only SensorProxy claim/release headers and that service owner's reply headers
are retained. Message bodies and unrelated system-bus traffic are discarded.
Successful replies are correlated to calls whose sender PID matches the target.
This observes the real service without exposing sensor diagnostics while locked.
"""
import ast
import os
from pathlib import Path
import re
import subprocess
import threading
import time


class SensorClaims:
    def __init__(self, pid):
        assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
        self.pid = pid
        self.identity = self._identity()
        self.owner = ast.literal_eval(self._bus('GetNameOwner', 'net.hadess.SensorProxy'))[0]
        assert re.fullmatch(r':\d+\.\d+', self.owner)
        assert self._number(self._bus('GetConnectionUnixUser', self.owner)) == 0
        rules = ["type='method_call',path='/net/hadess/SensorProxy',interface='net.hadess.SensorProxy',member='"+member+"'"
                 for member in ['ClaimAccelerometer', 'ReleaseAccelerometer']]
        rules += ["type='"+kind+"',sender='"+self.owner+"'" for kind in ['method_return', 'error']]
        self.events = []
        self.errors = []
        self.pending = {}
        self.mutex = threading.Lock()
        self.ready = threading.Event()
        self.process = subprocess.Popen(['dbus-monitor', '--system', *rules],
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        text=True, bufsize=1)
        self.thread = threading.Thread(target=self._read, daemon=True)
        self.thread.start()
        if not self.ready.wait(5):
            self.close()
            raise AssertionError('Sensor monitor did not confirm monitoring readiness')

    @staticmethod
    def _bus(method, argument):
        return subprocess.check_output(['gdbus', 'call', '--system', '--dest', 'org.freedesktop.DBus',
                                        '--object-path', '/org/freedesktop/DBus',
                                        '--method', 'org.freedesktop.DBus.'+method, argument],
                                       text=True, timeout=3).strip()

    @staticmethod
    def _number(value):
        match = re.fullmatch(r'\(uint32 (\d+),\)', value)
        assert match, 'Unexpected D-Bus identity reply'
        return int(match[1])

    def _identity(self):
        proc = Path('/proc', str(self.pid))
        assert proc.stat().st_uid == 10000 and (proc/'exe').resolve() == Path('/usr/bin/tarot')
        return (proc/'stat').read_text().rsplit(')', 1)[1].split()[19]

    def _read(self):
        try:
            for line in self.process.stdout:
                if line.startswith('signal ') and 'member=NameLost' in line:
                    self.ready.set()
                if not line.startswith(('method call ', 'method return ', 'error ')):
                    continue
                sender = re.search(r'\bsender=([^ ]+)', line)
                destination = re.search(r'\bdestination=([^ ]+)', line)
                serial = re.search(r'(?<!_)\bserial=(\d+)', line)
                if not sender or not serial:
                    continue
                sender = sender[1]
                if line.startswith('method call '):
                    member = re.search(r'\bmember=(ClaimAccelerometer|ReleaseAccelerometer)\b', line)
                    if not member:
                        continue
                    try:
                        target = self._number(self._bus('GetConnectionUnixProcessID', sender))
                    except subprocess.SubprocessError:
                        continue  # An unrelated short-lived client can disappear.
                    if target != self.pid:
                        continue
                    assert self._identity() == self.identity
                    event = {'member': member[1], 'sender': sender, 'serial': int(serial[1]),
                             'pid': target, 'requested_monotonic': time.monotonic(),
                             'reply': None}
                    with self.mutex:
                        self.events.append(event)
                        self.pending[(sender, int(serial[1]))] = event
                elif sender == self.owner and destination:
                    reply = re.search(r'\breply_serial=(\d+)', line)
                    if not reply:
                        continue
                    with self.mutex:
                        event = self.pending.pop((destination[1], int(reply[1])), None)
                        if event is not None:
                            event['reply'] = 'error' if line.startswith('error ') else 'success'
                            event['reply_monotonic'] = time.monotonic()
        except BaseException as error:
            with self.mutex:
                self.errors.append(type(error).__name__)

    def snapshot(self):
        assert self.process.poll() is None and not self.errors, self.errors
        with self.mutex:
            return [dict(e) for e in self.events]

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=3)
        self.thread.join(timeout=4)
        assert not self.thread.is_alive()
        self.process.stdout.close()
