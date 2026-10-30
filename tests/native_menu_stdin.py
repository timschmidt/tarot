#!/usr/bin/env python3
"""Catalog launch wrapper supplying an owned QA FIFO as the fixture's stdin."""
import argparse
import os
import stat

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--fifo', required=True)
p.add_argument('command', nargs=argparse.REMAINDER)
a = p.parse_args()
assert os.getuid() != 0 and a.command[0] == '--' and len(a.command) > 1
fd = os.open(a.fifo, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
metadata = os.fstat(fd)
assert stat.S_ISFIFO(metadata.st_mode) and metadata.st_uid == os.getuid() and stat.S_IMODE(metadata.st_mode) == 0o600
os.dup2(fd, 0, inheritable=True)
os.close(fd)
os.execv(a.command[1], a.command[1:])
