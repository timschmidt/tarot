#!/usr/bin/env python3
"""Rendered terminal keyboard and actual Wayland key/modifier/IME delivery.

Owns its private X server; never injects host desktop input. Synthetic text only.
"""
import argparse,json,os,select,subprocess,tempfile,time
from pathlib import Path
from hardware_replay import PrivateInput
from lock_replay import events
from notification_replay import await_value
from phone_gesture_replay import request
from phone_touch_keyboard import Keyboard
from card_matrix import physical
from with_xvfb import read_display_number


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--xvfb',default='Xvfb')
    args=parser.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    root=Path.cwd();runtime=tempfile.TemporaryDirectory(prefix='tarot-terminal-');work=Path(runtime.name)
    control=work/'control.sock';report=out/'app-events.jsonl';commands=out/'app-commands.jsonl';commands.write_text('')
    children=[];logs=[];mouse=None;record={'passed':False,'shortcuts':[]}
    def log(name):f=(out/name).open('w');logs.append(f);return f
    def state(label,condition=lambda s:True):
        value=await_value(lambda:request(control,'status'),condition,seconds=12);record[label]=value;return value
    def capture(label):
        request(control,'capture',path=str(out/(label+'.png')))
        state(label,lambda s:not s['capture_pending'] and not s['capture_error'])
    def click(x,y):
        s=request(control,'status');mouse.move(*physical(s,x,y));time.sleep(.05)
        mouse.button(True);time.sleep(.05);mouse.button(False);time.sleep(.12)
    def shortcut(name,code,mods,action):
        start=len(events(report));action()
        found=await_value(lambda:events(report)[start:],lambda es:
            sum(e['event']=='key' and e['key']==code for e in es)==2 and
            bool([e for e in es if e['event']=='modifiers']) and
            [e for e in es if e['event']=='modifiers'][-1]['depressed']==0,seconds=6)
        modifiers=[e['depressed'] for e in found if e['event']=='modifiers']
        assert mods in modifiers,(name,found)
        assert not [e for e in found if e['event']=='applied'],(name,found)
        record['shortcuts'].append({'name':name,'key':code,'mods':mods,'events':found})
    def text(expected):
        return await_value(lambda:events(report),lambda es:any(e['event']=='applied' and e['text']==expected for e in es),seconds=6)
    def purpose(value,height):
        with commands.open('a') as f:f.write(json.dumps({'command':'purpose','value':value})+'\n')
        state('purpose-'+value,lambda s:s['keyboard_visible'] and s['keyboard_rect']['height']==height)
        time.sleep(.25)
    try:
        read,write=os.pipe()
        try:
            server=subprocess.Popen([args.xvfb,'-displayfd',str(write),'-nolisten','tcp','-noreset','-ac','-screen','0','1000x1600x24'],pass_fds=(write,),stdout=log('xvfb.log'),stderr=subprocess.STDOUT)
            children.append(server);os.close(write);write=None
            number=read_display_number(read);assert server.poll() is None
        finally:
            os.close(read)
            if write is not None:os.close(write)
        env=dict(os.environ,DISPLAY=':'+number,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',XDG_RUNTIME_DIR=str(work),
            XDG_CONFIG_HOME=str(work/'config'),LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
        env.pop('WAYLAND_SOCKET',None);env.pop('WAYLAND_DISPLAY',None)
        session=subprocess.Popen([str(root/'target/debug/tarot'),'--control-socket',str(control),'--input-method',str(root/'target/debug/tarot-keyboard'),'--exit-after-ms','90000'],env=env,stdout=log('compositor.log'),stderr=subprocess.STDOUT);children.append(session)
        await_value(lambda:control.exists() or session.poll() is not None,bool,seconds=8);assert session.poll() is None
        initial=state('initial');mouse=PrivateInput(session.pid,server,env['DISPLAY']);await_value(mouse.find,bool)
        native=dict(env,WAYLAND_DISPLAY=initial['wayland_display']);native.pop('DISPLAY',None)
        app=subprocess.Popen([str(root/'target/debug/examples/text-probe'),'--role','app','--terminal','--report',str(report),'--commands',str(commands)],env=native,stdout=log('app.log'),stderr=subprocess.STDOUT);children.append(app)
        state('terminal-ready',lambda s:len(s['cards'])==1 and s['keyboard_visible'] and s['keyboard_rect']['height']==288 and s['presentation']['windows'][0]['rect']==s['app_area'])
        time.sleep(.3);capture('terminal-controls')
        shortcut('control-c',46,4,lambda:(click(28,423),Keyboard(click).type_ascii('c')))
        shortcut('alt-x',45,8,lambda:(click(79,423),Keyboard(click).type_ascii('x')))
        shortcut('control-shift-c',46,5,lambda:(click(28,423),Keyboard(click).type_ascii('C')))
        shortcut('control-alt-tab',15,12,lambda:(click(28,423),click(79,423),click(129,423)))
        start=len(events(report));click(28,423);click(30,663);click(69,609)
        capture('unsupported-unicode-shortcut')
        keyboard=Keyboard(click);keyboard.page=1;keyboard.letters();keyboard.type_ascii('q');text('hello q')
        fresh=events(report)[start:]
        assert not [e for e in fresh if e['event']=='key']
        assert [e['text'] for e in fresh if e['event']=='applied']==['hello q']
        record['unsupported_unicode_shortcut_does_not_commit']=True
        shortcut('escape',1,0,lambda:click(180,423))
        for name,code,x in [('up',103,231),('down',108,281)]:shortcut(name,code,0,lambda x=x:click(x,423))
        click(332,423);capture('terminal-tools')
        for name,code,x,mods in [('home',102,33,0),('end',107,92,0),('left',105,151,0),('right',106,210,0),('paste',47,269,5)]:
            shortcut(name,code,mods,lambda x=x:click(x,423))
        click(328,423);click(28,423);click(329,663)
        state('hidden',lambda s:not s['keyboard_visible'])
        click(329,702);state('reopened',lambda s:s['keyboard_visible'] and s['keyboard_rect']['height']==288)
        Keyboard(click).type_ascii('q');text('hello qq')
        shortcut('tab-after-hide',15,0,lambda:click(129,423))
        click(28,423);purpose('password',240);capture('private-purpose')
        Keyboard(click).type_ascii('q');text('hello qqq')
        purpose('terminal',288);Keyboard(click).type_ascii('q');text('hello qqqq')
        purpose('normal',240);Keyboard(click).type_ascii('q');text('hello qqqqq')
        purpose('terminal',288)
        request(control,'rotate',orientation='left-up')
        state('landscape',lambda s:s['output']['orientation']=='left-up' and s['output']['requested'] is None and s['keyboard_rect']['height']==180)
        time.sleep(.4);capture('landscape-controls')
        shortcut('landscape-tab',15,0,lambda:click(258,162))
        request(control,'rotate',orientation='normal')
        state('portrait-again',lambda s:s['output']['orientation']=='normal' and s['output']['requested'] is None and s['keyboard_rect']['height']==288)
        with commands.open('a') as f:f.write('{"command":"quit"}\n')
        assert app.wait(timeout=5)==0
        state('app-gone',lambda s:not s['cards'] and not s['keyboard_visible']);request(control,'exit')
        assert session.wait(timeout=5)==0
        record.update(passed=True,real_rendered_keyboard=True,actual_wayland_events=True,
            one_shot_modifiers=True,hide_resets=True,purpose_change_resets=True,landscape_control_reachable=True)
        print('PASS: actual terminal modifiers/navigation/paste, IME isolation, hide/purpose reset and landscape delivery')
    finally:
        if mouse is not None:mouse.close()
        for p in reversed(children):
            if p.poll() is None:
                p.terminate()
                try:p.wait(timeout=5)
                except subprocess.TimeoutExpired:p.kill();p.wait()
        for f in logs:f.close()
        (out/'validation.json').write_text(json.dumps(record,indent=2)+'\n');runtime.cleanup()


if __name__=='__main__':main()
