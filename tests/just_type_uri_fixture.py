#!/usr/bin/env python3
"""Owned default-URI fixture: real GIO activation, local IPC, no web requests."""
import argparse
import json
from pathlib import Path
import signal
import socket
import subprocess
from urllib.parse import parse_qs, urlsplit

QUERIES = {'café & one=two', 'reader & one=two'}
ADDRESS = 'http://example.invalid/path?q=one&x=two'
PROBE_LIFETIME_SECONDS = 240


def classification(uri):
    parsed = urlsplit(uri)
    web = (parsed.scheme == 'https' and parsed.hostname == 'duckduckgo.com'
           and parsed.path == '/' and not parsed.fragment
           and set(parse_qs(parsed.query)) == {'q'}
           and parse_qs(parsed.query).get('q', []) in [[q] for q in QUERIES])
    address = uri == ADDRESS
    return {'event': 'uri', 'matched': web or address,
            'kind': 'search' if web else 'address' if address else 'unrecognized',
            'uri_bytes': len(uri.encode())}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--socket', type=Path, required=True)
    p.add_argument('--report', type=Path, required=True)
    p.add_argument('--probe', type=Path, required=True)
    p.add_argument('--native-report', type=Path, required=True)
    p.add_argument('--notification-probe', type=Path)
    p.add_argument('--notification-dir', type=Path)
    p.add_argument('uri', nargs='?')
    args = p.parse_args()
    if args.socket.exists():
        assert args.uri is not None
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(4)
            connection.connect(str(args.socket))
            connection.sendall((json.dumps({'uri': args.uri}) + '\n').encode())
            reply = json.loads(connection.makefile().readline(4096))
            assert reply == {'ok': True}
        return
    server = socket.socket(socket.AF_UNIX)
    child = None
    notification = None
    stream = args.report.open('x')
    def record(value):
        stream.write(json.dumps(value) + '\n'); stream.flush()
    def received(uri):
        value = classification(uri); record(value)
        return value['matched']
    def stop(_signum, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, stop)
    try:
        server.bind(str(args.socket)); server.listen(2); server.settimeout(.1)
        child = subprocess.Popen([str(args.probe), '--fit-output', '--no-child',
                                  '--report', str(args.native_report), '--exit-after-seconds', str(PROBE_LIFETIME_SECONDS)])
        ready={'event': 'ready', 'native_pid': child.pid}
        assert (args.notification_probe is None) == (args.notification_dir is None)
        if args.notification_probe is not None:
            args.notification_dir.mkdir(mode=0o700,exist_ok=True)
            commands=args.notification_dir/'commands.jsonl';assert not commands.exists();commands.write_text('')
            notification=subprocess.Popen([str(args.notification_probe),'--report',str(args.notification_dir/'events.jsonl'),'--commands',str(commands),
                                           '--exit-after-seconds',str(PROBE_LIFETIME_SECONDS)])
            ready['notification_pid']=notification.pid
        record(ready)
        if args.uri is not None:
            assert received(args.uri)
        while child.poll() is None:
            try:
                connection, _ = server.accept()
            except socket.timeout:
                continue
            with connection:
                connection.settimeout(2)
                line = connection.makefile().readline(16385)
                assert len(line.encode()) <= 16384
                value = json.loads(line)
                assert set(value) == {'uri'} and isinstance(value['uri'], str)
                ok = received(value['uri'])
                connection.sendall((json.dumps({'ok': ok}) + '\n').encode())
    except KeyboardInterrupt:
        pass
    finally:
        if child is not None and child.poll() is None:
            child.terminate()
            try: child.wait(timeout=3)
            except subprocess.TimeoutExpired: child.kill(); child.wait()
        if notification is not None and notification.poll() is None:
            notification.terminate()
            try:notification.wait(timeout=3)
            except subprocess.TimeoutExpired:notification.kill();notification.wait()
        server.close(); args.socket.unlink(missing_ok=True); stream.close()


if __name__ == '__main__':
    main()
