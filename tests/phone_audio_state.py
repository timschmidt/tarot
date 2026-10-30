"""Ordinary-user audio control snapshots/restoration for the spare-phone QA.

No stream or raw daemon database is opened. Restore mode and routes before
channel values because mode/route changes may replace devices or reset levels.
"""
import json
import os
import pwd
import subprocess
import time


def pactl(*args):
    assert os.geteuid() == 0
    account = pwd.getpwuid(10000)
    return subprocess.check_output(['pactl', *map(str, args)],
        env=dict(HOME=account.pw_dir, XDG_RUNTIME_DIR='/run/user/10000', PATH='/usr/bin:/bin'),
        user=10000, group=account.pw_gid,
        extra_groups=os.getgrouplist(account.pw_name, account.pw_gid), text=True, timeout=8).strip()


def devices(kind):
    return json.loads(pactl('-f', 'json', 'list', kind))


def snapshot():
    sinks, sources = devices('sinks'), devices('sources')
    def controls(items):
        return {d['name']: dict(values=[v['value'] for v in d['volume'].values()],
                               muted=d['mute'], port=d['active_port']) for d in items}
    return dict(cards={c['name']: c['active_profile'] for c in devices('cards')},
                default_output=pactl('get-default-sink') if sinks else None,
                default_input=pactl('get-default-source') if sources else None,
                sinks=controls(sinks), sources=controls(sources))


def wait(predicate, seconds=15):
    deadline = time.monotonic() + seconds
    value = None
    while True:
        try:
            value = snapshot()
            if predicate(value):
                return value
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
            pass
        assert time.monotonic() < deadline, 'Audio snapshot did not reach its target'
        time.sleep(.1)


def restore(original):
    wait(lambda v: set(v['cards']) == set(original['cards']))
    for card, profile in original['cards'].items():
        pactl('set-card-profile', card, profile)
    wait(lambda v: v['cards'] == original['cards'] and
         set(v['sinks']) == set(original['sinks']) and set(v['sources']) == set(original['sources']))
    for kind, key in [('sink', 'sinks'), ('source', 'sources')]:
        for name, controls in original[key].items():
            if controls['port'] is not None:
                pactl('set-' + kind + '-port', name, controls['port'])
            pactl('set-' + kind + '-volume', name, *controls['values'])
            pactl('set-' + kind + '-mute', name, int(controls['muted']))
    for kind, key in [('sink', 'default_output'), ('source', 'default_input')]:
        if original[key] is not None:
            pactl('set-default-' + kind, original[key])
    return wait(lambda v: v == original)


def peer_identity(required_cgroup=None):
    import hashlib
    from pathlib import Path
    import socket
    import struct
    assert os.geteuid() == 0
    with socket.socket(socket.AF_UNIX) as peer:
        peer.connect('/run/user/10000/pulse/native')
        pid, uid, _ = struct.unpack('3i', peer.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
    proc = Path('/proc', str(pid))
    assert uid == 10000 and proc.stat().st_uid == uid
    assert os.readlink(proc / 'exe') == '/usr/bin/pulseaudio'
    group = (proc / 'cgroup').read_text()
    if required_cgroup is not None:
        assert group == required_cgroup
    return dict(pid=pid, uid=uid, start=(proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
                sha256=hashlib.sha256((proc / 'exe').read_bytes()).hexdigest(), cgroup=group)


def stop_verified_server(identity):
    """Gracefully stop only the verified idle user server; never an unbound PID."""
    import hashlib
    from pathlib import Path
    import select
    import signal
    proc = Path('/proc', str(identity['pid']))
    if not proc.exists():
        return False
    if (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] != identity['start']:
        return False
    fd = os.pidfd_open(identity['pid'])
    try:
        assert peer_identity() == identity
        assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == identity['sha256']
        assert not devices('sink-inputs') and not devices('source-outputs'), 'Do not interrupt real streams'
        assert peer_identity() == identity
        signal.pidfd_send_signal(fd, signal.SIGTERM)
        watcher = select.poll()
        watcher.register(fd, select.POLLIN)
        assert watcher.poll(5000), 'Audio daemon did not exit gracefully'
        return True
    finally:
        os.close(fd)


def start_standard_server(gui_pid):
    """Use the distribution's ordinary startup command as the graphical user."""
    from pathlib import Path
    account = pwd.getpwuid(10000)
    environment = dict(field.decode().split('=', 1) for field in
                       Path('/proc', str(gui_pid), 'environ').read_bytes().split(b'\0') if b'=' in field)
    environment.update(HOME=account.pw_dir, USER=account.pw_name, LOGNAME=account.pw_name,
                       XDG_RUNTIME_DIR='/run/user/10000')
    environment.pop('DISPLAY', None)
    environment.pop('PULSE_SERVER', None)
    subprocess.run(['/usr/bin/pulseaudio', '--start', '--log-target=stderr'], env=environment,
                   user=10000, group=account.pw_gid, extra_groups=os.getgrouplist(account.pw_name, account.pw_gid),
                   stdin=subprocess.DEVNULL, check=True, timeout=8)
    wait(lambda v: bool(v['cards']))
    return peer_identity()
