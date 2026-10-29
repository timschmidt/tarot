"""Exercise snapshot framing on real sockets and the shared browser producer."""
from http.server import BaseHTTPRequestHandler
import io
import json
import shutil
import socket
import struct
import subprocess
import threading
from types import SimpleNamespace
import unittest
from browser_fixture import ObserverServer
from browser_websocket import REPORT_WEBSOCKET, receive_snapshots


def frame(payload,opcode=1,final=True):
    mask=b'1234';length=len(payload)
    header=bytes([(128 if final else 0)|opcode,128|(length if length<126 else 126)])
    if length>=126:header+=struct.pack('!H',length)
    return header+mask+bytes(b ^ mask[i%4] for i,b in enumerate(payload))


class BrowserWebSocketTests(unittest.TestCase):
    def test_real_upgrade_masked_frames_fragmentation_ping_and_close(self):
        received=[];finished=threading.Event()
        class Handler(BaseHTTPRequestHandler):
            protocol_version='HTTP/1.1'
            def log_message(self,*args):pass
            def do_GET(self):
                try:receive_snapshots(self,lambda data:received.append(json.loads(data)))
                finally:finished.set()
        server=ObserverServer(('127.0.0.1',0),Handler)
        worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
        try:
            with socket.create_connection(server.server_address,timeout=2) as client:
                stream=client.makefile('rb')
                client.sendall(('GET /events HTTP/1.1\r\nHost: 127.0.0.1:'+str(server.server_port)+'\r\nOrigin: http://127.0.0.1:'+str(server.server_port)+'\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n').encode())
                self.assertIn(b'101',stream.readline());headers=[]
                while (line:=stream.readline())!=b'\r\n':headers.append(line)
                self.assertIn(b'Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n',headers)
                payload=json.dumps({'sequence':1,'padding':'x'*180}).encode()
                client.sendall(frame(payload[:130],final=False)+frame(b'ping',opcode=9)+frame(payload[130:],opcode=0)+frame(b'{"sequence":2}'))
                self.assertEqual(stream.read(6),b'\x8a\x04ping')
                client.sendall(frame(b'',opcode=8));self.assertEqual(stream.read(2),b'\x88\x00');stream.close()
            self.assertTrue(finished.wait(2));self.assertEqual([e['sequence'] for e in received],[1,2])
        finally:server.shutdown();server.server_close();worker.join(timeout=2)

    def test_oversized_message_is_rejected_before_delivery(self):
        delivered=[]
        handler=SimpleNamespace(server=SimpleNamespace(server_port=1234),
            headers={'Origin':'http://127.0.0.1:1234','Upgrade':'websocket','Connection':'Upgrade',
                     'Sec-WebSocket-Version':'13','Sec-WebSocket-Key':'dGhlIHNhbXBsZSBub25jZQ=='},
            rfile=io.BytesIO(frame(b'x'*8193)),wfile=io.BytesIO(),
            send_response=lambda *_:None,send_header=lambda *_:None,end_headers=lambda:None)
        with self.assertRaises(AssertionError):receive_snapshots(handler,delivered.append)
        self.assertEqual(delivered,[])

    @unittest.skipUnless(shutil.which('node'),'Node is optional for the shared browser producer')
    def test_producer_retains_events_coalesces_polls_and_reconnects(self):
        setup="""import assert from 'node:assert/strict';
        const sockets=[],timers=new Map();let timerId=0,sequence=0;
        const page='main',documentId='synthetic',location={host:'127.0.0.1:1234'};
        globalThis.setTimeout=(fn,delay)=>{timers.set(++timerId,{fn,delay});return timerId;};
        globalThis.clearTimeout=id=>timers.delete(id);
        globalThis.WebSocket=class {
          static OPEN=1;readyState=0;bufferedAmount=0;sent=[];
          constructor(url){assert.equal(url,'ws://127.0.0.1:1234/events');sockets.push(this);}
          send(text){this.sent.push(JSON.parse(text));}
          close(){this.readyState=3;this.onclose();}
          open(){this.readyState=1;this.onopen();}
        };
        """
        test="""
        send('input');send('heartbeat');send('heartbeat');send('focus');
        assert.equal(sockets.length,1);sockets[0].open();
        assert.deepEqual(sockets[0].sent.map(e=>e.sequence),[1,3,4]);
        sockets[0].bufferedAmount=65536;send('heartbeat');send('heartbeat');send('input');
        sockets[0].close();assert.equal(transportFailures,1);
        const retry=[...timers.values()].find(t=>t.delay===500);assert.ok(retry);retry.fn();
        sockets[1].open();assert.deepEqual(sockets[1].sent.map(e=>e.sequence),[6,7]);
        assert.deepEqual(sockets[1].sent.map(e=>e.transportFailures),[1,1]);
        assert.equal(pendingReports.length,0);
        """
        subprocess.run([shutil.which('node'),'--input-type=module'],input=setup+REPORT_WEBSOCKET+test,text=True,check=True,capture_output=True)


if __name__=='__main__':unittest.main()
