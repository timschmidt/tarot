#!/usr/bin/env python3
"""Ordinary catalog-owned claim pair; foreign exporter stays in the same login."""
import argparse,hashlib,json,os,subprocess
from pathlib import Path

p=argparse.ArgumentParser(description=__doc__);p.add_argument('--producer',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
assert os.getuid()==10000 and a.output.stat().st_uid==10000
children=[];logs=[]
def start(name,extra):
 f=(a.output/(name+'.log')).open('x');logs.append(f)
 child=subprocess.Popen([str(a.producer.resolve()),'--qa-stdin',*extra,'--report',str(a.output/(name+'.jsonl'))],stdin=None if name=='fixture' else subprocess.PIPE,text=True,stdout=f,stderr=subprocess.STDOUT);children.append(child);return child
def identity(pid):
 p=Path('/proc',str(pid));return {'pid':pid,'uid':p.stat().st_uid,'start':(p/'stat').read_text().rsplit(')',1)[1].split()[19],'cgroup':(p/'cgroup').read_text(),'executable_sha256':hashlib.sha256((p/'exe').read_bytes()).hexdigest()}
r={'passed':False,'runner':identity(os.getpid()),'controller_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
try:
 foreign=start('foreign',['--export-only']);fixture=start('fixture',['--no-claim'])
 r['foreign']=identity(foreign.pid);r['fixture']=identity(fixture.pid);(a.output/'processes.json').write_text(json.dumps(r,indent=2)+'\n')
 r['fixture_exit']=fixture.wait();assert r['fixture_exit']==0
 foreign.stdin.write('exit\n');foreign.stdin.flush();r['foreign_exit']=foreign.wait(timeout=5);assert r['foreign_exit']==0;r['passed']=True
finally:
 for child in reversed(children):
  if child.poll() is None:
   child.terminate()
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:child.kill();child.wait()
 for f in logs:f.close()
 (a.output/'result.json').write_text(json.dumps(r,indent=2)+'\n')
