"""Browser report connections must survive bursts while dispatch is delayed."""
from http.server import BaseHTTPRequestHandler
import http.client
import json
import threading
import unittest
from browser_fixture import ObserverServer


class BrowserObserverTransportTests(unittest.TestCase):
    def test_connections_queue_before_dispatch_and_reuse_http11(self):
        received=[]
        class Handler(BaseHTTPRequestHandler):
            protocol_version='HTTP/1.1'
            def log_message(self,*args):pass
            def do_POST(self):
                received.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
                self.send_response(204);self.send_header('Content-Length','0');self.end_headers()
        server=ObserverServer(('127.0.0.1',0),Handler)
        clients=[];worker=None
        try:
            # Deliberately delay accept/dispatch. More than the old five-entry
            # backlog must complete their actual TCP handshake without waiting
            # for a retransmission or an increased native text timeout.
            for _ in range(16):
                client=http.client.HTTPConnection('127.0.0.1',server.server_port,timeout=1)
                client.connect();clients.append(client)
            worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
            for index,client in enumerate(clients):
                for sequence in range(2):
                    body=json.dumps({'connection':index,'sequence':sequence})
                    client.request('POST','/event',body,{'Content-Type':'application/json'})
                    response=client.getresponse();self.assertEqual(response.status,204);response.read()
            self.assertEqual(len(received),32)
            self.assertEqual(received[-1],{'connection':15,'sequence':1})
        finally:
            for client in clients:client.close()
            if worker is not None:server.shutdown();worker.join(timeout=2)
            server.server_close()


if __name__=='__main__':unittest.main()
