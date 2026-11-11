"""Actual loopback HTTP interruption/resume; no remote service or credentials."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
from pathlib import Path
import socket
import tempfile
import threading
import unittest

spec=importlib.util.spec_from_file_location('accelerator',Path(__file__).with_name('setup-accelerator.py'))
accelerator=importlib.util.module_from_spec(spec)
spec.loader.exec_module(accelerator)
PAYLOAD=bytes(range(256))*4096

class Handler(BaseHTTPRequestHandler):
    mode='interrupt'
    requests=[]
    def log_message(self,*args): pass
    def do_GET(self):
        value=self.headers.get('Range')
        self.requests.append(value)
        offset=int(value.removeprefix('bytes=').split('-')[0]) if value else 0
        self.send_response(206 if offset else 200)
        self.send_header('Content-Length',str(len(PAYLOAD)-offset))
        if offset: self.send_header('Content-Range',f'bytes {offset}-{len(PAYLOAD)-1}/{len(PAYLOAD)}')
        self.end_headers()
        if self.mode=='interrupt':
            self.wfile.write(PAYLOAD[offset:offset+65536]); self.wfile.flush()
            self.connection.shutdown(socket.SHUT_WR)
        else:
            data=PAYLOAD[offset:]
            if self.mode=='corrupt': data=b'x'*len(data)
            self.wfile.write(data)

class NetworkDownloadTests(unittest.TestCase):
    def test_real_disconnection_resumes_then_reuses_verified_file(self):
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        Handler.requests=[];Handler.mode='interrupt'
        try:
            with tempfile.TemporaryDirectory(prefix='parlo-download-') as folder:
                target=Path(folder)/'public-fixture.bin'
                url=f'http://127.0.0.1:{server.server_port}/fixture'
                checksum=hashlib.sha256(PAYLOAD).hexdigest()
                with self.assertRaisesRegex(RuntimeError,'incompleto'):
                    accelerator.download(url,target,len(PAYLOAD),checksum)
                self.assertFalse(target.exists())
                self.assertEqual(target.with_suffix('.bin.part').stat().st_size,65536)
                Handler.mode='complete'
                accelerator.download(url,target,len(PAYLOAD),checksum)
                self.assertEqual(Handler.requests,[None,'bytes=65536-'])
                self.assertEqual(target.read_bytes(),PAYLOAD)
                self.assertFalse(target.with_suffix('.bin.part').exists())
                accelerator.download(url,target,len(PAYLOAD),checksum)
                self.assertEqual(len(Handler.requests),2)
                target.unlink();Handler.mode='corrupt'
                with self.assertRaisesRegex(RuntimeError,'Verifica'):
                    accelerator.download(url,target,len(PAYLOAD),checksum)
                self.assertFalse(target.exists())
                self.assertFalse(target.with_suffix('.bin.part').exists())
                Handler.mode='complete'
                accelerator.download(url,target,len(PAYLOAD),checksum)
                self.assertEqual(target.read_bytes(),PAYLOAD)
        finally:
            server.shutdown();server.server_close();thread.join()

if __name__=='__main__': unittest.main()
