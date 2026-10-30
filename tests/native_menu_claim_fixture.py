#!/usr/bin/env python3
"""Ordinary catalog-owned claim pair; foreign exporter stays in the same login."""
import argparse,hashlib,json,os,subprocess,stat,time,sys
from native_menu_traffic_cases import COUNT
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__);p.add_argument('--producer',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--traffic',action='store_true');a=p.parse_args()
assert os.getuid()==10000 and a.output.stat().st_uid==10000
children=[];logs=[];foreign_fd=None
def start(name,extra):
 f=(a.output/(name+'.log')).open('x');logs.append(f)
 child=subprocess.Popen([str(a.producer.resolve()),'--qa-stdin',*extra,'--report',str(a.output/(name+'.jsonl'))],stdin=None if name=='fixture' else (foreign_fd if a.traffic else subprocess.PIPE),text=True,stdout=f,stderr=subprocess.STDOUT);children.append(child);return child
def identity(pid):
 p=Path('/proc',str(pid));return {'pid':pid,'uid':p.stat().st_uid,'start':(p/'stat').read_text().rsplit(')',1)[1].split()[19],'cgroup':(p/'cgroup').read_text(),'executable_sha256':hashlib.sha256((p/'exe').read_bytes()).hexdigest()}
r={'passed':False,'runner':identity(os.getpid()),'controller_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
try:
 if a.traffic:
  foreign_fd=os.open(a.output/'foreign.fifo',os.O_RDWR|os.O_CLOEXEC|os.O_NOFOLLOW);meta=os.fstat(foreign_fd);assert stat.S_ISFIFO(meta.st_mode) and meta.st_uid==10000 and stat.S_IMODE(meta.st_mode)==0o600
 foreign=start('foreign',['--export-only']);fixture=start('fixture',['--no-claim'])
 if a.traffic:
  owners=[];deadline=time.monotonic()+10
  for name in ['fixture','foreign']:
   path=a.output/(name+'.jsonl')
   while True:
    values=[json.loads(l) for l in path.read_text().splitlines()] if path.exists() else []
    exports=[e['owner'] for e in values if e['event']=='export']
    if exports:owners.append(exports[0]);break
    assert time.monotonic()<deadline;time.sleep(.05)
  f=(a.output/'observer.log').open('x');logs.append(f)
  observer=subprocess.Popen([sys.executable,str(Path(__file__).with_name('native_menu_traffic_observer.py')),'--owners',*owners,'--count',str(COUNT),'--report',str(a.output/'traffic.jsonl')],stdout=f,stderr=subprocess.STDOUT);children.append(observer);r['observer']=identity(observer.pid)
 r['foreign']=identity(foreign.pid);r['fixture']=identity(fixture.pid);(a.output/'processes.json').write_text(json.dumps(r,indent=2)+'\n')
 r['fixture_exit']=fixture.wait();assert r['fixture_exit']==0
 if a.traffic:assert os.write(foreign_fd,b'exit\n')==5
 else:foreign.stdin.write('exit\n');foreign.stdin.flush()
 r['foreign_exit']=foreign.wait(timeout=5);assert r['foreign_exit']==0
 if a.traffic:r['observer_exit']=observer.wait(timeout=5);assert r['observer_exit']==0
 r['passed']=True
finally:
 for child in reversed(children):
  if child.poll() is None:
   child.terminate()
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:child.kill();child.wait()
 if foreign_fd is not None:os.close(foreign_fd)
 for f in logs:f.close()
 (a.output/'result.json').write_text(json.dumps(r,indent=2)+'\n')
