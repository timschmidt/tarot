#!/usr/bin/env python3
"""Bounded RTC alarm and actual stock-session suspend baseline on the spare.

Run detached for --phase stock: USB/SSH may pause. Refuses an existing RTC
alarm. Uses normal elogind Suspend, not a direct /sys/power/state write.
"""
import argparse,json,os,pwd,signal,subprocess,time
from pathlib import Path
from notification_replay import await_value
from phone_stock_login import processes

MANAGER=['gdbus','call','--system','--dest','org.freedesktop.login1','--object-path','/org/freedesktop/login1','--method']
RTC=Path('/sys/class/rtc/rtc0')


def clocks():return {'monotonic':time.monotonic(),'boottime':time.clock_gettime(time.CLOCK_BOOTTIME),'realtime':time.time()}


def stats():
    values={}
    for p in Path('/sys/power/suspend_stats').iterdir():
        value=p.read_text().strip();values[p.name]=int(value) if value.lstrip('-').isdigit() else value
    return values


def snapshot():
    return {'clocks':clocks(),'stats':stats(),'processes':processes(),'active_vt':Path('/sys/class/tty/tty0/active').read_text().strip(),'mem_sleep':Path('/sys/power/mem_sleep').read_text().strip(),'rtc_alarm':(RTC/'wakealarm').read_text().strip(),'battery':{n:(Path('/sys/class/power_supply/rk818-battery')/n).read_text().strip() for n in ['capacity','status','current_now','voltage_now']},'boot_id':Path('/proc/sys/kernel/random/boot_id').read_text().strip()}


def kernel_lines():
    result=subprocess.check_output(['dmesg','--raw'],text=True)
    return [line for line in result.splitlines() if any(text in line.lower() for text in ['pm:','suspend','wakeup','wakeirq','rk808-rtc'])]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--phase',choices=['alarm','stock'],required=True);p.add_argument('--seconds',type=int,default=15);p.add_argument('--output',type=Path,required=True);p.add_argument('--alarm-evidence',type=Path);args=p.parse_args()
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text() and 5<=args.seconds<=30
    assert not (RTC/'wakealarm').read_text().strip(),'An existing alarm belongs to another actor'
    assert (RTC/'device/power/wakeup').read_text().strip()=='enabled'
    assert abs(int((RTC/'since_epoch').read_text())-time.time())<3,'RTC UTC mapping unverified'
    assert not Path('/run/user/10000/tarot-drm.sock').exists() and Path('/sys/class/tty/tty0/active').read_text().strip()=='tty7'
    before=snapshot();assert any(p['uid']==10000 and p['name']=='phoc' for p in before['processes']) and any(p['uid']==10000 and p['name']=='phosh' for p in before['processes'])
    if args.phase=='stock':
        assert args.alarm_evidence is not None
        alarm=json.loads(args.alarm_evidence.read_text());assert alarm['validation']['rtc_alarm_wait_passed'] and alarm['before']['boot_id']==before['boot_id']
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=False);account=pwd.getpwnam('user');os.chown(args.output,account.pw_uid,account.pw_gid)
    signal.signal(signal.SIGHUP,signal.SIG_IGN)
    records={'phase':args.phase,'before':before,'kernel_before':kernel_lines(),'pid':os.getpid()};owned_alarm=None;child=None
    def save(name='running.json'):(args.output/name).write_text(json.dumps(records,indent=2)+'\n')
    save()
    try:
        with (args.output/'rtcwake.log').open('w') as log:
            command=['rtcwake','-u','-d','/dev/rtc0','-m','on' if args.phase=='alarm' else 'no','-s',str(args.seconds),'-v']
            child=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT)
            owned_alarm=await_value(lambda:(RTC/'wakealarm').read_text().strip(),bool,seconds=3);records['owned_alarm']=owned_alarm;save()
            assert child.wait(timeout=args.seconds+8)==0;records['rtcwake_exit']=0
        if args.phase=='stock':
            capability=subprocess.check_output(MANAGER+['org.freedesktop.login1.Manager.CanSuspend'],text=True).strip();assert capability=="('yes',)";records['can_suspend']=capability;records['stage']='requesting-actual-suspend';save()
            with (args.output/'suspend-request.log').open('w') as log:
                result=subprocess.run(MANAGER+['org.freedesktop.login1.Manager.Suspend','false'],stdout=log,stderr=subprocess.STDOUT,timeout=40)
            records['suspend_request_exit']=result.returncode;assert result.returncode==0
            await_value(stats,lambda s:s['success']>before['stats']['success'] or s['fail']>before['stats']['fail'],seconds=25)
        after=snapshot();records['after']=after;records['kernel_after']=kernel_lines();save()
        assert after['boot_id']==before['boot_id'] and after['active_vt']=='tty7'
        expected=[p for p in before['processes'] if p['uid']==10000 and p['name'] in ['phoc','phosh']]
        assert all(p in after['processes'] for p in expected),'Stock compositor/shell process changed'
        mono=after['clocks']['monotonic']-before['clocks']['monotonic'];boot=after['clocks']['boottime']-before['clocks']['boottime'];suspended=boot-mono
        records['elapsed']={'monotonic':mono,'boottime':boot,'suspended_seconds':suspended}
        if args.phase=='alarm':
            assert mono>=args.seconds-.5 and mono<args.seconds+8 and abs(suspended)<.5
            assert after['stats']['success']==before['stats']['success'] and after['stats']['fail']==before['stats']['fail']
            records['validation']={'passed':True,'rtc_alarm_wait_passed':True,'kernel_suspend_invoked':False,'same_stock_session_alive':True,'visual_input_check_complete':False}
        else:
            records['validation']={'passed':False,'rtc_alarm_wait_passed':False,'kernel_suspend_invoked':True,'same_stock_session_alive':True,'visual_input_check_complete':False}
            assert after['stats']['success']==before['stats']['success']+1 and after['stats']['fail']==before['stats']['fail']
            assert suspended>=3,'Kernel returned before a meaningful suspended interval; inspect wake source'
            records['validation']['passed']=True
        print('PASS:',args.phase,'RTC/suspend baseline; stock process identity preserved; inspect recorded timing/kernel evidence')
    finally:
        if child is not None and child.poll() is None:child.terminate();child.wait(timeout=3)
        current=(RTC/'wakealarm').read_text().strip()
        if current and current==owned_alarm:(RTC/'wakealarm').write_text('0')
        records['alarm_after_cleanup']=(RTC/'wakealarm').read_text().strip();records['cleanup_complete']=True
        save('status.json');(args.output/'running.json').unlink(missing_ok=True)


if __name__=='__main__':main()
