#!/usr/bin/env python3
"""Real Firefox/loopback site fixture; DOM reports contain no entered strings."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import subprocess
import threading
import time


class ObserverServer(ThreadingHTTPServer):
    # Field/input/resize snapshots can open several connections together.
    # Keep the loopback observer from turning an accept backlog into a long
    # retransmission delay while the phone schedules its server thread.
    request_queue_size=128


COLORS = {
    'primary': (197, 223, 249), 'password': (249, 219, 197),
    'bottom': (217, 241, 199), 'popup-input': (233, 207, 249),
    'navigate': (189, 227, 219), 'popup': (243, 221, 169),
    'fullscreen': (219, 219, 249), 'exit-fullscreen': (249, 207, 219),
    'back': (199, 239, 237), 'close-popup': (239, 199, 221),
}
EXPECTED = ['', 't', 'ta', 'tar', 'taro', 'tarot', 'tarotx', 'tarotxz',
            'tarotxza', 'tarotxzaf', 'c', 'ca', 'caf', 'café', 'caféx',
            'd', 'de', 'dem', 'demo', 'demo4', 'demo42', 'p', 'po', 'pop', 'popup', 'popu', 'f']

REPORT_TRANSPORT = '''let pendingReports=[], reportInFlight=false, transportFailures=0;
async function pumpReports() {
 if(reportInFlight || !pendingReports.length)return;
 reportInFlight=true;const value=pendingReports.shift();
 try {const response=await fetch('/event',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({...value,transportFailures}),keepalive:true});if(!response.ok)transportFailures++;}
 catch {transportFailures++;}
 finally {reportInFlight=false;pumpReports();}
}
function send(event,extra={}) {
 const value={event,page,document:documentId,sequence:++sequence,observed:performance.timeOrigin+performance.now(),...extra};
 // Preserve input/focus/navigation events; retain only the newest queued poll.
 if(event==='heartbeat')pendingReports=pendingReports.filter(e=>e.event!=='heartbeat');
 pendingReports.push(value);pumpReports();
}
'''


def text_input_metadata(line):
    match=re.search(r'\[\s*([\d.]+)\]\s*(?:\{[^}]*\}\s*)?(->\s*)?(zwp_text_input_v3[#@]\d+)\.(\w+)\((.*)\)',line)
    if not match:return None
    stamp,direction,obj,method,arguments=match.groups()
    value={'stamp':float(stamp),'direction':'request' if direction else 'event',
           'object':obj,'method':method}
    # Never retain protocol string arguments, even for synthetic input.
    if method in ['commit','done','set_content_type'] and re.fullmatch(r'\s*\d+(?:,\s*\d+)*\s*',arguments):
        value['numbers']=[int(n) for n in arguments.split(',')]
    return value


def page(path):
    section = {'/': 'main', '/second': 'second', '/popup': 'popup'}[path]
    style = '\n'.join('#'+key+' {background:rgb'+str(color)+';}' for key,color in COLORS.items())
    if section == 'main':
        content = '''<h1>Tarot browser</h1>
<label>Text<input id="primary" autocomplete="off"></label>
<label>Private<input id="password" type="password" autocomplete="off"></label>
<div class="buttons"><button id="navigate">Next</button><button id="popup">Popup</button><button id="fullscreen">Full</button><button id="exit-fullscreen">Leave</button></div>
<label class="bottom">Bottom<input id="bottom" autocomplete="off"></label>'''
    elif section == 'second':
        content = '<h1>Second page</h1><p>Real browser navigation</p><button id="back">Back</button>'
    else:
        content = '<h1>Popup page</h1><input id="popup-input" autocomplete="off"><button id="close-popup">Close</button>'
    script = '''const expected=EXPECTED, page=SECTION;
let sequence=0; const documentId=String(performance.timeOrigin)+'-'+Math.random().toString(36).slice(2);
TRANSPORT
function report(event) {
 const fields={}; document.querySelectorAll('input').forEach(e=>{
  const r=e.getBoundingClientRect(); fields[e.id]={state:expected.indexOf(e.value),length:e.value.length,focused:document.activeElement===e,x:r.x,y:r.y,width:r.width,height:r.height};
 });
 send(event,{fields,width:innerWidth,height:innerHeight,pixelRatio:devicePixelRatio,fullscreen:!!document.fullscreenElement});
}
document.addEventListener('pointerdown',e=>send('pointer',{field:e.target.id,x:e.clientX,y:e.clientY}));
document.querySelectorAll('input').forEach(e=>['input','focus','blur'].forEach(event=>e.addEventListener(event,()=>report(event))));
addEventListener('resize',()=>report('resize')); addEventListener('pageshow',()=>report('pageshow'));
document.addEventListener('fullscreenchange',()=>report('fullscreen'));
function button(id,action) {const e=document.getElementById(id);if(e)e.onclick=()=>{send('button',{button:id});action();};}
button('navigate',()=>location.href='/second');button('back',()=>history.back());
button('popup',()=>window.open('/popup','tarot-popup','popup,width=320,height=280'));
button('close-popup',()=>window.close());button('fullscreen',()=>document.documentElement.requestFullscreen().catch(()=>send('fullscreen-rejected')));
button('exit-fullscreen',()=>{if(document.fullscreenElement)document.exitFullscreen();});
// Independent, read-only state observations distinguish missed event delivery
// from actual DOM contents. They never focus, edit or submit a field.
setInterval(()=>report('heartbeat'),500);
report('ready');'''.replace('EXPECTED', json.dumps(EXPECTED)).replace('SECTION', json.dumps(section)).replace('TRANSPORT',REPORT_TRANSPORT)
    return ('''<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Tarot browser '''+section+'''</title><style>
*{box-sizing:border-box}body{margin:12px;font:16px sans-serif;background:#f5f1e9;color:#20262b}h1{font-size:21px;margin:0 0 10px}label{display:block;margin-bottom:10px}input{display:block;width:100%;height:46px;font:19px sans-serif;border:2px solid #45535c;border-radius:5px;padding:6px}button{height:44px;border:1px solid #45535c;border-radius:5px;font:16px sans-serif;padding:6px}.buttons{display:grid;grid-template-columns:repeat(4,1fr);gap:4px}.bottom{position:fixed;left:12px;right:12px;bottom:48px;margin:0}
'''+style+'</style></head><body>'+content+'<script>'+script+'</script></body></html>').encode()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--wayland-debug',action='store_true')
    parser.add_argument('--protocol-metadata',action='store_true',
                        help='Record text-input method names and numeric serials, excluding string arguments')
    args=parser.parse_args();out=args.output.resolve()
    assert os.geteuid()==10000 and out.is_dir() and out.stat().st_uid==10000
    os.umask(0o077)
    log=(out/'events.jsonl').open('x',buffering=1);lock=threading.Lock()
    def emit(event,**fields):
        with lock:log.write(json.dumps(dict(event=event,monotonic=time.monotonic(),**fields))+'\n')
    class Handler(BaseHTTPRequestHandler):
        protocol_version='HTTP/1.1'
        def log_message(self,*args):pass
        def do_GET(self):
            if self.path not in ['/', '/second', '/popup']:
                self.send_error(404);return
            data=page(self.path);self.send_response(200)
            emit('page-served',page={'/':'main','/second':'second','/popup':'popup'}[self.path])
            self.send_header('Content-Type','text/html; charset=utf-8')
            self.send_header('Content-Length',str(len(data)));self.send_header('Cache-Control','no-store')
            self.send_header('Content-Security-Policy',"default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'none'")
            self.end_headers();self.wfile.write(data)
        def do_POST(self):
            if self.path!='/event':self.send_error(404);return
            size=int(self.headers.get('Content-Length','0'))
            if not 0<size<=8192:self.send_error(400);return
            value=json.loads(self.rfile.read(size))
            # The page sends state indices, lengths and geometry, never values.
            assert set(value)<=set(['event','page','document','sequence','observed','transportFailures','fields','width','height','pixelRatio','fullscreen','button','field','x','y'])
            assert value.get('page') in ['main','second','popup']
            assert isinstance(value['document'],str) and 0<len(value['document'])<=64
            assert type(value['sequence']) is int and value['sequence']>0
            assert isinstance(value['observed'],(int,float)) and value['observed']>0
            assert type(value['transportFailures']) is int and value['transportFailures']>=0
            for field in value.get('fields',{}).values():
                assert set(field)==set(['state','length','focused','x','y','width','height'])
                assert isinstance(field['state'],int) and -1<=field['state']<len(EXPECTED)
            event=value.pop('event')
            emit(event,report_delivery_ms=time.time()*1000-value['observed'],**value)
            self.send_response(204);self.send_header('Content-Length','0');self.end_headers()
    server=ObserverServer(('127.0.0.1',0),Handler)
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    home=out/'home';home.mkdir(mode=0o700)
    env=dict(os.environ,HOME=str(home),GDK_BACKEND='wayland',MOZ_ENABLE_WAYLAND='1')
    env.pop('DISPLAY',None);env.pop('WAYLAND_SOCKET',None)
    assert not (args.wayland_debug and args.protocol_metadata)
    if args.wayland_debug or args.protocol_metadata:env['WAYLAND_DEBUG']='client'
    for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache'),('XDG_STATE_HOME','state')]:
        p=home/name;p.mkdir(mode=0o700);env[key]=str(p)
    profile=home/'profile';profile.mkdir(mode=0o700)
    prefs={'browser.shell.checkDefaultBrowser':False,'browser.aboutwelcome.enabled':False,
           'browser.startup.homepage_override.mstone':'ignore','browser.startup.page':0,
           'browser.newtabpage.enabled':False}
    (profile/'user.js').write_text(''.join('user_pref('+json.dumps(key)+','+json.dumps(value)+');\n' for key,value in prefs.items()))
    binary=Path('/usr/lib/firefox-esr/firefox-esr')
    command=[str(binary),'--no-remote','--profile',str(profile),'--new-window','http://127.0.0.1:'+str(server.server_port)+'/']
    with (out/'firefox.log').open('x') as output:
        browser=subprocess.Popen(command,env=env,stdout=output,
                                 stderr=subprocess.PIPE if args.protocol_metadata else subprocess.STDOUT)
        def protocol_metadata():
            ignored=0
            for raw in browser.stderr:
                line=raw.decode(errors='replace')
                value=text_input_metadata(line)
                if value is None:
                    ignored+=1;continue
                emit('protocol',**value)
            emit('protocol-end',ignored_stderr_lines=ignored)
        monitor=None
        if args.protocol_metadata:
            monitor=threading.Thread(target=protocol_metadata,daemon=True);monitor.start()
        emit('started',uid=os.geteuid(),pid=os.getpid(),browser_pid=browser.pid,
             cgroup=Path('/proc/self/cgroup').read_text(),browser_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
             profile=str(profile),port=server.server_port,command=command)
        try:code=browser.wait(timeout=900)
        except BaseException:
            browser.terminate()
            try:browser.wait(timeout=15)
            except subprocess.TimeoutExpired:browser.kill();browser.wait()
            raise
        finally:
            if monitor is not None:monitor.join(timeout=5)
            server.shutdown();server.server_close()
    emit('exit',code=code)
    raise SystemExit(code)


if __name__=='__main__':main()
