from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
ROOT=Path(__file__).resolve().parent.parent
AS=ROOT
S=2
im=Image.new('RGB',(2400,1600),'#211B12')
d=ImageDraw.Draw(im)
def font(sz,bold=False,serif=False):
    name='InstrumentSerif-Regular.ttf' if serif else ('Lato-Bold.ttf' if bold else 'Lato-Regular.ttf')
    return ImageFont.truetype(str(AS/'fonts'/name),round(sz*S))
def text(x,y,tx,sz=16,col='#F2F1E5',bold=False,serif=False):
    d.text((x*S,y*S),tx,font=font(sz,bold,serif),fill=col)
def rr(x,y,w,h,r,fill,outline=None,width=1):
    d.rounded_rectangle((round(x*S),round(y*S),round((x+w)*S),round((y+h)*S)),radius=round(r*S),fill=fill,outline=outline,width=width)
def dot(x,y,r,fill,outline=None):
    d.ellipse(((x-r)*S,(y-r)*S,(x+r)*S,(y+r)*S),fill=fill,outline=outline,width=2)
def device(x,y,bw,name):
    k=bw/70;bh=139*k
    rr(x,y,bw,bh,4.5*k,'#B9AC82','#DFD0A6',2)
    rr(x+2*k,y+1.7*k,bw-4*k,bh-3.4*k,3.2*k,'#292C27','#736F58',2)
    rr(x+2.6*k,y+1.9*k,bw-5.2*k,117*k,2.3*k,'#111715')
    rr(x+3*k,y+121*k,bw-6*k,15.2*k,1.5*k,'#9D936C','#C6BB90')
    for j in range(7):
        xx=x+(7+j*8)*k
        d.line([(xx*S,(y+124*k)*S),((xx+4*k)*S,(y+124*k)*S)],fill='#C6BA8E',width=1)
    rr(x+26*k,y+7.2*k,18*k,1.2*k,.4*k,'#747D70')
    dot(x+52*k,y+8*k,1.5*k,'#687566')
    dot(x+57*k,y+8*k,1*k,'#171E1A')
    dot(x+35*k,y+131.2*k,5.2*k,'#CBB881','#E6D6AA')
    dot(x+35*k,y+131.2*k,3.8*k,'#A6935B')
    sw=58.52*k;sh=sw*16/9
    ph=Image.open(AS/'screens'/'png'/(name+'.png')).convert('RGB').resize((round(sw*S),round(sh*S)),Image.Resampling.LANCZOS)
    im.paste(ph,(round((x+(bw-sw)/2)*S),round((y+15.5*k)*S)))
for i in range(17):
    d.line([(int((810+i*15)*S),0),(int((470+i*20)*S),1600)],fill='#40331F',width=1)
text(57,36,'TAROT',83,serif=True)
text(368,61,'A faithful webOS phone shell for Nura',24,col='#E9DDC5')
text(369,102,'LG Fx0  /  Special Gold edition  /  720 × 1280',15,col='#D8BD81')
d.line([(57*S,158*S),(1143*S,158*S)],fill='#6B5635',width=2)
items=[('01-card-view-fx0-gold','Card view','The same app, at another scale.'),('02-card-stack','Stacks','Related windows stay together.'),('03-launcher','Launcher','Named pages. Individual objects.'),('06-dashboard','Dashboard','Notifications live at the bottom.')]
for i,(name,title,cap) in enumerate(items):
    x=57+i*287
    device(x,185,225,name)
    text(x+8,657,title,21,bold=True)
    text(x+8,691,cap,12.8,col='#CEBB96')
d.line([(57*S,741*S),(1143*S,741*S)],fill='#6B5635',width=2)
text(57,760,'33-page packet · 26 screen studies · Editable SVGs · Tokens · Motion study',12,col='#BDA77F')
text(57,780,'Fx0 hardware frames are schematic; screen artwork is native resolution.',9,col='#A28C66')
text(1053,763,'30 OCT 2026',11,col='#BDA77F')
out=ROOT/'Tarot_Fx0_Visual_Overview.png';im.save(out)
print(out)
