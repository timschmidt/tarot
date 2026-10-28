"""Three/five-client fan matrix inside the ordinary authenticated phone session."""
import hashlib
import json
from pathlib import Path
import time
from card_matrix import physical,run
from notification_replay import await_value
from phone_gesture_replay import request

PROBE=Path('/home/user/tarot-dev/bin/rounded-input-probe')


def replay(socket,screen,state,capture,before,out,session_cgroup):
    audit=json.loads(Path('/home/user/tarot-dev/rounded-probe-audit.json').read_text())
    assert hashlib.sha256(PROBE.read_bytes()).hexdigest()==audit['binary_sha256']
    processes=[]
    def current():
        found=[]
        for p in Path('/proc').iterdir():
            if not p.name.isdecimal():continue
            try:
                if p.stat().st_uid==10000 and (p/'exe').resolve()==PROBE:
                    assert (p/'cgroup').read_text()==session_cgroup
                    assert hashlib.sha256((p/'exe').read_bytes()).hexdigest()==audit['binary_sha256']
                    found.append(int(p.name))
            except (FileNotFoundError,ProcessLookupError):pass
        return sorted(found)
    def launch(index,color):
        request(socket,'launch',id='test.tarot.Matrix'+str(index)+'.desktop')
        pids=await_value(current,lambda p:len(p)==index+1,seconds=10)
        processes.append(next(p for p in pids if p not in processes))
    def down(s,x,y):
        x,y=physical(s,x,y);screen.down(x/2,y/2);time.sleep(.04)
    def move(s,x,y):
        x,y=physical(s,x,y);screen.move(x/2,y/2);time.sleep(.06)
    def up():screen.up();time.sleep(.04)
    record=run(socket,state,capture,down,move,up,launch,before,out)
    await_value(lambda:all(not Path('/proc',str(p)).exists() for p in processes),bool,seconds=6)
    record.update(native_panfrost_kms=True,physical_finger_tested=False,fixture_sha256=audit['binary_sha256'],
        same_login_cgroup=True,processes=processes,fixture_processes_gone=True)
    (out/'stack-matrix-validation.json').write_text(json.dumps(record,indent=2)+'\n')
    return record
