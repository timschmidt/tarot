"""Real protocol, touch and PAM transient acceptance in the packaged phone session."""
import hashlib
import json
import os
from pathlib import Path
import socket as sockets
import struct
import time
from card_matrix import physical
from notification_replay import await_value
from phone_gesture_replay import request
from phone_lock_replay import Keyboard


def replay(control,screen,guard,state,authenticate,out,session_cgroup):
    folder=out/'transients';assert folder.is_dir() and folder.stat().st_uid==10000
    audit=json.loads(Path('/home/user/tarot-dev/transient-probe-audit.json').read_text())
    binary=Path('/home/user/tarot-dev/bin/transient-protocol-test')
    assert hashlib.sha256(binary.read_bytes()).hexdigest()==audit['binary_sha256']
    before=request(control,'status');assert not before['cards']
    record={'passed':False,'physical_finger_tested':False,'binary_sha256':audit['binary_sha256'],'input':[]}
    keyboard=None
    endpoint=folder/'input.sock';assert not endpoint.exists() and not endpoint.is_symlink()
    listener=sockets.socket(sockets.AF_UNIX)
    listener.bind(str(endpoint));os.chown(endpoint,10000,folder.stat().st_gid);os.chmod(endpoint,0o600)
    listener.listen(1);listener.settimeout(.25)
    def fixture_peer(stream):
        pid,uid,gid=struct.unpack('3i',stream.getsockopt(sockets.SOL_SOCKET,sockets.SO_PEERCRED,12))
        assert uid==10000 and Path('/proc',str(pid),'cgroup').read_text()==session_cgroup
        assert Path('/proc',str(pid),'exe').resolve()==binary
        assert hashlib.sha256(Path('/proc',str(pid),'exe').read_bytes()).hexdigest()==audit['binary_sha256']
    try:
        keyboard=Keyboard(guard)
        request(control,'launch',id='test.tarot.Transients.desktop')
        started=await_value(lambda:json.loads((folder/'fixture-started.json').read_text()) if (folder/'fixture-started.json').exists() else None,bool,seconds=20)
        assert started['cgroup']==session_cgroup and started['uid']==10000 and started['binary_sha256']==audit['binary_sha256']
        record['fixture']=started
        deadline=time.monotonic()+250
        while not (folder/'fixture-result.json').exists():
            assert time.monotonic()<deadline,'Native transient fixture timed out'
            guard()
            try:stream,_=listener.accept()
            except sockets.timeout:continue
            with stream:
                try:
                    fixture_peer(stream)
                    message=json.loads(stream.makefile('rb').readline(65536))
                    command=message['command']
                    if command=='lock':
                        request(control,'rotate',orientation='normal')
                        state('transient-before-lock-portrait',lambda s:s['output']['orientation']=='normal' and s['output']['requested'] is None and s['output']['presented_revision']==s['output']['revision'])
                        request(control,'lock')
                        protected=state('transient-protected',lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] and s['lock']['last_presented_generation']==s['lock']['generation'])
                        assert set(protected)=={'ok','pid','lock'}
                        record['protected_actual_kms']=True
                    elif command=='unlock':
                        authenticate('transient-dialog-arrival')
                        record['actual_pam_unlock']=True
                    else:
                        assert command=='click'
                        s=request(control,'status');assert s['lock']['phase']=='Unlocked' and s['output']==message['output']
                        x,y=physical(s,message['x'],message['y'])
                        screen.down(x/2,y/2);screen.up();time.sleep(.08)
                        if message['key']:keyboard.tap(30)
                        time.sleep(.06)
                    record['input'].append(message)
                    stream.sendall(b'{"ok":true}\n')
                except BaseException as error:
                    stream.sendall((json.dumps({'ok':False,'error':str(error)})+'\n').encode())
                    raise
        result=json.loads((folder/'fixture-result.json').read_text());record['fixture_result']=result
        assert result['passed'] and result['test_exit']==0
        assert len(record['input'])==18 and record['protected_actual_kms'] and record['actual_pam_unlock']
        state('transient-all-windows-gone',lambda s:not s['cards'] and s['lock']['phase']=='Unlocked')
        await_value(lambda:all(not Path('/proc',str(p)).exists() for p in [started['runner_pid'],started['fixture_pid']]),bool,seconds=10)
        record.update(passed=True,native_panfrost_kms=True,actual_touch_and_keyboard=True,same_login_cgroup=True,fixture_processes_gone=True)
    finally:
        if keyboard is not None:keyboard.close()
        listener.close();endpoint.unlink(missing_ok=True)
        (folder/'validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
