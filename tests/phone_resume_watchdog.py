"""Bounded QA failsafe: resume one already verified pidfd after a timeout.

The parent owns process-identity/cgroup checks. The descriptor binds this helper
to that same process even if numeric PIDs wrap. Ready is acknowledged before the
parent may pause it. No process lookup or arbitrary PID signal occurs here.
"""
import os
from pathlib import Path
import select
import signal
import sys


def main():
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    pidfd, ready, seconds = int(sys.argv[1]), int(sys.argv[2]), float(sys.argv[3])
    assert 0 < seconds <= 5
    assert os.readlink('/proc/self/fd/' + str(pidfd)) == 'anon_inode:[pidfd]'
    watcher = select.poll()
    watcher.register(pidfd, select.POLLIN)
    os.write(ready, b'R')
    os.close(ready)
    if not watcher.poll(round(seconds * 1000)):
        try:
            signal.pidfd_send_signal(pidfd, signal.SIGCONT)
        except ProcessLookupError:
            pass


if __name__ == '__main__':
    main()
