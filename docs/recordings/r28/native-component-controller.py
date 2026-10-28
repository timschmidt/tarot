import getpass,hashlib,json,os,pwd,signal,stat,subprocess,tempfile,traceback
from pathlib import Path
from types import SimpleNamespace
from notification_replay import await_value
from phone_stock_login import processes,login
from phone_installed_favorites_r28 import run
from PIL import Image
assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
os.umask(0o077)
account=pwd.getpwnam('user');assert account.pw_uid==10000
out=Path('artifacts/favorite-component-native-r28').resolve();out.mkdir(mode=0o700);os.chown(out,10000,account.pw_gid)
password=getpass.getpass('Device password for revision 28 native replay and normal Tarot restoration: ')
assert password
signal.signal(signal.SIGHUP,signal.SIG_IGN)
child=os.fork()
if child:
 password=None
 print('Detached native favorite/shell component controller PID',child,flush=True)
 raise SystemExit(0)
os.setsid()
fd=os.open('/dev/null',os.O_RDONLY);os.dup2(fd,0);os.close(fd)
fd=os.open(out/'controller.log',os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.fchown(fd,10000,account.pw_gid);os.dup2(fd,1);os.dup2(fd,2);os.close(fd)
config=Path('/home/user/.config/tarot/settings.toml');fixture=Path('/home/user/.local/share/applications/test.tarot.QuickLaunchFailure.desktop')
card_fixtures={}
original=None;metadata=None;parent_absent=False;config_changed=False;fixture_created=False
fixture_text='[Desktop Entry]\nType=Application\nName=A Tarot Quick Launch failure\nExec=/bin/false\n'
r={'controller_pid':os.getpid(),'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'validation':{'passed':False},'controller_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'replay_sha256':hashlib.sha256(Path('phone_installed_favorites_r28.py').read_bytes()).hexdigest(),'quick_replay_sha256':hashlib.sha256(Path('phone_quick_launch_replay.py').read_bytes()).hexdigest()}
r['full_native_acceptance']=False
r['component_scope']=['favorite-drag','quick-launch','rounded-cards','stack-matrix','gtk-close','hardware-keys','saved-restart']
def save():
 payload=json.dumps(r,indent=2)+'\n'
 p=out/'controller-status.tmp';p.write_text(payload);os.chown(p,10000,account.pw_gid);p.replace(out/'controller-status.json')
def greeter_ready():
 return await_value(processes,lambda ps:not any(p['uid']==10000 and p['name'] in ['tarot','tarot-session','tarot-lock','tarot-keyboard'] for p in ps) and any(p['uid']==109 and p['name']=='phrog' for p in ps),seconds=40)
def directory(path):
 assert not path.is_symlink()
 absent=not path.exists();path.mkdir(mode=0o700,parents=True,exist_ok=True)
 if absent:os.chown(path,10000,account.pw_gid)
 assert path.stat().st_uid==10000 and path.is_dir()
 return absent
def replace_config(data,mode=0o600):
 fd,name=tempfile.mkstemp(prefix='.quick-launch-qa-',dir=config.parent)
 try:
  with os.fdopen(fd,'wb') as stream:
   os.fchown(stream.fileno(),10000,account.pw_gid);os.fchmod(stream.fileno(),mode);stream.write(data);stream.flush();os.fsync(stream.fileno())
  os.replace(name,config)
 finally:Path(name).unlink(missing_ok=True)
def replay(label,quick,hardware):
 folder=out/label;folder.mkdir(mode=0o700);os.chown(folder,10000,account.pw_gid)
 if quick:
  directory(folder/'transients');directory(folder/'toolkits');directory(folder/'terminal-clipboard');directory(folder/'terminal-fullscreen');directory(folder/'virtual-keyboard-lock');directory(folder/'browser');directory(folder/'favorite-drag')
 run(SimpleNamespace(output=folder,cycles=0,crash=False,require_audio=True,rotate_protected=True,observe_sensors=False,panel_off=False,resource_sample=False,quick_launch=quick,rounded_cards=quick,stack_matrix=quick,gtk_close=quick,transients=False,toolkit_dialogs=False,terminal_clipboard=False,virtual_keyboard_lock=False,browser=False,browser_inspect=False,favorite_drag=quick,hardware_keys=hardware),password)
 return json.loads((folder/'status.json').read_text())
try:
 main=[p for p in processes() if p['uid']==10000 and p['name']=='tarot'];assert len(main)==1
 main=main[0];pid=main['pid']
 expected=json.loads(Path('package-r28-audit.json').read_text())['binaries_sha256']['tarot']
 assert hashlib.sha256(Path('/proc',str(pid),'exe').read_bytes()).hexdigest()==expected
 assert b'--control-socket' not in Path('/proc',str(pid),'cmdline').read_bytes().split(b'\0')
 assert Path('/proc',str(pid),'cgroup').read_text()!=Path('/proc/self/cgroup').read_text()
 r['ended_normal_session']=main;r['stage']='end-r28-normal';save()
 fd=os.pidfd_open(pid)
 try:
  assert main in processes();signal.pidfd_send_signal(fd,signal.SIGTERM)
 finally:os.close(fd)
 greeter_ready()
 r['stage']='verify-installed-r28';save()
 assert all(hashlib.sha256(Path('/usr/bin',name).read_bytes()).hexdigest()==digest for name,digest in json.loads(Path('package-r28-audit.json').read_text())['binaries_sha256'].items())
 r['package_reinstalled']=False;r['original_install_evidence']='artifacts/favorite-drag-native-r28/install/status.json';save()
 assert not config.is_symlink() and not config.parent.is_symlink() and not config.parent.parent.is_symlink()
 parent_absent=directory(config.parent)
 if config.exists():
  metadata=config.stat();assert config.is_file() and metadata.st_uid==10000 and metadata.st_size<=65536
  original=config.read_bytes();(out/'original-settings.toml').write_bytes(original)
  r['original_settings_metadata']={'uid':metadata.st_uid,'gid':metadata.st_gid,'mode':stat.S_IMODE(metadata.st_mode)}
 r['original_settings_absent']=original is None
 directory(fixture.parent);assert not fixture.exists() and not fixture.is_symlink()
 with fixture.open('x') as stream:stream.write(fixture_text)
 os.chown(fixture,10000,account.pw_gid);fixture_created=True
 for color,name,rgb,extra in [('Red','red','e91d31',' --no-child'),('Blue','blue','2255dd',' --corner-child')]:
  p=fixture.parent/('test.tarot.Card'+color+'.desktop');assert not p.exists() and not p.is_symlink()
  text='[Desktop Entry]\nType=Application\nName=Z Tarot rounded '+name+'\nExec=/home/user/tarot-dev/bin/rounded-input-probe --fit-output --opaque --solid-color '+rgb+extra+' --report '+str(out/'first-replay'/(name+'.jsonl'))+' --exit-after-seconds 180\n'
  with p.open('x') as stream:stream.write(text)
  os.chown(p,10000,account.pw_gid);card_fixtures[p]=text
 test=b'version = 3\n[notifications]\ndnd = true\nhide_previews = true\n[display]\nrotation_locked = true\nlocked_orientation = "normal"\n[quick_launch]\napplications = ["org.gnome.Calculator.desktop"]\n'
 replace_config(test);config_changed=True
 for index,color in enumerate(['e91d31','1db931','1db9b9','b91db9','2255dd']):
  p=fixture.parent/('test.tarot.Matrix'+str(index)+'.desktop');assert not p.exists() and not p.is_symlink()
  text='[Desktop Entry]\nType=Application\nName=Z Tarot stack '+str(index)+'\nExec=/home/user/tarot-dev/bin/rounded-input-probe --fit-output --opaque --solid-color '+color+(' --corner-child' if index==4 else ' --no-child')+' --report '+str(out/'first-replay'/('matrix-'+str(index)+'.jsonl'))+' --exit-after-seconds 300\n'
  with p.open('x') as stream:stream.write(text)
  os.chown(p,10000,account.pw_gid);card_fixtures[p]=text
 home=out/'editor-home';directory(home)
 env=dict(os.environ,HOME=str(home),GSETTINGS_BACKEND='keyfile',LANG='C.UTF-8',LC_ALL='C.UTF-8',GTK_A11Y='none',GTK_USE_PORTAL='0')
 for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache'),('XDG_STATE_HOME','state')]:
  directory(home/name);env[key]=str(home/name)
 subprocess.run(['gsettings','set','org.gnome.TextEditor','restore-session','false'],env=env,user=10000,group=account.pw_gid,check=True,timeout=10)
 document=home/'tarot-close-test.txt';document.write_bytes(b'Original Tarot test document\n');os.chown(document,10000,account.pw_gid)
 p=fixture.parent/'test.tarot.GtkClose.desktop';assert not p.exists() and not p.is_symlink()
 text='[Desktop Entry]\nType=Application\nName=Z Tarot GTK close\nExec=/usr/bin/env '+ ' '.join(key+'='+env[key] for key in ['HOME','GSETTINGS_BACKEND','LANG','LC_ALL','GTK_A11Y','GTK_USE_PORTAL','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','XDG_STATE_HOME'])+' /usr/bin/dbus-run-session -- /usr/bin/gnome-text-editor --ignore-session '+str(document)+'\n'
 with p.open('x') as stream:stream.write(text)
 os.chown(p,10000,account.pw_gid);card_fixtures[p]=text
 for label,command in [
  ('Browser','/usr/bin/python3 /home/user/tarot-dev/browser_fixture.py --output '+str(out/'first-replay'/'browser')),
  ('Terminal','/usr/bin/foot --app-id=test.tarot.Terminal --title=Tarot-terminal --font=monospace:size=14 -- /usr/bin/python3 /home/user/tarot-dev/terminal_clipboard_fixture.py --mode shell --output '+str(out/'first-replay'/'terminal-clipboard')),
  ('TerminalFullscreen','/usr/bin/foot --fullscreen --app-id=test.tarot.TerminalFullscreen --title=Tarot-fullscreen-terminal --font=monospace:size=14 -- /usr/bin/python3 /home/user/tarot-dev/terminal_clipboard_fixture.py --mode shell --output '+str(out/'first-replay'/'terminal-fullscreen')),
  ('Clipboard','/usr/bin/python3 /home/user/tarot-dev/terminal_clipboard_fixture.py --mode clipboard --output '+str(out/'first-replay'/'terminal-clipboard')),
  ('VirtualLock','/usr/bin/python3 /home/user/tarot-dev/native_virtual_keyboard_lock_fixture_r28.py --output '+str(out/'first-replay'/'virtual-keyboard-lock')+' --audit /home/user/tarot-dev/virtual-keyboard-probe-audit-r28.json'),
  ('Transients','/usr/bin/python3 /home/user/tarot-dev/native_transient_fixture.py --output '+str(out/'first-replay'/'transients')+' --binary /home/user/tarot-dev/bin/transient-protocol-test --audit /home/user/tarot-dev/transient-probe-audit.json'),
  ('GtkDialog','/usr/bin/python3 /home/user/tarot-dev/toolkit_transient_fixture.py --toolkit gtk --report '+str(out/'first-replay'/'toolkits'/'gtk-events.jsonl')),
  ('QtDialog','/usr/bin/python3 /home/user/tarot-dev/toolkit_transient_fixture.py --toolkit qt --report '+str(out/'first-replay'/'toolkits'/'qt-events.jsonl'))]:
  p=fixture.parent/('test.tarot.'+label+'.desktop');assert not p.exists() and not p.is_symlink()
  text='[Desktop Entry]\nType=Application\nName=Z Tarot '+label+'\nExec='+command+'\n'
  with p.open('x') as stream:stream.write(text)
  os.chown(p,10000,account.pw_gid);card_fixtures[p]=text
 for suffix in ['Beta','Delta','Epsilon','Gamma']:
  p=fixture.parent/('test.tarot.Favorite'+suffix+'.desktop');assert not p.exists() and not p.is_symlink()
  text='[Desktop Entry]\nType=Application\nName=Tarot Favorite '+suffix+'\nExec=/bin/false\n'
  with p.open('x') as stream:stream.write(text)
  os.chown(p,10000,account.pw_gid);card_fixtures[p]=text
 r['gtk_editor_version']=subprocess.check_output(['/usr/bin/gnome-text-editor','--version'],env=dict(env,GSETTINGS_BACKEND='memory'),user=10000,group=account.pw_gid,text=True,timeout=10)
 r['tesseract_version']=subprocess.check_output(['tesseract','--version'],text=True,timeout=10).splitlines()[0]
 r['temporary_entries_sha256']={str(p):hashlib.sha256(text.encode()).hexdigest() for p,text in card_fixtures.items()}
 r['temporary_entries_sha256'][str(fixture)]=hashlib.sha256(fixture_text.encode()).hexdigest()
 r['stage']='first-native-login';save()
 login(SimpleNamespace(session='tarot',diagnostics=True,output=out/'first-login'),password)
 r['stage']='native-quick-and-hardware';save()
 first=replay('first-replay',True,True)
 assert first['favorite_drag']['passed'] and first['validation']['passed'] and first['quick_launch']['passed'] and first['rounded_cards']['passed'] and first['stack_matrix']['passed'] and first['gtk_close']['passed'] and first['hardware_keys']['passed']
 r['native_favorite_drag_passed']=True;r['native_quick_and_hardware_passed']=True
 r['native_rounded_cards_passed']=True;r['native_stack_matrix_passed']=True;r['native_gtk_close_passed']=True
 r['stage']='restart-login';save()
 login(SimpleNamespace(session='tarot',diagnostics=True,output=out/'restart-login'),password)
 second=replay('restart-replay',False,False)
 assert second['validation']['passed']
 assert second['startup-ordinary']['favorites']['applications']==['org.gnome.Calculator.desktop']
 r['restart_persistence_passed']=True
except BaseException as e:
 r['exception_type']=type(e).__name__;traceback.print_exc()
finally:
 try:
  r['stage']='restore-preferences-and-normal-tarot';greeter_ready()
  if config_changed:
   assert not config.is_symlink()
   if original is None:
    config.unlink(missing_ok=True)
    if parent_absent:config.parent.rmdir()
   else:replace_config(original,stat.S_IMODE(metadata.st_mode))
   r['original_settings_restored']=(not config.exists()) if original is None else config.read_bytes()==original
   assert r['original_settings_restored']
  if fixture_created:
   assert fixture.read_text()==fixture_text;fixture.unlink();r['temporary_entry_removed']=True
  for p,text in card_fixtures.items():
   assert p.read_text()==text;p.unlink()
  r['card_entries_removed']=True
  login(SimpleNamespace(session='tarot',diagnostics=False,output=out/'normal-login'),password)
  r['normal_tarot_restored']=True
 except BaseException as e:
  r['restore_exception_type']=type(e).__name__;traceback.print_exc()
 password=None;r['stage']='terminal'
 r['validation']={'passed':all(r.get(k,False) for k in ['native_favorite_drag_passed', 'native_quick_and_hardware_passed', 'native_rounded_cards_passed', 'native_stack_matrix_passed', 'native_gtk_close_passed', 'card_entries_removed', 'restart_persistence_passed', 'original_settings_restored', 'temporary_entry_removed', 'normal_tarot_restored']),'password_logged':False};save()
 print('Native favorite/shell component result (full browser acceptance remains pending):',r['validation']['passed'],flush=True)
