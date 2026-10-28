from pathlib import Path
from html import escape
from math import sin,cos,pi
import json, math, re, shutil
from PIL import Image
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.lib.colors import HexColor, Color
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import Paragraph, Table, TableStyle

ROOT=Path(__file__).resolve().parent.parent
AS=ROOT
PNG=AS/'screens'/'png'
PDF=ROOT/'Tarot_Design_Packet.pdf'
PDF.parent.mkdir(parents=True,exist_ok=True)
TMP=ROOT/'tmp'/'pdfs';TMP.mkdir(parents=True,exist_ok=True)
for n,f in [('Lato','Lato-Regular.ttf'),('LatoBold','Lato-Bold.ttf'),('LatoLight','Lato-Light.ttf'),('LatoItalic','Lato-Italic.ttf'),('Instrument','InstrumentSerif-Regular.ttf')]:
    pdfmetrics.registerFont(TTFont(n,str(AS/'fonts'/f)))
pdfmetrics.registerFontFamily('Lato',normal='Lato',bold='LatoBold',italic='LatoItalic',boldItalic='LatoBold')

W,H=1000,650
INK='#17373C'; MUTED='#5D7472'; PAPER='#F1F0E9'; GOLD='#A5814C'; DARK='#0B242B'; LINE='#C8D0C5'; WHITE='#F4F3E8'
c=canvas.Canvas(str(PDF),pagesize=(W,H),pageCompression=1)
c.setTitle('Tarot Interface Design Packet')
c.setAuthor('Tarot design collaboration')
c.setSubject('Faithful webOS 2.x phone shell design for Nura on PinePhone Pro')
page_no=0
overflow=[]
page_titles=[]

def fill(col):c.setFillColor(HexColor(col))
def stroke(col):c.setStrokeColor(HexColor(col))
def box(x,y,w,h,col,rad=0,border=None,lw=.6):
    fill(col)
    if border:stroke(border);c.setLineWidth(lw)
    if rad:c.roundRect(x,H-y-h,w,h,rad,fill=1,stroke=1 if border else 0)
    else:c.rect(x,H-y-h,w,h,fill=1,stroke=1 if border else 0)
def rule(x,y,w,col=LINE,lw=.7):
    stroke(col);c.setLineWidth(lw);c.line(x,H-y,x+w,H-y)
def txt(x,y,s,size=12,font='Lato',col=INK,anchor='left',track=None):
    c.saveState()
    fill(col)
    if track is None:
        c.setFont(font,size)
        if anchor=='right':c.drawRightString(x,H-y-size*.78,s)
        elif anchor=='center':c.drawCentredString(x,H-y-size*.78,s)
        else:c.drawString(x,H-y-size*.78,s)
    else:
        t=c.beginText(x,H-y-size*.78);t.setFont(font,size);t.setCharSpace(track);t.textOut(s);c.drawText(t)
    c.restoreState()
def para(x,y,w,s,size=13,leading=None,col=INK,bold=False,max_bottom=598):
    st=ParagraphStyle('body',fontName='LatoBold' if bold else 'Lato',fontSize=size,leading=leading or size*1.36,textColor=HexColor(col),spaceAfter=0,allowWidows=0,allowOrphans=0)
    p=Paragraph(s,st);pw,ph=p.wrap(w,1000)
    c.saveState();p.drawOn(c,x,H-y-ph);c.restoreState()
    if y+ph>max_bottom:overflow.append((page_no,s[:70],round(y+ph,1)))
    return y+ph
def bullets(x,y,w,items,size=12.5,gap=11,col=INK):
    for i,tx in enumerate(items):
        txt(x,y+1,'•',size,col=GOLD)
        y=para(x+16,y,w-16,tx,size,col=col)+gap
    return y
def num(x,y,n,col=GOLD):
    fill(col);c.circle(x,H-y,10,fill=1,stroke=0);txt(x,y-5.6,str(n),11,'LatoBold',PAPER,'center')
def path(points,col=GOLD,width=1):
    stroke(col);c.setLineWidth(width);p=c.beginPath();p.moveTo(points[0][0],H-points[0][1])
    for x,y in points[1:]:p.lineTo(x,H-y)
    c.drawPath(p)
def arrow(x1,y1,x2,y2,col=GOLD,width=1.5):
    path([(x1,y1),(x2,y2)],col,width)
    a=math.atan2(y2-y1,x2-x1);l=6
    path([(x2-l*math.cos(a-.5),y2-l*math.sin(a-.5)),(x2,y2),(x2-l*math.cos(a+.5),y2-l*math.sin(a+.5))],col,width)
def image_file(p,x,y,w,h=None):
    p=Path(p)
    if h is None:
        im=Image.open(p);h=w*im.height/im.width
    c.drawImage(str(p),x,H-y-h,width=w,height=h,mask='auto')
    return h
def screen(name,x,y,w=190,device=False):
    p=PNG/(name+'.png');h=w*2
    if device:
        box(x-8,y-17,w+16,h+34,'#101719',21,border='#7B8B86',lw=.5)
        box(x+w*.40,y-11,w*.20,2,'#55625F',1)
    image_file(p,x,y,w,h)
    return h
def crop(name,region,x,y,w,h=None):
    src=PNG/(name+'.png');fn=TMP/(name+'_'+'_'.join(map(str,region))+'.png')
    if not fn.exists():Image.open(src).crop(region).save(fn)
    return image_file(fn,x,y,w,h)
def heading(section,title,sub='',dark=False):
    global page_no
    page_no+=1;page_titles.append(title)
    box(0,0,W,H,DARK if dark else PAPER)
    col=WHITE if dark else INK;mut='#A6BCB5' if dark else MUTED
    txt(44,28,'TAROT  /  INTERFACE DESIGN',10,'LatoBold',mut,track=1.3)
    txt(956,28,section.upper(),10,'LatoBold',mut,anchor='right')
    rule(44,52,912,'#37515A' if dark else LINE)
    txt(44,73,title,37,'Instrument',col)
    if sub:para(45,120,895,sub,12.6,col=mut,max_bottom=160)
    return col,mut
def footer(refs='',dark=False):
    col='#A3B7B0' if dark else MUTED
    rule(44,607,912,'#37515A' if dark else LINE)
    txt(44,621,'DESIGN PROPOSAL  /  v0.1  /  28 OCT 2026',8.4,'LatoBold',col,track=.6)
    if refs:txt(498,621,refs,8.6,'Lato',col,'center')
    txt(956,620,f'{page_no:02}',11,'LatoBold',col,'right')
    c.showPage()
def table(x,y,width,headers,rows,weights=None,fs=11.2):
    weights=weights or [1]*len(headers);ww=[width*z/sum(weights) for z in weights]
    sty=ParagraphStyle('cell',fontName='Lato',fontSize=fs,leading=fs*1.31,textColor=HexColor(INK))
    hd=ParagraphStyle('head',fontName='LatoBold',fontSize=fs,leading=fs*1.25,textColor=HexColor(WHITE))
    data=[[Paragraph(str(v),hd) for v in headers]]+[[Paragraph(str(v),sty) for v in row] for row in rows]
    t=Table(data,colWidths=ww,hAlign='LEFT')
    t.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),HexColor(INK)),('ROWBACKGROUNDS',(0,1),(-1,-1),[HexColor('#F7F7EF'),HexColor('#E6EAE0')]),('GRID',(0,0),(-1,-1),.45,HexColor('#CCD3C7')),('VALIGN',(0,0),(-1,-1),'MIDDLE'),('LEFTPADDING',(0,0),(-1,-1),10),('RIGHTPADDING',(0,0),(-1,-1),10),('TOPPADDING',(0,0),(-1,-1),8),('BOTTOMPADDING',(0,0),(-1,-1),8)]))
    _,hh=t.wrap(width,1000);t.drawOn(c,x,H-y-hh)
    if y+hh>598:overflow.append((page_no,'TABLE '+str(headers),round(y+hh,1)))
    return y+hh
def fan(x,y,size=64,col=GOLD):
    c.saveState();c.translate(x,H-y-size);c.scale(size/64,size/64);stroke(col);c.setLineWidth(1.3)
    for angle,xx in [(17,12),(-12,23),(0,18)]:
        c.saveState();c.translate(32,17);c.rotate(angle);c.translate(-32,-17);c.roundRect(xx,12,29,45,4,stroke=1,fill=0);c.restoreState()
    p=c.beginPath();p.moveTo(32.5,45);p.lineTo(36,37);p.lineTo(44,33.5);p.lineTo(36,30);p.lineTo(32.5,22);p.lineTo(29,30);p.lineTo(21,33.5);p.lineTo(29,37);p.close();c.drawPath(p);c.restoreState()

# 01
page_no+=1;page_titles.append('Tarot Interface Design Packet')
box(0,0,W,H,DARK)
for i in range(18):
    path([(375+i*12,650),(620+i*14,300),(960+i*8,38)],'#1A3C43',.65)
fan(48,43,52,'#C8B182')
txt(112,52,'INTERFACE DESIGN PACKET',10,'LatoBold','#A6BCB5',track=1.7)
txt(45,134,'TAROT',98,'Instrument',WHITE,track=4)
para(49,252,350,'A faithful webOS phone shell<br/>for Nura on PinePhone Pro',25,31,'#D5DFD1')
para(49,352,320,'Cards, gestures, quiet notifications, and the everyday detail that made the original feel complete.',16,23,'#A6BCB5')
rule(49,464,310,'#506663')
para(49,485,305,'Reference: Pre 2 era webOS 2.x.<br/>Direction: Nocturne.<br/>Canvas: 720 × 1440 physical pixels.',12.5,20,'#C1D0C1')
screen('03-launcher',494,155,160,True)
screen('06-dashboard',788,169,152,True)
screen('01-card-view-nocturne',627,85,191,True)
txt(49,608,'28 OCTOBER 2026   /   DESIGN PROPOSAL v0.1',9.5,'LatoBold','#8FA69F',track=.8)
txt(951,608,'01',11,'LatoBold','#B8C9BE','right');c.showPage()

# 02
heading('Design intent','Recreate the phone experience in full','This packet translates the brief into a visual target, an interaction contract, and an implementable handoff.')
b=para(44,169,490,'Tarot should be recognizable as phone-era webOS immediately, and remain recognizable after a day of use. The reference is Pre 2 era webOS 2.x, including stacks. Use the taller display for more content while preserving that structure.',17,23)
b=para(44,b+18,490,'Nocturne is the recommended release theme: deep teal, restrained warm metal, and original botanical linework. It keeps the dimensional light controls and individual icon silhouettes. The Tarot identity appears in the wallpaper, boot mark, and theme palette.',13.5,19)
txt(44,b+24,'Three kinds of decision',17,'LatoBold')
table(44,b+57,498,['Class','Meaning'],[
 ('Reference','Documented phone behavior and visual anatomy.'),
 ('Adaptation','A deliberate response to PinePhone hardware or modern clients.'),
 ('Proposal','A starting value or design treatment that still needs capture or testing.')
],[1,3.2],11)
txt(597,172,'The packet contains',18,'LatoBold')
bullets(597,211,350,[
 '25 editable portrait screen studies and matching 720 × 1440 renders.',
 'Cards, stacks, Launcher, wave, search, notifications, menus, settings, lock and calls.',
 'Three theme routes, an original mark, wallpapers, type and material guidance.',
 'Geometry and motion tokens, service boundaries, failure states, and fidelity acceptance gates.'
],13.1,13)
para(597,466,350,'The screens are design targets. Contacts and app data are fictional. App interiors depict cooperative applications; a compositor cannot automatically restyle every GTK or Qt client.',12.5,18,MUTED)
footer('Historical baseline [R1]  |  Client boundary [R14]')

# 03
heading('Historical reference','Start from the actual phone','Use one phone build as the final visual authority. These reference fragments come from Palm’s own Pre 2 guide.')
refimgs=[('ref-29-352.png','Card view and tray','Printed page 29'),('ref-29-351.png','Phone Launcher','Printed page 29'),('ref-41-459.png','Bottom dashboard','Printed page 41'),('ref-199-1954.png','Light preferences','Printed page 199')]
for i,(fn,title,cap) in enumerate(refimgs):
    x=44+i*231
    im=Image.open(ROOT/'reference'/fn)
    if i<2:
        # Exclude the manual's annotation margin; preserve the source screenshot itself.
        im=im.crop((0,0,345 if i==0 else 344,517))
    dest=TMP/('source-'+fn);im.save(dest)
    image_file(dest,x,172,184,276)
    txt(x,466,title,14,'LatoBold')
    txt(x,487,cap,11,col=MUTED)
para(44,531,560,'Read the whole composition: dense black status chrome, tactile light surfaces, freestanding icons, and a bottom notification well. These images are research references; the source bundle contains newly drawn assets.',12.5,18)
para(648,531,304,'Later LunaSysMgr code is useful evidence, but its tablet defaults are not a substitute for a phone capture.',12.5,18,MUTED)
footer('Reference images: Palm Pre 2 User Guide [R1]')

# 04
heading('Fidelity contract','The details that carry the identity','Preserve the relationship between surfaces before adding any theme treatment.')
screen('01-card-view-nocturne',388,166,192)
notes=[(1,44,180,'Black global frame','Center the clock. Keep app/menu identity left and device state right.',(488,180)),(2,44,306,'Actual app surfaces','A card is the window itself, scaled continuously. Its contents remain recognizable.',(430,320)),(3,44,445,'Quiet dimensionality','Small corners, soft contact shadows, gentle gradients and thin highlights.',(420,463)),(4,661,180,'The neighboring tasks','Keep lateral peeks and spatial ordering. Focus moves through a horizontal field.',(575,312)),(5,661,317,'Five Quick Launch slots','Four favorites plus the Launcher at the right. Give every icon its own silhouette.',(526,506)),(6,661,454,'A reserved gesture strip','A PinePhone adaptation, separated from client input. Keep the center glow quiet.',(491,542))]
for n,x,y,title,desc,end in notes:
    num(x+10,y+9,n);txt(x+30,y,title,15,'LatoBold');para(x+30,y+26,260,desc,12.1,17)
    xx=x+305 if x<100 else x-11
    path([(xx,y+15),(xx+(20 if x<100 else -20),y+15),end],GOLD,.65)
footer('Phone structure [R1, R2]  |  Black status renderer [R8]')

# 05
heading('Dimensions','Scale the interface with intent','All values on this page are proposed Tarot logical units. Output scale 2 produces the native panel raster.')
screen('01-card-view-nocturne',83,172,180)
# Geometry guides matching the 360 × 720 artboard at 0.5 presentation scale.
for yy,label in [(172,'0'),(188,'32'),(514,'684'),(532,'720')]:
    rule(70,yy,208,'#A5814C',.6);txt(281,yy-5,label,10,'LatoBold',GOLD)
arrow(62,175,62,529,GOLD,.7);txt(46,341,'720',10,'LatoBold',GOLD)
arrow(85,550,261,550,GOLD,.7);txt(174,559,'360 logical',11,'LatoBold',GOLD,'center')
table(355,172,601,['Token','Logical','Physical at 2×','Rule'],[
 ('Output','360 × 720','720 × 1440','Portrait reference'),
 ('Status bar','32 high','64 high','Black; centered clock'),
 ('Client rectangle','360 × 652','720 × 1304','Below status, above strip'),
 ('Gesture strip','36 high','72 high','Exclusive shell input'),
 ('App header / commands','56 high','112 high','Reference-scale chrome'),
 ('Focused card','0.66 scale','475 × 861 approx.','Uniform transform'),
 ('Quick Launch','354 × 69','708 × 138','Five positions; 56-unit art'),
 ('Banner / dashboard row','32 / 56 high','64 / 112 high','Space grows from bottom'),
 ('Primary action target','48 min.','96 min.','Compact chrome has alternatives')
],[2,1.2,1.5,2],10.6)
para(355,547,601,'The historical Pre guide documents 320 × 480, 28-unit bars and 50-unit headers. A width ratio of 360/320 = 1.125 suggests 31.5 and 56.25; Tarot rounds these to 32 and 56. Card geometry and the 36-unit strip remain calibration proposals.',11.4,15.5,MUTED)
footer('Panel [R12]  |  Historical dimensions [R2]  |  Tokens are proposals')

# 06
heading('Visual direction','Nocturne in everyday use','The wallpaper supplies the atmosphere. The task surfaces retain their clarity and familiar material.',True)
for x,name,title,desc in [(93,'01-card-view-nocturne','Card view','One continuous field of tasks.'),(405,'02-card-stack','A focused stack','A related set of windows.'),(716,'03-launcher','Launcher','Named pages, three columns.')]:
    screen(name,x,177,181,True)
    txt(x+90.5,568,title,15,'LatoBold',WHITE,'center')
    txt(x+90.5,589,desc,11,'Lato','#A8BEB4','center')
footer('Original Tarot studies  |  Cooperative app interiors shown',True)

# 07
heading('Cards and stacks','Keep each task spatially continuous','The unit is a task window. A process may own several windows; a transient dialog remains with its parent.')
screen('04-email',63,180,153)
screen('01-card-view-nocturne',274,180,153)
screen('02-card-stack',484,180,153)
arrow(229,334,260,334);arrow(438,334,470,334)
for x,a,b in [(63,'01  In use','One focused window.'),(274,'02  In Card view','The same surface at 0.66.'),(484,'03  In a stack','Related windows remain distinct.')]:
    txt(x,506,a,13,'LatoBold');para(x,528,167,b,11.4,15,MUTED)
txt(698,179,'Interaction contract',17,'LatoBold')
bullets(698,216,258,[
 'Tap a card to focus it. Preserve the buffer and the transform origin through the transition.',
 'Hold, then drag to reorder. Drag onto another task to group; drag away to separate.',
 'Close one card with an upward flick. Keep stack siblings intact.',
 'Auto-group only with known launch context. Timing or process identity alone is insufficient.'
],12.2,12)
para(698,510,258,'On Wayland, dismissal requests close. If a save dialog appears or the client refuses, restore the card with its state intact. Force quit is a separate action.',11.8,16.5,MUTED)
footer('Card model [R1, R7]  |  Close request semantics [R28]')

# 08
heading('Gestures','Give the gesture area a precise vocabulary','Shell gestures begin inside the reserved strip. Input that begins in the application remains application input.')
gestures=[('Tap the center','App → Card view; Card view → focused app. Launcher → Card view.','tap'),('Short stroke left','Back in a cooperating app. At its root, return to cards.','back'),('Flick upward','App → cards. Cards → Launcher. Launcher → cards.','up'),('Drag upward slowly','Reveal the wave, slide to a favorite, release to launch.','wave'),('Full width swipe','Optional direct app switch. Default off; distinguish from short Back.','long'),('Flick a card up','From Card view only. Close that window after the client accepts.','close')]
for i,(title,desc,kind) in enumerate(gestures):
    col=i%3;row=i//3;x=44+col*313;y=173+row*199
    box(x,y,283,65,'#183B43',8)
    box(x,y+46,283,19,'#09171D',0);box(x+126,y+54,31,2,'#89AAA8',1)
    if kind=='tap':
        stroke('#D3BF92');c.setLineWidth(1.4);c.circle(x+142,H-(y+54),10,stroke=1,fill=0)
    elif kind in ('back','long'):arrow(x+236 if kind=='long' else x+183,y+54,x+49 if kind=='long' else x+111,y+54,'#D3BF92')
    elif kind=='up':arrow(x+141,y+57,x+141,y+11,'#D3BF92')
    elif kind=='wave':
        path([(x+98,y+57),(x+112,y+27),(x+148,y+21),(x+193,y+33)],'#D3BF92',1.7);arrow(x+168,y+25,x+193,y+33,'#D3BF92')
    elif kind=='close':box(x+120,y+16,42,30,'#B4C8B8',4);arrow(x+141,y+38,x+141,y+4,'#D3BF92')
    txt(x,y+83,title,16,'LatoBold');para(x,y+111,283,desc,12.1,17)
para(44,563,912,'Initial tuning only: 10-unit tap slop; 14-unit axis lock; 54-unit Back travel; 140 ms wave dwell; 700 units/s home flick; 420 ms reorder hold. Cancel on a second touch and preserve a reversible path until release. These values need real-thumb testing.',11.2,15,MUTED)
footer('Historical gesture vocabulary [R1, R10]  |  Thresholds are Tarot proposals')

# 09
heading('Quick Launch','The wave is a continuous gesture','Its shape, icon lift, selection halo and release behavior matter as much as the dock artwork.')
screen('07-wave-launcher',80,177,193,True)
txt(341,177,'Resting tray in Card view',16,'LatoBold')
crop('01-card-view-nocturne',(0,1200,720,1370),341,208,585,138)
txt(341,372,'Wave selection above an application',16,'LatoBold')
crop('07-wave-launcher',(0,1095,720,1370),341,402,300,115)
para(672,405,270,'Use the same five favorites and ordering. The selected object rises over the curve. Release while selected to launch; return to the strip and release to cancel.',12.3,17.5)
para(341,547,601,'The resting tray appears in Card view. The ordinary Launcher has its own full page; dragging a favorite can reveal the tray for editing. Expanded notifications temporarily own the bottom well. Wave travel and card motion must remain interruptible.',12.1,17,MUTED)
footer('Quick Launch and Launcher placement [R1]  |  Proposed wave rendering')

# 10
heading('Launcher','A page of objects with a clear identity','Preserve the phone Launcher’s named pages, three-column rhythm and separate scrolling direction.')
screen('03-launcher',96,175,192,True)
txt(369,179,'Page behavior',17,'LatoBold')
bullets(369,216,570,[
 'Swipe horizontally between named pages; scroll vertically within a page. A disabled arrow indicates an end.',
 'Tap an icon to launch. Hold to move it; pause at an edge to move between pages.',
 'Manage page names and order from the Launcher menu. Keep favorites linked to their original entries.',
 'There is no persistent Quick Launch dock here. Show it only during favorite editing or an explicit wave gesture.'
],13,15)
txt(369,447,'Icon and text rules',17,'LatoBold')
para(369,480,570,'Use 58-unit illustrative art in a consistent optical area. Preserve native app silhouettes when there is no Tarot variant. Labels remain readable at 14 units, with up to two lines for long localized names. Crop neither the icon nor its shadow to a universal tile shape.',13,19)
para(369,558,570,'Launch metadata comes from Desktop Entry files. The page arrangement and favorite order belong to Tarot’s local preferences.',11.8,16.5,MUTED)
footer('Phone Launcher [R1]  |  Desktop Entry specification [R15]')

# 11
heading('Just Type and input','Search should lead directly to an action','Typing from Card view or Launcher is part of the reference. Touch keyboard invocation is a Tarot adaptation.')
screen('10-just-type',83,174,193,True)
txt(353,179,'One query, several useful routes',18,'LatoBold')
table(353,214,603,['Result class','What selection does'],[
 ('Applications','Launch or focus an application whose name or keywords match.'),
 ('Contacts / content','Open the exact local record through an enabled provider.'),
 ('Actions','Create a message, memo or task with the query carried forward.'),
 ('Search using','Send the text to the chosen provider only after the user selects it.')
],[1.2,3.1],12)
txt(353,435,'Keyboard contract',17,'LatoBold')
para(353,469,603,'Tap the search field to summon the trusted keyboard. An attached hardware keyboard can begin the query immediately in Card view or Launcher. The keyboard remains above the gesture strip, preserves composition text, and follows focus across app transitions.',13,19)
para(353,543,603,'The illustrated keyboard is a provisional skin for a compatible input method. Final key geometry, language layouts, candidate rows and text-input protocol versions must be validated together. Local content providers are opt-in; no web query is sent on each keystroke.',11.8,17,MUTED)
footer('Historical search categories [R5]  |  Input method boundary [R26]')

# 12
heading('Notifications','Three states in the same bottom space','Arrival, persistence and expansion should feel like one object changing size.')
for x,name,a,b in [(68,'05-banner','01  Banner','A short message enters below the app.'),(384,'22-notification-rail','02  Icon rail','Unhandled activity leaves a compact trace.'),(700,'06-dashboard','03  Dashboard','Tap the rail to reveal actionable rows.')]:
    screen(name,x,176,176,True)
    txt(x,555,a,14,'LatoBold')
    para(x,576,237,b,10.8,14.2,MUTED,max_bottom=604)
arrow(273,351,342,351);arrow(590,351,658,351)
footer('Bottom notification grammar [R4, R6]  |  Tarot persistence policy [R22]')

# 13
heading('System menus','Keep the two top corners distinct','The left belongs to the current application. The right belongs to the device.')
screen('09-app-menu',87,176,175,True)
screen('08-device-menu',361,176,175,True)
txt(622,178,'Application menu',18,'LatoBold')
para(622,214,331,'Use a compact anchored panel with separators and a real pressed state. Edit leads where supported; scene commands follow, with Preferences or Accounts and Help in consistent positions. The Edit row opens a submenu.',12.7,18)
txt(622,332,'Device menu',18,'LatoBold')
para(622,368,331,'Show date, battery, wireless state, brightness and a route to preferences. Changing a radio shows a pending state until the service reports the result. Expand a row for available networks or devices.',12.7,18)
para(622,492,331,'Unadapted clients cannot export arbitrary app commands through core Wayland. Their top-left menu offers genuine shell actions; the client keeps its internal menu.',12.3,17.5,MUTED)
txt(87,558,'App commands',14,'LatoBold');txt(361,558,'Device state',14,'LatoBold')
footer('App menu ordering [R3]  |  Client boundary [R14, R28]')

# 14
heading('Preferences','Make system settings feel native to the shell','Inset groups, small headings, dimensional controls and calm separators are part of the visual vocabulary.')
screen('15-settings',81,177,192,True)
image_file(AS/'components'/'control-states.png',353,180,595,178.5)
txt(353,381,'Components need complete states',18,'LatoBold')
table(353,418,603,['Control','Required states','Behavior'],[
 ('Toggle','Off / pending / on / unavailable','Observed backend state wins.'),
 ('Slider','Idle / dragging / focused / blocked','Preview locally; commit safely.'),
 ('Choice row','Current value / open / selected','Retain the prior value on failure.'),
 ('Action','Idle / pressed / busy / failed','Use a specific verb and recovery.')
],[1,1.8,2.5],11.1)
footer('Header and command-area reference [R2, R3]  |  Controls are Tarot targets')

# 15
heading('Lock and authentication','Keep the familiar unlock gesture meaningful','Dragging the lock control begins the configured unlock path. It does not bypass authentication.')
screen('11-lock',78,178,178,True)
screen('12-pin',346,178,178,True)
arrow(280,357,322,357)
txt(620,180,'Presentation',18,'LatoBold')
bullets(620,216,331,[
 'A quiet clock and date, a movable lock control, and a clear upward target.',
 'Notification counts are visible; private message bodies stay hidden by default.',
 'PIN entry provides clear progress and an accessible route to emergency calling when supported.'
],12.7,15)
txt(620,421,'Compositor contract',18,'LatoBold')
para(620,457,331,'Normal client content and input are blocked while locked. Establish a secure frame before revealing a resumed display. A lock-client crash leaves the session locked. The shell must not briefly show a cached card during wake.',12.7,18)
footer('Secure locking requirements [R25]  |  Phone visual reference [R1]')

# 16
heading('Phone behavior','Calls are a first-class interruption','Use the bottom region for an incoming call while unlocked, and a private call surface while locked.')
for x,name,a in [(66,'13-incoming-call','Unlocked arrival'),(372,'25-locked-incoming-call','Locked arrival'),(678,'14-active-call','Connected call')]:
    screen(name,x,177,166,True);txt(x,529,a,14,'LatoBold')
para(66,562,268,'Shrink the active app coherently. Answer and Decline are explicit, reachable actions.',11.5,16,MUTED)
para(372,562,268,'Drag up to answer without exposing private app content. Provide equivalent accessible actions.',11.5,16,MUTED)
para(678,562,278,'Reflect actual call and audio-route state. Mute and speaker changes can be pending or fail.',11.5,16,MUTED)
footer('Phone reference [R1, R10]  |  Call / audio services [R17, R18]')

# 17
heading('Application compatibility','Be exact about what Tarot can control','The shell can faithfully reproduce navigation and composition. Applications still own the contents of their windows.')
table(44,174,912,['Tier','Visual and behavioral promise','Boundary'],[
 ('1  Tarot-owned surfaces','Cards, gesture recognition, Launcher, search, menus, banners, dashboard, lock, call presentation and shell settings.','Full design-system control.'),
 ('2  Cooperating applications','Matching light chrome, content widgets, app menu, Back semantics and contextual launch into a stack.','A shared style layer and explicit app cooperation are required.'),
 ('3  Ordinary Wayland clients','Correct card transforms, focus, keyboard, lifecycle and notifications around their existing interface.','Preserve native controls; no invented app menu or universal scene history.')
],[1.1,3.3,2.0],12)
txt(44,419,'Back must have honest semantics',17,'LatoBold')
para(44,453,425,'For a cooperating app, Back traverses its navigation and returns to cards at its root. For an unadapted app, the default fallback returns to Card view. A per-app shortcut mapping is explicit and opt-in; never blindly inject Escape or Alt-Left.',13,19)
txt(532,419,'Grouping must have a real cause',17,'LatoBold')
para(532,453,424,'Use a known activation or task relationship to place newly opened windows in a stack. Keep dialogs attached to their parent. Do not infer a relationship merely because two windows appeared at similar times.',13,19)
para(44,562,912,'This packet specifies the style of cooperative Email, Calendar and Messaging screens so the whole experience can be judged. It does not imply that replacing the compositor alone changes those application interiors.',11.8,16.5,MUTED)
footer('Wayland composition model [R14]  |  xdg-shell window contract [R28]')

# 18
heading('Typography','Match the warmth and the metrics','Prelude is the historical target. Lato is the editable substitute used in this packet, and remains a fidelity dependency.')
txt(44,176,'Aa  09:41',83,'Lato',INK)
txt(49,271,'Lato Regular / Bold / Italic',13,'LatoBold',MUTED)
para(49,309,430,'The face should feel quiet, human and compact. Compare word widths, numeral rhythm, x-height, punctuation and condensed text against the chosen phone capture. Do not sign off typography by family resemblance alone.',14,20)
txt(49,431,'TAROT',66,'Instrument',INK,track=7)
txt(49,511,'Instrument Serif / identity only',12.5,'LatoBold',MUTED)
para(49,543,430,'The serif wordmark belongs on boot, About, packaging and wallpaper studies. Keep everyday controls in the UI face.',12,17,MUTED)
table(548,178,408,['Role','Size','Weight'],[
 ('Status / clock','15 / 16','Bold'),
 ('Scene title','24','Regular'),
 ('Main row label','19','Regular'),
 ('Primary list sender','18','Bold when unread'),
 ('Secondary / caption','15 / 13','Regular'),
 ('Launcher label','14','Regular'),
 ('Card text','Scaled with surface','No separate relayout')
],[2.4,1.2,1.4],11.7)
para(548,504,408,'All sizes are logical units at default scale. Larger text reflows owned panels and rows. Use real text shaping, locale-aware date and time formats, and fallback coverage for the languages supported by the distribution.',12.5,18,MUTED)
footer('Historical font references [R7, R8]  |  Bundled font sources [R29]')

# 19
heading('Materials and icon art','Keep the small tactile cues','Fine shading, object silhouettes and pressed states give the interface its character at higher resolution.')
crop('03-launcher',(0,175,720,615),44,173,434,265)
txt(44,460,'Application icons',18,'LatoBold')
para(44,496,434,'Use distinct objects with light from above, restrained highlights and a small contact shadow. Match optical size, not a common container. Draw native resolutions from vectors; do not enlarge low-resolution historical bitmaps.',12.7,18)
crop('15-settings',(17,194,706,398),530,173,426,126)
crop('06-dashboard',(0,1015,720,1360),530,358,426,204)
para(530,575,426,'Chrome and dashboard crops shown at large scale for inspection.',10.8,14,MUTED)
footer('Source asset anatomy [R7, R8]  |  All illustrated icon art is newly drawn')

# 20
heading('Theme routes','One interface with three atmospheres','Theme changes must not move controls, change gesture semantics or alter the information hierarchy.',True)
for x,name,title,desc,pal in [
 (75,'17-heritage','Heritage','Green and soft daylight.',['#1D3027','#4A6650','#9A9451']),
 (386,'01-card-view-nocturne','Nocturne','The recommended Tarot default.',['#091A21','#173D43','#C6AE78']),
 (697,'18-vellum','Vellum','Oxblood and warm paper.',['#34242C','#65454A','#EAD3A1'])]:
    screen(name,x,177,176,True)
    txt(x,550,title,16,'LatoBold',WHITE)
    txt(x,574,desc,11.5,'Lato','#A9BFB5')
    for i,col in enumerate(pal):box(x+183+i*14,550,10,31,col,2)
footer('Identical geometry and app surfaces across all three themes',True)

# 21
heading('Identity and theming','Let the name live in the material','A restrained card fan is enough to establish Tarot. Ordinary phone functions keep ordinary names.')
fan(79,179,154,INK)
txt(282,192,'TAROT',80,'Instrument',INK,track=8)
txt(287,284,'A three-card fan and a small compass-like star.',13,'Lato',MUTED)
rule(44,359,912)
txt(44,389,'Themeable surfaces',18,'LatoBold')
bullets(44,427,398,['Wallpaper and its contrast scrims.','Accent, focus and selection colors.','Original app icon variants, boot and About art.','Bounded material and shadow parameters.'],12.7,12)
txt(539,389,'Shared invariants',18,'LatoBold')
bullets(539,427,417,['Status placement and black frame.','Card scale, gesture meanings and task order.','Menu anchors and the bottom notification region.','Legibility, action semantics and lock state.'],12.7,12)
footer('Original Tarot identity  |  SVG mark and wordmark included')

# 22
heading('Motion','Animate the task rather than the decoration','Direct manipulation follows the finger. Release transitions settle quickly and keep their spatial explanation.')
table(44,174,526,['Transition','Initial duration','Direction'],[
 ('App → Card view','350 ms','Same buffer; uniform reduction'),
 ('Card → app','300 ms','Same anchor; continuous enlargement'),
 ('Launch','400 ms','A real new surface enters the field'),
 ('Dismiss card','300 ms','Follow the upward release velocity'),
 ('Launcher / Quick Launch','350 ms','Reveal from their spatial origin'),
 ('Reduced motion','100 ms fade','Preserve state and focus, reduce travel')
],[1.8,1.3,2.4],11.8)
screen('04-email',627,178,117)
screen('01-card-view-nocturne',817,178,117)
arrow(761,297,799,297)
txt(627,430,'Full task',12,'LatoBold');txt(817,430,'Same task',12,'LatoBold')
para(627,465,316,'The included motion study illustrates the proposed minimize and maximize envelope. It is a design animation, not a recording of Tarot or a measured Palm curve.',12.3,17.5,MUTED)
para(44,520,526,'These duration seeds come from later LunaSysMgr source, not a certified Pre 2 capture. Begin release tuning with a restrained ease-out; preserve live position and velocity when interrupted. Avoid decorative spring overshoot, screen-wide blur and unrelated fades.',12.4,17.5,MUTED)
footer('Reference timing seeds [R9]  |  Curves and performance require capture')

# 23
heading('Linux integration','Let ordinary services own ordinary services','This is a proposed boundary map. The user has not supplied a repository, so it is not a dependency inventory.')
table(44,173,912,['Experience','Standard interface','Tarot owns'],[
 ('Apps and cards','Desktop Entry + Wayland / xdg-shell [R14, R15, R28]','Discovery, launch, focus, transforms, grouping and close requests.'),
 ('Wi-Fi and data','NetworkManager [R16]','Selection, secrets prompts, progress and truthful state.'),
 ('Calls and SMS','ModemManager [R17]','Call / message presentation and supported actions.'),
 ('Audio','PipeWire + WirePlumber [R18]','Volume, routes and call controls through distro policy.'),
 ('Bluetooth','BlueZ [R19]','Pairing, discovery and connection feedback.'),
 ('Power and session','UPower + login1-compatible backend [R20, R21]','Battery, inhibitors, suspend and wake presentation.'),
 ('Notifications','org.freedesktop.Notifications [R22]','One session server; banners, rail and dashboard.'),
 ('Music dashboard','MPRIS [R23]','Render metadata and supported playback actions.'),
 ('Text and permissions','Wayland input method + portals + polkit [R24, R26, R30]','Trusted input surfaces and coherent authorized prompts.')
],[1.25,2.55,3.15],10.9)
para(44,563,912,'Keep service calls asynchronous. Nura can package UIs for OpenRC, systemd, or both; decide what Tarot supports explicitly. Do not add a new modem or audio policy daemon merely to reproduce the old presentation.',11.8,16.5,MUTED)
footer('Primary service references [R14-R30]  |  Service-manager policy [R27]')

# 24
heading('State and recovery','A beautiful shell must tell the truth','Transitions from a request to a confirmed service state are part of the interface, including failure and reconnection.')
table(44,173,562,['Situation','Visible response','Recovery'],[
 ('Radio changed','Pending row, then observed on/off state','Revert with a useful explanation if denied.'),
 ('Service disappears','Unavailable or reconnecting','Retain intent; refresh from new state.'),
 ('App launch is slow','A loading card using the app identity','Show first real buffer as soon as ready.'),
 ('App fails to launch','App-specific failure in the card','Try again or dismiss; keep other cards.'),
 ('Close is refused','Return the card and its dialog','Let the app save or cancel the close.'),
 ('Low memory','Honest retained snapshot or unavailable preview','Never pretend a dead app is live.')
],[1.2,2.0,2.2],11.3)
screen('23-launch-pending',658,175,122)
screen('24-launch-failed',822,175,122)
txt(658,438,'Waiting',13,'LatoBold');txt(822,438,'Recovery',13,'LatoBold')
para(658,479,286,'The wallpaper and other cards stay responsive while a service is busy. Report a hardware switch as the cause only if the backend can actually distinguish it.',12.3,17.5,MUTED)
para(44,539,562,'Notifications retain replacement IDs and explicit actions. Preserve active non-expired items in the rail; clear dismissed items correctly. A history feature can be separate, but must not silently turn an expired notification into a live request.',11.8,17,MUTED)
footer('Notification lifecycle [R22]  |  Client close [R28]  |  Recovery is Tarot policy')

# 25
heading('Accessible adaptations','Preserve the grammar while making room','The default studies prioritize the familiar composition. Accessible settings enlarge and simplify without changing navigation.')
txt(44,175,'Text and targets',18,'LatoBold')
bullets(44,214,427,[
 'Primary actions use 48-unit targets. Compact status chrome, gesture strip and keyboard keys need equivalent accessible routes.',
 'Larger text grows rows and panels. Wrap names and values; do not solve overflow by shrinking essential text.',
 'Target 4.5:1 for small text and 3:1 for meaningful graphics. Measure the least favorable gradient and wallpaper region.',
 'Offer reduced motion, visible gesture hints, keyboard navigation and a screen-reader-ready semantic shell tree.'
],12.7,13)
txt(532,175,'Landscape is an explicit layout',18,'LatoBold')
box(532,214,424,212,'#172F34',10)
box(532,214,424,19,'#030505',5)
txt(545,220,'Email',9,'LatoBold',WHITE);txt(744,220,'9:41',9,'LatoBold',WHITE,'center')
box(532,233,424,172,'#DCE3D6')
rule(549,262,390,'#A7B59D');txt(550,243,'Client content reflows within a new work area',11,'LatoBold','#49603E')
for yy in [280,300,320,340,360]:rule(550,yy,386,'#BAC5B0',.6)
box(532,405,424,21,'#09161A');box(724,415,40,2,'#6D8988',1)
txt(744,445,'720 × 360 logical; gesture strip stays at the visual bottom',11,'Lato',MUTED,'center')
para(532,481,424,'Portrait remains the reference orientation. On rotation, cancel an in-progress shell gesture, relayout at a stable state, and preserve the focused task. Treat minimum-size desktop apps explicitly; do not stretch their buffers to fill.',12.7,18)
para(532,562,424,'The landscape diagram is a layout rule, not a finished keyboard or application study.',11.3,15.5,MUTED)
footer('Wayland scale / transform foundation [R14, R32]  |  Accessibility values are project targets')

# 26
heading('Rendering and performance','Spend the budget on contact and continuity','The high-resolution treatment should improve edge quality while the shell stays responsive and frugal.')
table(44,174,912,['Area','Implementation intent','How to judge it'],[
 ('Frame pacing','Render to the actual display cadence; do not inherit the old source FPS limit.','At 60 Hz the frame interval is 16.67 ms. Record missed frames and p95 frame time.'),
 ('Card composition','Reuse client buffers, clip corners, cache shadows, and damage only what changed.','Inspect drag, stack movement and incoming banners together.'),
 ('Resizing','Animate a stable surface while negotiating the target size; reconcile new commits.','No black flash, accidental stretching, duplicate input or torn chrome.'),
 ('Idle power','Static wallpaper and chrome; no perpetual shimmer or repeated full-screen blur.','Compare redraw count and power at idle, overview and lock.'),
 ('Input feedback','Show contact immediately; do not block drawing on D-Bus or app startup.','Measure touch-to-visible response on the device, not just desktop emulation.'),
 ('Secure surfaces','Treat lock and privileged input as real compositor state.','Lock during every transition; test crash, wake and focus changes.')
],[1.2,3.0,3.1],11.5)
para(44,549,912,'These are acceptance targets, not measured Tarot results. Validate the chosen Nura image, kernel, firmware and services on the PinePhone Pro. Calls, wake, sensors and audio routing must be exercised together; a successful desktop mockup does not certify them.',12.5,18,MUTED)
footer('Composition architecture [R14]  |  Historical animation config [R9]  |  Lock protocol [R25]')

# 27
heading('Build sequence','Finish the shell in visible slices','A small coherent surface should become exact before a wide collection of screens becomes approximate.')
table(44,173,912,['Slice','Build','Definition of done'],[
 ('01  Reference and frame','Pin the reference phone build; capture its states; implement scale, status bar, client bounds and gesture strip.','Native screenshots agree on geometry; input never leaks across the strip boundary.'),
 ('02  The card experience','Launch, minimize, maximize, scroll, group, unstack, reorder and request close.','One buffer remains one task through every transition; refused close is recoverable.'),
 ('03  Everyday shell','Launcher, Quick Launch wave, Just Type, app/device menus and bottom notifications.','The complete daily navigation loop works with keyboard and service interruption.'),
 ('04  Phone and trust','Lock, authentication, incoming/active/missed calls, audio routes, pairing and permissions.','Locked wake is private; real calls and service failures remain comprehensible.'),
 ('05  Conformance and themes','Font matching, icon refinement, motion capture, accessibility, Nocturne and alternate palettes.','A recorded audit passes; theme changes preserve the same geometry and behaviors.')
],[1.4,3.1,3.1],12)
para(44,553,912,'Keep a dedicated plain reference theme while tuning geometry. Apply Nocturne after that baseline is stable, then compare both against the same test sequence. This keeps theming from disguising an interaction mismatch.',12.5,18,MUTED)
footer('Proposed implementation sequence  |  No repository assumptions')

# 28
heading('Fidelity acceptance','Define what nearly indistinguishable means','The goal needs an evidence trail: a named reference build, matched state captures and repeatable interaction videos.')
cols=[(44,'Still images',[
 'Match status baselines, icon spacing and menu anchors.',
 'Compare card radius and shadow falloff at 1× and at native output.',
 'Check Launcher pages, tray visibility and the wave silhouette.',
 'Check banner, icon rail, dashboard and urgency states.',
 'Test long names, empty lists, disabled controls and loading cards.'
]),(357,'Motion and input',[
 'Record minimize, maximize, throw and aborted throw.',
 'Record stack focus, reorder, merge and unstack.',
 'Separate slow wave, fast flick, Back and advanced switch.',
 'Interrupt each animation without jumping or losing focus.',
 'Verify notification space changes while keyboard and calls are active.'
]),(670,'System behavior',[
 'Exercise app startup failure and unsaved-document closure.',
 'Restart services; confirm honest pending and unavailable state.',
 'Lock during card motion; wake without leaking app content.',
 'Test calls after suspend, audio routes and modem return.',
 'Verify large text, reduced motion and external keyboard navigation.'
])]
for x,title,items in cols:
    txt(x,179,title,19,'LatoBold');bullets(x,220,277,items,12.3,15)
rule(44,529,912)
para(44,552,912,'Packet status: researched and visually reviewed design proposal. Not yet certified against one captured Pre 2 firmware build or implemented on the user’s device. The remaining visual dependencies are font metrics, icon micro-detail, measured card/stack geometry and measured motion curves.',12.5,18,MUTED)
footer('Capture checklist included in source bundle  |  Historical sources [R1-R11]')

SOURCES=[
 ('R1','Palm Pre 2 User Guide','Palm, 2010; manufacturer PDF hosted by Rogers. Printed pp. 22-42, 52-56, 69-75, 199-200.','https://www.rogers.com/cms/support/pdfs/Wireless/English/Palm_Pre2_EN.pdf'),
 ('R2','User Interface Summary','HP developer documentation, 2011; preserved mirror. Reference dimensions and scene anatomy.','https://www.banneret.nl/webos/documentation/all/design/mojo/user-interface-summary.html'),
 ('R3','Menus UI Guidelines','HP developer documentation, 2011; preserved mirror. Menu ordering and anchors.','https://www.banneret.nl/webos/documentation/all/design/mojo/menus.html'),
 ('R4','Dashboards and Notifications','HP developer documentation, 2011; preserved mirror. Bottom surfaces and urgency.','https://www.banneret.nl/webos/documentation/all/dev-guide/mojo/dashboards-notifications.html'),
 ('R5','Just Type','HP developer documentation, 2011; preserved mirror. Actions, content and search providers.','https://www.banneret.nl/webos/documentation/all/dev-guide/mojo/just-type.html'),
 ('R6','Human Interface Guidelines','HP developer documentation, 2011; preserved mirror. Non-blocking notifications.','https://www.banneret.nl/webos/documentation/all/design/mojo/hi-guidelines.html'),
 ('R7','LunaSysMgr Settings.cpp','Open webOS source. Font names and card-scale seeds; later than Pre 2.','https://github.com/openwebos/luna-sysmgr/blob/master/Src/base/settings/Settings.cpp'),
 ('R8','LunaSysMgr StatusBar.cpp','Open webOS source. Phone black fill and clock placement; later than Pre 2.','https://github.com/openwebos/luna-sysmgr/blob/master/Src/lunaui/status-bar/StatusBar.cpp'),
 ('R9','LunaSysMgr animation configuration','Open webOS source. Provisional duration seeds, not phone motion certification.','https://github.com/openwebos/luna-sysmgr/blob/master/conf/lunaAnimations.conf'),
 ('R10','Palm Pre User Guide','Manufacturer guide hosted by Bell. Earlier phone gesture vocabulary.','https://support.bell.ca/_web/guides/User-Guides/Mobile/PalmOne/Palm-EN/palm_pre_userguide_en(en).pdf'),
 ('R11','LunaSysMgr repository','Official Open webOS source and shell responsibilities.','https://github.com/openwebos/luna-sysmgr'),
 ('R12','PinePhone Pro specifications','PINE64. Panel dimensions, physical hardware and privacy switches.','https://pine64.org/documentation/PinePhone_Pro/Further_information/Specifications/'),
 ('R13','Project rebrand Nura','Nura, 27 Sep 2026. Current project name.','https://nura.eco/blog/2026/09/27/nura-rename/'),
 ('R14','Wayland architecture','Wayland project. Client buffers and compositor ownership.','https://wayland.freedesktop.org/architecture.html'),
 ('R15','Desktop Entry specification','freedesktop.org. Application discovery and launching.','https://specifications.freedesktop.org/desktop-entry/latest/'),
 ('R16','NetworkManager developer resources','NetworkManager. Network state and connection API.','https://networkmanager.dev/docs/developers/'),
 ('R17','ModemManager D-Bus reference','ModemManager. Modems, SIM, SMS, voice and call objects.','https://www.freedesktop.org/software/ModemManager/doc/latest/ModemManager/ref-dbus.html'),
 ('R18','WirePlumber documentation','PipeWire project. Session and audio policy.','https://pipewire.pages.freedesktop.org/wireplumber/'),
 ('R19','BlueZ Adapter API','BlueZ upstream. Discovery, power and device state.','https://github.com/bluez/bluez/blob/master/doc/org.bluez.Adapter.rst'),
 ('R20','UPower Device interface','UPower. Battery and charging properties.','https://upower.freedesktop.org/docs/Device.html'),
 ('R21','login1 D-Bus interface','systemd project. Session/power contract where provided.','https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html'),
 ('R22','Desktop Notifications specification','freedesktop.org. Server, replacement, expiry and actions.','https://specifications.freedesktop.org/notification/latest/basic-design.html'),
 ('R23','MPRIS specification','freedesktop.org. Player metadata and controls.','https://specifications.freedesktop.org/mpris/latest/'),
 ('R24','XDG Desktop Portal documentation','Desktop portals project. Mediated desktop functions.','https://flatpak.github.io/xdg-desktop-portal/docs/'),
 ('R25','ext-session-lock-v1','Protocol author documentation. Lock isolation and lifetime.','https://isaacfreund.com/docs/wayland/ext-session-lock-v1/'),
 ('R26','Smithay input-method implementation','Smithay. Protocol implementation and privileged client filtering.','https://smithay.github.io/smithay/src/smithay/wayland/input_method/mod.rs.html'),
 ('R27','Nura service-manager support policy','Nura, 10 Jul 2026. UI packages can declare supported managers.','https://nura.eco/edge/2026/07/10/explicit-service-managers/'),
 ('R28','xdg_toplevel close request','Smithay generated protocol bindings. Client may refuse or ask to save.','https://smithay.github.io/smithay/wayland_protocols/xdg/shell/server/xdg_toplevel/struct.XdgToplevel.html'),
 ('R29','Google Fonts source families','Lato and Instrument Serif. Font files and OFL notices are included.','https://github.com/google/fonts'),
 ('R30','polkit overview','polkit project. Authorities and authentication agents.','https://www.freedesktop.org/software/polkit/docs/latest/polkit.8'),
 ('R31','callaudiod upstream status','Upstream README marks it deprecated; new call-profile policy is in WirePlumber.','https://gitlab.com/mobian1/callaudiod/-/blob/master/README.md'),
 ('R32','Wayland protocol reference','Wayland project. Surface/output scaling and transforms.','https://wayland.freedesktop.org/docs/html/apa.html'),
]

def source_entry(x,y,w,ref,title,desc,url,small=False):
    txt(x,y,ref,10.5,'LatoBold',GOLD)
    txt(x+32,y,title,12 if not small else 10.8,'LatoBold',INK)
    yy=para(x+32,y+19,w-32,desc,10.4 if not small else 9.5,14 if not small else 12.5,MUTED)
    c.linkURL(url,(x,H-yy-2,x+w,H-y+2),relative=0,thickness=0)
    return yy

# 29
heading('Source ledger','Historical and visual evidence','Primary authored documents and upstream source. Titles are clickable; full URLs are in the source manifest.')
hist=SOURCES[:11]
for i,s in enumerate(hist):
    x=44 if i<6 else 528;y=173+(i if i<6 else i-6)*68
    source_entry(x,y,428,*s)
para(528,526,428,'Historical illustrations on page 3 are attributed research excerpts from the Palm guide. The editable Tarot artwork is newly drawn. Later open-source defaults are labeled as seeds and are never presented as measured Pre 2 values.',11.3,16,MUTED)
footer('Sources accessed 28 Oct 2026  |  No long quotations reproduced')

# 30
heading('Source ledger','Platform and asset evidence','The service map is a proposed integration contract based on the interface owners’ documentation.')
plat=[s for s in SOURCES if int(s[0][1:])>=12]
for i,s in enumerate(plat):
    col=i//11;j=i%11;x=44+col*484;y=171+j*37.2
    rr,ti,de,ur=s
    txt(x,y,rr,9.2,'LatoBold',GOLD);txt(x+31,y,ti,10.5,'LatoBold',INK)
    short=de.split('. ')[0]+'.'
    para(x+31,y+14,415,short,8.9,11.5,MUTED,max_bottom=604)
    c.linkURL(ur,(x,H-y-32,x+450,H-y+2),relative=0,thickness=0)
footer('Sources accessed 28 Oct 2026  |  Full URLs and notices in the source bundle')

# 31
heading('Handoff','A concrete starting point for Tarot','The package is designed to be inspected, changed and implemented without reconstructing the design from screenshots.')
table(44,174,912,['Deliverable','Contents','Status'],[
 ('Design packet','31 pages of reference, art direction, interaction specification and handoff.','Reviewed design proposal'),
 ('screens/svg','25 editable portrait studies at 360 × 720 logical units.','Original vector art; live text'),
 ('screens/png','Matching 720 × 1440 screen renders.','Ready for comparison and issue discussion'),
 ('tokens/tarot.tokens.json','Geometry, type, palette, theme and motion seeds.','Provisional implementation values'),
 ('brand + wallpapers','Card-fan mark, wordmark and three wallpaper themes.','Editable SVG plus wallpaper PNGs'),
 ('motion','A minimize/maximize motion study at the proposed timings.','Illustrative; not measured hardware footage'),
 ('handoff + fonts + source','Engineering notes, capture audit, source manifest, font notices and generators.','Reusable implementation reference')
],[1.6,3.6,2.1],11.7)
txt(44,524,'The next fidelity decision',17,'LatoBold')
para(44,555,912,'Pin the exact webOS 2.x phone build to emulate, then compare this packet’s geometry and motion against controlled captures. Keep Nocturne as the lead theme while settling Prelude-compatible typography, the app cooperation surface and real-device gesture tuning.',12.8,18)
footer('Tarot  /  WebOS reincarnated on a modern stack')

c.save()
(TMP/'layout-report.json').write_text(json.dumps({'pages':page_no,'titles':page_titles,'overflow':overflow},indent=2))
(AS/'source-manifest.txt').write_text('TAROT DESIGN PACKET SOURCE MANIFEST\nAccessed 28 October 2026\n\n'+ '\n\n'.join(f'{r}  {ti}\n{de}\n{ur}' for r,ti,de,ur in SOURCES)+'\n\nFont source files\nhttps://raw.githubusercontent.com/google/fonts/main/ofl/lato/Lato-Regular.ttf\nhttps://raw.githubusercontent.com/google/fonts/main/ofl/instrumentserif/InstrumentSerif-Regular.ttf\nOFL notices are bundled under fonts/.\n')
print(json.dumps({'pdf':str(PDF),'pages':page_no,'overflow':overflow},indent=2))


