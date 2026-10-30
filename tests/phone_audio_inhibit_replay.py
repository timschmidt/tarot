"""Native audio sleep-block ownership and protected display idle; null audio only."""
import json
import os
from pathlib import Path
import pwd
import select
import subprocess
import sys

from lock_replay import any_request
from notification_replay import await_value
from phone_audio_state import devices, pactl, restore, snapshot
from phone_gesture_replay import request


def replay(socket, state, capture, before, directory, authenticate):
    assert os.geteuid()==0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text()
    assert not devices('sink-inputs') and not devices('source-outputs')
    import gi
    gi.require_version('Gio','2.0')
    from gi.repository import Gio
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
    original = snapshot(); idle = before['display_settings']['idle']
    panels=[p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip()=='connected']
    assert len(panels)==1
    def panel():
        return dict(dpms=(panels[0]/'dpms').read_text().strip(),
                    backlight_power=int(Path('/sys/class/backlight/backlight/bl_power').read_text()))
    account = pwd.getpwuid(10000); pid = before['pid']
    env = dict(field.decode().split('=',1) for field in Path('/proc',str(pid),'environ').read_bytes().split(b'\0') if b'=' in field)
    env.update(HOME=account.pw_dir,USER=account.pw_name,LOGNAME=account.pw_name,XDG_RUNTIME_DIR='/run/user/10000')
    owner = dict(user=10000,group=account.pw_gid,extra_groups=os.getgrouplist(account.pw_name,account.pw_gid))
    module = None; peers = []; logfiles = []
    sink = 'tarot_qa_awake_r57'
    result = dict(passed=False,physical_audio_used=False,recording_stream_opened=False,
                  recording_source='Owned null sink monitor',samples_inspected=False,
                  audio_playback_requested=False,playback_pcm='Zeros on owned null sink',suspend_called=False)
    def inhibitor_descriptors():
        found={}
        for fd in Path('/proc',str(pid),'fd').iterdir():
            try:
                target=os.readlink(fd)
                if target.startswith('/run/systemd/inhibit/') and target.endswith('.ref'):
                    fields=dict(line.split(':',1) for line in (fd.parent.parent/'fdinfo'/fd.name).read_text().splitlines() if ':' in line)
                    flags=int(fields['flags'].strip(),8)
                    assert flags & os.O_CLOEXEC
                    found[target]=dict(fd=int(fd.name),close_on_exec=True)
            except FileNotFoundError:continue
        return found
    def grants():
        rows=bus.call_sync('org.freedesktop.login1','/org/freedesktop/login1','org.freedesktop.login1.Manager',
                          'ListInhibitors',None,None,Gio.DBusCallFlags.NONE,5000,None).unpack()[0]
        found=[row for row in rows if row[1]=='Tarot' and row[2]=='Audio playback or recording is active']
        for row in found: assert row[0]=='sleep' and row[3]=='block' and row[4]==10000 and row[5]==pid
        assert len(found)<=1
        return found
    def held(label, playback, recording):
        value=state('audio-inhibit-'+label,lambda s:s['audio']['activity']==dict(playback=playback,recording=recording)
                    and s['audio_inhibition']['active']==bool(playback or recording)
                    and s['audio_inhibition']['held']==bool(playback or recording))
        rows=await_value(grants,lambda rows:len(rows)==int(bool(playback or recording)))
        result[label]={'activity':value['audio']['activity'],'grants':rows}
        return value
    class Peer:
        def __init__(self, recording=False):
            args=['python3',str(Path(__file__).parent/'pulse_stream_fixture.py'),'--server','unix:/run/user/10000/pulse/native',
                  '--device',sink+'.monitor' if recording else sink]
            if recording:args.append('--record')
            log=(directory/('audio-inhibit-stream-'+str(len(peers))+'.log')).open('x');logfiles.append(log)
            self.process=subprocess.Popen(args,env=env,**owner,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True)
            peers.append(self);assert self.read()['corked']
        def read(self):
            assert select.select([self.process.stdout],[],[],5)[0]
            line=self.process.stdout.readline();assert line,self.process.poll();return json.loads(line)
        def cork(self,value):
            self.process.stdin.write(json.dumps(dict(command='cork',value=value))+'\n');self.process.stdin.flush()
            assert self.read()==dict(event='corked',value=value)
        def close(self):
            if self.process.poll() is None:
                self.process.stdin.write('{"command":"exit"}\n');self.process.stdin.flush()
                assert self.process.wait(timeout=5)==0
    try:
        held('inactive',0,0)
        initial=state('audio-inhibit-manager-ready',lambda s:s['audio_inhibition']['available'])
        assert not grants() and not any(d['name']==sink for d in devices('sinks'))
        original_descriptors=inhibitor_descriptors()
        argument='sink_name='+sink
        module=int(pactl('load-module','module-null-sink',argument).strip())
        null=next(d for d in devices('sinks') if d['name']==sink)
        assert null['driver']=='module-null-sink.c'
        playback=Peer();held('corked',0,0)
        playback.cork(False);held('playback',1,0)
        result['audio_playback_requested']=True
        descriptors={k:v for k,v in inhibitor_descriptors().items() if k not in original_descriptors}
        assert len(descriptors)==1
        result['actual_owned_descriptor']=next(iter(descriptors.values()))
        request(socket,'system_menu',open=True);request(socket,'system_page',page='Power');capture('audio-inhibit-power-status')
        recording=Peer(True);result['recording_stream_opened']=True
        recording.cork(False);held('combined',1,1)
        playback.cork(True);held('recording-retains',0,1)
        recording.cork(True);held('all-corked',0,0)
        playback.cork(False);held('before-display-idle',1,0)
        request(socket,'system_menu',open=False)
        request(socket,'idle_timeouts',settings=dict(lock_after_seconds=15,off_after_seconds=15,screen_off_enabled=True))
        protected=state('audio-inhibit-panel-off',lambda s:s['lock']['phase']=='Locked' and s['lock']['idle']['output_off']
                        and s['lock']['generation']==s['lock']['last_presented_generation'],seconds=35)
        assert set(protected)=={'ok','pid','lock'} and 'audio_inhibition' not in json.dumps(protected)
        result['panel_off_driver']=await_value(panel,lambda value:value==dict(dpms='Off',backlight_power=4))
        assert grants(), 'Real manager must retain the audio sleep block during display off'
        result['panel_off_with_grant']=True;result['protected_status_redacted']=True
        playback.cork(True);await_value(grants,lambda rows:not rows)
        result['last_stream_releases_while_panel_off']=True
        authenticate('audio-inhibit-pam');result['actual_pam']=True
        result['panel_on_driver']=await_value(panel,lambda value:value==dict(dpms='On',backlight_power=0))
        request(socket,'idle_timeouts',settings=idle)
        state('audio-inhibit-idle-restored',lambda s:s['display_settings']['idle']==idle and s['display_settings']['settings_saved'])
        held('after-authentication',0,0)
        # Exit an active client without corking first; server removal must also
        # release the block, not rely solely on explicit UI/peer pause commands.
        playback.cork(False);held('before-client-exit',1,0)
        playback.close();held('client-exit',0,0)
        result['passed']=True
        return result
    finally:
        failed=sys.exc_info()[0] is not None
        if failed:result['passed']=False
        try:
            for peer in peers:
                try:peer.close()
                finally:
                    if peer.process.poll() is None:peer.process.terminate();peer.process.wait(timeout=5)
            for log in logfiles:log.close()
            if module is not None:
                matches=[line.split('\t') for line in pactl('list','short','modules').splitlines() if line.split('\t')[0]==str(module)]
                assert len(matches)==1 and matches[0][1]=='module-null-sink' and matches[0][2].split()==['sink_name='+sink]
                pactl('unload-module',module)
            assert not any(d['name']==sink for d in devices('sinks'))
            result['controls_after']=restore(original);result['exact_audio_controls_restored']=result['controls_after']==original
            assert not devices('sink-inputs') and not devices('source-outputs')
            await_value(grants,lambda rows:not rows)
            result['no_remaining_audio_block']=True
        except BaseException:
            result['passed']=False
            raise
        finally:
            (directory/'audio-inhibit-result.json').write_text(json.dumps(result,indent=2)+'\n')
