#!/usr/bin/env python3
"""Real locker touch-like pointer entry at all orientations; no PAM attempt.

Only the test's own X11 compositor window receives input. Captures contain an
empty field or one synthetic character. They are locker-client diagnostics, not
compositor permission to capture protected applications.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from PIL import Image
from notification_replay import await_value
from phone_gesture_replay import request
from x11_replay_input import X11Input


def loaded_image(path):
    try:
        with Image.open(path) as image:
            return image.convert('RGB')
    except (OSError, ValueError):
        return None


def keyboard_layout(image):
    width,height=image.size
    pixels=list(image.getdata())
    mask=bytearray(int(all(abs(c-t)<=2 for c,t in zip(pixel,(224,228,217)))) for pixel in pixels)
    rectangles=[]
    for start in range(width*(height//2),width*height):
        if not mask[start]:continue
        mask[start]=0;stack=[start];area=0;xs=[];ys=[]
        while stack:
            index=stack.pop();y,x=divmod(index,width);area+=1;xs.append(x);ys.append(y)
            neighbors=[]
            if x:neighbors.append(index-1)
            if x+1<width:neighbors.append(index+1)
            if y>height//2:neighbors.append(index-width)
            if y+1<height:neighbors.append(index+width)
            for other in neighbors:
                if mask[other]:mask[other]=0;stack.append(other)
        if area>100 and max(xs)-min(xs)>15 and max(ys)-min(ys)>15:
            rectangles.append([(min(xs)+max(xs))/2,(min(ys)+max(ys))/2])
    rows=[]
    for point in sorted(rectangles,key=lambda p:p[1]):
        if not rows or abs(point[1]-rows[-1][0][1])>8:rows.append([])
        rows[-1].append(point)
    assert len(rows)==4 and len(rows[0])==10,rows
    rows=[sorted(row) for row in rows]
    return {'logical_size':[width,height],'first_row':rows[0],'mode_key':rows[-1][0],
            'source':'Independent connected components of actual empty-entry keyboard pixels'}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    out=args.output.resolve();out.mkdir(mode=0o700,parents=True,exist_ok=False)
    root=Path.cwd();results=[]
    env=dict(os.environ,WINIT_UNIX_BACKEND='x11',XDG_SESSION_TYPE='x11',LIBGL_ALWAYS_SOFTWARE='1',__EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
    env.pop('WAYLAND_DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
    for orientation in ['normal','left-up','bottom-up','right-up']:
        folder=out/orientation;folder.mkdir(mode=0o700)
        captures=folder/'client-ui';captures.mkdir(mode=0o700)
        runtime=tempfile.TemporaryDirectory(prefix='tarot-locker-orientation-')
        control=Path(runtime.name)/'control.sock';proc=mouse=None
        try:
            with (folder/'compositor.log').open('w') as log:
                proc=subprocess.Popen([str(root/'target/debug/tarot'),'--width','360','--height','720','--verify-lock-blank','--orientation',orientation,'--lock-client',str(root/'target/debug/tarot-lock'),'--lock-client-arg=--capture-dir','--lock-client-arg='+str(captures),'--control-socket',str(control),'--exit-after-ms','30000'],env=env,stdout=log,stderr=subprocess.STDOUT)
                await_value(lambda:control.exists() or proc.poll() is not None,bool);assert proc.poll() is None
                status=await_value(lambda:request(control,'status'),lambda s:s['lock']['phase']=='Locked' and s['lock']['last_presented_surface'] is True and s['lock']['output']['presented_revision']==s['lock']['output']['revision'])
                assert status['lock']['unprotected_presentations']==0
                locker=status['lock']['locker']['pid'];geometry=status['lock']['output']
                w,h=geometry['physical_width'],geometry['physical_height']
                width,height=(h,w) if orientation in ['left-up','right-up'] else (w,h)
                mouse=X11Input(proc.pid);await_value(mouse.find,bool)
                time.sleep(.5)  # Let the parent window manager finish startup focus.

                def tap(x,y):
                    x,y=x/width,y/height
                    x,y={'normal':(x,y),'left-up':(y,1-x),'bottom-up':(1-x,1-y),'right-up':(1-y,x)}[orientation]
                    await_value(lambda: (mouse.move(x*w,y*h),time.sleep(.08),mouse.focus()[0])[2],lambda focused:focused==mouse.window,seconds=3)
                    mouse.button(True);time.sleep(.07);mouse.button(False);time.sleep(.1)

                await_value(lambda:(captures/f'{locker}-cover.png').exists(),bool)
                tap(width/2,height-112)
                empty=captures/f'{locker}-entry-empty.png'
                image=await_value(lambda:loaded_image(empty),lambda image:image is not None)
                assert image.size==(width,height)
                layout=keyboard_layout(image)
                (folder/'keyboard-layout.json').write_text(json.dumps(layout,indent=2)+'\n')
                # Locate light keyboard rows in actual rendered pixels, rather
                # than assuming a fixed portrait keyboard height.
                rows=[]
                for y in range(height//2,height):
                    count=sum(all(abs(c-t)<=2 for c,t in zip(image.getpixel((x,y)),(224,228,217))) for x in range(width))
                    if count>width*.35:rows.append(y)
                assert rows
                first=rows[0];band=[first]
                for y in rows[1:]:
                    if y>band[-1]+1:break
                    band.append(y)
                tap(width*.05+3,(band[0]+band[-1])/2)
                masked=captures/f'{locker}-entry-masked.png'
                await_value(lambda:loaded_image(masked),lambda image:image is not None)
                current=request(control,'status');assert current['lock']['phase']=='Locked' and current['lock']['unprotected_presentations']==0
                results.append({'orientation':orientation,'logical_size':[width,height],'private_entry_opened':True,'synthetic_key_accepted':True,'protected':current})
                mouse.close();mouse=None
                request(control,'exit');assert proc.wait(timeout=5)==0
        finally:
            if mouse is not None:mouse.close()
            if proc is not None and proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=5)
                except subprocess.TimeoutExpired:proc.kill();proc.wait()
            runtime.cleanup()
            (out/'status.json').write_text(json.dumps({'results':results,'validation':{'passed':len(results)==4,'actual_egui_locker_entry_and_keyboard':len(results)==4,'pam_attempted':False,'native_touch_tested':False}},indent=2)+'\n')
    print('PASS: real locker private entry and synthetic keyboard input in all four orientations')


if __name__=='__main__':main()
