#!/usr/bin/env python3
"""Authenticate the spare phone's normal Phosh session through greetd IPC.

No greeter/session policy changes. Password is obtained with getpass and sent
only to the local authentication socket, never logs, files, argv or environment.
Python/socket/PAM copies are not a comprehensive memory-zeroization guarantee.
"""
import argparse,getpass,json,os,pwd,signal,socket,stat,struct,time
from pathlib import Path
from notification_replay import await_value


def processes():
    found=[]
    for p in Path('/proc').iterdir():
        if not p.name.isdecimal():continue
        try:
            name=(p/'comm').read_text().strip()
            if name in ['phoc','phosh','phrog','greetd','tarot','tarot-session','tarot-lock','tarot-keyboard']:
                found.append({'pid':int(p.name),'uid':p.stat().st_uid,'name':name,'start':(p/'stat').read_text().rsplit(')',1)[1].split()[19]})
        except (FileNotFoundError,ProcessLookupError,PermissionError):pass
    return found


def login(args, password=None):
    assert not args.diagnostics or args.session=='tarot'
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    assert Path('/sys/class/tty/tty0/active').read_text().strip()=='tty7' and not Path('/run/user/10000/tarot-drm.sock').exists()
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid)
    before=processes();assert not any(p['uid']==10000 and p['name'] in ['phoc','tarot'] for p in before)
    greeter=pwd.getpwnam('greetd');compositors=[p for p in before if p['uid']==greeter.pw_uid and p['name']=='phoc'];assert len(compositors)==1
    manager=int(Path('/run/greetd.pid').read_text().strip());endpoint=Path(f'/run/greetd-{manager}.sock');metadata=endpoint.stat();assert stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid==greeter.pw_uid
    if password is None:
        password=getpass.getpass('Device password for normal greetd session authentication: ')
    assert password
    records={'before':before,'greetd_pid':manager,'secret_prompts':0,'selected_session':args.session};stream=socket.socket(socket.AF_UNIX);stream.settimeout(15);authenticated=False
    def receive(length):
        data=bytearray()
        while len(data)<length:
            chunk=stream.recv(length-len(data));assert chunk,'greetd closed the conversation';data.extend(chunk)
        return data
    def request(value):
        payload=json.dumps(value).encode();assert len(payload)<65536
        stream.sendall(struct.pack('=I',len(payload))+payload)
        size=struct.unpack('=I',receive(4))[0];assert size<65536
        return json.loads(receive(size))
    try:
        stream.connect(str(endpoint));assert request({'type':'cancel_session'})['type']=='success'
        response=request({'type':'create_session','username':'user'})
        for _ in range(12):
            kind=response['type']
            if kind=='success':authenticated=True;break
            assert kind=='auth_message','Stock authentication failed; no credential recorded'
            message_type=response['auth_message_type']
            if message_type=='secret':
                records['secret_prompts']+=1;assert records['secret_prompts']==1,'Unexpected additional secret factor; canceled'
                response=request({'type':'post_auth_message_response','response':password})
            else:
                assert message_type in ['info','error'],'Unexpected visible authentication question; canceled'
                response=request({'type':'post_auth_message_response'})
        assert authenticated and records['secret_prompts']==1
        desktop='Phosh:GNOME' if args.session=='phosh' else 'Tarot'
        command=['/usr/bin/phosh-session'] if args.session=='phosh' else ['/usr/bin/tarot-session']+(['--diagnostics'] if args.diagnostics else [])
        response=request({'type':'start_session','cmd':command,'env':['XDG_SESSION_TYPE=wayland','XDG_CURRENT_DESKTOP='+desktop,'XDG_SESSION_DESKTOP='+args.session]})
        assert response['type']=='success','Session launch refused'
        # This auxiliary test client does not own the greeter. Its verified
        # compositor must end so greetd can start the authenticated session.
        current=next(p for p in processes() if p['pid']==compositors[0]['pid']);assert current==compositors[0]
        os.kill(current['pid'],signal.SIGTERM)
        expected=['phoc','phosh'] if args.session=='phosh' else ['tarot','tarot-session','tarot-lock','tarot-keyboard']
        after=await_value(processes,lambda ps:all(any(p['uid']==10000 and p['name']==name for p in ps) for name in expected),seconds=30)
        records['after']=after;records['active_vt']=Path('/sys/class/tty/tty0/active').read_text().strip();assert records['active_vt']=='tty7'
        records['validation']={'passed':True,'normal_greetd_pam_authentication':True,'phoc_and_phosh_uid_10000':args.session=='phosh','tarot_and_trusted_helpers_uid_10000':args.session=='tarot','greeter_policy_changed':False,'password_recorded':False,'visual_input_check_complete':False}
        print('PASS: normal greetd PAM login launched',args.session,'as UID 10000 on VT7; no login policy changed')
    finally:
        password=None
        if not records.get('validation',{}).get('passed'):
            try:request({'type':'cancel_session'})
            except (OSError,AssertionError):pass
        stream.close();(args.output/'status.json').write_text(json.dumps(records,indent=2)+'\n')


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--session',choices=['phosh','tarot'],default='phosh');p.add_argument('--diagnostics',action='store_true')
    login(p.parse_args())


if __name__=='__main__':main()
