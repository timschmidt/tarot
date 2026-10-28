#!/usr/bin/env python3
"""Real touch Wi-Fi scan, saved activation and radio controls while USB remains up."""
import argparse,hashlib,json,os,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request


def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'));p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
 args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False);owner=args.socket.stat();os.chown(args.output,owner.st_uid,owner.st_gid)
 records={};screen=None
 def usb():
  assert Path('/sys/class/net/usb0/operstate').read_text().strip()=='up'
  devices=subprocess.check_output(['nmcli','-t','-f','DEVICE,STATE','device'],text=True)
  assert 'usb0:connected' in devices
 def uuid():return subprocess.check_output(['nmcli','-g','GENERAL.CON-UUID','device','show','wlan0'],text=True).strip()
 original_uuid=uuid();assert original_uuid and original_uuid!='--';usb()
 initial=await_value(lambda:request(args.socket,'status'),lambda s:s['network']['available'] and s['network']['state']=='Connected' and s['network']['active_profile'] is not None)
 original_profile=initial['network']['active_profile'];connected=next(r for r in initial['network']['networks'] if r['connected'] and original_profile in r['profiles'])
 assert initial['network']['scan_permission']==initial['network']['control_permission']==initial['network']['radio_permission']=='Yes',initial['network']
 assert Path(f'/proc/{initial["pid"]}').stat().st_uid==10000
 records['initial']=initial;records['original_profile_hash']=hashlib.sha256(original_uuid.encode()).hexdigest()
 def state(label,condition=lambda s:True,seconds=25):
  s=await_value(lambda:request(args.socket,'status'),condition,seconds=seconds);assert s['pid']==initial['pid'];records[label]=s;usb();return s
 def wifi(label,condition,seconds=25):return state(label,lambda s:condition(s['network']),seconds)['network']
 def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(args.socket,'status')['pid']==initial['pid']
 def tap(x,y):screen.down(x,y);screen.up();time.sleep(.15)
 def capture(name):
  request(args.socket,'capture',path=str(args.output/(name+'.png')));state('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
 try:
  screen=Touchscreen(guard);tap(310,14);state('menu-open',lambda s:s['system_menu_open']);time.sleep(.3)
  tap(180,277);state('wifi-page',lambda s:s['system_page']=='Wifi');time.sleep(.3);capture('wifi-initial')
  baseline=state('before-scan')['network']['last_scan'];tap(65,262)
  wifi('scan-completed',lambda n:not n['busy'] and n['last_scan']>baseline and n['action_error'] is None)
  capture('wifi-scanned')
  tap(194,262);wifi('disconnected',lambda n:not n['busy'] and n['state']=='Disconnected')
  disconnected=state('before-saved-tap')['network']
  index=next(i for i,r in enumerate(disconnected['networks']) if r['id']==connected['id'])
  assert index==0,'The current fixture expects the previously connected saved network at the top'
  row=disconnected['networks'][0];profile_index=row['profiles'].index(original_profile)
  time.sleep(.3);tap(180,356+52*profile_index)
  wifi('saved-connected',lambda n:not n['busy'] and n['active_profile']==original_profile and n['state']=='Connected')
  assert uuid()==original_uuid;capture('wifi-saved-connected')
  # Global Wi-Fi radio, never NetworkManager's global networking switch.
  tap(60,174);wifi('radio-off',lambda n:not n['busy'] and not n['radio']);capture('wifi-off')
  time.sleep(.2);tap(60,174);wifi('radio-on',lambda n:not n['busy'] and n['radio'])
  wifi('autoconnect-restored',lambda n:n['state']=='Connected' and any(r['connected'] for r in n['networks']),seconds=35)
  assert uuid()==original_uuid;capture('wifi-restored')
  assert state('native-view-preserved')['view']==initial['view']
  tap(180,124);state('back-to-system',lambda s:s['system_page']=='Power');tap(310,14);state('menu-closed',lambda s:not s['system_menu_open'])
  records['validation']={'passed':True,'usb_retained':True,'original_profile_restored':True,'compositor_uid':10000}
  print('PASS: real Wi-Fi scan, saved-network activation, radio off/on, restored original profile, native view retained and USB continuously up')
 finally:
  if screen is not None:screen.close()
  # Restore only this verified phone's original Wi-Fi profile if the replay
  # failed. This admin recovery is not evidence of an unprivileged UI success.
  current=request(args.socket,'status')['network']
  if not current['radio'] or uuid()!=original_uuid:
   subprocess.run(['nmcli','radio','wifi','on'],check=True,timeout=8)
   subprocess.run(['nmcli','--wait','30','connection','up','uuid',original_uuid,'ifname','wlan0'],check=True,timeout=35,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   records['admin_recovery_used']=True
  else:records['admin_recovery_used']=False
  request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
  usb();(args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')

if __name__=='__main__':main()
