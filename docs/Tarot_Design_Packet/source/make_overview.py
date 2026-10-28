from pathlib import Path
from PIL import Image,ImageDraw,ImageFont

ROOT=Path(__file__).resolve().parent.parent
AS=ROOT
S=2
im=Image.new('RGB',(2400,1600),'#0B242B')
d=ImageDraw.Draw(im)
def f(sz,bold=False,serif=False):
    name='InstrumentSerif-Regular.ttf' if serif else ('Lato-Bold.ttf' if bold else 'Lato-Regular.ttf')
    return ImageFont.truetype(str(AS/'fonts'/name),round(sz*S))
def text(x,y,tx,sz=16,col='#F2F1E5',bold=False,serif=False):d.text((x*S,y*S),tx,font=f(sz,bold,serif),fill=col)
for i in range(17):
    d.line([(int((810+i*15)*S),0),(int((470+i*20)*S),1600)],fill='#163940',width=1)
text(57,36,'TAROT',83,serif=True)
text(368,61,'A faithful webOS phone shell for Nura',24,col='#D4E0D4')
text(369,102,'PinePhone Pro  /  Nocturne theme  /  Design proposal',14,col='#92AAA2')
d.line([(57*S,158*S),(1143*S,158*S)],fill='#34565B',width=2)
items=[('01-card-view-nocturne','Card view','The same app, at another scale.'),('02-card-stack','Stacks','Related windows stay together.'),('03-launcher','Launcher','Named pages. Individual objects.'),('06-dashboard','Dashboard','Notifications live at the bottom.')]
for i,(name,title,cap) in enumerate(items):
    x=65+i*287;y=203;w=209;h=418
    d.rounded_rectangle(((x-8)*S,(y-17)*S,(x+w+8)*S,(y+h+17)*S),radius=20*S,fill='#0A1418',outline='#58716E',width=1)
    d.rounded_rectangle(((x+83)*S,(y-11)*S,(x+125)*S,(y-9)*S),radius=2*S,fill='#3D5555')
    ph=Image.open(AS/'screens'/'png'/(name+'.png')).convert('RGB').resize((w*S,h*S),Image.Resampling.LANCZOS)
    im.paste(ph,(x*S,y*S))
    text(x,657,title,21,bold=True)
    text(x,691,cap,12.8,col='#B0C5BA')
d.line([(57*S,741*S),(1143*S,741*S)],fill='#34565B',width=2)
text(57,763,'31-page packet · 25 screen studies · Editable SVGs · Tokens · Motion study',12.5,col='#9BB5AC')
text(1053,763,'28 OCT 2026',11,col='#9BB5AC')
out=ROOT/'Tarot_Visual_Overview.png';im.save(out)
print(out)
