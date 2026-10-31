#!/usr/bin/env python3
"""Verify a packaged Tarot session already authenticated through normal greetd.

Root creates temporary input and observes/ends only this verified spare-phone
session. Graphics, keyboard, locker, PAM worker and applications remain ordinary
user processes. No synthetic PAM result, greeter policy or root seatd is used.
"""
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess
import tempfile
import time
from notification_replay import await_value
from phone_gesture_replay import Touchscreen, request
from phone_touch_keyboard import Keyboard as TouchKeyboard
from phone_stock_login import processes
from phone_suspend_probe import RTC, clocks, stats
from lock_replay import any_request


def run(args, password):
    socket = Path('/run/user/10000/tarot-session/control.sock')
    initial = request(socket, 'status')
    pid = initial['pid']
    exe = Path(f'/proc/{pid}/exe')
    assert exe.resolve() == Path('/usr/bin/tarot') and exe.parent.stat().st_uid == 10000
    identity = (Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
    cgroup = Path(f'/proc/{pid}/cgroup').read_text()
    binaries = {name: hashlib.sha256(Path('/usr/bin', name).read_bytes()).hexdigest()
                for name in ['tarot', 'tarot-session', 'tarot-lock', 'tarot-keyboard']}
    records = {'initial': initial, 'compositor_cgroup': cgroup, 'cycles': [], 'package_owner': subprocess.check_output(['apk', 'info', '--who-owns', '/usr/bin/tarot-session'], text=True).strip(), 'binary_sha256': binaries['tarot'], 'binaries_sha256': binaries,
               'boot_id': Path('/proc/sys/kernel/random/boot_id').read_text().strip(),
               'controller_pid': os.getpid(),
               'replay_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    screen = None
    owned_alarm = None
    typing = False
    last_input = None
    sensor_monitor = None
    timeline = (args.output/'timing.jsonl').open('x', buffering=1)
    os.chmod(args.output/'timing.jsonl', 0o600)
    os.chown(args.output/'timing.jsonl', 10000, pwd.getpwnam('user').pw_gid)
    resource_config = Path('/home/user/.config/tarot/settings.toml')
    resource_original = None
    resource_metadata = None
    resource_parent_absent = False
    if getattr(args, 'resource_sample', False):
        assert not resource_config.is_symlink() and not resource_config.parent.is_symlink()
        resource_parent_absent = not resource_config.parent.exists()
        if resource_config.exists():
            resource_metadata = resource_config.stat()
            assert resource_config.is_file() and resource_metadata.st_uid == 10000 and resource_metadata.st_size <= 65536
            resource_original = resource_config.read_bytes()
            backup = args.output/'original-settings.toml'
            backup.write_bytes(resource_original)
            os.chmod(backup, 0o600)
            os.chown(backup, 10000, pwd.getpwnam('user').pw_gid)
        records['resource_settings_original_absent'] = resource_original is None

    def mark(event, **fields):
        timeline.write(json.dumps({'event':event, 'clocks':clocks(), 'process_cpu_seconds':time.process_time(), **fields})+'\n')

    def save():
        mark('save-start')
        # Serializing the accumulated report can exceed the card edge dwell.
        # Preserve held-contact observations in memory, then checkpoint them
        # after release. The final report still includes them on failure.
        if screen is not None and screen.active:
            mark('save-deferred-while-contact', contacts=len(screen.active))
            return
        payload = json.dumps(records, indent=2)+'\n'
        mark('save-serialized', bytes=len(payload))
        (args.output/'running.json').write_text(payload)
        mark('save-written')

    def state(label, condition=lambda s: True, seconds=25):
        started = clocks()
        records['pending_observation'] = {'label': label, 'started': started}
        value = await_value(lambda: request(socket, 'status'), condition, seconds=seconds)
        assert value['pid'] == pid
        records[label] = value
        records.setdefault('observation_clocks', {})[label] = {'started': started, 'completed': clocks()}
        records.pop('pending_observation')
        save()
        return value

    def alive():
        try:
            return exe.resolve() == Path('/usr/bin/tarot') and Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19] == identity
        except (FileNotFoundError, ProcessLookupError):
            return False

    def signal_current(number):
        descriptor = None
        try:
            descriptor = os.pidfd_open(pid)
            if not alive():return False
            signal.pidfd_send_signal(descriptor, 0)
            signal.pidfd_send_signal(descriptor, number)
            return True
        except (FileNotFoundError, ProcessLookupError):return False
        finally:
            if descriptor is not None:os.close(descriptor)

    def guard():
        assert Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty7' and alive()
        value = request(socket, 'status')
        assert value['pid'] == pid
        if typing:
            assert value['lock']['phase'] == 'Locked', 'Stop credential input if protection changes'
            geometry = value['lock'].get('output')
            assert geometry is None or geometry['orientation']=='normal', 'Stop fixed-layout credential input if the phone rotates'

    def tap(x, y):
        nonlocal last_input
        screen.down(x, y)
        screen.up()
        last_input = clocks()
        time.sleep(.08)

    def protected(label):
        return state(label, lambda s: s['lock']['phase'] == 'Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation'] == s['lock']['last_presented_generation'] and s['lock']['sleep']['armed'] and not s['lock']['sleep']['initializing'] and not s['lock']['sleep']['rearming'] and s['lock']['sleep']['gate']['barrier'] is None)

    def authenticate(label):
        nonlocal typing
        # A completed display-off intentionally presents compositor black,
        # rather than a locker buffer. Wake from verified protection first;
        # require the full current locker presentation before credentials.
        # A replacement owner arriving while the panel is off cannot present
        # until wake, so it remains protected in Preparing until the touch.
        covered = state(label+'-wake-ready', lambda s: (s['lock']['phase']=='Locked'
                        or s['lock']['phase'] in ['Awaiting','Preparing'] and s['lock']['idle']['output_off']
                        and s['lock']['locker'] is not None and s['lock']['locker']['pid'] is not None
                        and not s['lock']['locker']['restart_pending'])
                        and s['lock']['sleep']['armed'] and not s['lock']['sleep']['initializing']
                        and not s['lock']['sleep']['rearming'] and s['lock']['sleep']['gate']['barrier'] is None)
        if covered['lock']['idle']['output_off']:
            # A session may idle while the operator answers getpass. Consume a
            # neutral wake contact and wait for its new protected completion
            # before the unlock puck or any credential input is touched.
            tap(180,350)
            covered = state(label+'-panel-woken', lambda s: s['lock']['phase']=='Locked' and not s['lock']['idle']['output_off'] and s['lock']['generation']==s['lock']['last_presented_generation'])
        covered = protected(label+'-protected')
        covered = state(label+'-portrait-ready', lambda s: s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'] and (s['lock'].get('output') is None or (s['lock']['output']['orientation']=='normal' and s['lock']['output']['requested'] is None and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])))
        typing = True
        tap(180, 608)
        state(label+'-entry-repaint', lambda s: s['lock']['presented_frames'] > covered['lock']['presented_frames'])
        TouchKeyboard(lambda x, y: tap(x, y+28)).type_ascii(password)
        # Submission can unlock before its release; no more credential text is
        # emitted after this point. Ordinary input still waits for actual KMS.
        typing = False
        tap(180, 330)
        return state(label+'-ordinary', lambda s: s['lock']['phase'] == 'Unlocked' and s['lock']['unprotected_presentations'] > covered['lock']['unprotected_presentations'])

    def capture(label):
        request(socket, 'capture', path=str(args.output/(label+'.png')))
        state(label, lambda s: not s['capture_pending'] and not s['capture_error'])

    def clear_alarm():
        nonlocal owned_alarm
        if owned_alarm and (RTC/'wakealarm').read_text().strip() == owned_alarm:
            (RTC/'wakealarm').write_text('0')
        owned_alarm = None

    idle_observer = None
    idle_mode = getattr(args, 'session_idle_test', None)
    assert not idle_mode or args.cycles == 0 and not args.crash
    save()
    try:
        if idle_mode:
            from phone_session_idle_replay import Observer
            idle_observer = Observer(pid, 'suspend' if idle_mode == 'automatic' else 'ignore')
        initial = protected('startup-protected')
        assert initial['lock']['first_presented_protected'] and initial['lock']['unprotected_presentations'] == 0
        assert not Path('/run/seatd.sock').exists()
        assert any_request(socket, 'capture', path=str(args.output/'denied.png')) == {'ok': False, 'error': 'Session is locked'}
        if getattr(args, 'observe_sensors', False):
            from phone_sensor_claims import SensorClaims
            sensor_monitor = SensorClaims(pid)
        if getattr(args, 'rotate_protected', False):
            ordinary_frames = initial['lock']['unprotected_presentations']
            for orientation in ['left-up','bottom-up','right-up','normal']:
                request(socket, 'rotate', orientation=orientation)
                rotated = state('protected-rotation-'+orientation, lambda s: s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['output']['orientation']==orientation and s['lock']['output']['requested'] is None and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])
                assert rotated['lock']['unprotected_presentations']==ordinary_frames
                assert any_request(socket,'capture',path=str(args.output/('denied-'+orientation+'.png'))) == {'ok':False,'error':'Session is locked'}
        screen = Touchscreen(guard)
        authenticate('startup')
        if args.require_audio:
            state('audio-ready', lambda s: s['audio']['available'] and bool(s['audio']['outputs']))
        if getattr(args, 'keyboard_context', False):
            from phone_keyboard_reactivation_replay import replay as keyboard_context_replay
            records['keyboard_context'] = keyboard_context_replay(socket,screen,tap,state,capture,args.output,cgroup,authenticate,require_remap=False)
        if getattr(args, 'keyboard_reactivation', False):
            from phone_keyboard_reactivation_replay import replay as keyboard_reactivation_replay
            records['keyboard_reactivation'] = keyboard_reactivation_replay(socket,screen,tap,state,capture,args.output,cgroup,authenticate)
        if getattr(args, 'browser', False):
            from phone_browser_replay import replay as browser_replay
            records['browser'] = browser_replay(socket,tap,state,capture,args.output,cgroup,authenticate,getattr(args,'browser_inspect',False),getattr(args,'browser_focus_cycles',0))
        if getattr(args, 'transients', False):
            from phone_transient_replay import replay as transient_replay
            records['transients'] = transient_replay(socket, screen, guard, state,
                authenticate, args.output, cgroup,
                fixture_binary=getattr(args,'transient_fixture_binary',None),
                fixture_audit=getattr(args,'transient_fixture_audit',None))
        if getattr(args, 'notification_well', False):
            from phone_transient_replay import replay as notification_well_replay
            records['notification_well'] = notification_well_replay(socket, screen, guard, state,
                authenticate, args.output, cgroup, notifications=True)
        if getattr(args, 'toolkit_dialogs', False):
            from phone_toolkit_dialog_replay import replay as toolkit_replay
            records['toolkit_dialogs'] = toolkit_replay(socket, tap, state, capture,
                args.output, cgroup, authenticate)
        if getattr(args, 'terminal_clipboard', False):
            from phone_terminal_clipboard_replay import replay as terminal_replay
            records['terminal_clipboard'] = terminal_replay(socket, tap, guard, state,
                capture, args.output, cgroup, authenticate)
        if getattr(args, 'virtual_keyboard_lock', False):
            folder=args.output/'virtual-keyboard-lock'
            assert not request(socket,'status')['cards']
            request(socket,'launch',id='test.tarot.VirtualLock.desktop')
            result=await_value(lambda:json.loads((folder/'result.json').read_text()) if (folder/'result.json').exists() else None,bool,seconds=110)
            assert result['passed'] and result['uid']==10000 and result['cgroup']==cgroup
            assert result['audit']['compositor']==binaries['tarot']
            state('virtual-keyboard-lock-clients-gone',lambda s:not s['cards'])
            records['virtual_keyboard_lock']=result
        if getattr(args, 'fullscreen_terminal', False):
            from phone_fullscreen_terminal_replay import replay as fullscreen_terminal
            records['fullscreen_terminal'] = fullscreen_terminal(socket,tap,state,capture,args.output,cgroup,authenticate)
        catalog = request(socket, 'applications')['applications']
        # The GTK demo deliberately has NoDisplay=true and is correctly absent
        # from the normal launcher. Exercise an actual visible installed app.
        app = next(a for a in catalog if a['id'] == 'org.gnome.Calculator.desktop')
        request(socket, 'launch', id=app['id'])
        before = state('native-app', lambda s: len(s['cards']) == 1 and isinstance(s['view'], dict) and 'App' in s['view'])
        candidates = []
        for proc in Path('/proc').iterdir():
            if not proc.name.isdecimal():
                continue
            try:
                if proc.stat().st_uid == 10000 and (proc/'exe').resolve() == Path('/usr/bin/gnome-calculator'):
                    candidates.append({'pid': int(proc.name), 'cgroup': (proc/'cgroup').read_text()})
            except (FileNotFoundError, ProcessLookupError):
                pass
        assert candidates and all(p['cgroup'] == cgroup for p in candidates)
        records['native_app_processes'] = candidates
        capture('app-before')
        if getattr(args, 'headphones', False):
            from phone_headphones_replay import replay as headphones_replay
            records['headphones'] = headphones_replay(socket,screen,tap,state,capture,before,args.output,authenticate)
        if getattr(args, 'authorization', False):
            from phone_authorization_replay import replay as authorization_replay
            records['authorization'] = authorization_replay(socket,screen,tap,state,capture,before,args.output,cgroup,authenticate,password)
        if getattr(args, 'bluetooth_peer', None):
            from phone_bluetooth_pair_replay import replay as bluetooth_pair_replay
            records['bluetooth_pair'] = bluetooth_pair_replay(socket,tap,state,capture,before,args.output,args.bluetooth_peer,screen)
        if getattr(args, 'media_app', False):
            from phone_media_app_replay import replay as media_app_replay
            records['media_app'] = media_app_replay(socket,state,capture,tap,before,args.output)
        if getattr(args, 'waydroid', False):
            from phone_waydroid_replay import replay as waydroid_replay
            records['waydroid'] = waydroid_replay(socket,state,capture,before,args.output)
        if getattr(args, 'media', False):
            from phone_media_replay import replay as media_replay
            records['media'] = media_replay(socket,state,capture,tap,before,args.output)
        if getattr(args, 'showcase', False):
            from phone_showcase_replay import replay as showcase_replay
            records['showcase'] = showcase_replay(socket,state,capture,before,args.output)
        if getattr(args, 'keyring', False):
            from phone_keyring_replay import replay as keyring_replay
            records['keyring'] = keyring_replay(socket,state,before,args.output,authenticate)
        if getattr(args, 'boot_destination', False):
            from phone_boot_destination_replay import replay as boot_destination_replay
            records['boot_destination'] = boot_destination_replay(socket,screen,tap,state,capture,before,args.output,password)
        if getattr(args, 'network_authorization', False):
            from phone_network_authorization_replay import replay as network_authorization_replay
            records['network_authorization'] = network_authorization_replay(socket,screen,tap,state,capture,before,args.output,cgroup,authenticate,password)
        if getattr(args, 'audio_profiles', False):
            from phone_audio_profiles_replay import replay as audio_profiles_replay
            records['audio_profiles'] = audio_profiles_replay(socket,screen,tap,state,capture,before,args.output,cgroup,authenticate,getattr(args,'audio_profiles_save',False))
        if getattr(args, 'audio_inhibit', False):
            from phone_audio_inhibit_replay import replay as audio_inhibit_replay
            records['audio_inhibit'] = audio_inhibit_replay(socket,state,capture,before,args.output,authenticate)
        if getattr(args, 'audio_input', False):
            from phone_audio_input_replay import replay as audio_input_replay
            records['audio_input'] = audio_input_replay(socket,screen,tap,state,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'feedback', False):
            from phone_feedback_replay import replay as feedback_replay
            records['feedback'] = feedback_replay(socket,screen,tap,state,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'native_menus', False):
            from phone_native_menu_replay import replay as native_menu_replay
            records['native_menus'] = native_menu_replay(socket,screen,state,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'bluetooth', False):
            from phone_bluetooth_replay import replay as bluetooth_replay
            records['bluetooth'] = bluetooth_replay(socket,tap,state,capture,before,args.output,authenticate)
        if getattr(args, 'text_enable_reset', False):
            from phone_text_enable_reset_replay import replay as text_reset_replay
            records['text_enable_reset'] = text_reset_replay(socket,tap,state,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'native_menu_claims', False) or getattr(args, 'native_menu_traffic', False):
            from phone_native_menu_claim_replay import replay as claim_replay
            records['native_menu_claims'] = claim_replay(socket,screen,state,capture,before,args.output,cgroup,authenticate,guard,traffic=getattr(args,'native_menu_traffic',False))
        if getattr(args, 'menus', False):
            from phone_menus_replay import replay as menu_replay
            records['menus'] = menu_replay(socket,screen,state,tap,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'just_type', False):
            from phone_just_type_replay import replay as search_replay
            records['just_type'] = search_replay(socket,screen,state,tap,capture,before,args.output,cgroup,authenticate)
        if getattr(args, 'favorite_drag', False):
            from phone_favorite_drag_replay import replay as favorite_replay
            records['favorite_drag'] = favorite_replay(socket,screen,state,tap,capture,before,args.output,authenticate)
        if getattr(args, 'quick_launch', False):
            from phone_quick_launch_replay import replay as quick_replay
            records['quick_launch'] = quick_replay(socket, screen, state, tap, capture, before, args.output)
        if getattr(args, 'rounded_cards', False):
            from phone_rounded_cards_replay import replay as cards_replay
            records['rounded_cards'] = cards_replay(socket, screen, state, tap, capture, before, args.output, cgroup)
        if getattr(args, 'stack_matrix', False):
            from phone_stack_matrix_replay import replay as matrix_replay
            records['stack_matrix'] = matrix_replay(socket, screen, state, capture, before, args.output, cgroup)
        if getattr(args, 'gtk_close', False):
            from phone_gtk_close_replay import replay as gtk_replay
            records['gtk_close'] = gtk_replay(socket, screen, guard, tap, state, capture, before, args.output, cgroup)
        if getattr(args, 'hardware_keys', False):
            from phone_hardware_replay import replay
            records['hardware_keys'] = replay(socket, pid, screen, guard, tap, state, authenticate,
                                             capture, before, args.output)
        if getattr(args, 'launcher_pages', False):
            from phone_launcher_pages_replay import replay as pages_replay
            records['launcher_pages'] = pages_replay(socket,screen,state,tap,capture,before,args.output,authenticate)
        if getattr(args, 'native_icons', False):
            from phone_native_icons_replay import replay as icons_replay
            records['native_icons'] = icons_replay(socket,screen,state,tap,capture,before,args.output,authenticate)
        if getattr(args, 'panel_off', False):
            assert sensor_monitor is not None
            state('sensor-before-panel', lambda s:s['sensors']['available'] and s['sensors']['claimed'])
            # Check observer correlation while ordinary diagnostics can confirm
            # accepted claim state independently of the service's reply header.
            request(socket, 'rotation_lock', locked=True)
            state('sensor-lock-released', lambda s:not s['sensors']['claimed'])
            await_value(sensor_monitor.snapshot, lambda es:any(e['member']=='ReleaseAccelerometer' and e['reply']=='success' for e in es))
            request(socket, 'rotation_lock', locked=False)
            state('sensor-lock-reclaimed', lambda s:s['sensors']['claimed'] and s['output']['orientation']=='normal')
            await_value(sensor_monitor.snapshot, lambda es:any(e['member']=='ClaimAccelerometer' and e['reply']=='success' for e in es))
            off_after = time.monotonic()
            idle = state('sensor-panel-off', lambda s:s['lock']['idle']['output_off'] and s['lock']['idle']['policy']['phase']=='Off', seconds=200)
            assert idle['lock']['phase']=='Locked' and idle['lock']['last_presented_surface'] is True
            panels = [p.parent for p in Path('/sys/class/drm').glob('card*-DSI-*/status') if p.read_text().strip()=='connected']
            assert len(panels)==1

            def panel():
                return {'dpms':(panels[0]/'dpms').read_text().strip(),
                        'backlight_power':int(Path('/sys/class/backlight/backlight/bl_power').read_text())}

            records['sensor-panel-off-driver'] = await_value(panel, lambda s:s['dpms']=='Off' and s['backlight_power']==4)
            await_value(sensor_monitor.snapshot, lambda es:any(e['member']=='ReleaseAccelerometer' and e['reply']=='success' and e['requested_monotonic']>=off_after for e in es))
            if getattr(args, 'resource_sample', False):
                # Measure after actual PAM, native app rendering and capture,
                # retaining the same app across the normal idle/wake cycle.
                sampler = Path(__file__).with_name('phone_resource_sample.py')
                output = args.output/'locked-off-resource-sample'
                mark('resource-sample-start')
                subprocess.run(['python3', str(sampler), '--session', 'tarot', '--output', str(output),
                                '--label', 'locked-panel-off-after-app', '--seconds', '300', '--interval', '5'], check=True, timeout=360)
                sample_path = output/'status.json'
                sample = json.loads(sample_path.read_text())
                assert sample['collection_complete'] and sample['sampler_sha256']==hashlib.sha256(sampler.read_bytes()).hexdigest()
                records['locked-off-resources'] = {'sampler_sha256': sample['sampler_sha256'],
                    'record_sha256': hashlib.sha256(sample_path.read_bytes()).hexdigest(),
                    'warmed_with_actual_pam_and_native_app': True, 'sample_count': len(sample['samples'])}
                save()
                mark('resource-sample-complete')
            wake_after = time.monotonic()
            tap(180, 608)
            state('sensor-panel-woken-protected', lambda s:s['lock']['phase']=='Locked' and not s['lock']['idle']['output_off'] and s['lock']['last_presented_surface'] is True and s['lock']['generation']==s['lock']['last_presented_generation'])
            records['sensor-panel-on-driver'] = await_value(panel, lambda s:s['dpms']=='On' and s['backlight_power']==0)
            await_value(sensor_monitor.snapshot, lambda es:any(e['member']=='ClaimAccelerometer' and e['reply']=='success' and e['requested_monotonic']>=wake_after for e in es))
            restored = authenticate('sensor-panel-wake')
            assert restored['cards']==before['cards'] and restored['view']==before['view']
            state('sensor-after-panel', lambda s:s['sensors']['available'] and s['sensors']['claimed'])
            if args.require_audio:
                state('audio-after-panel', lambda s:s['audio']['available'] and bool(s['audio']['outputs']))
            capture('app-after-panel')
        for index in range(1, args.cycles+1):
            mark('cycle-preflight', cycle=index)
            current = request(socket, 'status')
            if current['lock']['phase'] != 'Unlocked':
                policy = current['lock']['idle']['policy']
                threshold = policy['lock_after']['secs']+policy['lock_after']['nanos']/1e9
                elapsed = clocks()['monotonic']-last_input['monotonic']
                locker = current['lock']['locker']
                # A controller can pause long enough for the real idle policy
                # to lock correctly. Require fresh PAM; never retry unexplained
                # protection, coordinator failure or locker recovery here.
                assert elapsed >= threshold and current['lock']['phase'] in ['Awaiting','Locked']
                assert current['lock']['sleep']['armed'] and not current['lock']['sleep']['gate']['preparing']
                assert locker['consecutive_failures']==0 and locker['recoveries']==0 and not locker['error']
                records.setdefault('extra_idle_authentications', []).append({'before_cycle':index,'controller_inactive_seconds':elapsed,'policy_seconds':threshold,'protected':current})
                restored = authenticate(f'cycle-{index:02}-idle')
                assert restored['cards']==before['cards'] and restored['view']==before['view']
                if args.require_audio:
                    state(f'cycle-{index:02}-idle-audio', lambda s:s['audio']['available'] and bool(s['audio']['outputs']))
            mark('cycle-awake', cycle=index)
            assert not (RTC/'wakealarm').read_text().strip()
            cycle = {'number': index, 'before_clocks': clocks(), 'before_stats': stats(), 'before': request(socket, 'status')}
            records['cycles'].append(cycle)
            folder = args.output/f'cycle-{index:02}'
            folder.mkdir(mode=0o700)
            os.chown(folder, 10000, pwd.getpwnam('user').pw_gid)
            with (folder/'rtcwake.log').open('w') as log:
                mark('rtc-program-start', cycle=index)
                subprocess.run(['rtcwake', '-u', '-d', '/dev/rtc0', '-m', 'no', '-s', '15'], check=True, stdout=log, stderr=log, timeout=4)
                mark('rtc-program-complete', cycle=index)
            owned_alarm = (RTC/'wakealarm').read_text().strip()
            assert owned_alarm
            save()
            with (folder/'observer.log').open('w') as log:
                subprocess.run(['/home/user/tarot-dev/bin/suspend-request', '--control-socket', str(socket), '--pid', str(pid), '--report', str(folder/'observer.jsonl')], check=True, stdout=log, stderr=log, timeout=50)
            os.chown(folder/'observer.jsonl', 10000, pwd.getpwnam('user').pw_gid)
            cycle['after_stats'] = stats()
            after = clocks()
            cycle['suspended_seconds'] = after['boottime']-cycle['before_clocks']['boottime']-(after['monotonic']-cycle['before_clocks']['monotonic'])
            assert cycle['suspended_seconds'] >= 3 and cycle['after_stats']['success'] == cycle['before_stats']['success']+1 and cycle['after_stats']['fail'] == cycle['before_stats']['fail']
            clear_alarm()
            resumed = protected(f'cycle-{index:02}-resume')
            assert resumed['lock']['sleep']['gate']['epoch'] == cycle['before']['lock']['sleep']['gate']['epoch']
            restored = authenticate(f'cycle-{index:02}')
            assert restored['cards'] == before['cards'] and restored['view'] == before['view']
            if args.require_audio:
                state(f'cycle-{index:02}-audio-ready', lambda s: s['audio']['available'] and bool(s['audio']['outputs']))
            capture(f'app-after-{index:02}')
            cycle['validated'] = True
            save()
            mark('cycle-validated', cycle=index)
        if idle_mode:
            if idle_mode == 'automatic':
                from phone_session_idle_replay import automatic_replay
                records['session_idle'] = automatic_replay(socket,state,before,args.output,guard,authenticate,idle_observer)
            else:
                from phone_session_idle_replay import replay as session_idle_replay
                records['session_idle'] = session_idle_replay(socket,state,before,args.output,guard,authenticate,idle_mode,idle_observer)
        records['before-end'] = request(socket, 'status')
        if idle_observer is not None:
            idle_observer.before_exit(idle_mode != 'automatic')
        if sensor_monitor is not None:
            records['sensor_claims'] = sensor_monitor.snapshot()
            sensor_monitor.close()
            sensor_monitor = None
        screen.close()
        screen = None
        if args.crash:
            guard()
            assert signal_current(signal.SIGKILL)
        else:
            request(socket, 'exit')
        after = await_value(processes, lambda ps: not alive() and any(p['uid'] == pwd.getpwnam('greetd').pw_uid and p['name'] == 'phrog' for p in ps), seconds=35)
        records['after-end'] = after
        remaining = []
        for proc in Path('/proc').iterdir():
            if not proc.name.isdecimal():
                continue
            try:
                if (proc/'cgroup').read_text() == cgroup:
                    remaining.append({'pid': int(proc.name), 'name': (proc/'comm').read_text().strip()})
            except (FileNotFoundError, ProcessLookupError):
                pass
        records['remaining_session_processes'] = remaining
        assert not remaining and Path('/sys/class/tty/tty0/active').read_text().strip() == 'tty7'
        assert not socket.exists(), 'Supervisor must remove its diagnostic socket before ending the login session'
        if idle_observer is not None:
            try:
                records['session_idle_observation'] = idle_observer.finish(require_teardown=True, expect_true=idle_mode != 'automatic')
            finally:idle_observer=None
        records['validation'] = {'passed': True, 'packaged_greetd_session': True, 'root_seatd_used': False, 'protected_startup_and_actual_pam': True, 'native_app_in_same_login_session': True, 'actual_kernel_suspend_cycles': args.cycles, 'audio_ready_checked':args.require_audio, 'compositor_killed': args.crash, 'returned_to_phrog': True, 'login_cgroup_empty': True, 'diagnostic_socket_removed': True, 'native_panel_off_sensor_release_reclaim':getattr(args,'panel_off',False), 'manual_sensor_tested': False, 'password_logged': False}
        print('PASS: installed unprivileged login/session, native app, real PAM and greeter recovery; actual suspend cycles:', len(records['cycles']), flush=True)
    except BaseException as error:
        records['validation'] = {'passed': False, 'exception_type': type(error).__name__}
        # Preserve evidence before cleanup destroys a stalled process. Never
        # retry a failed request or silently extend the responsiveness gate.
        diagnosis = {'clocks': clocks(), 'compositor_alive': alive(), 'threads': [], 'power': {}}
        if diagnosis['compositor_alive']:
            try:
                threads = list(Path(f'/proc/{pid}/task').iterdir())[:256]
            except OSError as failure:
                threads = []
                diagnosis['thread_inspection_errno'] = failure.errno
            for thread in threads:
                entry = {'tid': int(thread.name)}
                for name in ['comm', 'stat', 'wchan', 'stack']:
                    try:
                        entry[name] = (thread/name).read_text()[:8192]
                    except OSError as failure:
                        entry[name] = {'errno': failure.errno}
                diagnosis['threads'].append(entry)
        for supply in Path('/sys/class/power_supply').iterdir():
            values = {}
            for name in ['capacity', 'status', 'voltage_now', 'current_now', 'input_current_limit', 'current_max', 'temp']:
                try:
                    values[name] = (supply/name).read_text().strip()
                except OSError:
                    pass
            diagnosis['power'][supply.name] = values
        records['failure_diagnosis'] = diagnosis
        save()
        raise
    finally:
        password = None
        if idle_observer is not None:
            try:records['session_idle_observation_after_failure'] = idle_observer.finish()
            except BaseException as error:
                records['session_idle_observer_cleanup_error'] = type(error).__name__
                records.setdefault('validation', {})['passed'] = False
            finally:idle_observer=None
        if sensor_monitor is not None:
            try:
                records['sensor_claims'] = sensor_monitor.snapshot()
            except BaseException as error:
                records['sensor_monitor_cleanup_observation_error'] = type(error).__name__
            finally:
                try:
                    sensor_monitor.close()
                except BaseException as error:
                    records['sensor_monitor_cleanup_error'] = type(error).__name__
        if owned_alarm:
            while time.time() < int(owned_alarm)+3:
                time.sleep(.1)
            clear_alarm()
        if screen is not None:
            screen.close()
        if alive():
            signal_current(signal.SIGTERM)
        if getattr(args, 'resource_sample', False):
            try:
                def remaining_group():
                    members = []
                    for proc in Path('/proc').iterdir():
                        if not proc.name.isdecimal(): continue
                        try:
                            if (proc/'cgroup').read_text() == cgroup: members.append(int(proc.name))
                        except (FileNotFoundError, ProcessLookupError): pass
                    return members
                await_value(remaining_group, lambda members:not members, seconds=35)
                assert not resource_config.is_symlink()
                if resource_original is None:
                    resource_config.unlink(missing_ok=True)
                    if resource_parent_absent and resource_config.parent.exists(): resource_config.parent.rmdir()
                else:
                    fd, name = tempfile.mkstemp(prefix='.tarot-qa-restore-', dir=resource_config.parent)
                    temporary = Path(name)
                    try:
                        with os.fdopen(fd, 'wb') as stream:
                            os.fchmod(stream.fileno(), resource_metadata.st_mode & 0o777)
                            os.fchown(stream.fileno(), resource_metadata.st_uid, resource_metadata.st_gid)
                            stream.write(resource_original); stream.flush(); os.fsync(stream.fileno())
                        os.replace(temporary, resource_config)
                    finally:
                        temporary.unlink(missing_ok=True)
                records['resource_settings_restored'] = (not resource_config.exists()) if resource_original is None else resource_config.read_bytes()==resource_original
                assert records['resource_settings_restored']
            except BaseException as error:
                records['resource_settings_restore_error'] = type(error).__name__
                records.setdefault('validation', {})['passed'] = False
        records['alarm_after'] = (RTC/'wakealarm').read_text().strip()
        (args.output/'status.json').write_text(json.dumps(records, indent=2)+'\n')
        (args.output/'running.json').unlink(missing_ok=True)
        timeline.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cycles', type=int, default=1)
    parser.add_argument('--crash', action='store_true')
    parser.add_argument('--require-audio', action='store_true')
    parser.add_argument('--rotate-protected', action='store_true')
    parser.add_argument('--observe-sensors', action='store_true')
    parser.add_argument('--panel-off', action='store_true')
    parser.add_argument('--resource-sample', action='store_true')
    parser.add_argument('--hardware-keys', action='store_true')
    parser.add_argument('--quick-launch', action='store_true')
    parser.add_argument('--favorite-drag', action='store_true')
    parser.add_argument('--launcher-pages', action='store_true')
    parser.add_argument('--rounded-cards', action='store_true')
    parser.add_argument('--stack-matrix', action='store_true')
    parser.add_argument('--gtk-close', action='store_true')
    parser.add_argument('--transients', action='store_true')
    parser.add_argument('--transient-fixture-binary', type=Path)
    parser.add_argument('--transient-fixture-audit', type=Path)
    parser.add_argument('--notification-well', action='store_true')
    parser.add_argument('--toolkit-dialogs', action='store_true')
    parser.add_argument('--terminal-clipboard', action='store_true')
    parser.add_argument('--browser', action='store_true')
    parser.add_argument('--browser-inspect', action='store_true')
    parser.add_argument('--keyboard-reactivation',action='store_true')
    parser.add_argument('--browser-focus-cycles',type=int,choices=range(21),default=0,
                        help='Additional native overview-return input cycles for browser diagnosis')
    parser.add_argument('--fullscreen-terminal', action='store_true')
    parser.add_argument('--virtual-keyboard-lock', action='store_true')
    parser.add_argument('--keyboard-context', action='store_true')
    parser.add_argument('--native-icons', action='store_true')
    parser.add_argument('--just-type', action='store_true')
    parser.add_argument('--menus', action='store_true')
    parser.add_argument('--native-menus', action='store_true')
    parser.add_argument('--native-menu-claims', action='store_true')
    parser.add_argument('--native-menu-traffic', action='store_true')
    parser.add_argument('--bluetooth', action='store_true')
    parser.add_argument('--authorization', action='store_true')
    parser.add_argument('--bluetooth-peer', help='Address of an owner-provided test peer for real pairing by touch')
    parser.add_argument('--media-app', action='store_true', help='GNOME Decibels as a real MPRIS player on a null sink')
    parser.add_argument('--media', action='store_true', help='Media controls by touch against a private MPRIS fixture')
    parser.add_argument('--showcase', action='store_true', help='Capture ordinary screens for documentation')
    parser.add_argument('--keyring', action='store_true', help='Read the login keyring lock before/after a real Tarot unlock')
    parser.add_argument('--boot-destination', action='store_true', help='At startup by finger through the real Polkit agent; restores the login screen')
    parser.add_argument('--network-authorization', action='store_true', help='Real Wi-Fi scan authorization with a temporary restrictive process-scoped policy; no radio/profile changes')
    parser.add_argument('--audio-profiles', action='store_true', help='Card modes, Off recovery, bounded server pause/reconnect, label touch and PAM without streams')
    parser.add_argument('--audio-inhibit', action='store_true', help='Real user audio sleep-block ownership, null streams, protected panel off/PAM and exact restoration; no Suspend call')
    parser.add_argument('--audio-profiles-save', action='store_true', help='Leave the selected test mode for a controller-owned saved-login check and exact restoration')
    parser.add_argument('--audio-input', action='store_true', help='Input level/mute/default/routes with independent readback, no recording, exact restoration and PAM')
    parser.add_argument('--feedback', action='store_true', help='Feedback profile controls, silent no-output and qualified quiet single-play events, daemon recovery and PAM')
    parser.add_argument('--headphones', action='store_true', help='Owner-assisted JLab Studio test; requires live readiness handoff and independent normal-session audit')
    parser.add_argument('--text-enable-reset', action='store_true')
    parser.add_argument('--session-idle-test', choices=['battery','external','automatic'], help='Qualify actual GUI idle hints with manager action ignore; no kernel sleep')
    parser.add_argument('--detach', action='store_true')
    args = parser.parse_args()
    assert bool(args.transient_fixture_binary)==bool(args.transient_fixture_audit)
    assert not args.panel_off or args.observe_sensors
    assert not args.resource_sample or (args.panel_off and args.cycles == 0)
    assert os.geteuid() == 0 and 'PinePhone Pro' in Path('/proc/device-tree/model').read_text() and 0 <= args.cycles <= 20
    args.output = args.output.resolve()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    os.chown(args.output, 10000, pwd.getpwnam('user').pw_gid)
    password = getpass.getpass('Device password for installed-session PAM replay: ')
    assert password and all(' ' <= c <= '~' for c in password)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    if args.detach:
        child = os.fork()
        if child:
            password = None
            print('Detached installed-session replay PID', child, flush=True)
            return
        os.setsid()
        null = os.open('/dev/null', os.O_RDONLY)
        os.dup2(null, 0)
        os.close(null)
        log = os.open(args.output/'controller.log', os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.chown(args.output/'controller.log', 10000, pwd.getpwnam('user').pw_gid)
        os.dup2(log, 1)
        os.dup2(log, 2)
        os.close(log)
    run(args, password)


if __name__ == '__main__':
    main()
