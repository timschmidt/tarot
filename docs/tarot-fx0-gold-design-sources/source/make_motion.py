from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter
import subprocess, json

ROOT=Path(__file__).resolve().parent.parent
AS=ROOT
PNG=AS/'screens'/'png'
DEST=AS/'motion';DEST.mkdir(exist_ok=True)
TMP=ROOT/'tmp'/'motion';TMP.mkdir(parents=True,exist_ok=True)

overview=Image.open(PNG/'01-card-view-fx0-gold.png').convert('RGBA')
base=Image.open(PNG/'16-empty-card-view.png').convert('RGBA')
base.paste(overview.crop((0,64,122,1208)),(0,64))
base.paste(overview.crop((598,64,720,1208)),(598,64))
full=Image.open(PNG/'04-email.png').convert('RGBA')
client=full.crop((0,64,720,1208))
status_full=full.crop((0,0,720,64))
status_cards=overview.crop((0,0,720,64))

def frame(a):
    im=base.copy()
    scale=1-.34*a
    w=round(720*scale);h=round(1144*scale)
    x=round((720-w)/2);y=round(64+160*a)
    mask=Image.new('L',(w,h),0);ImageDraw.Draw(mask).rounded_rectangle((0,0,w-1,h-1),radius=22*scale,fill=255)
    if a>.01:
        sh=Image.new('RGBA',(720,1280),(0,0,0,0));sd=ImageDraw.Draw(sh)
        sd.rounded_rectangle((x,y+10,x+w,y+h+10),radius=22*scale,fill=(0,0,0,95))
        sh=sh.filter(ImageFilter.GaussianBlur(10));im=Image.alpha_composite(im,sh)
    cl=client.resize((w,h),Image.Resampling.LANCZOS)
    im.paste(cl,(x,y),mask)
    blank=status_full.copy();ImageDraw.Draw(blank).rectangle((0,0,256,63),fill=(3,5,5,255))
    bar=Image.blend(status_full,blank,a*2) if a<.5 else Image.blend(blank,status_cards,(a-.5)*2)
    im.paste(bar,(0,0))
    return im.convert('RGB')

def ease(t):return 1-(1-t)**3
sequence=[0.0]*60+[ease(i/21) for i in range(1,22)]+[1.0]*72+[1-ease(i/18) for i in range(1,19)]+[0.0]*60
fps=60
out=DEST/'Tarot_Fx0_Card_Motion.mp4'
cmd=['/usr/bin/ffmpeg','-y','-v','error','-f','rawvideo','-vcodec','rawvideo','-pix_fmt','rgb24','-s','720x1280','-r',str(fps),'-i','-','-an','-c:v','libx264','-preset','veryfast','-crf','19','-pix_fmt','yuv420p','-movflags','+faststart',str(out)]
p=subprocess.Popen(cmd,stdin=subprocess.PIPE)
for a in sequence*2:p.stdin.write(frame(a).tobytes())
p.stdin.close();rc=p.wait()
if rc:raise SystemExit(rc)
for i,a in enumerate([0,.5,1]):frame(a).save(TMP/f'keyframe-{i}.png')
(DEST/'motion-notes.txt').write_text('TAROT Fx0 GOLD CARD MOTION STUDY\n\nThis is an illustrative design animation, not captured Tarot software or measured Palm footage.\n720 x 1280 pixels; 60 frames per second. Two repetitions.\nMinimize: 350 ms (21 frames). Maximize: 300 ms (18 frames).\nEase-out cubic, f(t)=1-(1-t)^3. Direct-manipulation gesture input is not demonstrated.\nThe client buffer scales uniformly; the status remains in the global frame.\nThe exact historical curve and hardware frame pacing remain to be measured.\n')
print(json.dumps({'movie':str(out),'frames':len(sequence)*2,'fps':fps,'duration_seconds':len(sequence)*2/fps,'bytes':out.stat().st_size}))
