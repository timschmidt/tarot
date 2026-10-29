import ast,hashlib,json,os,pwd,subprocess
from pathlib import Path
from phone_stock_login import processes
assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
os.umask(0o077)
names=['tarot','tarot-session','tarot-lock','tarot-keyboard']
roles=[p for p in processes() if p['uid']==10000 and p['name'] in names]
assert {p['name'] for p in roles} >= {'tarot','tarot-session','tarot-keyboard'}
assert all(sum(p['name']==n for p in roles)==1 for n in ['tarot','tarot-session','tarot-keyboard'])
assert sum(p['name']=='tarot-lock' for p in roles)<=1
main=next(p for p in roles if p['name']=='tarot')
assert not any(p['name']=='phosh' and p['uid']==10000 for p in processes())
expected=json.loads(Path('package-r28-audit.json').read_text())['binaries_sha256']
loaded={p['name']:hashlib.sha256(Path('/proc',str(p['pid']),'exe').read_bytes()).hexdigest() for p in roles}
assert all(loaded[n]==expected[n] for n in loaded)
argv=Path('/proc',str(main['pid']),'cmdline').read_bytes().split(b'\0')
assert b'--hardware-keys' in argv and b'--control-socket' not in argv and b'--lock-probe-unlocked-start' not in argv
assert not Path('/run/user/10000/tarot-session/control.sock').exists()
base=['gdbus','call','--system','--dest','org.freedesktop.login1']
a=subprocess.check_output(base+['--object-path','/org/freedesktop/login1','--method','org.freedesktop.login1.Manager.GetSessionByPID',str(main['pid'])],text=True)
obj=ast.literal_eval(a.replace('objectpath ',''))[0]
a=subprocess.check_output(base+['--object-path',obj,'--method','org.freedesktop.DBus.Properties.Get','org.freedesktop.login1.Session','Id'],text=True)
sid=ast.literal_eval(a.replace('<','').replace('>',''))[0]
props=subprocess.check_output(['loginctl','show-session',sid,'--no-pager','-p','User','-p','VTNr','-p','Remote','-p','Active','-p','Type','-p','Class'],text=True)
assert all(x in props.splitlines() for x in ['User=10000','VTNr=7','Remote=no','Active=yes','Type=wayland','Class=user'])
g=pwd.getpwnam('greetd');env={'HOME':g.pw_dir,'USER':g.pw_name,'LOGNAME':g.pw_name,'PATH':'/usr/bin:/bin'}
def setting(key):
 return ast.literal_eval(subprocess.check_output(['gsettings','get','mobi.phosh.phrog',key],env=env,user=g.pw_uid,group=g.pw_gid,extra_groups=[],text=True,timeout=8).strip())
settings={k:setting(k) for k in ['last-session','last-user']};assert settings=={'last-session':'tarot','last-user':'user'}
services={s:subprocess.run(['rc-service',s,'status'],capture_output=True,timeout=10).returncode for s in ['greetd','polkit','modemmanager','networkmanager','sshd','iio-sensor-proxy']};assert all(c==0 for c in services.values())
devices=[p.read_text().strip() for p in Path('/sys/class/input').glob('input*/name') if 'Tarot development' in p.read_text()];assert not devices
alarm=Path('/sys/class/rtc/rtc0/wakealarm').read_text().strip();assert not alarm
policies={p:hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in ['/etc/phrog/greetd-config.toml','/etc/pam.d/greetd','/etc/pam.d/tarot-lock','/etc/elogind/logind.conf']}
assert policies==json.loads(Path('artifacts/favorite-drag-native-r28/install/status.json').read_text())['before']['policies_sha256']
restored_settings=Path('/home/user/.config/tarot/settings.toml')
backup=Path('artifacts/browser-ime-native-r28-trace6/original-settings.toml')
assert restored_settings.read_bytes()==backup.read_bytes() if backup.exists() else not restored_settings.exists()
assert not Path('/home/user/.local/share/applications/test.tarot.QuickLaunchFailure.desktop').exists()
assert not any(Path('/home/user/.local/share/applications', 'test.tarot.Card'+c+'.desktop').exists() for c in ['Red','Blue'])
assert not any(p['name']=='rounded-input-p' for p in processes())
assert not any(Path('/home/user/.local/share/applications','test.tarot.Matrix'+str(i)+'.desktop').exists() for i in range(5))
assert not Path('/home/user/.local/share/applications/test.tarot.GtkClose.desktop').exists()
for name in ['Transients','GtkDialog','QtDialog','Terminal','Clipboard','VirtualLock','TerminalFullscreen','Browser','FavoriteBeta','FavoriteDelta','FavoriteEpsilon','FavoriteGamma']:
 assert not Path('/home/user/.local/share/applications','test.tarot.'+name+'.desktop').exists()
for p in Path('/proc').iterdir():
 if not p.name.isdecimal():continue
 try:
  argv=(p/'cmdline').read_bytes().split(b'\0')
  assert b'/home/user/tarot-dev/toolkit_transient_fixture.py' not in argv
  assert b'/home/user/tarot-dev/native_transient_fixture.py' not in argv
  assert b'/home/user/tarot-dev/terminal_clipboard_fixture.py' not in argv
  assert b'/home/user/tarot-dev/browser_fixture.py' not in argv
  assert b'/home/user/tarot-dev/native_virtual_keyboard_lock_fixture_r28.py' not in argv
  assert (p/'exe').resolve()!=Path('/home/user/tarot-dev/bin/transient-protocol-test')
  assert (p/'exe').resolve() not in [Path('/home/user/tarot-dev/bin/text-probe-r26'),Path('/home/user/tarot-dev/bin/lock-probe-r26')]
 except (FileNotFoundError,ProcessLookupError):pass
controller=json.loads(Path('artifacts/browser-ime-native-r28-trace6/controller-status.json').read_text())
settings_metadata=None
if backup.exists():
 meta=restored_settings.stat();settings_metadata={'uid':meta.st_uid,'gid':meta.st_gid,'mode':meta.st_mode & 0o7777}
 assert settings_metadata==controller['original_settings_metadata']
r={'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip(),'session_id':sid,'session_properties':props,'roles':roles,'loaded_binaries_sha256':loaded,'greeter_settings':settings,'services':services,'policies_sha256':policies,'development_input_devices':devices,'rtc_alarm':alarm,'settings_metadata':settings_metadata,'settings_absent':not Path('/home/user/.config/tarot/settings.toml').exists(),'validation':{'passed':True,'normal_tarot_without_diagnostics':True,'hardware_keys_enabled':True,'exact_audited_payloads_loaded':True,'active_local_ordinary_user_vt7':True,'persisted_default_tarot':True,'phosh_not_running':True,'test_input_and_alarm_absent':True,'original_settings_preserved':True}}
p=Path('artifacts/browser-ime-r28-trace6-normal.json');assert not p.exists();p.write_text(json.dumps(r,indent=2)+'\n');os.chown(p,10000,10000)
print('PASS: normal revision 28 Tarot running with hardware keys; saved default, no diagnostics, exact loaded payloads, policies and cleanup verified')
