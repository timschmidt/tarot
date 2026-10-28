"""Opt-in QA recording of actual compositor captures, never interpolated frames.

PNG readback/encoding perturbs rendering; this is visual evidence, not a frame
pacing or performance benchmark. Protected capture denial remains an error.
"""
import hashlib
import json
import os
from pathlib import Path
import threading
import time
from PIL import Image
from phone_gesture_replay import request


class Motion:
    def __init__(self,socket,out,label):
        self.socket=socket;self.path=out/'motion'/label;self.stop_event=threading.Event();self.ready=threading.Event();self.error=None;self.frames=[];self.thread=None
        owner=socket.stat()
        for p in [out/'motion',self.path]:
            p.mkdir(mode=0o700,exist_ok=True);os.chown(p,owner.st_uid,owner.st_gid)
    def __enter__(self):
        self.thread=threading.Thread(target=self.record,name='tarot-motion-capture');self.thread.start()
        try:
            assert self.ready.wait(12),'First motion frame did not complete'
            if self.error is not None:raise self.error
            return self
        except BaseException:
            self.stop_event.set();self.thread.join(timeout=12);raise
    def record(self):
        started=time.monotonic()
        try:
            while not self.stop_event.is_set() and time.monotonic()-started<10:
                file=self.path/('%04d.png'%len(self.frames));begin=time.monotonic()
                request(self.socket,'capture',path=str(file))
                deadline=time.monotonic()+8
                while True:
                    s=request(self.socket,'status')
                    assert s.get('lock',{}).get('phase')=='Unlocked','Motion recording crossed protection'
                    assert not s['capture_error'],s['capture_error']
                    if not s['capture_pending'] and file.exists():break
                    assert time.monotonic()<deadline,'Motion capture did not complete'
                    time.sleep(.01)
                with Image.open(file) as im:im.load()
                self.frames.append({'file':file.name,'requested_seconds':begin-started,'completed_seconds':time.monotonic()-started,
                    'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'view':s['view'],'selected':s['selected'],
                    'presentation':s['presentation'],'output':s['output'],'render_metrics':s['render_metrics']})
                self.ready.set();self.stop_event.wait(.025)
        except BaseException as e:self.error=e;self.ready.set()
    def __exit__(self,kind,value,traceback):
        # Keep the resulting endpoint, with native buffers still changing.
        if kind is None:time.sleep(.25)
        self.stop_event.set();self.thread.join(timeout=12);assert not self.thread.is_alive()
        record={'frames':self.frames,'interpolated':False,'performance_benchmark':False,'error':repr(self.error) if self.error else None}
        (self.path/'frames.json').write_text(json.dumps(record,indent=2)+'\n')
        if kind is None:
            if self.error is not None:raise self.error
            assert len(self.frames)>=2,'Insufficient real motion frames'
