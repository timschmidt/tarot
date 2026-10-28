from pathlib import Path
from html import escape
import json, shutil, math

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT
SCREENS = OUT / 'screens' / 'svg'
for p in [SCREENS, OUT/'screens'/'png', OUT/'brand', OUT/'wallpapers', OUT/'tokens', OUT/'fonts', OUT/'source', OUT/'components', OUT/'icons', OUT/'glyphs']:
    p.mkdir(parents=True, exist_ok=True)
# Bundled font files are already present in fonts/.

THEMES = {
    'nocturne': {'name':'Nocturne','top':'#091A21','mid':'#173D43','bottom':'#31534D','glow':'#708B79','line':'#C6AE78','accent':'#A3763F','focus':'#D7BE89'},
    'heritage': {'name':'Heritage','top':'#1D3027','mid':'#4A6650','bottom':'#9A9451','glow':'#CBC883','line':'#E8DCAD','accent':'#3C7C96','focus':'#A9D9E6'},
    'vellum': {'name':'Vellum','top':'#34242C','mid':'#65454A','bottom':'#926653','glow':'#C3A477','line':'#EAD3A1','accent':'#934C4C','focus':'#E0BF95'},
}

def text(x,y,s,size=16,fill='#242727',weight='normal',anchor='start',family='Lato',extra=''):
    return f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{fill}" text-anchor="{anchor}" {extra}>{escape(str(s))}</text>'

def rect(x,y,w,h,fill,rx=0,stroke=None,sw=1,extra=''):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+f' {extra}/>'

def line(x1,y1,x2,y2,c='#b8bab4',sw=1,extra=''):
    return f'<path d="M{x1} {y1} L{x2} {y2}" stroke="{c}" stroke-width="{sw}" fill="none" {extra}/>'

def circle(x,y,r,fill,stroke=None,sw=1,extra=''):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}"'+(f' stroke="{stroke}" stroke-width="{sw}"' if stroke else '')+f' {extra}/>'

def defs(theme='nocturne'):
    t=THEMES[theme]
    return f'''<defs>
    <linearGradient id="wall" x1="0" y1="0" x2="1" y2="1"><stop stop-color="{t['top']}"/><stop offset=".57" stop-color="{t['mid']}"/><stop offset="1" stop-color="{t['bottom']}"/></linearGradient>
    <radialGradient id="glow"><stop stop-color="{t['glow']}" stop-opacity=".50"/><stop offset="1" stop-color="{t['glow']}" stop-opacity="0"/></radialGradient>
    <linearGradient id="chrome" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F0F0EB"/><stop offset=".45" stop-color="#D6D8D1"/><stop offset="1" stop-color="#B7BCB5"/></linearGradient>
    <linearGradient id="surface" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F2F3ED"/><stop offset="1" stop-color="#DEE1D8"/></linearGradient>
    <linearGradient id="row" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FAFAF5"/><stop offset="1" stop-color="#E5E7DE"/></linearGradient>
    <linearGradient id="tray" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFFFFF" stop-opacity=".32"/><stop offset=".48" stop-color="#999E99" stop-opacity=".21"/><stop offset="1" stop-color="#171D20" stop-opacity=".60"/></linearGradient>
    <linearGradient id="dark" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#484C4D"/><stop offset=".45" stop-color="#303536"/><stop offset="1" stop-color="#202527"/></linearGradient>
    <linearGradient id="menu" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#484D4C"/><stop offset="1" stop-color="#262D2E"/></linearGradient>
    <linearGradient id="green" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#B4D368"/><stop offset=".50" stop-color="#70A849"/><stop offset="1" stop-color="#34733D"/></linearGradient>
    <linearGradient id="blue" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#7AC6EC"/><stop offset=".50" stop-color="#388FB8"/><stop offset="1" stop-color="#1E536F"/></linearGradient>
    <linearGradient id="gold" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#F6D985"/><stop offset=".5" stop-color="#C7A052"/><stop offset="1" stop-color="#92713F"/></linearGradient>
    <linearGradient id="red" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#E5816D"/><stop offset=".5" stop-color="#CB4D40"/><stop offset="1" stop-color="#9B302C"/></linearGradient>
    <linearGradient id="button" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FAFBF8"/><stop offset=".48" stop-color="#DEE1DA"/><stop offset=".5" stop-color="#D2D6CE"/><stop offset="1" stop-color="#C0C6BC"/></linearGradient>
    <radialGradient id="halo"><stop stop-color="#FFF1BF" stop-opacity=".65"/><stop offset="1" stop-color="#FFDC81" stop-opacity="0"/></radialGradient>
    <filter id="shadow" x="-40%" y="-20%" width="180%" height="150%"><feDropShadow dx="0" dy="6" stdDeviation="6" flood-color="#000" flood-opacity=".55"/></filter>
    <filter id="iconShadow" x="-40%" y="-40%" width="180%" height="190%"><feDropShadow dx="0" dy="2" stdDeviation="1.4" flood-color="#000" flood-opacity=".5"/></filter>
    <filter id="tinyShadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="1" stdDeviation=".7" flood-color="#000" flood-opacity=".65"/></filter>
    <filter id="blur"><feGaussianBlur stdDeviation="12"/></filter>
    <clipPath id="screenClip">{rect(0,0,360,720,'white')}</clipPath>
    <clipPath id="clientClip">{rect(0,0,360,652,'white',11)}</clipPath>
    </defs>'''

def wallpaper(theme='nocturne'):
    t=THEMES[theme]
    s=rect(0,0,360,720,'url(#wall)')
    s+='<ellipse cx="330" cy="472" rx="320" ry="355" fill="url(#glow)"/>'
    s+='<ellipse cx="40" cy="160" rx="190" ry="260" fill="url(#glow)" opacity=".27"/>'
    # Original botanical linework, kept outside the most legible central card area.
    s+=f'<g fill="none" stroke="{t["line"]}" stroke-width=".65" opacity=".16">'
    for j in range(14):
        x=255+j*9
        s+=f'<path d="M {x} 747 C {x-52} 570 {x-140} 480 {x-72} 258 C {x-46} 168 {x+13} 120 {x+45} 95"/>'
    for i in range(9):
        yy=258+i*44
        s+=f'<path d="M {221+i*7} {yy+80} Q {157+i*3} {yy+22} {127+i*10} {yy-35} Q {204+i*5} {yy-9} {221+i*7} {yy+80}"/>'
    s+='</g>'
    s+=f'<g fill="{t["top"]}" opacity=".32"><path d="M-80 680 Q130 520 58 245 Q24 435 -80 680Z"/><path d="M360 720 Q200 600 410 405 Q299 595 360 720Z"/></g>'
    if theme=='heritage':
        s+='<g fill="#D7BF6A" opacity=".18" filter="url(#blur)"><ellipse cx="-12" cy="438" rx="32" ry="191" transform="rotate(-18 -12 438)"/><ellipse cx="349" cy="285" rx="24" ry="220" transform="rotate(22 349 285)"/></g>'
    return s

def glyph(name,x,y,s=20,c='#FFFFFF',sw=1.8):
    # All paths use a 24-unit artboard.
    paths={
      'search':'<circle cx="10" cy="10" r="6.6"/><path d="M15 15 22 22"/>',
      'chevron':'<path d="m9 5 7 7-7 7"/>',
      'back':'<path d="m15 5-7 7 7 7"/>',
      'plus':'<path d="M12 4v16M4 12h16"/>',
      'check':'<path d="m4 12 5 5L21 5"/>',
      'close':'<path d="m5 5 14 14M19 5 5 19"/>',
      'up':'<path d="m4 12 8-9 8 9h-5v8H9v-8Z"/>',
      'wifi':'<path d="M2 8q10-9 20 0M6 12q6-6 12 0M10 16q2-2 4 0"/><circle cx="12" cy="20" r=".8" fill="currentColor"/>',
      'bluetooth':'<path d="m7 6 12 12-7 5V1l7 5L7 18"/>',
      'plane':'<path d="m2 12 9-2V3q1-4 2 0v7l9 2v3l-9-1v6l3 1v2l-4-1-4 1v-2l3-1v-6l-9 1Z"/>',
      'speaker':'<path d="M3 9h5l6-5v16l-6-5H3ZM17 8q5 4 0 8M20 5q8 7 0 14"/>',
      'mic':'<rect x="9" y="2" width="6" height="13" rx="3"/><path d="M5 10v2q0 8 7 8t7-8v-2M12 20v3M7 23h10"/>',
      'lock':'<rect x="5" y="10" width="14" height="12" rx="2"/><path d="M8 10V6q0-8 8 0v4"/><path d="M12 15v3"/>',
      'refresh':'<path d="M20 8A9 9 0 1 0 20 17M20 3v6h-6"/>',
      'trash':'<path d="M5 7h14M8 7V4h8v3M6 7l1 15h10l1-15M10 11v7M14 11v7"/>',
      'pencil':'<path d="m4 16 12-13 5 5L8 21l-6 1ZM14 5l5 5"/>',
      'power':'<path d="M12 1v11M6 5a10 10 0 1 0 12 0"/>',
      'sun':'<circle cx="12" cy="12" r="4"/><path d="M12 0v4M12 20v4M0 12h4M20 12h4M3 3l3 3M18 18l3 3M3 21l3-3M18 6l3-3"/>',
    }
    return f'<g transform="translate({x} {y}) scale({s/24})" fill="none" stroke="{c}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round">{paths.get(name,paths["plus"])}</g>'

def icon(kind,x,y,size=56):
    s=f'<g transform="translate({x} {y}) scale({size/64})" filter="url(#iconShadow)">'
    if kind=='phone':
        s+='<path d="M13 6Q5 9 7 24q7 27 33 34 15 3 18-10l-12-9q-4-2-9 6-13-5-18-18 9-6 6-10Z" fill="url(#green)" stroke="#294C29" stroke-width="1.7"/><path d="M13 10q-4 11 1 19" fill="none" stroke="#F2FFCA" stroke-width="2.5" opacity=".8"/><path d="M37 8q17 2 19 19M38 16q9 1 10 10" fill="none" stroke="#D7EDAA" stroke-width="3.5" stroke-linecap="round"/>'
    elif kind=='contacts':
        s+='<g transform="rotate(-8 32 32)"><rect x="8" y="8" width="46" height="47" rx="3" fill="#E8DFCA" stroke="#736B56"/><path d="M8 8h46v8H8Z" fill="url(#gold)"/><path d="M16 4v11M26 4v11M36 4v11M46 4v11" stroke="#BEB8A6" stroke-width="4"/>'+circle(24,29,7,'#BE895F')+'<path d="M13 49v-5q0-10 11-10t11 10v5" fill="#6F9A9B"/><path d="M39 27h9M39 35h9M39 43h9" stroke="#797C71" stroke-width="2"/></g>'
    elif kind=='mail':
        s+='<g transform="rotate(-10 32 33)"><rect x="5" y="15" width="54" height="36" rx="2" fill="url(#surface)" stroke="#6C7677" stroke-width="1.3"/><path d="M6 49 26 31M58 49 38 31" fill="none" stroke="#A6ADA6"/><path d="m5 15 27 22 27-22" fill="#FAFBF6" stroke="#8A9699"/><path d="M8 15h48" stroke="#C4D9E6" stroke-width="3"/><path d="M8 51h48" stroke="#7598AD" stroke-width="2"/></g>'
    elif kind=='calendar':
        s+='<g transform="rotate(3 32 32)"><rect x="8" y="8" width="48" height="50" rx="3" fill="url(#surface)" stroke="#727A77"/><path d="M8 9h48v14H8Z" fill="url(#blue)"/><path d="M20 4v12M44 4v12" stroke="#D5DDD9" stroke-width="4" stroke-linecap="round"/>'+text(32,48,'28',30,'#20282B','bold','middle')+'</g>'
    elif kind=='launcher':
        s+=circle(32,33,27,'url(#dark)','#B5C0B9',1.2)+circle(32,33,23,'none','#131B20',1.2)+glyph('up',17,15,31,'#E4EAE4',1.8)
    elif kind=='web':
        s+=circle(32,32,26,'url(#blue)','#214C65',1.3)+'<path d="m23 9-8 10 5 5-2 10 13 3 4 18 7-8-3-10 11-10-5-7-9 2-4-8Z" fill="#94B95D" stroke="#698A44" stroke-width="1"/><path d="M11 16q10-13 24-8" stroke="#D7F5F9" stroke-width="3" fill="none" opacity=".8"/>'
    elif kind=='messages':
        s+='<path d="M5 10q28-9 54 0v31q-7 5-19 5L27 58l1-12Q12 47 5 41Z" fill="url(#green)" stroke="#52733B"/><path d="M12 14q20-6 40 0" stroke="#EEF8D3" stroke-width="2" fill="none"/>'+text(32,35,'···',28,'#F5F9E4','bold','middle')
    elif kind=='camera':
        s+='<path d="M6 18h12l5-7h19l5 7h11v35H6Z" fill="url(#dark)" stroke="#BBC1BE" stroke-width="1.5"/><rect x="7" y="20" width="50" height="7" fill="#B8BDB6"/>'+circle(33,37,15,'#182126','#87999C',3)+circle(33,37,10,'url(#blue)','#111B21',2)+circle(29,33,3,'#D9F2F6')+rect(48,30,6,4,'#D6E5AF',1)
    elif kind=='photos':
        s+='<g transform="rotate(-9 32 32)"><rect x="8" y="11" width="46" height="42" fill="#F4F2E9" stroke="#A6ACA9"/><rect x="12" y="15" width="38" height="30" fill="url(#blue)"/><path d="m12 42 13-19 10 11 7-6 8 17H12Z" fill="#729151"/><circle cx="41" cy="22" r="5" fill="#EDCD78"/></g>'
    elif kind=='music':
        s+='<path d="M21 14 50 6v37" fill="none" stroke="#B9C8C8" stroke-width="8" stroke-linejoin="round"/><path d="M21 14v38" stroke="#CAD5D0" stroke-width="6"/>'+circle(15,51,10,'url(#blue)','#4D6A7B',1)+circle(44,44,10,'url(#blue)','#4D6A7B',1)+'<path d="m23 14 25-7" stroke="#E6EDE7" stroke-width="2"/>'
    elif kind=='clock':
        s+=circle(32,32,27,'url(#dark)','#B4BCB8',2)+circle(32,32,22,'#E9ECE4','#182429',1)+'<path d="M32 14v19l12 7" fill="none" stroke="#263337" stroke-width="3" stroke-linecap="round"/>'+circle(32,32,3,'#B59C68')
        for a in range(12):
            aa=a*math.pi/6;s+=line(32+18*math.sin(aa),32-18*math.cos(aa),32+20*math.sin(aa),32-20*math.cos(aa),'#56676A',1.5)
    elif kind=='notes':
        s+='<g transform="rotate(5 32 32)"><path d="M9 7h46v50H9Z" fill="#E8D595" stroke="#A79666"/><path d="M9 8h46v9H9Z" fill="#BE9E5A"/><path d="M19 5v15M29 5v15M39 5v15M49 5v15" stroke="#5D6664" stroke-width="3"/><path d="M16 28h31M16 36h31M16 44h20" stroke="#B9AB76" stroke-width="1.2"/></g>'
    elif kind=='settings':
        s+='<g fill="#AEBAB9" stroke="#4D6267" stroke-width="1.3">'
        for a in range(8):s+=f'<rect x="27" y="3" width="10" height="15" rx="2" transform="rotate({a*45} 32 32)"/>'
        s+=circle(32,32,21,'url(#chrome)','#51686D',1.7)+circle(32,32,10,'url(#blue)','#52686B',2)+'</g>'
    elif kind=='files':
        s+='<path d="M5 15h23l6 6h25v34H5Z" fill="#A9844E" stroke="#665740"/><path d="M5 24h54l-5 31H1Z" fill="url(#gold)" stroke="#8A724C"/><path d="M8 28h44" stroke="#F4DA95" stroke-width="2"/>'
    elif kind=='terminal':
        s+=rect(5,9,54,45,'url(#dark)',4,'#ADBDB8',1.5)+rect(7,11,50,6,'#647671',2)+text(13,43,'>_',25,'#BDDEAA','bold',family='DejaVu Sans Mono')
    elif kind=='store':
        s+='<path d="M13 17h38l5 40H8Z" fill="#D9CBA9" stroke="#8A8069"/><path d="M23 23V12q9-15 18 0v11" fill="none" stroke="#8D9B99" stroke-width="4"/>'+text(32,48,'+',29,'#F8FAEF','bold','middle')
    else:
        s+=circle(32,32,26,'url(#blue)')+glyph('plus',20,20,24)
    return s+'</g>'

def status(title='Tarot',menu=False,light=True):
    s=rect(0,0,360,32,'#030505')
    if menu:s+=rect(0,1,min(128,29+len(title)*7.3),30,'url(#dark)',4)
    s+=text(9,22,title,15,'#F6F7F4','bold')
    if menu:s+=f'<path d="m{15+len(title)*7.4} 14 7 0-3.5 4Z" fill="#BEC6C2"/>'
    s+=text(180,22,'9:41',16,'#FFFFFF','bold','middle')
    s+=text(278,21,'4G',10.5,'#EBF0E8','bold','middle')
    for i in range(4):s+=rect(290+i*4.6,22-(i+1)*3.6,3.2,(i+1)*3.6,'#EDF4E7',.5)
    s+=glyph('wifi',311,8,17,'#EDF4E7',2)
    s+=rect(336,7,14,20,'none',2,'#C3CCC2',1)+rect(339,10,8,14,'#9BCB76',1)+rect(340,4,6,3,'#BCC8B9',.6)
    return s

def gesture(active=False):
    s=rect(0,684,360,36,'#070D10')
    s+=line(0,684,360,684,'#374345',.55)
    if active:s+='<ellipse cx="180" cy="702" rx="53" ry="13" fill="url(#halo)" opacity=".7"/>'
    s+=rect(161,701,38,2,'#B9D3D2' if active else '#536C70',1)
    return s

def frame_corners(top=32,bottom=684,r=11):
    return f'<g fill="#030505"><path d="M0 {top}H{r}Q0 {top} 0 {top+r}Z"/><path d="M360 {top}H{360-r}Q360 {top} 360 {top+r}Z"/><path d="M0 {bottom}H{r}Q0 {bottom} 0 {bottom-r}Z"/><path d="M360 {bottom}H{360-r}Q360 {bottom} 360 {bottom-r}Z"/></g>'

def just_type():
    return rect(9,40,342,40,'#0A1519',20,'#BDCCC3',.6,extra='fill-opacity=".25" stroke-opacity=".38"')+text(25,67,'Just type...',20,'#DAE1D8',extra='font-style="italic" opacity=".78"')+glyph('search',318,49,21,'#D4DDD3',2)

def dock(wave=False):
    s=''
    if not wave:
        s+=rect(3,611,354,69,'url(#tray)',18,'#C5CFC5',.8)+line(20,612,340,612,'#E5EADF',.7)
        for i,k in enumerate(['phone','contacts','mail','calendar','launcher']):s+=icon(k,10+i*71,619,56)
    else:
        s+='<path d="M-5 651 C61 654 79 580 142 574 C205 565 241 610 365 654 L365 684H-5Z" fill="url(#tray)" stroke="#E1E7D9" stroke-opacity=".7" stroke-width=".8"/>'
        ys=[626,593,562,593,626]
        for i,k in enumerate(['phone','contacts','mail','calendar','launcher']):
            if i==2:s+='<ellipse cx="180" cy="594" rx="51" ry="48" fill="url(#halo)"/>'
            s+=icon(k,10+i*71,ys[i],56)
        s+=text(180,551,'Email',15,'#F7F4E6','bold','middle',extra='filter="url(#tinyShadow)"')
    return s

def app_header(title,kind=None,right=None):
    s=rect(0,0,360,56,'url(#chrome)')+line(0,55.5,360,55.5,'#9BA39B',1)+line(0,1,360,1,'#F7F8F1',1)
    if kind:s+=icon(kind,9,9,35);x=51
    else:x=16
    s+=text(x,37,title,24,'#303936')
    if right:s+=text(341,35,right,15,'#5B777E','bold','end')
    return s

def command_bar(y=596):
    s=rect(0,y,360,56,'url(#chrome)')+line(0,y,360,y,'#9FA69F',1)
    s+=rect(118,y+8,124,39,'url(#dark)',18,'#BCC5BA',.7)
    s+=glyph('refresh',134,y+16,21,'#DDE7D8')+line(178,y+10,178,y+44,'#151C1C',.8)+glyph('pencil',199,y+16,21,'#DDE7D8')
    return s

def email_content(height=652):
    s=rect(0,0,360,height,'#E5E8DF')+app_header('Inbox','mail','Edit')
    s+=rect(9,64,342,35,'#D2D7CC',17,'#ADB6AC',.8)+glyph('search',20,72,18,'#6D7D78')+text(47,88,'Search mail',17,'#596A55')
    s+=rect(0,107,360,25,'#BCC5B6')+text(14,125,'TODAY',12,'#536B60','bold')
    rows=[('Mara Ellis','Saturday by the water','The weather looks good. Shall we meet...', '9:28',True),('Jun Park','Photos from the coast','I finally sorted the pictures from our trip.', '8:52',True),('The Bookshop','Your order is ready','Your books are waiting at the front desk.', '8:10',False),('Daniel Moore','Dinner on Friday','I booked a table for six. See you there.', '7:46',False),('Mara Ellis','A little list','Tea, bread, lemons, and something sweet.', 'Yesterday',False),('Garden Club','A new season','Our next gathering is on Sunday.', 'Yesterday',False)]
    y=132
    for name,subject,body,tm,unread in rows:
        if y+73>height-56:break
        s+=rect(0,y,360,73,'url(#row)')+line(0,y+72,360,y+72,'#C4CCC0',.8)
        if unread:s+=circle(12,y+20,3.2,'#628CA0')
        s+=text(24,y+22,name,18,'#2D3933','bold' if unread else 'normal')+text(345,y+21,tm,12,'#65765D',anchor='end')
        s+=text(24,y+44,subject,16,'#435247','bold' if unread else 'normal')+text(24,y+63,body,13.4,'#65725F')
        y+=73
    s+=command_bar(height-56)
    return s

def calendar_content(height=652):
    s=rect(0,0,360,height,'#F1F0E7')+app_header('Wednesday 28','calendar')
    s+=rect(0,56,360,35,'url(#button)')+text(180,80,'October 2026',17,'#46544B','bold','middle')+glyph('back',15,63,18,'#738177')+glyph('chevron',326,63,18,'#738177')
    for i,h in enumerate(range(8,18)):
        y=91+i*45
        if y>height-56:break
        s+=text(12,y+16,f'{h:02}:00',12,'#859083')+line(58,y+5,360,y+5,'#D0D4C8',.6)
    s+=rect(64,163.5,285,45,'#D3E7DF',5,'#97B6AD',.6)+rect(64,163.5,4,45,'#6F9F91',2)+text(77,183,'Coffee with Jun',17,'#405D54','bold')+text(77,201,'9:30 - 10:30',13,'#688377')
    s+=rect(64,298.5,285,67.5,'#E7DFC2',5,'#BAAE83',.6)+rect(64,298.5,4,67.5,'#AB965D',2)+text(77,322,'Walk by the water',17,'#645C3B','bold')+text(77,346,'12:30 - 14:00',13,'#857A50')
    yy=height-56
    s+=rect(0,yy,360,56,'url(#chrome)')+line(0,yy,360,yy,'#9FA69F',1)+rect(75,yy+8,209,39,'url(#dark)',18,'#BCC5BA',.7)
    for j,lab in enumerate(['Day','Week','Month']):
        xx=110+j*70;s+=text(xx,yy+33,lab,15,'#EFF2E7' if j==0 else '#B8C5B0','bold' if j==0 else 'normal','middle')
        if j<2:s+=line(xx+35,yy+11,xx+35,yy+44,'#172122',.8)
    return s

def message_content(height=652):
    s=rect(0,0,360,height,'#DADFD2')+app_header('Mara Ellis','messages')+text(180,82,'Today',13,'#7C8877',anchor='middle')
    for x,y,w,h,txt,fill in [(14,99,278,72,'Shall we take the long way home?','#F4F5EB'),(69,191,276,52,'Yes. Meet you by the water.','#C4D6B4'),(14,268,280,72,'I will bring the map and some tea.','#F4F5EB')]:
        s+=rect(x,y,w,h,fill,10,'#AEB9A5',.7)
        if h>60:
            words=txt.split();mid=5 if len(words)>8 else 4
            s+=text(x+13,y+27,' '.join(words[:mid]),17,'#40513D')+text(x+13,y+51,' '.join(words[mid:]),17,'#40513D')
        else:s+=text(x+13,y+32,txt,17,'#40513D')
    s+=rect(9,height-60,342,48,'url(#row)',8,'#A7B39E',.8)+text(24,height-29,'Write a message...',17,'#86927E')+glyph('plus',314,height-47,22,'#6B8171')
    return s

def web_content(height=652):
    s=rect(0,0,360,height,'#F6F4EB')+app_header('The quiet coast','web')
    s+=rect(10,66,340,33,'#E3E6DB',7,'#B7C0B0',.7)+text(24,88,'example.org / coast',15,'#7C8778')
    s+=rect(0,110,360,214,'#859F99')+'<path d="M0 230Q120 160 240 250T400 240V324H0Z" fill="#607E7B"/><path d="M0 292Q150 237 360 286V324H0Z" fill="#CDD0B6"/>'
    s+=text(24,366,'A day beside',30,'#394D45',family='Instrument Serif')+text(24,399,'the water',30,'#394D45',family='Instrument Serif')
    for i,tx in enumerate(['Take the path that follows the coast.','There is time to stop, look around,','and take the long way home.']):s+=text(24,438+i*25,tx,17,'#6C7B6A')
    s+=command_bar(height-56)
    return s

def group_box(x,y,w,label,rows,accent='#537B8D'):
    hh=28+len(rows)*51
    s=rect(x,y,w,hh,'url(#row)',10,'#A7AFA4',1)+rect(x+1,y+1,w-2,28,'#ACB6A5',9)+rect(x+1,y+18,w-2,10,'#ACB6A5')+text(x+11,y+20,label,13,'#F8FAF2','bold')
    for i,(lab,value,typ) in enumerate(rows):
        yy=y+28+i*51
        if i:s+=line(x+1,yy,x+w-1,yy,'#CCD2C5',.8)
        s+=text(x+13,yy+33,lab,19,'#34432F')
        if typ=='toggle':
            on=value=='On';s+=rect(x+w-80,yy+14,65,28,'url(#blue)' if on else '#A4ADA0',14,'#7E8D7A',.8)+circle(x+w-(28 if on else 66),yy+28,12,'url(#button)','#B1BAAB',.7)+text(x+w-(57 if on else 37),yy+32,'ON' if on else 'OFF',10,'#F9FFF3','bold','middle')
        elif typ=='chevron':s+=text(x+w-34,yy+32,value,15,'#71866D',anchor='end')+glyph('chevron',x+w-25,yy+17,17,'#9AA891')
        else:s+=text(x+w-15,yy+32,value,16,accent,anchor='end')
    return s

def settings_content(height=652):
    s=rect(0,0,360,height,'#DFE3D8')+app_header('Screen & Lock','settings')
    s+=rect(10,67,340,130,'url(#row)',10,'#A7AFA4',1)+rect(11,68,338,28,'#ACB6A5',9)+rect(11,85,338,10,'#ACB6A5')+text(22,87,'SCREEN',13,'#F6F8EF','bold')+text(24,125,'Brightness',20,'#394835')
    s+=rect(27,149,306,5,'#A1B5B6',2)+rect(27,149,178,5,'#76A8BD',2)+circle(205,151,14,'url(#chrome)','#789AA6',1)+circle(205,151,9,'#A6CDDC')
    s+=group_box(10,208,340,'DISPLAY',[('Turn off after','1 minute','chevron'),('Wallpaper','Nocturne','chevron')])
    s+=group_box(10,349,340,'GESTURES',[('Switch applications','Off','toggle'),('Gesture hints','On','toggle')])
    s+=group_box(10,490,340,'SECURITY',[('Screen lock','PIN','chevron')])
    s+=text(21,597,'Start gestures from the bottom of any app.',14,'#63725B')
    return s

CONTENTS={'email':email_content,'calendar':calendar_content,'messages':message_content,'web':web_content,'settings':settings_content}

def card(kind,x,y,scale=.66,angle=0,opacity=1):
    cont=CONTENTS[kind]()
    return f'<g transform="translate({x} {y}) rotate({angle} {180*scale} {326*scale})" opacity="{opacity}"><g transform="scale({scale})">{rect(0,0,360,652,"#E9EDE3",11,extra="filter=\"url(#shadow)\"")}<g clip-path="url(#clientClip)">{cont}</g>{rect(.5,.5,359,651,"none",11,"#DAE3D7",1)}</g></g>'

def overview(theme='nocturne',stack=False,empty=False):
    s=wallpaper(theme)+status()+just_type()
    if not empty:
        if stack:
            s+=card('calendar',-209,164,.61,-2,.9)
            s+=card('email',43,151,.64,-5,.95)+card('messages',62,145,.64,0,.98)+card('web',81,151,.64,5,1)
        else:
            s+=card('calendar',-204,160,.61,0,.9)+card('messages',345,160,.61,0,.9)+card('email',61.2,139,.66)
    s+=dock()+gesture()
    return s

def launcher(theme='nocturne'):
    s=wallpaper(theme)+rect(0,32,360,652,'#102327',extra='fill-opacity=".50"')+status('Launcher',True)
    s+=text(180,70,'Applications',21,'#F2F4EA',anchor='middle',extra='filter="url(#tinyShadow)"')
    s+=glyph('back',15,50,18,'#63756C')+glyph('chevron',327,50,18,'#BFCABE')
    apps=[('phone','Phone'),('web','Web'),('store','Software'),('mail','Email'),('calendar','Calendar'),('messages','Messaging'),('camera','Camera'),('photos','Photos'),('music','Music'),('clock','Clock'),('notes','Memos'),('settings','Settings'),('contacts','Contacts'),('files','Files'),('terminal','Terminal')]
    for i,(kind,title) in enumerate(apps):
        col=i%3;row=i//3;x=29+col*122;y=95+row*112
        s+=icon(kind,x,y,58)+text(x+29,y+77,title,14,'#F3F4E9',anchor='middle',extra='filter="url(#tinyShadow)"')
    s+='<path d="m174 662 12 0-6 7Z" fill="#A8BBAA"/>'
    s+=frame_corners()+gesture()
    return s

def full_app(kind='email',theme='nocturne',banner=False,dashboard=False,wave=False,device_menu=False,app_menu=False):
    title={'email':'Email','calendar':'Calendar','messages':'Messaging','web':'Web','settings':'Preferences'}[kind]
    s=wallpaper(theme)+status(title,True)
    h=620 if banner else (469 if dashboard else 652)
    s+=f'<g transform="translate(0 32)">{CONTENTS[kind](h)}</g>'
    s+=frame_corners(32,32+h)
    if banner:
        s+=rect(0,652,360,32,'#101719')+icon('messages',5,654,27)+text(40,674,'Mara: I will bring the map.',15,'#EFF3E8')
    if dashboard:
        s+=rect(0,501,360,183,'#050B0C')
        items=[('messages','Mara Ellis','I will bring the map.',2),('calendar','Walk by the water','Today 12:30',0),('music','The long way home','Juniper / Evening light',0)]
        for i,(ic,a,b,n) in enumerate(items):
            yy=510+i*56;s+=rect(0,yy,360,53,'url(#dark)',12)
            s+=icon(ic,9,yy+7,38)+text(59,yy+23,a,17,'#FAF9F1','bold')+text(59,yy+43,b,14,'#CBD3C7')
            if n:s+=circle(45,yy+9,9,'#8DABBD','#E0E7DD',.6)+text(45,yy+13,str(n),10,'white','bold','middle')
    if wave:s+=rect(0,32,360,652,'#071015',extra='fill-opacity=".23"')+dock(True)
    if device_menu:
        s+=rect(0,32,360,652,'#000',extra='fill-opacity=".30"')
        s+=rect(107,32,249,337,'url(#menu)',10,'#88958C',.65,extra='filter="url(#shadow)"')+rect(107,32,249,15,'url(#menu)')
        s+=text(125,63,'Wednesday, October 28',16,'#E4EBE0','bold')+text(125,89,'Battery 82%',15,'#C5D0C0')+line(116,105,347,105,'#111A1B',1)
        for i,(g,title,val) in enumerate([('wifi','Wi-Fi','Home'),('bluetooth','Bluetooth','On'),('plane','Airplane mode','Off')]):
            yy=119+i*57;s+=glyph(g,124,yy+3,22,'#D4DFCD')+text(158,yy+21,title,17,'#F4F6EF')+text(334,yy+41,val,12,'#ADBFA5',anchor='end')+line(116,yy+49,347,yy+49,'#172122',1)
        s+=glyph('sun',124,302,21,'#D5DDCD')+rect(158,311,167,5,'#141E20',2)+rect(158,311,105,5,'#9AAB91',2)+circle(263,313,11,'url(#chrome)','#9AA890',.6)
        s+=text(233,353,'Preferences',15,'#D3DED0',anchor='middle')
    if app_menu:
        s+=rect(0,32,360,652,'#000',extra='fill-opacity=".25"')+rect(4,32,235,259,'url(#menu)',10,'#89968D',.65,extra='filter="url(#shadow)"')+rect(4,32,235,10,'url(#menu)')
        for i,tt in enumerate(['Edit','New message','Preferences & Accounts','Help']):
            yy=41+i*58;s+=text(21,yy+32,tt,18,'#F2F5EA')
            if i==0:s+=glyph('chevron',209,yy+19,16,'#BECBB6')
            if i<3:s+=line(11,yy+51,232,yy+51,'#131D1E',1)
    s+=gesture(wave)
    return s

def keyboard(y=464):
    s=rect(0,y,360,220,'#9FA8A2')+line(0,y,360,y,'#E5EBDF',1)
    for row,chars in enumerate(['qwertyuiop','asdfghjkl','zxcvbnm']):
        xx=3 if row==0 else (21 if row==1 else 57)
        for i,ch in enumerate(chars):
            x=xx+i*35.5;yy=y+10+row*48
            s+=rect(x,yy,32,42,'url(#button)',5,'#737F73',.75)+text(x+16,yy+29,ch,22,'#303D30',anchor='middle')
    s+=rect(3,y+106,46,42,'url(#dark)',5)+text(26,y+135,'⇧',22,'#F7F7ED',anchor='middle')
    s+=rect(309,y+106,48,42,'url(#dark)',5)+glyph('back',321,y+117,22,'#F2F6E9')
    for x,w,tx in [(3,54,'?123'),(61,39,','),(104,153,'space'),(261,36,'.'),(301,56,'Go')]:
        s+=rect(x,y+159,w,47,'url(#dark)' if tx=='Go' else 'url(#button)',5,'#74816F',.7)+text(x+w/2,y+189,tx,15,'#F5F7ED' if tx=='Go' else '#59634F','bold' if tx=='Go' else 'normal','middle')
    return s

def search_screen(theme='nocturne'):
    s=wallpaper(theme)+status('Just Type',True)+rect(0,32,360,432,'#E6EADF')
    s+=rect(10,43,340,41,'#F6F8EE',20,'#A7B2A1',.9)+glyph('search',24,54,19,'#779174')+text(53,71,'Mara',20,'#34462E')+line(96,51,96,75,'#5C8172',1.2)+glyph('close',320,56,15,'#9BAC98')
    for y,tx in [(99,'CONTACTS'),(225,'CONTENT'),(332,'ACTIONS')]:s+=rect(0,y,360,25,'#C1CCB8')+text(14,y+18,tx,12,'#5F7654','bold')
    s+=icon('contacts',13,136,48)+text(74,154,'Mara Ellis',20,'#354C2E','bold')+text(74,179,'Mobile  ·  Send a message',14,'#6B8362')
    s+=icon('photos',15,261,43)+text(73,286,'Photos',20,'#354C2E')+text(73,307,'3 photos with Mara',13,'#7A8E6F')
    s+=text(20,387,'New message',19,'#49673E')+text(20,431,'Search the web for “Mara”',18,'#49673E')
    s+=keyboard()+gesture()
    return s

def notification_rail(theme='nocturne'):
    s=wallpaper(theme)+f'<g transform="translate(0 32)">{email_content(620)}</g>'+frame_corners(32,652)
    s+=rect(0,652,360,32,'#0B1113')+text(14,674,'',14,'#B8C6B5')
    for i,k in enumerate(['messages','calendar','mail']):s+=icon(k,255+i*32,655,25)
    s+=gesture()
    return s

def launch_pending(theme='nocturne',failed=False):
    s=wallpaper(theme)+just_type()
    s+=rect(61,139,238,430,'url(#dark)',8,'#7B9187',.7,extra='filter="url(#shadow)"')
    s+=icon('mail',148,276,64)
    if failed:
        s+=text(180,373,'Email did not open',18,'#EFF2E5','bold','middle')+text(180,401,'Try opening it again.',15,'#BDCEB5',anchor='middle')
        s+=rect(111,438,138,43,'url(#button)',8,'#8A9C83',.8)+text(180,466,'Try again',17,'#45583D','bold','middle')
    else:
        s+=text(180,373,'Email',19,'#EFF2E5','bold','middle')
        for i in range(3):s+=circle(166+i*14,402,3,'#D1DFC7' if i==0 else '#687E66')
    return s+dock()+gesture()

def lock_screen(theme='nocturne',pin=False):
    s=wallpaper(theme)+rect(0,0,360,720,'#010B10',extra='fill-opacity=".18"')
    s+=status('Tarot')
    if not pin:
        s+=text(180,191,'9:41',70,'#F5F4E8','normal','middle',extra='filter="url(#tinyShadow)"')+text(180,225,'Wednesday, October 28',18,'#D4DED0',anchor='middle')
        s+=rect(16,280,328,56,'#122629',12,'#536C66',.6,extra='fill-opacity=".74"')+icon('messages',25,290,35)+text(76,303,'Messaging',16,'#ECF1E5','bold')+text(76,323,'2 new messages',14,'#B9CAB8')
        s+='<circle cx="180" cy="578" r="61" fill="#101E23" fill-opacity=".34" stroke="#CFD8C5" stroke-opacity=".16"/>'
        s+=circle(180,594,32,'url(#dark)','#C3CFC2',1.2)+glyph('lock',167,579,26,'#E4EADB',1.8)
        s+=text(180,651,'Drag up to unlock',16,'#BFCDBC',anchor='middle')
    else:
        s+=text(180,140,'Enter PIN',28,'#EDF1E4',anchor='middle')
        for x in [144,168,192,216]:s+=circle(x,177,4,'#D8E1D0' if x<192 else 'none','#D8E1D0',1)
        for i in range(9):
            x=47+(i%3)*104;y=225+(i//3)*76;s+=rect(x,y,58,59,'url(#tray)',10,'#AABBA8',.6)+text(x+29,y+40,i+1,29,'#F1F1E3',anchor='middle')
        s+=rect(151,453,58,59,'url(#tray)',10,'#AABBA8',.6)+text(180,494,'0',29,'#F1F1E3',anchor='middle')
        s+=text(78,490,'Cancel',15,'#CFDCC9',anchor='middle')+glyph('back',272,475,21,'#D7E1D0')+text(180,603,'Emergency call',17,'#D4E0CC',anchor='middle')
    s+=gesture()
    return s

def incoming(theme='nocturne',locked=False):
    s=wallpaper(theme)+status('Phone',True)
    if not locked:s+=f'<g transform="translate(0 32)">{email_content(424)}</g>'
    else:s+=text(180,142,'9:41',60,'#F4F3E7',anchor='middle')
    s+=rect(0,456,360,228,'url(#dark)',15,'#747E74',.7)+rect(0,666,360,18,'#202527')
    s+=icon('contacts',18,478,57)+text(89,502,'Mara Ellis',28,'#F4F5E9')+text(89,528,'Mobile',17,'#C0CEB8')+text(89,555,'Incoming call',15,'#A7BA9F')
    s+=rect(17,590,155,65,'url(#red)',10,'#B3988D',.6)+text(94,629,'Decline',21,'#F9ECE6','bold','middle')
    s+=rect(187,590,155,65,'url(#green)',10,'#B7C19C',.6)+text(264,629,'Answer',21,'#F4F8DF','bold','middle')
    return s+gesture()

def incoming_locked(theme='nocturne'):
    s=wallpaper(theme)+rect(0,0,360,720,'#061117',extra='fill-opacity=".2"')
    s+=text(180,163,'Mara Ellis',33,'#F1F1E4',anchor='middle')+text(180,195,'Mobile',17,'#B7CCB6',anchor='middle')
    s+=text(180,225,'Incoming call',16,'#B7CCB6',anchor='middle')
    s+=circle(180,565,64,'#152C2D','#6A8577',.7)+circle(180,596,34,'url(#dark)','#BCCBB5',1)
    s+=icon('phone',157,572,46)+text(180,652,'Drag up to answer',16,'#C4D5BE',anchor='middle')
    return s+gesture()

def ongoing_call(theme='nocturne'):
    s=wallpaper(theme)+status('Phone',True)+rect(0,32,360,652,'#172D28')
    s+=text(180,102,'Mara Ellis',30,'#E6EDDC',anchor='middle')+text(180,133,'Mobile',17,'#9FB797',anchor='middle')
    s+=circle(180,240,67,'#647C6B')+circle(180,218,24,'#B3C1A9')+'<path d="M132 284v-8q0-38 48-38t48 38v8" fill="#B3C1A9"/>'
    s+=text(180,347,'03:24',24,'#DDE8D4',anchor='middle')
    for x,gl,lab in [(29,'speaker','Speaker'),(137,'mic','Mute'),(245,'plus','Add call')]:
        s+=rect(x,395,86,90,'url(#dark)',9,'#73836E',.7)+glyph(gl,x+29,411,27,'#DFE8D1')+text(x+43,470,lab,14,'#BBCBB0',anchor='middle')
    s+=rect(22,572,316,65,'url(#red)',9,'#A69783',.6)+text(180,613,'End call',23,'#FAEDE3','bold','middle')
    return s+gesture()

def boot(theme='nocturne'):
    s=wallpaper(theme)+rect(0,0,360,720,'#06121A',extra='fill-opacity=".50"')
    s+=brand_mark(144,230,72,'#CDB98B')+text(180,393,'TAROT',43,'#F0E9D4','normal','middle',family='Instrument Serif',extra='letter-spacing="6"')
    return s

def brand_mark(x,y,size=64,c='#D0BA87'):
    s=f'<g transform="translate({x} {y}) scale({size/64})" fill="none" stroke="{c}" stroke-width="1.6">'
    s+='<rect x="12" y="7" width="29" height="45" rx="4" transform="rotate(-17 26 49)"/><rect x="21" y="6" width="29" height="45" rx="4" transform="rotate(12 35 49)"/><rect x="18" y="5" width="29" height="45" rx="4" fill="#0B232A"/><path d="m32.5 16 3.5 8 8 3.5-8 3.5-3.5 8-3.5-8-8-3.5 8-3.5Z"/><path d="M23 11h4M38 44h4"/>'
    return s+'</g>'

def svg(body,theme='nocturne',w=360,h=720):
    return f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w*2}" height="{h*2}" viewBox="0 0 {w} {h}">{defs(theme)}<defs><clipPath id="outerClip">{rect(0,0,w,h,"white")}</clipPath></defs><g clip-path="url(#outerClip)">{body}</g></svg>'

screens={
 '01-card-view-nocturne':overview(),
 '02-card-stack':overview(stack=True),
 '03-launcher':launcher(),
 '04-email':full_app('email'),
 '05-banner':full_app('email',banner=True),
 '06-dashboard':full_app('email',dashboard=True),
 '07-wave-launcher':full_app('calendar',wave=True),
 '08-device-menu':full_app('email',device_menu=True),
 '09-app-menu':full_app('email',app_menu=True),
 '10-just-type':search_screen(),
 '11-lock':lock_screen(),
 '12-pin':lock_screen(pin=True),
 '13-incoming-call':incoming(),
 '14-active-call':ongoing_call(),
 '15-settings':full_app('settings'),
 '16-empty-card-view':overview(empty=True),
 '17-heritage':overview('heritage'),
 '18-vellum':overview('vellum'),
 '19-messaging':full_app('messages'),
 '20-boot':boot(),
 '21-calendar':full_app('calendar'),
 '22-notification-rail':notification_rail(),
 '23-launch-pending':launch_pending(),
 '24-launch-failed':launch_pending(failed=True),
 '25-locked-incoming-call':incoming_locked(),
}
titles={'03-launcher':'Launcher','04-email':'Email','05-banner':'Email','06-dashboard':'Email','07-wave-launcher':'Calendar','08-device-menu':'Email','09-app-menu':'Email','10-just-type':'Just Type','13-incoming-call':'Phone','14-active-call':'Phone','15-settings':'Preferences','19-messaging':'Messaging','21-calendar':'Calendar','22-notification-rail':'Email'}
for name,body in screens.items():
    theme='heritage' if name=='17-heritage' else ('vellum' if name=='18-vellum' else 'nocturne')
    if name!='20-boot':body+=status(titles.get(name,'Tarot'),name in titles)
    for old,new in {'#65725F':'#58674F','#65765D':'#58674F','#596A55':'#4E5F48','#7C8877':'#5D6C55','#7A8E6F':'#5D6C55','#71866D':'#5D6C55','#7B8873':'#5D6C55','#63725B':'#5D6C55','#859083':'#5D6C55'}.items():
        body=body.replace(old,new)
    (SCREENS/f'{name}.svg').write_text(svg(body,theme))

for theme in THEMES:
    (OUT/'wallpapers'/f'{theme}.svg').write_text(svg(wallpaper(theme),theme))
mark=f'<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 64 64">{brand_mark(0,0,64)}</svg>'
(OUT/'brand'/'tarot-mark.svg').write_text(mark)
logo=f'<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="280" viewBox="0 0 600 140">{brand_mark(6,23,94)}{text(128,102,"TAROT",83,"#173A40",family="Instrument Serif",extra="letter-spacing=\"12\"")}</svg>'
(OUT/'brand'/'tarot-wordmark.svg').write_text(logo)

components=rect(0,0,720,216,'#E5E8DF')
for i,label in enumerate(['Default','Pressed','Disabled','Busy']):
    x=18+i*174
    components+=text(x+75,27,label,15,'#596B53','bold','middle')
    components+=rect(x,43,150,42,'#31534D' if i==1 else ('#D2D7CA' if i==2 else 'url(#button)'),8,'#8B9B81',.9)
    components+=text(x+75,70,'Connecting...' if i==3 else 'Connect',16,'#F3F6EA' if i==1 else ('#98A28E' if i==2 else '#3F5137'),'bold','middle')
    components+=text(x+75,127,['Off','On','Pending','Unavailable'][i],15,'#596B53','bold','middle')
    components+=rect(x+39,145,72,30,'url(#blue)' if i==1 else '#AAB39F',15,'#7D8F73',.8)
    knob=x+(94 if i==1 else 55)
    components+=circle(knob,160,13,'url(#button)' if i!=3 else '#C4CBBC','#A5B09C',.7)
    components+=text(x+(61 if i==1 else 88),164,'ON' if i==1 else 'OFF',10,'#F3F5E9','bold','middle')
    if i==2:components+='<path d="M'+str(x+125)+' 154a8 8 0 1 1-6 13" fill="none" stroke="#486C5D" stroke-width="2"/>'
    components+=text(x+75,204,['Ready to change','Confirmed by service','Waiting for service','Control is unavailable'][i],11,'#596B53',anchor='middle')
(OUT/'components'/'control-states.svg').write_text(svg(components,'nocturne',720,216))
for name in ['phone','contacts','mail','calendar','launcher','web','messages','camera','photos','music','clock','notes','settings','files','terminal','store']:
    (OUT/'icons'/f'{name}.svg').write_text(svg(icon(name,0,0,64),'nocturne',64,64))
for name in ['search','chevron','back','plus','check','close','up','wifi','bluetooth','plane','speaker','mic','lock','refresh','trash','pencil','power','sun']:
    (OUT/'glyphs'/f'{name}.svg').write_text(svg(glyph(name,2,2,24,'#E4EAE0'),'nocturne',28,28))

# These are proposed Tarot dimensions, intentionally distinct from measured historical values.
tokens={
 'schema_version':1,
 'status':'design proposal; calibrate against one captured webOS 2.x build before claiming conformance',
 'reference':'Palm Pre 2 webOS 2.x phone interaction model',
 'units':{'layout':'logical pixels','default_scale':2,'output_px':[720,1440],'canvas_dp':[360,720]},
 'geometry':{'status_bar_h':32,'gesture_strip_h':36,'app_rect':[0,32,360,652],'header_h':56,'command_bar_h':56,'list_row_min_h':51,'primary_action_touch_min':48,'compact_target_policy':'status, gesture and keyboard have equivalent accessible routes; do not silently extend hitboxes over clients','card_scale':0.66,'inactive_card_scale':0.61,'card_radius_before_scale':11,'card_anchor':[61.2,139],'stack_preview_poses':[{'x':43,'y':151,'scale':0.64,'degrees':-5},{'x':62,'y':145,'scale':0.64,'degrees':0},{'x':81,'y':151,'scale':0.64,'degrees':5}],'quick_launch_rect':[3,611,354,69],'quick_launch_slots':5,'quick_launch_icon_size':56,'launcher_columns':3,'launcher_icon_size':58,'banner_h':32,'banner_accessible_h':48,'dashboard_row_h':56,'keyboard_h':220},
 'type':{'mockup_ui_family':'Lato','historical_target_family':'Prelude','brand_family':'Instrument Serif','status':15,'clock':16,'header':24,'body':19,'secondary':15,'metadata':13,'launcher_label':14,'font_match_pending':True},
 'fixed_palette':{'status':'#030505','gesture':'#070D10','app_surface':'#E5E8DF','app_text':'#2D3933','muted_text':'#58674F','divider':'#C4CCC0','success':'#34733D','destructive':'#9B302C','dashboard_text':'#F4F6EF','dashboard_bg':'#202527'},
 'themes':THEMES,
 'motion_ms':{'maximize':300,'minimize':350,'launch':400,'dismiss':300,'launcher':350,'quick_launch':350,'press_feedback':70,'reduced_motion_fade':100},
 'motion_status':'starting values from later LunaSysMgr config, not a captured Pre 2 timing match',
 'gesture_proposals':{'start_zone':'reserved strip only for shell navigation','tap_slop_dp':10,'axis_lock_dp':14,'back_min_dp':54,'home_flick_velocity_dp_s':700,'wave_dwell_ms':140,'wave_drag_min_dp':24,'reorder_hold_ms':420,'card_dismiss_distance_h':0.28,'card_dismiss_velocity_dp_s':900,'advanced_swipe_min_dp':240,'cancel_on_second_touch':True,'calibration_pending':True},
 'theme_invariants':['status position and black fill','card geometry','menu anchors','gesture semantics','notification placement','icon slot count','text contrast','security state'],
 'content_disclaimer':'Fictional contacts and app data. App interiors show cooperative design targets, not automatic GTK or Qt restyling.'
}
(OUT/'tokens'/'tarot.tokens.json').write_text(json.dumps(tokens,indent=2))
print('Wrote',len(screens),'editable screens to',SCREENS)
