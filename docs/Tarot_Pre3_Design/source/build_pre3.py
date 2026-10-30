#!/usr/bin/env python3
"""Tarot Palm Pre 3 design packet generator. All runtime-oriented graphics drawn here as original vectors."""
from pathlib import Path
import json, math, re, os, textwrap, subprocess, zipfile, hashlib, html, random
from datetime import datetime
from PIL import Image, ImageDraw, ImageFont
import cairosvg
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor, Color
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfbase import pdfmetrics
import fitz

OUT=Path('/mnt/data/Tarot_Pre3_Design'); OUT.mkdir(exist_ok=True)
for folder in ['screens/svg','screens/png','icons','wallpapers','brand','tokens','handoff','motion','qa']:(OUT/folder).mkdir(parents=True,exist_ok=True)
W,H=360,600; PXW,PXH=480,800; SCALE=4/3
DARK='#081E23'; DEEP='#0B2F34'; MID='#184449'; WHITE='#F0F2E8'; PAPER='#EAEDE3'; INK='#243537'; MUTE='#819F9B'; GOLD='#D7BE89'; CYAN='#6FBCC0'
T=[('Nocturne',DARK,DEEP,WHITE,GOLD),('Heritage','#18352C','#5D8168','#F5F0E5','#ACC4A3'),('Vellum','#3B202A','#603541','#F3EAD9','#D5AB79'),('Fx0 Gold','#2E241E','#554139','#F4E7D2','#D8AF6C')]
C={
'dark':DARK,'deep':DEEP,'mid':MID,'white':WHITE,'paper':PAPER,'ink':INK,'muted':MUTE,'gold':GOLD,'cyan':CYAN,'red':'#B95955','green':'#42836A',
}

def esc(t):return html.escape(str(t),quote=False)
def g(tag,**kw):return '<'+tag+' '+' '.join(f'{k.replace("_","-")}="{esc(v)}"' for k,v in kw.items())+'/>'
def rect(x,y,w,h,fill,rx=0,stroke=None,sw=1,opacity=1):
 return g('rect',x=f'{x:.2f}',y=f'{y:.2f}',width=f'{w:.2f}',height=f'{h:.2f}',rx=rx,fill=fill,stroke=stroke or 'none',stroke_width=sw,opacity=opacity)
def line(x1,y1,x2,y2,c='#FFF',sw=1,op=1):return g('line',x1=x1,y1=y1,x2=x2,y2=y2,stroke=c,stroke_width=sw,opacity=op)
def circ(x,y,r,fill,stroke=None,sw=1,op=1):return g('circle',cx=x,cy=y,r=r,fill=fill,stroke=stroke or 'none',stroke_width=sw,opacity=op)
def path(d,fill='none',stroke=None,sw=1,op=1):return g('path',d=d,fill=fill,stroke=stroke or 'none',stroke_width=sw,opacity=op,stroke_linecap='round',stroke_linejoin='round')
def text(x,y,v,size=12,color=INK,weight=400,anchor=None,italic=False,spacing=None):
 attrs=dict(x=f'{x:.2f}',y=f'{y:.2f}',fill=color,font_family='Lato, DejaVu Sans, sans-serif',font_size=size,font_weight=weight)
 if anchor:attrs['text_anchor']=anchor
 if italic:attrs['font_style']='italic'
 if spacing:attrs['letter_spacing']=spacing
 return f"<text {' '.join(k.replace('_','-')+'=\"'+esc(val)+'\"' for k,val in attrs.items())}>{esc(v)}</text>"
def drawdefs(theme='Nocturne'):
 b,b2,p,a=next((v[1],v[2],v[3],v[4]) for v in T if v[0]==theme)
 return f'''<defs>
 <linearGradient id="wall" x1="0" y1="0" x2="1" y2="1"><stop stop-color="{b}"/><stop offset="0.52" stop-color="{b2}"/><stop offset="1" stop-color="{b}"/></linearGradient>
 <radialGradient id="lit"><stop stop-color="{a}" stop-opacity="0.13"/><stop offset="1" stop-color="{a}" stop-opacity="0"/></radialGradient>
 <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F3F4EC"/><stop offset="0.14" stop-color="#E5E8E0"/><stop offset="0.68" stop-color="#CFD4CC"/><stop offset="1" stop-color="#F5F5EC"/></linearGradient>
 <linearGradient id="bar" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#4A5554"/><stop offset="1" stop-color="#111A1A"/></linearGradient>
 <linearGradient id="dock" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#D3E1D6" stop-opacity="0.52"/><stop offset="0.6" stop-color="#5B7F7B" stop-opacity="0.7"/><stop offset="1" stop-color="#162C30" stop-opacity="0.86"/></linearGradient>
 <linearGradient id="cardbg" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#EFF2E9"/><stop offset="1" stop-color="#DDE3D9"/></linearGradient>
 <filter id="shadow" x="-35%" y="-30%" width="170%" height="175%"><feGaussianBlur stdDeviation="7"/></filter>
 <filter id="soft" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="3"/></filter>
 <clipPath id="viewport"><rect x="0" y="0" width="360" height="600"/></clipPath>
 </defs>'''
def wallpaper(theme='Nocturne'):
 b,b2,p,a=next((v[1],v[2],v[3],v[4]) for v in T if v[0]==theme)
 s=rect(0,0,W,H,'url(#wall)')+circ(196,365,240,'url(#lit)')
 for i in range(16):
  x=255+i*8
  s+=path(f'M{x} -40 Q{x-120} 230 {x+18} 635','none',a,.46,.045+((i%4)/500))
 for i in range(6):
  s+=path(f'M{44+i*19} 635 Q {105+i*15} 415 {314-i*17} 180','none','#E7DEC6',.6,.025)
 return s

def svg(content,theme='Nocturne',w=W,h=H,defs=None):return f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{drawdefs(theme) if defs is None else defs}{content}</svg>'

def status(app='Tarot'):
 s=rect(0,0,360,24,'#020808')+line(0,24,360,24,'#2D3C3A',.6)
 s+=text(8,17,app,12,'#E5ECE8',700)+text(180,17,'9:41',12,'#F7F8F6',700,'middle')
 s+=text(294,17,'4G',8,'#E9EFEB',700)
 s+=path('M316 16 l3 -3 l3 3 l3 -6 l3 6','none','#DFE9E2',1.6)+rect(338,7,12,10,'none',1,'#E9F2E9',1)+rect(340,9,8,6,'#83B566')+rect(341,5,6,2,'#E9F2E9')
 return s

def topbar(title,action=None):
 s=rect(0,24,360,44,'url(#glass)')+line(0,68,360,68,'#97A09B',1)
 s+=text(17,52,title,21,'#283B3A',400)
 if action:s+=text(342,50,action,12,'#52706A',700,'end')
 return s

def section_header(y,label):return rect(0,y,360,21,'#C4D4C6')+text(12,y+15,label,10,'#5F7567',700)

def mail_rows(y,small=False):
 s=''; data=[('Mara Ellis','Saturday by the water','The weather looks good. Shall we meet...', '9:28'),('Jun Park','Photos from the coast','I finally sorted the pictures from our trip.', '8:52'),('The Bookshop','Your order is ready','Your books are waiting at the front desk.', '8:10'),('Daniel Moore','Dinner on Friday','I booked a table for six. See you there.', '7:46'),('Garden Club','A little list','Our next gathering is on Sunday.', 'Yesterday')]
 ht=59 if not small else 55
 for i,(name,sub,prev,time) in enumerate(data):
  cy=y+i*ht;s+=rect(0,cy,360,ht-1, '#F0F2EA' if i%2==0 else '#E6E9E1')
  s+=text(15,cy+17,name,14,'#1C3839',700)+text(343,cy+15,time,9,'#728578',400,'end')
  s+=text(15,cy+34,sub,12,'#4B665A')+text(15,cy+49,prev[:49],10,'#7C8C7F')
 return s

def search_field(y=34,txt='Just type…'):
 return rect(9,y,342,37,'#0B2328',19,'#4A7778',1.1,.86)+text(23,y+24,txt,15,'#B9D3D1',400,italic=True)+circ(321,y+17,6,'none','#D2E5DF',1.4)+line(325,y+22,332,y+28,'#D2E5DF',1.5)

def icon_draw(name,x,y,size=46):
 colors={'Phone':'#4C8F58','Contacts':'#D0A571','Email':'#CFDCD6','Calendar':'#E6E8DE','Web':'#63A9C3','Software':'#D8C09A','Messaging':'#64AD68','Camera':'#6E858B','Photos':'#76B1B9','Music':'#4E9BAD','Clock':'#C9D3C9','Memos':'#DBC786','Settings':'#8FA9B0','Files':'#D6AC60','Terminal':'#3E5C55','Maps':'#84B6A8'}
 fill=colors.get(name,'#9AB8B0');s=rect(x,y,size,size,fill,size*.18,'#EBF3E8',.7)
 cx=x+size/2;cy=y+size/2;k=size/46
 if name=='Phone':
  s+=path(f'M{x+12*k} {y+10*k} Q {x+9*k} {y+21*k} {x+22*k} {y+33*k} Q {x+29*k} {y+38*k} {x+35*k} {y+30*k}','none','#F0F6DE',6*k)
 elif name=='Email':
  s+=rect(x+7*k,y+12*k,32*k,22*k,'#F5F8F3',2,'#71898B',1)+path(f'M{x+8*k} {y+13*k} L{x+23*k} {y+25*k} L{x+38*k} {y+13*k}','none','#78928F',1.2*k)
 elif name=='Calendar':
  s+=rect(x+8*k,y+9*k,30*k,29*k,'#F7F9F4',2,'#65787D',1)+rect(x+8*k,y+9*k,30*k,8*k,'#4F8F9D')+text(cx,y+31*k,'30',18*k,'#2D3E3F',700,'middle')
 elif name=='Web':
  s+=circ(cx,cy,17*k,'#4AAAC3','#D2E3C5',.8)+path(f'M{x+11*k} {y+17*k} q 13 -12 21 0 l-7 8 l11 7 l-12 7 l-6 -9 Z','#8BC26E')
 elif name=='Messaging':
  s+=rect(x+8*k,y+10*k,30*k,23*k,'#D4ED8A',6,'#36713A',1)+path(f'M{x+14*k} {y+32*k} l-2 7 l10 -7','#D4ED8A')
  for dx in [-8,0,8]:s+=circ(cx+dx*k,cy-1*k,2*k,'#7FA155')
 elif name=='Settings':
  s+=circ(cx,cy,16*k,'#BDC8CA','#586E75',3*k)+circ(cx,cy,7*k,'#658EA6','#D7F1F0',2*k)
 elif name=='Camera':
  s+=rect(x+7*k,y+14*k,32*k,22*k,'#34474B',3,'#D6E1DA',1)+rect(x+13*k,y+10*k,12*k,6*k,'#CAD7CB',2)+circ(cx,cy+3*k,8*k,'#6EB1D1','#DAEAEC',2*k)
 elif name=='Contacts':
  s+=rect(x+10*k,y+6*k,26*k,34*k,'#EFE7CD',2,'#866543',.9)+circ(cx,y+19*k,7*k,'#6A9D91')+path(f'M{x+13*k} {y+36*k} Q{x+23*k} {y+22*k} {x+34*k} {y+36*k}','none','#6A9D91',6*k)
 elif name=='Clock':
  s+=circ(cx,cy,17*k,'#F6F9F1','#435357',2*k)+line(cx,cy,cx,cy-10*k,'#283D41',2*k)+line(cx,cy,cx+7*k,cy+4*k,'#283D41',2*k)
 elif name=='Music':
  s+=path(f'M{x+21*k} {y+12*k} l14 -4 v23','none','#243D49',3*k)+circ(x+16*k,y+33*k,6*k,'#5BB9C4')+circ(x+30*k,y+31*k,6*k,'#5BB9C4')
 elif name=='Memos':
  s+=rect(x+9*k,y+9*k,28*k,28*k,'#F0DEA0',1,'#A98D52',1)+path(f'M{x+14*k} {y+18*k} h18 M{x+14*k} {y+24*k} h18 M{x+14*k} {y+30*k} h14','none','#AE9251',1.2)
 elif name=='Files':
  s+=rect(x+6*k,y+17*k,34*k,19*k,'#F6C878',3,'#8C6729',1)+path(f'M{x+8*k} {y+17*k} h12 l4 -5 h14 v7 Z','#D89D44')
 elif name=='Terminal':
  s+=rect(x+7*k,y+9*k,32*k,30*k,'#1E3435',2,'#A9C5BA',1)+text(x+12*k,y+30*k,'>_',15*k,'#D3E8D6',700)
 else:
  s+=circ(cx,cy,12*k,'#E9F2E0',op=.9)
 return s

def dock(y=523,small=False):
 h=62 if not small else 48
 s=rect(5,y,350,h,'url(#dock)',17,'#C5D8CC',.8)+line(18,y+3,341,y+3,'#ECF8E7',.8,.52)
 names=['Phone','Contacts','Email','Calendar','Software']
 step=68.2; size=46 if not small else 38
 for i,n in enumerate(names):
  x=14+i*69;s+=icon_draw(n,x,y+(h-size)/2,size)
  if i==4:s+=circ(x+size/2,y+h/2,size*.52,'#182A2E','#C9D6CE',1.5,.88)+path(f'M{x+size/2} {y+h/2-9} l8 9 h-5 v10 h-7 v-10 h-5 Z','none','#F4F7EC',1.4)
 return s

def bottom_toolbar(y=560):return rect(0,y,360,40,'url(#glass)')+rect(122,y+7,116,27,'url(#bar)',15)+text(180,y+27,'Refresh    Reply',10,'#F3F5ED',700,'middle')

def mail_full(variant='inbox'):
 s=topbar('Inbox','Edit')+rect(0,68,360,532,PAPER)
 s+=rect(10,77,340,35,'#DBE2D7',18,'#BBC7B8',1)+text(28,99,'Search mail',14,'#839186')
 s+=section_header(119,'TODAY')+mail_rows(140)
 s+=bottom_toolbar(560);return s

def mail_scaled(x,y,w,h):
 content=mail_full(); sx=w/360;sy=h/600
 # Scale a lightweight, designed fragment, not pasted screenshots.
 return f'<g transform="translate({x},{y}) scale({sx},{sy})">{content}</g>'

def card(x,y,w=258,h=347,tilt=0,variant='mail'):
 s=rect(x+4,y+10,w,h,'#000000',8,opacity=.18)
 inner=rect(x,y,w,h,'#DFE4D9',8,'#91A29B',1.3)
 if variant=='mail':
  inner+=rect(x,y,w,34,'url(#glass)',7)+text(x+15,y+23,'Inbox',15,'#2B4545')+text(x+w-12,y+22,'Edit',11,'#58736F',anchor='end')
  inner+=rect(x+12,y+41,w-24,24,'#CDD7CC',12)+text(x+23,y+58,'Search mail',11,'#7B9088')
  inner+=rect(x,y+75,w,18,'#C2D1C3')+text(x+12,y+88,'TODAY',8,'#607E6B',700)
  for i,(who,title) in enumerate([('Mara Ellis','Saturday by the water'),('Jun Park','Photos from the coast'),('The Bookshop','Your order is ready'),('Daniel Moore','Dinner on Friday')]):
   yy=y+93+i*54
   inner+=rect(x+1,yy,w-2,53,'#EDF0E7' if i%2==0 else '#E4E8DE')
   inner+=text(x+12,yy+15,who,13,'#294545',700)+text(x+12,yy+31,title,11,'#4D675B')+text(x+12,yy+45,'A message awaits your reply.',9,'#84948A')
  inner+=rect(x+1,y+h-38,w-2,37,'url(#glass)')+rect(x+w/2-54,y+h-29,108,22,'url(#bar)',12)+text(x+w/2,y+h-13,'Refresh    Reply',10,'#F7FBF5',700,'middle')
 elif variant=='browser':
  inner+=rect(x,y,w,34,'url(#glass)',6)+text(x+15,y+23,'◉  The quiet coast',14,'#2A4443')
  inner+=rect(x+10,y+42,w-20,23,'#CFDACF',2)+text(x+18,y+57,'example.org / coast',10,'#769287')
  inner+=rect(x+1,y+75,w-2,150,'#92B9AD')+path(f'M{x+1} {y+204} Q{x+75} {y+116} {x+152} {y+190} Q{x+210} {y+145} {x+w-1} {y+191} v36 h{-w+2} Z','#436F6C')
  inner+=rect(x+1,y+225,w-2,h-226,'#F2F1E7')+text(x+15,y+249,'A day beside',18,'#46605C',italic=True)+text(x+15,y+269,'the water',18,'#46605C',italic=True)
  inner+=text(x+15,y+296,'Take the path that follows the coast.',10,'#7C8B7F')
 elif variant=='messages':
  inner+=rect(x,y,w,34,'url(#glass)',6)+text(x+14,y+22,'Messaging',16,'#29433E')
  for i,(a,b) in enumerate([('Mara Ellis','I will bring the map.'),('Jun Park','Photos are ready.'),('Garden Club','Sunday looks good.')]):
   yy=y+46+i*72;inner+=rect(x+12,yy,w-24,61,'#F1F3EA',8)+text(x+23,yy+23,a,14,'#2A4746',700)+text(x+23,yy+45,b,12,'#5D756A')
 return '<g transform="rotate('+str(tilt)+f' {x+w/2} {y+h/2})">'+s+inner+'</g>'

# screen builder
SCREENS=[
('01-card-view','Card view','One native client, another scale'),('02-card-stack','Card stacks','Independent windows remain in a group'),
('03-launcher','Launcher','Three columns and named pages'),('04-email','Email','Cooperating light application chrome'),
('05-banner','Notification banner','Bottom arrival, foreground retained'),('06-dashboard','Dashboard','Persistent, actionable bottom history'),
('07-wave','Quick Launch wave','Hardware gesture area reveals favorites'),('08-device-menu','Device menu','Status corner, real services'),
('09-app-menu','Application menu','Top-left app actions'),('10-just-type','Just Type','Hardware keyboard starts intent'),
('11-lock','Lock screen','Private notifications, protected compositor'),('12-authentication','Authentication','Keyboard-first account credential'),
('13-incoming-call','Incoming call','Trusted urgent controls'),('14-active-call','Active call','Call-state truth and routes'),
('15-settings','Settings','Warm tactile list and toggles'),('16-empty-card-view','Empty card view','Wallpaper and dock carry composition'),
('17-heritage','Heritage','Period color alternative'),('18-vellum','Vellum','Warm editorial alternative'),
('19-messaging','Messaging','Conversations and input ownership'),('20-boot','Boot','Quiet Tarot identity and startup'),
('21-calendar','Calendar','Date and schedule'),('22-notification-rail','Notification rail','Persistent counts at the base'),
('23-launch-pending','Launch pending','One identity, honest waiting state'),('24-launch-failed','Launch failed','Recover without ghost windows'),
('25-locked-incoming-call','Locked incoming call','Answer without leaking lock context'),
('26-keyboard-open-search','Slider open','Just Type begins with physical keys'),
('27-keyboard-fallback','Keyboard fallback','On-screen keys only where needed'),
('28-gesture-help','Gesture area guide','Hardware-origin gestures explained')]

def pre3_screen(kind,theme='Nocturne'):
 s=wallpaper(theme)
 if kind in ('01-card-view','17-heritage','18-vellum'):
  s+=status()+search_field()+card(-210,146,254,321)+card(53,130,254,334)+card(333,146,254,321)+dock()
 elif kind=='02-card-stack':
  s+=status()+search_field()+card(-224,156,252,325)
  s+=card(75,141,242,323,-7,'messages')+card(67,129,242,325,-3,'browser')+card(60,119,242,325,2,'mail')
  s+=dock()
 elif kind=='03-launcher':
  s+=status('Launcher')+rect(0,24,360,576,'#081F22',opacity=.42)
  s+=text(180,64,'Applications',20,WHITE,400,'middle')+text(21,65,'‹',26,'#94B9B1')+text(339,65,'›',26,'#94B9B1','end')
  names=['Phone','Web','Software','Email','Calendar','Messaging','Camera','Photos','Music','Clock','Memos','Settings','Contacts','Files','Terminal']
  for i,n in enumerate(names):
   col=i%3;row=i//3;x=32+col*116;y=87+row*95
   s+=icon_draw(n,x,y,49)+text(x+25,y+65,n,12,'#EAF2E8',anchor='middle')
  s+=text(180,592,'•',18,'#A1C1B7',anchor='middle')
 elif kind=='04-email':s+=status('Email')+mail_full()
 elif kind=='05-banner':
  s+=status('Email')+mail_full()
  s+=rect(0,516,360,84,'#0B1111',0,opacity=.89)+rect(8,524,344,64,'url(#bar)',8)
  s+=icon_draw('Messaging',18,535,39)+text(69,547,'Mara Ellis',15,'#F6F7F2',700)+text(69,567,'I will bring the map.',13,'#D6DBD7')+circ(335,535,7,'#88AFAB')+text(335,539,'2',9,'#11282C',700,'middle')
 elif kind=='06-dashboard':
  s+=status('Email')+mail_full()+rect(0,344,360,256,'#091418',opacity=.62)
  for i,(n,a,b) in enumerate([('Messaging','Mara Ellis','I will bring the map.'),('Calendar','Walk by the water','Today 12:30'),('Music','The long way home','Juniper / Evening light')]):
   yy=379+i*69;s+=rect(7,yy,346,63,'url(#bar)',8,'#71807D',.6)+icon_draw(n,17,yy+11,39)+text(69,yy+25,a,14,WHITE,700)+text(69,yy+44,b,12,'#D4DDD8')
  s+=rect(8,349,344,24,'#0A262A',7)+text(21,365,'Notifications',12,'#DDE8DE',700)+text(338,365,'Clear',11,'#9DC0B2',anchor='end')
 elif kind=='07-wave':
  s+=status()+search_field()+card(55,119,250,336)+dock(525)
  s+=path('M45 551 Q100 465 171 475 Q225 463 317 543','none','#D6E2D0',3,.8)
  for i,n in enumerate(['Phone','Contacts','Email','Calendar','Software']):
   x=20+i*71;y=516-(16 if i==2 else 0)
   s+=icon_draw(n,x,y,56 if i==2 else 43)
  s+=text(177,480,'Email',12,'#F4FAF0',700,'middle')
 elif kind=='08-device-menu':
  s+=status()+search_field()+card(46,162,266,332)+dock()
  s+=rect(73,24,281,336,'#15282A',12,'#617C77',1,opacity=.96)
  s+=text(93,50,'Device',16,'#F0F5E9',700)
  for i,(a,b) in enumerate([('Wi-Fi','Connected'),('Bluetooth','Off'),('Airplane mode','Off'),('Brightness','52%'),('Sound','On'),('Battery','86%')]):
   yy=68+i*47;s+=line(86,yy+46,339,yy+46,'#3B5B59',.9)+text(94,yy+23,a,13,'#E5F0E9')+text(332,yy+23,b,11,'#9FC1AF',anchor='end')
 elif kind=='09-app-menu':
  s+=status('Email')+mail_full()+rect(6,24,242,218,'#162D30',9,'#67877F',1,opacity=.97)
  for i,n in enumerate(['Email', 'Preferences', 'Accounts', 'Sort', 'Help']):
   yy=39+i*40;s+=text(25,yy+18,n,15,WHITE,700 if i==0 else 400)
   if i<4:s+=line(15,yy+29,238,yy+29,'#4E6E6B',.7)
 elif kind in ('10-just-type','26-keyboard-open-search'):
  s+=status()+search_field(35,'coast')
  s+=rect(10,84,340,293,'#E9EEE5',8,opacity=.97)+text(24,111,'SEARCH & ACTIONS',11,'#68887C',700)
  for i,(n,sub) in enumerate([('The quiet coast','Firefox — open tab'),('Coastal map','Files — document'),('Mara Ellis','Contacts — call or message'),('Search the web','Using default browser')]):
   y=126+i*61;s+=line(18,y+50,343,y+50,'#C9D7CB',.8)+text(25,y+23,n,15,'#223F3F',700)+text(25,y+41,sub,11,'#738A7E')
  s+=text(28,403,'Slide open, type, act.',14,'#DCEBE4',italic=True)
  s+=dock()
 elif kind=='11-lock':
  s+=status()+text(180,194,'9:41',60,'#F2F5EB',400,'middle')+text(180,229,'Friday, 30 October',16,'#CBD9D0',400,'middle')
  s+=circ(180,442,35,'none','#BDDDC9',2)+text(180,451,'↑',34,'#E6F2E5',anchor='middle')+text(180,502,'Drag upward to unlock',15,'#CFE1D7',anchor='middle')
  s+=rect(40,541,280,37,'#071517',13,'#51716E',1)+text(180,565,'2 notifications hidden',12,'#C1D8CD',anchor='middle')
 elif kind=='12-authentication':
  s+=status()+text(180,118,'Unlock Tarot',25,WHITE,700,'middle')+text(180,153,'Enter your account password',13,'#C5D9D0',anchor='middle')
  s+=rect(32,177,296,46,'#E5EDE4',8)+text(49,208,'••••••••',22,'#23423E')+rect(33,236,294,52,'#588A7C',12)+text(180,270,'Unlock',17,'#F7F8ED',700,'middle')
  s+=text(180,318,'Physical keyboard preferred',13,'#A7C1B7',anchor='middle')
  s+=text(180,550,'Cancel',14,'#D4E8E2',anchor='middle')
 elif kind in ('13-incoming-call','25-locked-incoming-call'):
  s+=status('Phone')
  if kind=='25-locked-incoming-call':s+=text(180,89,'Phone call',16,'#E3EEE2',anchor='middle')
  s+=circ(180,165,50,'#719A8D')+circ(180,155,16,'#D9E3D4')+path('M146 190 Q179 149 214 190','#D9E3D4')
  s+=text(180,249,'Mara Ellis',26,WHITE,700,'middle')+text(180,276,'Mobile',13,'#B5CEC3',anchor='middle')
  s+=rect(22,450,150,69,'#437F64',14)+text(97,490,'Answer',18,'#F0F7EC',700,'middle')
  s+=rect(188,450,150,69,'#AB4A4A',14)+text(263,490,'Decline',18,'#FFF4EC',700,'middle')
  if kind=='25-locked-incoming-call':s+=text(180,561,'Unlock remains required for other apps',11,'#B4CEC0',anchor='middle')
 elif kind=='14-active-call':
  s+=status('Phone')+circ(180,136,48,'#658B7D')+circ(180,126,14,'#E1EEE7')+path('M150 163 Q180 131 210 163','#E1EEE7')
  s+=text(180,224,'Mara Ellis',24,WHITE,700,'middle')+text(180,253,'01:42',17,'#D4E7D8','middle')
  for i,(lbl,sym) in enumerate([('Mute','◌'),('Keypad','▦'),('Speaker','◖'),('Hold','Ⅱ')]):
   x=24+i*82;s+=circ(x+34,371,31,'#1B4546','#6F9A8B',1)+text(x+34,379,sym,27,'#F0F2E7',700,'middle')+text(x+34,423,lbl,12,'#C9D9CE',anchor='middle')
  s+=rect(89,494,182,58,'#A54C4B',16)+text(180,530,'End call',18,'#FFF4EF',700,'middle')
 elif kind=='15-settings':
  s+=status('Settings')+topbar('Settings')
  for i,(lbl,v) in enumerate([('Wi-Fi','Connected'),('Bluetooth','Off'),('Sounds','On'),('Display','Automatic'),('Notifications','On'),('Privacy','Configure'),('Date & Time','Automatic'),('About Tarot','')]):
   yy=80+i*59;s+=rect(8,yy,344,55,'#EDF1E9' if i%2==0 else '#E0E8DE',5)+text(24,yy+31,lbl,17,'#273E3E')+text(332,yy+31,(v+'  ›') if v else '›',12,'#6B8C7C',anchor='end')
 elif kind=='16-empty-card-view':s+=status()+search_field()+dock()+text(180,333,'TAROT',43,'#E1E8DD',400,'middle',spacing=9)+text(180,365,'Your applications are ready.',13,'#B0C6BB',anchor='middle')
 elif kind=='19-messaging':
  s+=status('Messaging')+topbar('Conversations','New')
  for i,(a,b) in enumerate([('Mara Ellis','I will bring the map.'),('Jun Park','Photos from the coast are ready.'),('Garden Club','Sunday looks good to everyone.'),('Daniel Moore','See you Friday!')]):
   yy=75+i*81;s+=rect(7,yy,346,77,'#F0F3EA' if i%2==0 else '#E4EAE2',6)+circ(35,yy+37,22,'#86AB9A')+text(75,yy+28,a,15,'#213F3F',700)+text(75,yy+52,b,12,'#657C73')
  s+=bottom_toolbar()
 elif kind=='20-boot':
  s+=text(180,246,'TAROT',56,WHITE,400,'middle',spacing=10)+path('M119 277 Q 180 256 241 277','none',GOLD,1.4)
  for i in range(3):s+=rect(153+i*24,342,14,3,GOLD,1,opacity=.4+i*.22)
  s+=text(180,554,'A familiar idea, independently built.',13,'#9EBDB1',anchor='middle')
 elif kind=='21-calendar':
  s+=status('Calendar')+topbar('October 2026','Today')
  s+=rect(8,82,344,244,'#EFF3EA',8)+text(180,110,'OCTOBER 2026',14,'#597A6A',700,'middle')
  for j,lab in enumerate(['S','M','T','W','T','F','S']):s+=text(40+j*46,137,lab,11,'#8EA394',700,'middle')
  for i in range(35):
   xx=40+i%7*46;yy=166+i//7*31;dd=i-3
   if 1<=dd<=31:
    if dd==30:s+=circ(xx,yy-5,15,'#4F9B93')
    s+=text(xx,yy,str(dd),13,'#FDFCF2' if dd==30 else '#364B43',700 if dd==30 else 400,'middle')
  s+=section_header(337,'FRIDAY 30 OCTOBER')
  for i,(tm,v) in enumerate([('10:00','Project review'),('12:30','Walk by the water'),('17:00','Dinner on Friday')]):
   yy=362+i*66;s+=rect(12,yy,336,58,'#EAF0E6',5)+text(24,yy+23,tm,13,'#6D8A7B')+text(101,yy+23,v,15,'#294842',700)
 elif kind=='22-notification-rail':
  s+=status('Email')+mail_full()
  s+=rect(0,520,360,80,'#0C1D20',opacity=.9)+rect(8,526,345,65,'url(#bar)',10)
  for i,n in enumerate(['Messaging','Calendar','Email']):s+=icon_draw(n,18+i*66,534,39)+circ(51+i*66,533,8,'#B6D9BD')+text(51+i*66,537,'2' if i==0 else '1',9,'#244444',700,'middle')
  s+=text(340,566,'Tap to expand',11,'#CADBD0',anchor='end')
 elif kind=='23-launch-pending':
  s+=status()+search_field()+card(55,129,252,341)+dock()
  s+=rect(53,129,253,341,'#06191D',7,opacity=.79)+circ(180,269,37,'none','#CED9CA',3)+path('M170 248 A22 22 0 0 1 202 264','none',GOLD,5)
  s+=text(180,330,'Opening Firefox…',16,'#EEF4EC',700,'middle')+text(180,355,'Waiting for an application surface',11,'#C1D1C6',anchor='middle')
 elif kind=='24-launch-failed':
  s+=status()+search_field()+dock()+rect(28,204,304,174,'#E6EBE2',13,'#7A9186',1.2)
  s+=text(180,246,'Firefox did not start',17,'#293F3A',700,'middle')
  s+=text(180,275,'The application did not open a window.',12,'#667C70','middle')
  s+=rect(51,304,124,43,'#597F74',10)+rect(185,304,124,43,'#D1DBD2',10)
  s+=text(113,332,'Retry',14,'#F3F5EA',700,'middle')+text(246,332,'Dismiss',14,'#4E7068',700,'middle')
 elif kind=='27-keyboard-fallback':
  s+=status()+search_field(35,'coast')+rect(10,89,340,211,'#EAF0E6',9)
  s+=text(20,115,'Search results',15,'#24483D',700)
  for i,n in enumerate(['Firefox — coastal map','Files — saved document','Search online']):s+=text(23,153+i*40,n,14,'#41675C')
  s+=rect(0,306,360,294,'#2C3737')
  letters=['QWERTYUIOP','ASDFGHJKL','ZXCVBNM']
  for j,row in enumerate(letters):
   n=len(row);keyw=31 if j<2 else 38;x0=(360 - (keyw+3)*n+3)/2
   for i,a in enumerate(row):
    x=x0+i*(keyw+3);y=323+j*60
    s+=rect(x,y,keyw,52,'#DCE3DB',5,'#A9B8AD',.7)+text(x+keyw/2,y+34,a,18,'#293E3B',700,'middle')
  s+=rect(74,503,212,48,'#E3E8DC',7)+text(180,535,'space',15,'#4C6861','middle')
  s+=text(180,579,'Only when physical keys are unavailable',11,'#D5E1D8','middle')
 elif kind=='28-gesture-help':
  s+=status('Tarot')+topbar('Gesture controls')
  for i,(sym,a,b) in enumerate([('←','Swipe back','On hardware gesture area'),('↑','Upward swipe','Return to Card view'),('↗','Up then hold','Open Quick Launch wave'),('↔','Full swipe','Switch apps when enabled'),('●','Center tap','Card view / maximize selected')]):
   y=83+i*94;s+=rect(8,y,344,84,'#E6ECE3',10)+circ(48,y+42,27,'#416E69')+text(48,y+50,sym,26,WHITE,700,'middle')+text(91,y+31,a,16,'#294542',700)+text(91,y+52,b,12,'#698579')
 else:s+=status()+dock()
 return svg(s,theme)

# Create screen SVG + PNG at native 480 x 800
screen_index=[]
for key,title,desc in SCREENS:
 theme='Heritage' if key=='17-heritage' else 'Vellum' if key=='18-vellum' else 'Nocturne'
 name=key+'.svg'; (OUT/'screens/svg'/name).write_text(pre3_screen(key,theme),encoding='utf-8')
 cairosvg.svg2png(bytestring=(OUT/'screens/svg'/name).read_bytes(),write_to=str(OUT/'screens/png'/(key+'.png')),output_width=480,output_height=800)
 screen_index.append(dict(id=key,title=title,description=desc,theme=theme,svg=f'screens/svg/{key}.svg',png=f'screens/png/{key}.png'))

# Wallpaper variants at native resolution
for theme in ['Nocturne','Heritage','Vellum','Fx0 Gold']:
 name=theme.lower().replace(' ','-');(OUT/'wallpapers'/f'{name}.svg').write_text(svg(wallpaper(theme),theme),encoding='utf-8')
 cairosvg.svg2png(url=str(OUT/'wallpapers'/f'{name}.svg'),write_to=str(OUT/'wallpapers'/f'{name}.png'),output_width=480,output_height=800)

# Icon vectors as standalone editable assets
for name in ['Phone','Contacts','Email','Calendar','Web','Software','Messaging','Camera','Photos','Music','Clock','Memos','Settings','Files','Terminal']:
 content=icon_draw(name,0,0,64)
 name_slug=name.lower()
 data=svg(content,w=64,h=64)
 (OUT/'icons'/f'{name_slug}.svg').write_text(data,encoding='utf-8')
 cairosvg.svg2png(bytestring=data.encode(),write_to=str(OUT/'icons'/f'{name_slug}.png'),output_width=128,output_height=128)

# Branded mark graphic
brand=svg(rect(0,0,440,180,DARK)+text(220,102,'TAROT',83,WHITE,400,'middle',spacing=9)+line(74,126,366,126,GOLD,1.3)+text(220,151,'PRE 3 / NOCTURNE',12,MUTE,700,'middle',spacing=2.4),w=440,h=180)
(OUT/'brand'/'tarot-pre3-lockup.svg').write_text(brand,encoding='utf-8')

# Hardware design frame (schematic, nonphotographic), supports screen compositing
hardware_svg='''<svg xmlns="http://www.w3.org/2000/svg" width="780" height="1230" viewBox="0 0 780 1230">
 <defs><linearGradient id="body" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#546467"/><stop offset=".35" stop-color="#12181B"/><stop offset="1" stop-color="#020606"/></linearGradient></defs>
 <rect x="85" y="15" width="610" height="1190" rx="118" fill="#05090B" stroke="#556468" stroke-width="5"/>
 <rect x="108" y="20" width="564" height="925" rx="98" fill="url(#body)" stroke="#384348" stroke-width="3"/>
 <rect x="134" y="99" width="512" height="741" rx="18" fill="#050C0E" stroke="#566367" stroke-width="3"/>
 <rect x="148" y="113" width="484" height="713" fill="#173038"/>
 <rect x="348" y="61" width="84" height="9" rx="4" fill="#59676A"/><circle cx="468" cy="66" r="9" fill="#253A42"/>
 <rect x="359" y="870" width="62" height="6" rx="3" fill="#C9D4D2" opacity=".9"/>
 <rect x="109" y="915" width="562" height="290" rx="100" fill="#14191D" stroke="#353D40" stroke-width="3"/>
 <g fill="#273136" stroke="#586469" stroke-width="1.7">'''
for row,n,offset,yy in [(0,10,0,956),(1,9,19,1012),(2,9,19,1068)]:
 for i in range(n):hardware_svg+=f'<rect x="{142+offset+i*49}" y="{yy}" width="40" height="39" rx="8"/>'
hardware_svg+='''<rect x="264" y="1123" width="248" height="43" rx="12"/></g>
 <g fill="#D5DED8" font-family="Lato, sans-serif" font-size="21" text-anchor="middle">'''
for row,labels,offset,yy in [(0,'QWERTYUIOP',0,984),(1,'ASDFGHJKL',19,1040),(2,'ZXCVBNM',40,1096)]:
 for i,a in enumerate(labels):hardware_svg+=f'<text x="{162+offset+i*49}" y="{yy}">{a}</text>'
hardware_svg+='''<text x="388" y="1152">SPACE</text></g></svg>'''
(OUT/'brand'/'pre3-slider-open-frame.svg').write_text(hardware_svg,encoding='utf-8')

# Geometry, shared profiles, state logic
profiles={
 'pinephone-pro':{'native_px':[720,1440],'logical':[360,720],'scale':2.0,'gesture_source':'screen-owned strip','gesture_strip_dp':36,'keyboard':'software','status_dp':32,'quicklaunch_total':5},
 'lg-fx0':{'native_px':[720,1280],'logical':[360,640],'scale':2.0,'gesture_source':'screen-owned strip + home key','gesture_strip_dp':36,'keyboard':'software','status_dp':32,'quicklaunch_total':5},
 'hp-pre3':{'native_px':[480,800],'logical':[360,600],'scale':1.3333333333333333,'gesture_source':'hardware capacitive area (driver-mapped)','gesture_strip_dp':0,'keyboard':'physical slider QWERTY preferred; software fallback','status_dp':24,'quicklaunch_total':5,'hardware_ram_mb':512,'device_dpi_approx':261,'porting_assumption':'Nura/postmarketOS available (requested scenario); runtime inputs unverified'}
}
(OUT/'tokens'/'device-profiles.json').write_text(json.dumps(profiles,indent=2)+'\n')
tokens={
 'name':'Tarot Pre 3 Nocturne','version':'0.1','date':'2026-10-30','device_profile':'hp-pre3','output_px':[480,800],'logical_dp':[360,600],
 'scale':4/3,'safe_insets_dp':{'top':24,'bottom':0,'left':0,'right':0},'gesture_area':{'kind':'external_hardware','onscreen_reserved_dp':0,'touch_region':'outside_LCD','requires_evdev_mapping':True,'fallback':'optional discoverable overlay; no default reserved strip'},
 'keyboard':{'primary':'hardware QWERTY slider','show_software_ime_by_default':False,'slider_open_does_not_change_LCD_geometry':True,'ime_fallback':'explicit / accessibility / physical unavailable'},
 'shell':{'status_height_dp':24,'quick_launch':{'slots':5,'editable_slots':4,'launcher_slot_index':4,'rect_dp':[5,523,350,62]},'card_focus':{'rect_dp':[53,130,254,334],'scale_seed':0.69},'app_content_rect_dp':[0,24,360,576], 'launcher_columns':3,'launcher_first_row_y_dp':87,'launcher_row_pitch_dp':95,'min_hit_dp':44},
 'palette':{'Nocturne':{'wallpaper_base':'#081E23','wallpaper_light':'#184449','panel':'#EAEDE3','ink':'#243537','accent':'#D7BE89'},'Heritage':{'wallpaper_base':'#18352C','wallpaper_light':'#5D8168'},'Vellum':{'wallpaper_base':'#3B202A','wallpaper_light':'#603541'},'Fx0 Gold':{'wallpaper_base':'#2E241E','wallpaper_light':'#554139'}},
 'motion_seed_ms':{'launch':400,'maximize':300,'minimize':350,'dismiss':300,'stack_reorder':350,'dashboard_expand':500,'wave_reveal':350},
 'memory_budget_proposed':{'running_surface_previews':'only visible; no permanent copies','retained_preview_count_soft_cap':3,'retained_preview_cache_mib_soft_cap':12,'effects':'blur/downsample only when measurable','waydroid':'disabled by default pending feasibility; Nura port assumption is not Android compatibility'},
 'evidence_level':'proposed design; numerical UI values are not historical measurements',
 'sources':['https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/12','https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/13','https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/43','https://www.gsmarena.com/hp_pre_3-3770.php']}
(OUT/'tokens'/'pre3-nocturne.tokens.json').write_text(json.dumps(tokens,indent=2)+'\n')
(OUT/'screens'/'index.json').write_text(json.dumps(screen_index,indent=2)+'\n')

# Motion study 480x800 using native screen comps and increasing scale/position; true same artwork and card
im_full=Image.open(OUT/'screens/png/04-email.png').convert('RGBA');bg=Image.open(OUT/'wallpapers/nocturne.png').convert('RGBA')
from PIL import ImageFilter
frames_dir=OUT/'motion'/'frames';frames_dir.mkdir(exist_ok=True)
frame_n=48
for f in range(frame_n):
 t=f/(frame_n-1)
 # minimize 0..16, hold 17..30, maximize 31..47
 if f<17:p=f/16
 elif f<31:p=1
 else:p=1-(f-31)/16
 p=p*p*(3-2*p)
 # status bar remains top, shrinking app content as a surface
 src=im_full.crop((0,32,480,800))
 cw=int(480*(1-.32*p));ch=int(768*(1-.40*p))
 target=src.resize((cw,ch),Image.Resampling.LANCZOS)
 base=bg.copy()
 # clip rounded corners on minimized card
 mask=Image.new('L',(cw,ch),0);md=ImageDraw.Draw(mask);md.rounded_rectangle((0,0,cw-1,ch-1),radius=int(15*p),fill=255)
 x=(480-cw)//2;y=32+int(121*p)
 shadow=Image.new('RGBA',(480,800));sd=ImageDraw.Draw(shadow);sd.rounded_rectangle((x+5,y+8,x+cw+4,y+ch+7),radius=max(3,int(16*p)),fill=(0,0,0,int(105*p)));shadow=shadow.filter(ImageFilter.GaussianBlur(11));base=Image.alpha_composite(base,shadow)
 base.paste(target,(x,y),mask)
 # tray appears in card mode
 if p>.2:
  dock_art=Image.open(OUT/'screens/png/16-empty-card-view.png').convert('RGBA').crop((0,690,480,800))
  dock_art.putalpha(int(255*max(0,min(1,(p-.2)/.45)))) if False else None
  # only tray clip from screen scaled, alpha ramp
  dock_art.putalpha(dock_art.getchannel('A').point(lambda v:int(v*max(0,min(1,(p-.2)/.7)))))
  base.alpha_composite(dock_art,(0,690))
 # overlay status
 base.alpha_composite(im_full.crop((0,0,480,32)),(0,0))
 base.convert('RGB').save(frames_dir/f'f_{f:03}.png')
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-framerate','12','-i',str(frames_dir/'f_%03d.png'),'-c:v','libx264','-pix_fmt','yuv420p','-crf','22',str(OUT/'motion'/'tarot-pre3-card-study.mp4')],check=True)
# remove intermediary frames from final folder
for p in frames_dir.iterdir():p.unlink()
frames_dir.rmdir()

# Hardware presentation board: nonphotographic slider/gesture-area schematic.
from PIL import ImageDraw as ID, ImageFont as IF
caseboard=Image.new('RGB',(1480,1140),(8,31,36)); d=ID.Draw(caseboard)
fname='/usr/share/fonts/truetype/lato/Lato-Regular.ttf'; fbname='/usr/share/fonts/truetype/lato/Lato-Bold.ttf'
f=lambda sz,bold=False:IF.truetype(fbname if bold else fname,sz)
d.text((54,40),'TAROT / PRE 3',font=f(45,True),fill='#EAF1E9')
d.text((57,101),'The gesture area lives below the display. The keyboard slides out, but LCD geometry does not change.',font=f(22),fill='#B4CCC2')
for j,open_slide in enumerate([False,True]):
 x=156+j*755;sy=178
 if open_slide:
  # fixed upper half slides upward over keyboard lower shell
  d.rounded_rectangle((x-22,sy+24,x+407,sy+822),radius=80,fill='#0C1214',outline='#536267',width=4)
  for row,letters,off in [(0,'QWERTYUIOP',0),(1,'ASDFGHJKL',15),(2,'ZXCVBNM',49)]:
   ky=sy+637+row*58
   for i,ch in enumerate(letters):
    kx=x+10+off+i*38
    d.rounded_rectangle((kx,ky,kx+32,ky+44),radius=8,fill='#243037',outline='#566A70',width=2)
    d.text((kx+10,ky+9),ch,font=f(16,True),fill='#E6ECE8')
  d.rounded_rectangle((x+80,sy+809,x+299,sy+845),radius=12,fill='#283538',outline='#768786',width=1)
  d.text((x+147,sy+816),'SPACE',font=f(16,True),fill='#DBE4DF')
  upper_bottom=sy+620
 else:upper_bottom=sy+696
 d.rounded_rectangle((x-22,sy,x+407,upper_bottom),radius=78,fill='#10191D',outline='#516469',width=4)
 d.rounded_rectangle((x-7,sy+10,x+392,sy+41),radius=23,fill='#202C31')
 d.rounded_rectangle((x+142,sy+29,x+241,sy+37),radius=3,fill='#657375')
 d.ellipse((x+278,sy+23,x+294,sy+39),fill='#263F49')
 disp=Image.open(OUT/'screens/png/01-card-view.png' if j==0 else OUT/'screens/png/26-keyboard-open-search.png').convert('RGB').resize((300,500),Image.Resampling.LANCZOS)
 d.rectangle((x+39,sy+66,x+339,sy+566),fill='#020607')
 caseboard.paste(disp,(x+39,sy+66))
 # pale capsule is a hardware gesture indicator, not UI controls
 d.rounded_rectangle((x+160,sy+592,x+215,sy+598),radius=3,fill='#E7F4EE')
 d.text((x+54,sy+710 if not open_slide else sy+894),'SLIDER CLOSED' if not open_slide else 'SLIDER OPEN / QWERTY',font=f(22,True),fill='#D8E2D8')
 d.text((x+54,sy+745 if not open_slide else sy+928),'480 × 800 LCD / external gestures',font=f(18),fill='#9AB6A9')
caseboard.save(OUT/'Tarot_Pre3_Hardware_Study.png')

# PDF creation
pdfmetrics.registerFont(TTFont('Lato','/usr/share/fonts/truetype/lato/Lato-Regular.ttf'))
pdfmetrics.registerFont(TTFont('LatoBold','/usr/share/fonts/truetype/lato/Lato-Bold.ttf'))
pdfmetrics.registerFont(TTFont('DejaVuSerif','/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf'))
PW,PH=792,612
PDF=OUT/'Tarot_Pre3_Design_Packet.pdf'
c=canvas.Canvas(str(PDF),pagesize=(PW,PH),pageCompression=1);c.setTitle('Tarot / HP Pre 3 - Design Packet');c.setAuthor('Tarot design study');N=0

def background_page(dark=True):
 c.setFillColor(HexColor(DARK if dark else '#F5F4ED'));c.rect(0,0,PW,PH,fill=1,stroke=0)
 if dark:
  c.setStrokeColor(HexColor('#193D41'));c.setLineWidth(.55)
  for i in range(15):c.line(530+i*20,0,762-i*5,612)

def ptxt(x,y,tx,size=12,col=WHITE,font='Lato',maxwidth=None,leading=None):
 c.setFillColor(HexColor(col));c.setFont(font,size)
 if maxwidth is None:c.drawString(x,y,str(tx));return y
 lead=leading or (size*1.43);words=str(tx).split();linebuf='';cur=y
 for w in words:
  test=(linebuf+' '+w).strip()
  if pdfmetrics.stringWidth(test,font,size)>maxwidth and linebuf:
   c.drawString(x,cur,linebuf);cur-=lead;linebuf=w
  else:linebuf=test
 if linebuf:c.drawString(x,cur,linebuf);cur-=lead
 return cur

def page(hd=None,sub=None,dark=True):
 global N;N+=1;background_page(dark)
 if hd:
  ptxt(44,560,hd,26,WHITE if dark else DARK,'LatoBold')
 if sub:ptxt(44,533,sub,11,MUTE if dark else '#71827B')
 c.setStrokeColor(HexColor('#446061' if dark else '#CACCC1'));c.setLineWidth(.7);c.line(44,48,748,48)
 ptxt(44,30,'TAROT   /   HP PRE 3   /   30 OCT 2026',8,MUTE if dark else '#667872','LatoBold');ptxt(710,30,f'{N:02}',9,MUTE if dark else '#667872','LatoBold')

def callout(x,y,w,h,title,body,dark=True):
 c.setFillColor(HexColor('#13353A' if dark else '#E7ECE6'));c.roundRect(x,y,w,h,10,stroke=0,fill=1)
 ptxt(x+16,y+h-29,title,15,WHITE if dark else INK,'LatoBold')
 ptxt(x+16,y+h-54,body,11,'#CBDCD2' if dark else '#526B63',maxwidth=w-32,leading=15)

def image_fit(path,x,y,w,h):
 from PIL import Image as PI
 im=PI.open(path);ww,hh=im.size;sc=min(w/ww,h/hh);iw=ww*sc;ih=hh*sc
 c.drawImage(str(path),x+(w-iw)/2,y+(h-ih)/2,iw,ih,mask='auto')

def note_card(x,y,w,h,a,b):
 c.setFillColor(HexColor('#F0F1E8'));c.roundRect(x,y,w,h,8,stroke=0,fill=1)
 ptxt(x+13,y+h-22,a,13,'#244641','LatoBold');ptxt(x+13,y+h-43,b,10,'#698077',maxwidth=w-26,leading=13)

page(None,None)
ptxt(51,497,'TAROT',79,WHITE,'DejaVuSerif');ptxt(52,442,'PALM PRE 3 / DESIGN PACKET',16,'#D8C39A','LatoBold')
ptxt(53,407,'A webOS-inspired phone shell on a modern Linux stack',15,'#C9DCD2')
ptxt(53,364,'Nocturne / native 480 × 800 / slide-out keyboard / hardware gestures',11,MUTE)
image_fit(OUT/'screens/png/01-card-view.png',500,112,175,310)
ptxt(54,105,'A separate edition for the original webOS hardware form.',11,'#B7D1C7')
c.showPage()

page('Design thesis','Re-create the coherent phone interface; do not re-create the OS beneath it.')
callout(45,360,337,139,'Fidelity before novelty','Cards, stacks, Quick Launch, Just Type and bottom notifications preserve the Pre 3 interaction grammar. No new app chrome is imposed on unrelated Wayland clients.')
callout(403,360,344,139,'Own the shell, not the platform','Tarot owns composition and trusted UI. Nura/postmarketOS, system services and independent application toolkits remain responsible for their domains.')
callout(45,204,337,139,'Hardware is a first-class input','The Pre 3 gesture sensor and physical keyboard remove two necessary compromises of the PinePhone Pro edition.')
callout(403,204,344,139,'One system, multiple devices','The same UI components and activity model vary by hardware profile. Fx0 Gold remains Fx0-specific; Pre 3 defaults to Nocturne.')
ptxt(48,130,'This is a proposed visual/interaction specification, not a running Pre 3 port or firmware capture.',11,'#CBD8CF')
c.showPage()

page('The physical canvas','Documented hardware facts and requested porting assumption')
info=[('DISPLAY','480 × 800 px; 3.58-inch WVGA; ~261 ppi'),('INPUT','Touchscreen + dedicated capacitive gesture area'),('TEXT','Slide-out QWERTY; Option, Shift, Sym, Enter'),('MEMORY','512 MB RAM; model-specific capacity may vary'),('PLATFORM','Nura/postmarketOS port assumed for design purposes'),('GRAPHICS','Adreno-class hardware; working DRM/GBM/GLES unverified')]
y=482
for a,b in info:
 c.setFillColor(HexColor('#15353A'));c.roundRect(45,y-25,697,47,7,stroke=0,fill=1)
 ptxt(59,y-5,a,11,GOLD,'LatoBold');ptxt(210,y-5,b,13,WHITE);y-=57
ptxt(48,110,'Hardware keys are outside the exported display pixels. Slider motion does not resize the LCD.',12,'#C8DDD0')
c.showPage()

page('The slider and the sensor','Schematic shell study: keyboard slides below a fixed-size LCD.')
image_fit(OUT/'Tarot_Pre3_Hardware_Study.png',30,72,733,438)
ptxt(46,69,'Black-glass / charcoal hardware; schematic only, not dimensional CAD or original HP artwork.',10,MUTE)
c.showPage()

page('The three phone profiles','Shared design system, deliberately different input surfaces')
cols=[44,206,372,539];cw=[162,166,167,208]
for x,w,t in zip(cols,cw,['ROLE','PINEPHONE PRO','LG Fx0','HP PRE 3']):
 c.setFillColor(HexColor('#284A4D'));c.rect(x,459,w-3,42,fill=1,stroke=0);ptxt(x+8,477,t,10,WHITE,'LatoBold')
rows=[('Output','720 × 1440','720 × 1280','480 × 800'),('Logical','360 × 720','360 × 640','360 × 600'),('UI scale','2.0','2.0','4/3'),('Gesture','On-screen strip','On-screen strip','Dedicated sensor'),('Extra input','Touchscreen','Home key','Slide keyboard'),('Primary IME','Software','Software','Hardware'),('Theme','Nocturne','Fx0 Gold','Nocturne')]
y=454
for j,row in enumerate(rows):
 y-=45
 if j%2==0:c.setFillColor(HexColor('#123035'));c.rect(44,y,699,42,fill=1,stroke=0)
 for i,v in enumerate(row):ptxt(cols[i]+7,y+15,v,10 if i<3 else 10.2,GOLD if i==0 else '#E0EBE3','LatoBold' if i==0 else 'Lato')
ptxt(48,91,'Pre 3 design removes software navigation reservation entirely. The device must report gesture input.',11,'#C4D5C9')
c.showPage()

page('Anatomy of the interface','A logical canvas of 360 × 600; exports are native 480 × 800')
image_fit(OUT/'screens/png/01-card-view.png',58,118,238,385)
entries=[('Status 0-24','Black, centered clock, app/device corners'),('Just Type','Always visible in Card view; physical keys activate it'),('Card 53,130','Card is the same Wayland client, transformed'),('Quick Launch','Five slots, only four user-editable'),('Gesture area','Below LCD; zero screen pixels reserved')]
y=455
for a,b in entries:
 ptxt(339,y,a,15,GOLD,'LatoBold');y=ptxt(339,y-21,b,12,'#D1E2D5',maxwidth=402,leading=15)-32
c.showPage()

page('Typography and materials','Nocturne palette with restrained Palm-era surface treatment')
for i,(name,col) in enumerate([('Night / 081E23',DARK),('Deep teal / 0B2F34',DEEP),('Warm paper / EAEDE3',PAPER),('Ink / 243537',INK),('Brass / D7BE89',GOLD),('Sea glass / 6FBCC0',CYAN)]):
 x=48+(i%3)*232;y=341-(i//3)*122
 c.setFillColor(HexColor(col));c.roundRect(x,y,211,82,10,fill=1,stroke=1)
 ptxt(x+5,y-22,name,11,'#D3E4DA')
ptxt(52,150,'Lato / everyday UI and status',22,WHITE,'Lato')
ptxt(52,111,'TAROT / display identity',30,WHITE,'DejaVuSerif')
ptxt(52,80,'Prelude remains the historical comparison target, not an assumed licensed font.',11,MUTE)
c.showPage()

page('Hardware gesture ownership','The physical gesture region is not a black software strip.')
notes=[('Center tap','Focused app ↔ Card view / selected card maximize'),('Short back swipe','Back only in cooperating app or declared bridge'),('Upward gesture','Overview; when already there, Launcher'),('Upward hold & drag','Quick Launch wave; release over favorite'),('Full swipe','Advanced switch-app behavior, when enabled'),('Fallback','Optional on-screen gesture control if driver is absent')]
y=481
for idx,(a,b) in enumerate(notes):
 c.setFillColor(HexColor('#193B3F'));c.roundRect(46,y-37,696,48,8,fill=1,stroke=0)
 ptxt(62,y-18,a,14,GOLD,'LatoBold');ptxt(242,y-18,b,12,WHITE);y-=61
ptxt(48,75,'Critical: the sensor needs an explicit Linux input-event contract; do not synthesize from app touch.',11,'#C4DBCD')
c.showPage()

page('Keyboard-first interaction','The slide-out QWERTY changes the default state machine')
notes=[('Closed','Card view and Launcher retain Just Type field. No permanent on-screen IME.'),('Slider opens','Do not resize or rotate the panel. Keep current focus and surface coordinates stable.'),('Typing in home','Begin Just Type immediately; do not require a tap before typing.'),('Typing in app','Send physical key events to focused client; reserve global shortcuts deliberately.'),('Symbols / fallback','Honor Option, Shift and Sym. Software IME for accessibility or absent keys.'),('Slider closes','Keep edit state. Do not silently close a form or discard pending text.')]
y=475
for a,b in notes:
 ptxt(49,y,a,13,GOLD,'LatoBold');ptxt(207,y,b,12,'#DDE7DD',maxwidth=530,leading=15);y-=66
c.showPage()

page('Core visual system','Surface hierarchy should be recognizable before icons are inspected')
image_fit(OUT/'screens/png/02-card-stack.png',56,100,235,395)
callout(322,366,422,120,'Cards & stacks','Window content is continuous through maximize/minimize; stack members preserve independent identity and pending-close state.')
callout(322,226,422,120,'Top corners','App actions are left, device services right. Menus are restrained, rounded, shadowed planes.')
callout(322,87,422,120,'Bottom edge','Quick Launch rests in Card view; the Dashboard expands from the bottom while respecting content and touch ownership.')
c.showPage()

page('App integration boundaries','Semantic cards can be layered onto ordinary Wayland surfaces')
for i,(a,b) in enumerate([('Native clients','GTK, Qt, Rust and Waydroid (if viable) present ordinary Wayland windows.'),('Cooperating clients','Firefox and Thunderbird bridges may promote a tab or message to a distinct window.'),('Trusted shell','Tarot draws gesture feedback, Card view, status, app menus, notifications and lock.'),('System services','Nura/postmarketOS supplies audio, network, power, authentication and IPC.'),('Activity bridge','Optional app capabilities; no compulsory scriptable shell runtime.')]):
 y=472-i*81;callout(49,y-60,694,73,a,b)
c.showPage()

page('Performance design for 512 MB','Avoid promising desktop-class behavior on a constrained 2011 handset')
callout(49,367,694,131,'Render only what is needed','Keep visible native surfaces live. Favor direct texture reuse, bounded shadows, and no full-resolution history of hidden clients.')
callout(49,219,694,131,'Initial cache policy','Three retained previews and 12 MiB owned cache are planning caps, not benchmarks. Measure actual resident GPU and process memory.')
callout(49,71,694,131,'Waydroid is conditional','A Nura port assumption is not Android binder, GPU, memory or kernel compatibility. Keep Waydroid opt-in and disabled by default on Pre 3 pending evidence.')
c.showPage()

page('The screen atlas','28 editable screen studies • 480 × 800 PNG and SVG variants')
# 7 x 4 contact spread as thumbnails
for i,item in enumerate(SCREENS):
 col=i%7;row=i//7;x=50+col*100;y=410-row*130
 image_fit(OUT/'screens/png'/(item[0]+'.png'),x,y,61,111)
 ptxt(x+31,y-13,f'{i+1:02d} {item[1][:10]}',8,'#B3D0C1',anchor if False else 'Lato')
c.showPage()

# 2 screen studies per page, fourteen pages
for i in range(0,len(SCREENS),2):
 a=SCREENS[i];b=SCREENS[i+1]
 page(f'{i+1:02d}–{i+2:02d} / screen studies','Editable SVGs preserve component geometry; PNGs verify native resolution.')
 for x,item in [(62,a),(425,b)]:
  image_fit(OUT/'screens/png'/(item[0]+'.png'),x,133,238,376)
  ptxt(x,116,item[1],15,GOLD,'LatoBold')
  ptxt(x,96,item[2],10,'#CBDCD0',maxwidth=307,leading=13)
 c.showPage()

page('Motion study','Card motion is proportional to a physical gesture, not a fixed screenshot transition.')
for j,name in enumerate(['Start: native window','Middle: live transform','End: card in overview']):
 # screenshot frames from video times by extracting via fitz? derive still programmatically
 img=Image.open(OUT/'screens/png/04-email.png' if j==0 else OUT/'screens/png/01-card-view.png')
 fn=OUT/'qa'/f'motion-stage-{j}.png';img.save(fn)
 image_fit(fn,64+j*244,169,174,312)
 ptxt(64+j*244,145,name,12,'#DDEBDD')
ptxt(52,87,'Reference seeds: minimize 350 ms / maximize 300 ms / dismiss 300 ms / launch 400 ms.',12,MUTE)
c.showPage()

page('Trusted states and risk boundaries','No fidelity detail may undermine security, focus or accessibility.')
for i,(a,b) in enumerate([('Lock','Render a protected scene before accepting sensitive input. Never leak app thumbnails or notifications.'),('Auth','PAM is authoritative. Slide key and gesture hardware must not bypass the protected input owner.'),('Calls','Incoming calls use trusted urgent controls; call state comes from ModemManager, not animations.'),('Menus','Wayland client headerbars remain client-owned unless the client cooperates.'),('Close','Card flick requests protocol close. Confirmation dialogs remain visible and accessible.'),('Text entry','Physical key maps, repeated keys, dead keys and accessibility IME require a test matrix.')]):
 y=480-i*65
 ptxt(49,y,a,15,GOLD,'LatoBold');ptxt(184,y,b,12,'#D5E5D8',maxwidth=548,leading=15)
c.showPage()

page('Port handoff: assumed Nura / postmarketOS','A design assumption is not a claim of verified device hardware support.')
sections=[('Display','480 × 800 output, scale 4/3; direct KMS mode, rotation, format and refresh to inspect.'),('Input','Capacitive gesture sensor Linux event shape, slider switch, keyboard keymap, keyboard illumination.'),('GPU','Adreno driver stack and available EGL/GLES/GBM; no assumed Panfrost or cross-GPU path.'),('Power','Backlight, proximity, suspend/resume, wake sources, thermal and charging policies.'),('Telephony','Radio/modem compatibility, voice routing, call notification, emergency-call safety tests.'),('Packages','Tarot as compositor session, protected login, native app launcher, constrained RAM behavior.')]
y=467
for a,b in sections:
 ptxt(48,y,a,13,GOLD,'LatoBold');ptxt(167,y,b,12,'#E2EEE5',maxwidth=560,leading=15);y-=66
c.showPage()

page('Fidelity acceptance','A screen-by-screen comparison is needed before calling the UI indistinguishable.')
for i,(a,b) in enumerate([('A','Historical build & locale: record exact Pre 3 variant, firmware, timezone and captured state.'),('B','Status and icons: compare baseline, menu-open, focused, overview, lock and radio-unknown states.'),('C','Card continuity: capture one app across launch, minimize, stack, reorder, dismiss and refusal.'),('D','Gestures: verify physical sensor origin, trajectory, cancellation, tap/back and wave arbitration.'),('E','Slider: open/close focus, live keymaps, Sym/Option, search, clipboard and safe wake.'),('F','Native clients: GTK/Qt/Firefox/Thunderbird parent dialogs, popups, text input and fullscreen.'),('G','Resource limits: cold/warm app startup, compositor RSS, GPU memory, 20+ sleep cycles.'),('H','Accessible fallback: enlarged text, gesture alternatives, reduced motion, physical key failure.')]):
 y=477-i*50;ptxt(51,y,a,14,GOLD,'LatoBold');ptxt(90,y,b,11,'#D7E7DB',maxwidth=644,leading=13)
c.showPage()

page('Research and licensing','Original vectors are studies; historical documentation remains external.')
sources=[
('Pre 3 manual / hardware','hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/12'),
('Pre 3 manual / Card view','hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/13'),
('Pre 3 manual / stacks','hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/43'),
('Pre 3 manual / Just Type','hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/27'),
('HP Pre 3 hardware specifications','gsmarena.com/hp_pre_3-3770.php'),
('Tarot implementation / current architecture','github.com/timschmidt/tarot')]
y=470
for a,b in sources:
 ptxt(52,y,a,14,GOLD,'LatoBold');ptxt(52,y-19,b,10,'#B6CFC0');y-=63
ptxt(50,80,'No historical manual images, proprietary Palm fonts or extracted firmware assets are distributed.',11,'#CADBCD')
c.showPage()

page('Implementation sequence','Respect the existing PinePhone Pro work and add Pre 3 via a device profile.')
for i,(a,b) in enumerate([('01  Device profile','Introduce native 480 × 800 / 360 × 600 and input capability flags.'),('02  Hardware test','Prove keyboard, slider, gesture sensor, KMS, authentication and suspend.'),('03  Shell layout','Render live Card view, Launcher, status, Quick Launch and dashboard at native size.'),('04  Interaction test','Close/refuse, stack, gesture wave, Just Type from physical keys.'),('05  Visual QA','Use 28-state atlas and same Pre 3 firmware build reference; record deviations.'),('06  Optional bridges','Firefox/Thunderbird semantic cards and Waydroid only after resource validation.')]):
 y=474-i*70;ptxt(47,y,a,15,GOLD,'LatoBold');ptxt(244,y,b,12,'#D4E6DB',maxwidth=498,leading=16)
c.showPage()

page('One metaphor, three devices','Tarot can be faithful without pretending hardware differences do not exist.')
for j,(name,img,desc) in enumerate([('PinePhone Pro','01-card-view','Soft gesture strip'),('LG Fx0','15-settings','Gold theme + Home key'),('HP Pre 3','26-keyboard-open-search','Hardware gesture sensor + slider')]):
 x=61+j*244
 image_fit(OUT/'screens/png'/(img+'.png'),x,164,188,325)
 ptxt(x,137,name,15,GOLD,'LatoBold');ptxt(x,119,desc,11,'#C8DCD0')
ptxt(58,69,'Shared card model and design language; the Pre 3 edition is a standalone companion, not a replacement.',11,MUTE)
c.showPage()

c.save()

# handoff text
(OUT/'handoff'/'pre3-port-and-input.md').write_text('''# Tarot / HP Pre 3 integration handoff

**Scenario:** Assume Nura/postmarketOS has been ported to the Pre 3, per the user's request. This document does **not** claim that the required display, input, modem, graphics, or suspend drivers are verified.

## Core profile

- Native LCD 480 × 800. Design coordinate space 360 × 600, output scale 4/3; verify fractional compositor scaling support before production.
- Status height 24 units; no on-screen gesture strip in normal operation.
- Hardware gesture sensor directly below display is privileged input owned by the shell; its Linux evdev node and reports require discovery.
- Physical QWERTY slider; Option, Shift, Sym, Enter and key repeat require input mapping tests. Slider-open state does not change LCD dimensions or imply rotation.
- Quick Launch has five total positions, four editable and a fixed Launcher slot.
- Full portrait display used by application excluding status and transient dashboard; software IME only when needed.
- No default Waydroid support claim; 512 MB memory requires profiling and binder/GPU validation.

## Input recognizer contract

1. Hardware-origin gesture-area events are distinct from touchscreen events. Do not misclassify app touches as navigation.
2. Recognize center tap, back swipe, upward navigation, upward-hold Quick Launch wave, optional advanced full-swipe.
3. Cancel recognizers on lost hardware, lock, incoming call, rotation, modal grab, client disappearance and second contacts.
4. Synthetic keyboard events must never target a protected lock screen unless originating from trusted physical keyboard routing.
5. Provide accessible soft navigation fallback if the gesture sensor driver is absent, but label it a compatibility deviation.

## Rendering and resource limits

- The Pre 3 has Adreno-class graphics (not PinePhone Pro Panfrost). Select verified DRM/KMS, GBM, GLES and buffer import paths.
- Favor direct compositing of client surfaces and budget shadows; do not hold full-screen texture copies for every historical card.
- Proposed 3 retained previews and 12 MiB preview cache are **starting budgets**, not measured targets.
- Run low-memory, high-thermal, app-crash, suspend and power-key sequences before claiming hardware readiness.

## App compatibility

GTK/Qt/Firefox/Thunderbird remain normal Wayland clients. Cooperative browser/mail bridges may supply semantic card metadata or promote a tab/message to its own native toplevel. Never rasterize app interiors as shell-owned screens. Any Android/Waydroid support is optional, subject to driver and capacity evidence.

## Legal provenance

All vector UI studies in this generated packet are newly constructed for Tarot and are provided as reference/source assets. The historical HP guide is linked, not copied. Lato is referred to as a system typography choice; this packet does not distribute font files. The user should assign the intended project license and preserve third-party notices when distributing integrated assets.
''')
(OUT/'handoff'/'interaction-and-fidelity.md').write_text('''# Tarot Pre 3 / interaction and fidelity audit

Source baseline: HP Pre 3 with webOS 2.2, not Pre 2. Historical gestures, keyboard-first Just Type, stacks and bottom notifications drive this packet. The screenshots are newly drawn concept studies, **not** screenshots from a measured historical firmware build. Numerical geometry and animation values are proposals.

## Behavior and acceptance

| State | Expected behavior | Evidence required |
| --- | --- | --- |
| Focused card | Same client content scales into card | source-to-target screenshot pixel continuity |
| Stacks | Hold to stack, reorder, pull out, close one | full actual device touch replay |
| Quick Launch | Five positions, transient wave, Launcher fixed | gesture sensor and wave timing capture |
| Launcher | Named pages and 3 columns; no persistent dock | reference screenshot and tap mapping |
| Just Type | Physical keyboard begins actions from Card view/Launcher | physical slider key events + search |
| Status | App menu left, connection menu right, centered time | actual service states and menus |
| Notifications | Banner and persistent bottom dashboard | native client, popup and keyboard tests |
| Lock | Protected pre-rendered scene, PAM or local auth | no input/capture disclosure under protection |
| Incoming call | Real modem state plus urgent trusted controls | hardware carrier and audio routing gates |

## Do not silently substitute

- Pre 2 keyboard/gesture layout for Pre 3 hardware.
- A permanent software keyboard or navigation strip.
- Fx0 Home-key or Gold default theme.
- Fake 9:41 in runtime; it is illustrative only.
- Application interiors that a normal Wayland client doesn't own.
- Nura availability for proven driver support.
''')
(OUT/'handoff'/'source-ledger.md').write_text('''# Source ledger and reproduction notes

Primary behavioral sources (linked, not bundled):

- HP Pre 3 user guide, introduction: https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/7
- Hardware and keyboard: https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/12
- Card view, Quick Launch and status: https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/13
- Just Type: https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/27
- Stack manipulation: https://hp.manymanuals.com/pc-workstation-barebones/pre3/users-guide-1137/43
- Hardware specification: https://www.gsmarena.com/hp_pre_3-3770.php
- Existing Tarot architecture: https://github.com/timschmidt/tarot

The entire packet's visual source is generated by the original vector definitions in the supplied authoring script. No original HP/Palm manual images, screenshot fragments, logos or proprietary font binaries are included. Design references are intentionally outside the runtime asset boundary.

SVG documents are in 360 x 600 logical view boxes with 480 x 800 PNG exports. Original images of the HP Pre 3 were used for *visual inspection* of its physical format only. The hardware frame is a schematic, not a claim of exact mechanical dimensions.
''')
(OUT/'README.md').write_text('''# Tarot / HP Pre 3 design packet

**Edition:** 0.1 — 30 October 2026. Companion to the existing PinePhone Pro Nocturne and LG Fx0 Gold design packets.

## Files

- `Tarot_Pre3_Design_Packet.pdf`: illustrated design, interaction, hardware, port and fidelity handoff.
- `Tarot_Pre3_Visual_Overview.png`: four-screen overview.
- `screens/svg/` and `screens/png/`: 28 editable 360×600 SVG studies and 480×800 PNG exports.
- `icons/`: 15 original pictorial icon studies (editable SVG and PNG).
- `wallpapers/`: native-resolution vector/raster wallpaper sets (Nocturne, Heritage, Vellum, optional Fx0 Gold).
- `brand/`: Tarot wordmark and nonphotographic Pre 3 slider frame.
- `tokens/`: design values and three-device matrix in JSON.
- `handoff/`: port, interaction, legal provenance and historical sources.
- `motion/`: a 480×800 video study of card transition.
- `screens/index.json`: screen manifest.
- `qa/`: generated render verification summary.

## Device-specific decisions

Pre 3 uses **480 × 800 native pixels**, **360 × 600 logical units at 4/3 scale**, a **hardware gesture area** below the LCD, and **physical slide-out QWERTY** as primary text input. Screen-visible software navigation and keyboard are off by default. Keyboard fallback is an explicit 27th study. Quick Launch: five positions total, four editable.

## Assumption

Per the request, **assume a Nura/postmarketOS Pre 3 port exists**. This is not a test report of working GPU, KMS, slider, gesture, modem, suspend or Android drivers. Waydroid on this 512 MB device is deliberately not assumed viable.

## Design status

The PNGs and PDF contain original SVG-based design studies, not historical firmware captures. Layout values, gesture thresholds, timings and memory budgets are *proposed*, and must be compared to a pinned Pre 3 software build and measured on the actual device. Lato is a freely licensed available system font, not included as a redistributed font file; Prelude is a historical comparison target.

## Licensing

Original graphics in this deliverable are newly drawn and suitable for project review; integrating them into a public repository requires the maintainer to choose the intended license. Historical source images were not bundled. See `handoff/source-ledger.md`.

## Regeneration

The PNG screen exports were rendered with CairoSVG. Authoring source for the generated packet is included in the ZIP at `source/build_pre3.py`.
''')

# Overview image as consistent editorial board
FONT='/usr/share/fonts/truetype/lato/Lato-Regular.ttf';BOLD='/usr/share/fonts/truetype/lato/Lato-Bold.ttf';SER='/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf'
board=Image.new('RGB',(1520,1010),(8,31,36));draw=ImageDraw.Draw(board)
fn=lambda p,s:ImageFont.truetype(p,s)
draw.text((64,43),'TAROT',font=fn(SER,88),fill='#ECEEE5');draw.text((470,76),'Palm Pre 3 / Nocturne',font=fn(FONT,33),fill='#DEE8DD');draw.text((473,124),'480 × 800   /   Dedicated gestures   /   Physical QWERTY',font=fn(FONT,22),fill='#9FC1B6')
draw.line((60,180,1460,180),fill='#45696B',width=2)
for i,idx in enumerate(['01-card-view','02-card-stack','03-launcher','10-just-type']):
 x=74+i*359
 img=Image.open(OUT/'screens/png'/f'{idx}.png').convert('RGB'); img=img.resize((281,468),Image.Resampling.LANCZOS)
 # handset surround only; keyboard schematic separate later
 draw.rounded_rectangle((x-9,209,x+290,705),radius=32,fill='#030809',outline='#47666A',width=2)
 board.paste(img,(x,221))
 draw.rounded_rectangle((x+107,695,x+163,699),radius=2,fill='#C8DBD6')
 lbl={'01-card-view':'Card view','02-card-stack':'Stacks','03-launcher':'Launcher','10-just-type':'Just Type'}[idx]
 draw.text((x,732),lbl,font=fn(BOLD,30),fill='#EDF1E7')
 sub={'01-card-view':'Continuous client surface','02-card-stack':'Related windows together','03-launcher':'Three columns, named pages','10-just-type':'Physical-keyboard first'}[idx]
 draw.text((x,777),sub,font=fn(FONT,20),fill='#A9C9BD')
draw.line((60,875,1460,875),fill='#45696B',width=2)
draw.text((61,914),'28 screen studies   •   Editable SVGs   •   Device profile   •   Motion and port handoff',font=fn(FONT,20),fill='#A9C8BE')
draw.text((1339,914),'30 OCT 2026',font=fn(FONT,15),fill='#A9C8BE')
board.save(OUT/'Tarot_Pre3_Visual_Overview.png')

# QA
issues=[]
for ix in screen_index:
 pp=OUT/ix['png'];ss=OUT/ix['svg'];img=Image.open(pp)
 if img.size!=(480,800):issues.append('invalid size '+ix['id'])
 if '<svg' not in ss.read_text():issues.append('missing SVG '+ix['id'])
for a in [OUT/'Tarot_Pre3_Design_Packet.pdf',OUT/'motion'/'tarot-pre3-card-study.mp4',OUT/'Tarot_Pre3_Visual_Overview.png']:
 if not a.exists() or a.stat().st_size<100:issues.append('missing '+str(a))
with fitz.open(str(PDF)) as doc:
 page_count=len(doc)
 if page_count<30:issues.append('expected >=30 pages')
 # render selected pages for QA contact sheet
 for ii in [0,3,10,11,16,25,page_count-1]:
  pg=doc[ii];pix=pg.get_pixmap(matrix=fitz.Matrix(1.4,1.4),alpha=False)
  pix.save(str(OUT/'qa'/f'page-{ii+1:02}.png'))
# make comprehensive PDF gallery contact sheet for own review
with fitz.open(str(PDF)) as doc:
 shots=[]
 for ii in range(len(doc)):
  pg=doc[ii];p=pg.get_pixmap(matrix=fitz.Matrix(.42,.42),alpha=False)
  from io import BytesIO
  shots.append(Image.open(BytesIO(p.tobytes('png'))).convert('RGB'))
 tw,th=shots[0].size
 atlas=Image.new('RGB',(tw*5,th*math.ceil(len(shots)/5)),(230,230,230))
 for i,im in enumerate(shots):atlas.paste(im,((i%5)*tw,(i//5)*th))
 atlas.save(OUT/'qa'/'page-atlas.jpg',quality=85)
qa={'packet_pages':page_count,'native_output_px':[480,800],'screens_svg':len(list((OUT/'screens/svg').glob('*.svg'))),'screens_png':len(list((OUT/'screens/png').glob('*.png'))),'icons_svg':len(list((OUT/'icons').glob('*.svg'))),'wallpaper_variants':len(list((OUT/'wallpapers').glob('*.png'))),'motion_present':(OUT/'motion'/'tarot-pre3-card-study.mp4').exists(),'issues':issues,'historical_capture_verification':'not performed','driver_port_verification':'not performed'}
(OUT/'qa'/'validation.json').write_text(json.dumps(qa,indent=2)+'\n')
# Copy own source into packet
(OUT/'source').mkdir(exist_ok=True)
import shutil
shutil.copy(__file__,OUT/'source'/'build_pre3.py')
# Zip without PDF QA internal intermediate images except validation.json, and all production files
ZIP=Path('/mnt/data/Tarot_Pre3_Design_Sources.zip')
with zipfile.ZipFile(ZIP,'w',zipfile.ZIP_DEFLATED,compresslevel=8) as z:
 for p in OUT.rglob('*'):
  if p.is_file() and 'qa/page-' not in str(p) and 'qa/motion-stage-' not in str(p) and 'qa/page-atlas' not in str(p):
   z.write(p,arcname=str(Path('Tarot_Pre3_Design')/p.relative_to(OUT)))
print('DONE',json.dumps(qa,indent=2));print('PDF',PDF.stat().st_size,'ZIP',ZIP.stat().st_size)
