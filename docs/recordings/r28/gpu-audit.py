import hashlib,json,os
from pathlib import Path
base=Path('/home/user/tarot-dev')
a=json.loads((base/'artifacts/favorite-r28-component-normal.json').read_text())
main=next(p for p in a['roles'] if p['name']=='tarot')
p=Path('/proc',str(main['pid']))
assert p.stat().st_uid==10000 and p.joinpath('stat').read_text().rsplit(')',1)[1].split()[19]==main['start']
assert hashlib.sha256((p/'exe').read_bytes()).hexdigest()==a['loaded_binaries_sha256']['tarot']
fds=[]
for f in (p/'fd').iterdir():
 try:
  target=os.readlink(f)
  if target.startswith('/dev/dri/'):
   fds.append({'fd':int(f.name),'target':target,'fdinfo':(p/'fdinfo'/f.name).read_text()})
 except FileNotFoundError:pass
assert any('panfrost' in f['fdinfo'] for f in fds),fds
record={'pid':main['pid'],'start':main['start'],'fds':fds,'panfrost_fd_verified':True}
out=base/'artifacts/favorite-r28-component-gpu-observation.json';out.write_text(json.dumps(record,indent=2)+'\n');os.chown(out,10000,10000)
print('PASS: audited normal Tarot process owns the Panfrost DRM render device')
