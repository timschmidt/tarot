#!/usr/bin/env python3
"""Full touch Wi-Fi password entry and remembered profile across shell sessions.

Run save then restore in separate bounded local sessions. The root-only ledger
contains profile UUIDs, never a password. Save intentionally retains only the
test profile for restore. Recover removes it after an interrupted experiment.
"""
import argparse,hashlib,hmac,json,os,stat,subprocess,time,uuid
from pathlib import Path
from notification_replay import await_value
from phone_gesture_replay import Touchscreen,request
from phone_touch_keyboard import Keyboard


def nm(*args):
    return subprocess.check_output(['nmcli','--escape','no',*args],text=True,timeout=12).removesuffix('\n')


def active_uuid():
    return nm('-g','GENERAL.CON-UUID','device','show','wlan0')


def password_for(identifier):
    return nm('--show-secrets','-g','802-11-wireless-security.psk','connection','show','uuid',identifier)


def disk_profile(identifier,expected):
    # Root QA reads only this experiment's newly created keyfile. No contents
    # enter evidence, errors or process arguments; public results are booleans.
    from gi.repository import Gio,GLib
    bus=Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
    def call(path,interface,method,signature,values,result):
        return bus.call_sync('org.freedesktop.NetworkManager',path,interface,method,GLib.Variant(signature,values),GLib.VariantType.new(result),Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
    path=call('/org/freedesktop/NetworkManager/Settings','org.freedesktop.NetworkManager.Settings','GetConnectionByUuid','(s)',(identifier,),'(o)')
    props=call(path,'org.freedesktop.DBus.Properties','GetAll','(s)',('org.freedesktop.NetworkManager.Settings.Connection',),'(a{sv})')
    filename=Path(props['Filename']);metadata=filename.lstat()
    assert stat.S_ISREG(metadata.st_mode) and metadata.st_uid==0 and stat.S_IMODE(metadata.st_mode)==0o600
    assert not props['Unsaved'] and filename.parent==Path('/etc/NetworkManager/system-connections')
    keyfile=GLib.KeyFile();keyfile.load_from_file(str(filename),GLib.KeyFileFlags.NONE)
    section='wifi-security' if keyfile.has_group('wifi-security') else '802-11-wireless-security'
    matches=hmac.compare_digest(keyfile.get_string(section,'psk'),expected)
    assert matches,'The saved test profile did not retain the expected credential'
    settings=call(path,'org.freedesktop.NetworkManager.Settings.Connection','GetSettings','()',(),'(a{sa{sv}})')
    # GetSettings may omit properties equal to their documented defaults.
    assert settings['connection']['permissions']==['user:user:'] and settings['connection'].get('autoconnect',True)
    return filename,{'file_backed':True,'root_owned':True,'mode':0o600,'private_user':True,'autoconnect':True,'password_matches':matches,'unsaved':props['Unsaved']}


def read_ledger(path):
    metadata=path.lstat();assert stat.S_ISREG(metadata.st_mode) and metadata.st_uid==0 and stat.S_IMODE(metadata.st_mode)==0o600
    value=json.loads(path.read_text());assert value['version']==1
    assert str(uuid.UUID(value['original_uuid']))==value['original_uuid']
    assert str(uuid.UUID(value['saved_uuid']))==value['saved_uuid'] and value['saved_uuid']!=value['original_uuid']
    return value


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase',choices=['save','restore','recover'],required=True)
    parser.add_argument('--socket',type=Path,default=Path('/run/user/10000/tarot-drm.sock'))
    parser.add_argument('--ledger',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    if args.phase=='recover':
        ledger=read_ledger(args.ledger)
        if active_uuid()!=ledger['original_uuid']:
            subprocess.run(['nmcli','--wait','30','connection','up','uuid',ledger['original_uuid'],'ifname','wlan0'],check=True,timeout=35,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        if ledger['saved_uuid'] in nm('-g','UUID','connection','show').splitlines():
            subprocess.run(['nmcli','connection','delete','uuid',ledger['saved_uuid']],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        args.ledger.unlink();print('Recovered original connection and removed only the recorded test profile');return
    ledger=read_ledger(args.ledger) if args.phase=='restore' else None
    if args.phase=='save':assert not args.ledger.exists(),'Use a fresh ledger or recover the previous experiment'
    await_value(args.socket.exists,bool);owner=args.socket.stat();os.chown(args.output,owner.st_uid,owner.st_gid)
    initial=await_value(lambda:request(args.socket,'status'),lambda s:s['network']['state']=='Connected' and s['network']['agent_available'] and len(s['cards'])==1)
    assert Path(f'/proc/{initial["pid"]}').stat().st_uid==10000
    original_uuid=active_uuid();assert original_uuid and original_uuid!='--'
    if ledger:assert original_uuid==ledger['original_uuid'] and initial['pid']!=ledger['compositor_pid']
    original_profile=initial['network']['active_profile'];row=next(r for r in initial['network']['networks'] if r['connected'] and original_profile in r['profiles'])
    assert row['security']=='Personal' and len(row['profiles'])==(1 if args.phase=='save' else 2)
    secret=password_for(original_uuid);assert 8<=len(secret)<=64 and all(' '<=ch<='~' for ch in secret)
    records={'initial':initial,'phase':args.phase,'original_profile_hash':hashlib.sha256(original_uuid.encode()).hexdigest()}
    before_profiles=set(nm('-g','UUID','connection','show').splitlines());saved_uuid=ledger['saved_uuid'] if ledger else None
    screen=None;passed=False;keep_profile=False;filename=None

    def usb():
        assert Path('/sys/class/net/usb0/operstate').read_text().strip()=='up'
        assert 'usb0:connected' in nm('-t','-f','DEVICE,STATE','device')
    def state(label,condition=lambda s:True,seconds=35):
        value=await_value(lambda:request(args.socket,'status'),condition,seconds=seconds)
        assert value['pid']==initial['pid'] and value['network']['secret_prompt'] is None
        records[label]=value;usb();return value
    def guard():assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty8' and request(args.socket,'status')['pid']==initial['pid']
    def tap(x,y):screen.down(x,y);screen.up();time.sleep(.13)
    def capture(name):
        time.sleep(.3);request(args.socket,'capture',path=str(args.output/(name+'.png')))
        state('capture-'+name,lambda s:not s['capture_pending'] and not s['capture_error'])
    def select_profile(profile,label):
        current=state('before-'+label);network=next(r for r in current['network']['networks'] if r['connected'])
        index=network['profiles'].index(profile);assert index<2
        time.sleep(.3);tap(180,356+52*index)
        return state(label,lambda s:not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile']==profile,seconds=50)
    try:
        usb();request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
        screen=Touchscreen(guard);tap(310,14);state('menu-open',lambda s:s['system_menu_open']);tap(180,277);state('wifi-page',lambda s:s['system_page']=='Wifi');time.sleep(.3)
        if args.phase=='save':
            request(args.socket,'network_setup',network=row['id'])  # Select a new-profile form on this controlled, already-saved AP.
            state('new-form',lambda s:s['credential_input'] is not None and s['keyboard_visible']);time.sleep(.3)
            tap(60,211);state('remember-off',lambda s:not s['credential_input']['remember'])
            tap(60,211);state('remember-on',lambda s:s['credential_input']['remember'])
            Keyboard(tap).type_ascii(secret)
            state('full-touch-entry',lambda s:s['credential_input']['valid']);capture('remembered-password-masked')
            tap(250,676)
            connected=state('remembered-connected',lambda s:s['credential_input'] is None and not s['network']['busy'] and s['network']['state']=='Connected' and s['network']['active_profile'] not in [None,original_profile],seconds=50)
            saved_uuid=active_uuid();assert saved_uuid!=original_uuid and saved_uuid not in before_profiles
            filename,records['disk_profile']=disk_profile(saved_uuid,secret)
            capture('remembered-connected')
            select_profile(original_profile,'original-restored')
            assert active_uuid()==original_uuid and filename.exists()
            ledger={'version':1,'original_uuid':original_uuid,'saved_uuid':saved_uuid,'compositor_pid':initial['pid']}
            fd=os.open(args.ledger,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
            with os.fdopen(fd,'w') as output:json.dump(ledger,output);output.flush();os.fsync(output.fileno())
            keep_profile=True
        else:
            filename,records['disk_profile']=disk_profile(saved_uuid,secret)
            saved_profile=next(profile for profile in row['profiles'] if profile!=original_profile)
            select_profile(saved_profile,'remembered-reconnected-without-entry')
            assert active_uuid()==saved_uuid and state('no-password-form')['credential_input'] is None
            capture('reconnected-after-session-restart')
            select_profile(original_profile,'original-restored')
            assert active_uuid()==original_uuid
            subprocess.run(['nmcli','connection','delete','uuid',saved_uuid],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=10)
            assert not filename.exists() and saved_uuid not in nm('-g','UUID','connection','show').splitlines()
            args.ledger.unlink();saved_uuid=None
        assert state('native-view-retained')['view']==initial['view']
        records['validation']={'passed':True,'phase':args.phase,'full_touch_password_entry':args.phase=='save','new_compositor_process':args.phase=='restore','disk_password_verified':True,'usb_retained':True,'original_profile_restored':True,'networkmanager_restarted':False}
        passed=True;print('PASS:',args.phase,'remembered private Wi-Fi profile, full touch entry or reconnect after shell restart, original connection and USB retained')
    finally:
        if screen is not None:screen.close()
        if active_uuid()!=original_uuid:
            subprocess.run(['nmcli','--wait','30','connection','up','uuid',original_uuid,'ifname','wlan0'],check=True,timeout=35,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            records['admin_recovery_used']=True
        else:records['admin_recovery_used']=False
        if args.socket.exists():
            request(args.socket,'credential_cancel');request(args.socket,'system_menu',open=False);request(args.socket,'system_page',page='Power')
        if not passed:
            # Only a new-profile UUID absent before this verified experiment is
            # eligible for removal. The original profile is never deleted.
            candidates=nm('-g','UUID','connection','show').splitlines()
            owned=[identifier for identifier in candidates if identifier not in before_profiles and nm('-g','connection.id','connection','show','uuid',identifier).startswith('Tarot ')]
            if saved_uuid and saved_uuid!=original_uuid and saved_uuid not in owned:owned.append(saved_uuid)
            for identifier in owned:subprocess.run(['nmcli','connection','delete','uuid',identifier],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            if args.ledger.exists():args.ledger.unlink()
        records['test_profile_retained_for_next_session']=keep_profile and passed
        text=json.dumps(records,indent=2)+'\n';assert secret not in text;secret=None
        usb();(args.output/'status.json').write_text(text)

if __name__=='__main__':main()
