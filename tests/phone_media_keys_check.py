#!/usr/bin/env python3
"""Owner-assisted headset media-key check in the running normal Tarot session.

Plays a generated, audible WAV in Decibels on Tarot's own session bus and
display, then for the given seconds logs every key from the given input device
(read-only, with the kernel's event time) next to the player's MPRIS
PlaybackStatus, as JSON lines. Decibels is closed afterwards. Run as the
session user while the screen is on: Decibels exports MPRIS only once its
window is active.

Usage: phone_media_keys_check.py SECONDS /dev/input/eventN
"""
import json
import math
import os
import select
import signal
import struct
import subprocess
import sys
import time
import wave
from pathlib import Path

seconds = int(sys.argv[1])
event = sys.argv[2]
EVENT = struct.Struct('llHHi')
work = Path('/home/user/tarot-dev/media-keys')
work.mkdir(exist_ok=True)


def say(**kw):
    print(json.dumps(dict(t=round(time.monotonic() - start, 2), **kw)), flush=True)


tarot = [p for p in Path('/proc').iterdir() if p.name.isdigit() and p.stat().st_uid == os.getuid()
         and (p / 'cmdline').read_bytes().split(b'\0')[0] == b'/usr/bin/tarot']
assert len(tarot) == 1, 'exactly one normal Tarot compositor'
environ = dict(v.split('=', 1) for v in (tarot[0] / 'environ').read_bytes().decode().split('\0') if '=' in v)
child_env = {k: environ[k] for k in ['DBUS_SESSION_BUS_ADDRESS', 'XDG_RUNTIME_DIR'] if k in environ}
child_env.update(PATH='/usr/bin:/bin', HOME='/home/user', USER='user', LOGNAME='user',
                 XDG_SESSION_TYPE='wayland', XDG_CURRENT_DESKTOP='Tarot')
wayland = sorted(Path(environ['XDG_RUNTIME_DIR']).glob('wayland-*[0-9]'))
assert wayland, 'no Wayland socket'
child_env['WAYLAND_DISPLAY'] = wayland[0].name

# A soft repeating three-note figure, two minutes, at moderate level.
track = work / 'Headset Check.wav'
if not track.exists():
    rate = 44100
    with wave.open(str(track), 'wb') as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(rate)
        note = int(rate * .4)
        frames = bytearray()
        for f in [523.25, 659.25, 783.99, 659.25]:
            for i in range(note):
                v = int(5000 * math.sin(2 * math.pi * f * i / rate) * min(1, i / 400, (note - i) / 4000))
                frames += struct.pack('<hh', v, v)
        w.writeframes(bytes(frames) * 75)


def status():
    out = subprocess.run(['busctl', '--user', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus',
                          'org.freedesktop.DBus', 'ListNames'], env=child_env, capture_output=True, text=True).stdout
    names = [n.strip('"') for n in out.split() if n.startswith('"org.mpris.MediaPlayer2.')]
    result = {}
    for name in names:
        r = subprocess.run(['busctl', '--user', 'get-property', name, '/org/mpris/MediaPlayer2',
                            'org.mpris.MediaPlayer2.Player', 'PlaybackStatus'], env=child_env,
                           capture_output=True, text=True)
        result[name] = r.stdout.split()[-1].strip('"') if r.returncode == 0 else 'error'
    return result


signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
start = time.monotonic()
player = subprocess.Popen(['/usr/bin/org.gnome.Decibels', str(track)], env=child_env, cwd='/home/user',
                          stdin=subprocess.DEVNULL,
                          stdout=subprocess.DEVNULL, stderr=open(work / 'decibels.log', 'w'))
try:
    last = None
    # Unbuffered, so select never misses events already read into a buffer.
    keys = os.open(event, os.O_RDONLY | os.O_NONBLOCK)
    end = start + seconds
    while time.monotonic() < end:
        now = status()
        if now != last:
            say(players=now)
            last = now
        if select.select([keys], [], [], .3)[0]:
            data = os.read(keys, EVENT.size * 64)
            for offset in range(0, len(data) - EVENT.size + 1, EVENT.size):
                sec, usec, kind, code, value = EVENT.unpack_from(data, offset)
                if kind == 1:
                    say(key=code, state={0: 'up', 1: 'down', 2: 'repeat'}[value],
                        kernel_time=round(sec + usec / 1e6, 3))
finally:
    player.terminate()
    player.wait(10)
    say(decibels_exit=player.returncode, players_after=status())
