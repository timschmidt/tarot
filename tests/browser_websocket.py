"""Small loopback-only snapshot receiver for the synthetic browser fixture.

RFC 6455 framing; at most 8192 bytes per text message. This is QA transport,
not a service exposed by Tarot. Payload validation remains with the fixture.
"""
import base64
import hashlib
import struct


REPORT_WEBSOCKET = '''let pendingReports=[], transportFailures=0, reportSocket;
function pumpReports() {
 while(reportSocket?.readyState===WebSocket.OPEN && pendingReports.length && reportSocket.bufferedAmount<65536) {
  const value=pendingReports[0];
  try {reportSocket.send(JSON.stringify({...value,transportFailures}));pendingReports.shift();}
  catch {reportSocket.close();break;}
 }
}
function connectReports() {
 const socket=reportSocket=new WebSocket('ws://'+location.host+'/events');
 const deadline=setTimeout(()=>socket.close(),3000);
 socket.onopen=()=>{clearTimeout(deadline);pumpReports();};
 socket.onerror=()=>{};
 socket.onclose=()=>{clearTimeout(deadline);transportFailures++;setTimeout(connectReports,500);};
}
function send(event,extra={}) {
 const value={event,page,document:documentId,sequence:++sequence,observed:performance.timeOrigin+performance.now(),...extra};
 if(event==='heartbeat')pendingReports=pendingReports.filter(e=>e.event!=='heartbeat');
 pendingReports.push(value);pumpReports();
}
connectReports();
'''


def receive_snapshots(handler, observe):
    """Upgrade an exact-loopback origin and deliver bounded JSON text frames."""
    origin='http://127.0.0.1:'+str(handler.server.server_port)
    assert handler.headers.get('Origin')==origin
    assert handler.headers.get('Upgrade','').lower()=='websocket'
    assert 'upgrade' in [v.strip().lower() for v in handler.headers.get('Connection','').split(',')]
    assert handler.headers.get('Sec-WebSocket-Version')=='13'
    key=handler.headers['Sec-WebSocket-Key']
    assert len(base64.b64decode(key,validate=True))==16
    accept=base64.b64encode(hashlib.sha1((key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
    handler.send_response(101)
    for name,value in [('Upgrade','websocket'),('Connection','Upgrade'),('Sec-WebSocket-Accept',accept)]:
        handler.send_header(name,value)
    handler.end_headers();handler.close_connection=True

    def read(count):
        data=handler.rfile.read(count)
        if len(data)!=count:raise EOFError
        return data

    def reply(opcode,payload):
        assert len(payload)<=125
        handler.wfile.write(bytes([0x80|opcode,len(payload)])+payload)
        handler.wfile.flush()

    pending=None
    try:
        while True:
            first,second=read(2);opcode=first & 15;final=bool(first & 128)
            assert not first & 0x70 and second & 128
            length=second & 127
            assert length!=127  # Larger than this bounded fixture permits.
            if length==126:length=struct.unpack('!H',read(2))[0]
            assert length<=8192
            mask=read(4);payload=bytes(b ^ mask[i%4] for i,b in enumerate(read(length)))
            if opcode & 8:
                assert final and length<=125
                if opcode==8:
                    assert length!=1;reply(8,payload);return
                if opcode==9:reply(10,payload)
                else:assert opcode==10
                continue
            if opcode==1:
                assert pending is None;pending=bytearray(payload)
            else:
                assert opcode==0 and pending is not None;pending.extend(payload)
            assert len(pending)<=8192
            if final:
                observe(pending.decode('utf-8'));pending=None
    except (EOFError,ConnectionError):
        return
