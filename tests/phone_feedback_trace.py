"""Owned tracefs instance for feedback-device syscall metadata, never buffers.

Filters cover one verified process's existing threads and ioctl/write IDs.
Raw syscall events never dereference user buffers; results retain only the
verified device descriptors.
Only descriptor numbers, ioctl commands, byte counts and return codes are read.
The global tracer configuration is observed and must remain unchanged.
"""
import hashlib
import os
from pathlib import Path
import re


EVENTS = ['sys_enter', 'sys_exit']
# AArch64 ABI; raw events contain numeric arguments and never copy buffers.
SYSCALLS = {29: 'ioctl', 64: 'write'}


def global_state():
    root = Path('/sys/kernel/tracing')
    files = ['tracing_on', 'current_tracer', 'buffer_size_kb']
    files += ['events/raw_syscalls/' + event + '/' + field
              for event in EVENTS for field in ['enable', 'filter']]
    return {name: (root / name).read_text() for name in files}


def parse(text):
    pending, calls = {}, []
    for line in text.splitlines():
        if not line.strip() or line.startswith('#'):
            continue
        event = re.search(r'\b(sys_enter|sys_exit):\s*NR (29|64) (.*)', line)
        assert event is not None, 'Unexpected raw syscall record: ' + line
        thread = re.search(r'-(\d+)\s+(?:\(\s*\d+\)\s+)?\[', line[:event.start()])
        assert thread is not None, line
        tid = int(thread[1])
        name, number, body = event.groups()
        kind = SYSCALLS[int(number)]
        if name == 'sys_enter':
            values = re.fullmatch(r'\(([0-9a-fA-F]+(?:, [0-9a-fA-F]+){5})\)', body)
            assert values is not None, line
            args = [int(value, 16) for value in values[1].split(', ')]
            call = dict(thread=tid, kind=kind, fd=args[0])
            call['cmd' if kind == 'ioctl' else 'count'] = args[1] if kind == 'ioctl' else args[2]
            pending[(tid, kind)] = call
        else:
            call = pending.pop((tid, kind), None)
            if call is not None:
                value = re.fullmatch(r'= (-?\d+)', body)
                assert value is not None, line
                calls.append(dict(call, result=int(value[1])))
    return calls


class DeviceTrace:
    def __init__(self, identity, descriptors, directory):
        assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
        self.identity = identity
        self.descriptors = descriptors
        self.directory = directory
        self.before = global_state()
        self.path = Path('/sys/kernel/tracing/instances', 'tarot-feedback-' + str(os.getpid()))
        self.path.mkdir()
        self.closed = False
        try:
            self._write('tracing_on', '0')
            self._write('current_tracer', 'nop')
            self._write('buffer_size_kb', '64')
            self._write('events/enable', '0')
            clocks = (self.path / 'trace_clock').read_text()
            if 'mono' in clocks.split() or '[mono]' in clocks:
                self._write('trace_clock', 'mono')
            self.guard()
            tasks = sorted(int(p.name) for p in Path('/proc', str(identity['pid']), 'task').iterdir())
            assert 0 < len(tasks) <= 32 and 0 < len(descriptors) <= 8
            task_filter = '(' + ' || '.join('common_pid == ' + str(tid) for tid in tasks) + ')'
            for event in EVENTS:
                expression = task_filter + ' && (id == 29 || id == 64)'
                self._write('events/raw_syscalls/' + event + '/filter', expression)
                assert all(str(tid) in (self.path / 'events/raw_syscalls' / event / 'filter').read_text() for tid in tasks)
                self._write('events/raw_syscalls/' + event + '/enable', '1')
            self._write('trace', '')
            self._write('tracing_on', '1')
        except BaseException:
            self.close()
            raise

    def _write(self, name, value):
        (self.path / name).write_text(value + '\n')

    def guard(self):
        identity = self.identity
        proc = Path('/proc', str(identity['pid']))
        assert proc.stat().st_uid == identity['uid']
        assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == identity['start']
        assert (proc / 'cgroup').read_text() == identity['cgroup']
        assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == identity['sha256']
        for fd, expected in self.descriptors.items():
            assert os.readlink(proc / 'fd' / str(fd)) == expected

    def snapshot(self, label):
        self.guard()
        with (self.path / 'trace').open() as stream:
            text = stream.read(1024 * 1024 + 1)
        assert len(text) <= 1024 * 1024
        (self.directory / (label + '.trace')).write_text(text)
        return [call for call in parse(text) if call['fd'] in self.descriptors]

    def close(self):
        if self.closed:
            return
        self.closed = True
        try:
            self._write('tracing_on', '0')
            for event in EVENTS:
                self._write('events/raw_syscalls/' + event + '/enable', '0')
        finally:
            self.path.rmdir()
        assert global_state() == self.before, 'Global tracing configuration changed'
