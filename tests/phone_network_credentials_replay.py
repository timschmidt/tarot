#!/usr/bin/env python3
"""Verified phone: masked touch entry and new temporary personal Wi-Fi connection.

Root QA reads the existing profile's credential into memory only. Never writes it
to evidence or passes it in process arguments. The unprivileged shell creates a
separate volatile user profile, then the replay restores the original via UI.
"""
import argparse,hashlib,json,os,subprocess,time,uuid as uuid_module
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request


def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'));p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
 args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False);await_value(args.socket.exists,bool);owner=args.socket.stat();os.chown(args.output,owner.st_uid,owner.st_gid)
 records={};screen=None;temporary_uuid=None;agent_uuid=None;password=None
 def uuid():return subprocess.check_output(['nmcli','-g','GENERAL.CON-UUID','device','show','wlan0'],text=True).strip()
 original_uuid=uuid();assert original_uuid and original_uuid!='--'
 initial=await_value(lambda:request(args.socket,'status'),lambda s:s['network']['agent_available'] and s['network']['state']=='Connected' and len(s['cards'])==1)
 original_profile=initial['network']['active_profile'];row=next(r for r in initial['network']['networks'] if r['connected'] and original_profile in r['profiles'])
 assert row['security']=='Personal' and initial['network']['create_permission']==initial['network']['control_permission']=='Yes'
 assert Path(f'/proc/{initial["pid"]}').stat().st_uid==10000
 # A QA-only read of this spare device's original profile, not an adapter call.
 password=subprocess.check_output(['nmcli','--show-secrets','--escape','no','-g','802-11-wireless-security.psk','connection','show','uuid',original_uuid],text=True).removesuffix('\n')
 assert password and password!='--'
 records['original_profile_hash']=hashlib.sha256(original_uuid.encode()).hexdigest();records['initial']=initial
 def usb():
  assert Path('/sys/class/net/usb0/operstate').read_text().strip()=='up'
  assert 'usb0:connected' in subprocess.check_output(['nmcli','-t','-f','DEVICE,STATE','device'],text=True)
 def state(label,condition=lambda s:True,seconds=25):
  s=await_value(lambda:request(args.socket,'status'),condition,seconds=seconds);assert s['pid']==initial['pid'];records[label]=s;usb();return s
 def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(args.socket,'status')['pid']==initial['pid']
 def tap(x,y):screen.down(x,y);screen.up();time.sleep(.15)
 def capture(name):
  time.sleep(.3);request(args.socket,'capture',path=str(args.output/(name+'.png')));state('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
 try:
  request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
  usb();screen=Touchscreen(guard);tap(310,14);state('menu-open',lambda s:s['system_menu_open']);tap(180,277);state('wifi-page',lambda s:s['system_page']=='Wifi');time.sleep(.3)
  # Existing profiles normally show the saved action. The development command
  # selects a separate new-profile form on the same controlled AP for this QA.
  request(args.socket,'network_setup',network=row['id']);state('new-form',lambda s:s['credential_input'] is not None and s['keyboard_visible']);time.sleep(.3)
  tap(21,504);state('touch-letter',lambda s:s['credential_input']['has_password'])
  tap(330,619);state('touch-backspace',lambda s:not s['credential_input']['has_password'])
  capture('empty-private-form')
  request(args.socket,'credential_text',value=password);state('credential-ready',lambda s:s['credential_input']['valid']);capture('masked-private-form')
  # Keyboard control toggles the same model used by the visible checkbox.
  request(args.socket,'credential_remember',remember=False)
  tap(250,676)  # Shared keyboard's Enter submits the valid form.
  state('new-connection-pending',lambda s:s['credential_input'] is None and s['network']['busy'])
  connected=state('new-connection-confirmed',lambda s:not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile'] is not None and s['network']['active_profile']!=original_profile,seconds=50)
  temporary_uuid=uuid();assert temporary_uuid and temporary_uuid!=original_uuid and temporary_uuid!='--'
  records['temporary_profile_hash']=hashlib.sha256(temporary_uuid.encode()).hexdigest();capture('new-connected')
  # Restore using the ordinary saved-profile UI action and verify exact identity.
  request(args.socket,'network_action',action={'Connect':{'network':row['id'],'profile':original_profile}})
  state('original-connected',lambda s:not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile']==original_profile,seconds=50)
  assert uuid()==original_uuid
  known=subprocess.check_output(['nmcli','-g','UUID','connection','show'],text=True).splitlines();assert temporary_uuid not in known
  # A distinct memory-only profile has no stored password. This forces the real
  # NetworkManager daemon to request credentials from Tarot's registered agent.
  before_profiles=set(next(r for r in state('before-agent-setup')['network']['networks'] if r['id']==row['id'])['profiles'])
  ssid=subprocess.check_output(['nmcli','--escape','no','-g','802-11-wireless.ssid','connection','show','uuid',original_uuid],text=True).removesuffix('\n')
  name='Tarot credential QA '+str(uuid_module.uuid4())
  subprocess.run(['nmcli','connection','add','save','no','type','wifi','ifname','wlan0','con-name',name,'ssid',ssid,'connection.autoconnect','no','connection.permissions','user:user','wifi-sec.key-mgmt','wpa-psk','wifi-sec.psk-flags','2'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
  agent_uuid=subprocess.check_output(['nmcli','-g','connection.uuid','connection','show','id',name],text=True).strip()
  inventory=state('agent-profile-available',lambda s:any(r['id']==row['id'] and set(r['profiles'])-before_profiles for r in s['network']['networks']))
  agent_profile=next(iter(set(next(r for r in inventory['network']['networks'] if r['id']==row['id'])['profiles'])-before_profiles))
  request(args.socket,'network_action',action={'Connect':{'network':row['id'],'profile':agent_profile}})
  state('native-agent-prompt',lambda s:s['credential_input'] is not None and s['credential_input']['target']=='Agent' and s['keyboard_visible'],seconds=35)
  capture('native-agent-empty');request(args.socket,'credential_text',value=password);state('native-agent-ready',lambda s:s['credential_input']['valid']);capture('native-agent-masked')
  tap(250,676)
  state('native-agent-connected',lambda s:s['credential_input'] is None and not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile']==agent_profile,seconds=50)
  assert uuid()==agent_uuid
  request(args.socket,'network_action',action={'Connect':{'network':row['id'],'profile':original_profile}})
  state('original-restored-after-agent',lambda s:not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile']==original_profile,seconds=50)
  assert uuid()==original_uuid
  subprocess.run(['nmcli','connection','delete','uuid',agent_uuid],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=8);agent_uuid=None
  assert state('native-view-preserved')['view']==initial['view']
  request(args.socket,'system_menu',open=False)
  records['validation']={'passed':True,'temporary_profile_removed':True,'real_secret_agent':True,'original_profile_restored':True,'usb_retained':True,'compositor_uid':10000,'qa_credential_source':'original profile read into memory only','credential_entry':'touch letter/delete; private diagnostic injection of existing credential; touch Enter'}
  print('PASS: masked touch entry/delete, unprivileged new temporary Wi-Fi activation, real NetworkManager secret-agent reply, temporary-profile removal, original saved connection and USB retained')
 finally:
  if screen is not None:screen.close()
  if uuid()!=original_uuid:
   subprocess.run(['nmcli','radio','wifi','on'],check=True,timeout=8)
   subprocess.run(['nmcli','--wait','30','connection','up','uuid',original_uuid,'ifname','wlan0'],check=True,timeout=35,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   records['admin_recovery_used']=True
  else:records['admin_recovery_used']=False
  request(args.socket,'credential_cancel');request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
  if temporary_uuid:
   known=subprocess.check_output(['nmcli','-g','UUID','connection','show'],text=True).splitlines()
   if temporary_uuid in known:subprocess.run(['nmcli','connection','delete','uuid',temporary_uuid],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  if agent_uuid:subprocess.run(['nmcli','connection','delete','uuid',agent_uuid],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  text=json.dumps(records,indent=2)+'\n';assert not password or password not in text
  password=None;usb();(args.output/'status.json').write_text(text)

if __name__=='__main__':main()
