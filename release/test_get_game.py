"""Exercise the published wrapper with real aria2 and a local tracker/seed."""
import hashlib
import http.server
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import threading
import unittest
import build_torrent

HERE = Path(__file__).resolve().parent

def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]

class Download(unittest.TestCase):
    def test_real_transfer_and_diagnostics(self):
        aria2, pwsh = shutil.which('aria2c'), shutil.which('pwsh')
        self.assertTrue(aria2 and pwsh, 'aria2c and pwsh are required')
        with tempfile.TemporaryDirectory(prefix='skhid-get test ') as folder:
            root = Path(folder)
            seed = root / 'seed'; seed.mkdir()
            data = os.urandom(1024 * 1024)
            (seed / 'sample.bin').write_bytes(data)
            peer_port = port()
            class Tracker(http.server.BaseHTTPRequestHandler):
                def do_GET(self):
                    if self.path.startswith('/denied'):
                        response = {b'failure reason': b'fixture tracker refused'}
                    else:
                        response = {b'interval': 30, b'complete': 1, b'incomplete': 1,
                                    b'peers': socket.inet_aton('127.0.0.1') + struct.pack('!H', peer_port)}
                    body = build_torrent.bencode(response)
                    self.send_response(200); self.end_headers(); self.wfile.write(body)
                def log_message(self, *args): pass
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Tracker)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            base = f'http://127.0.0.1:{server.server_port}'
            info = {b'name': b'sample.bin', b'length': len(data), b'piece length': 262144,
                    b'pieces': b''.join(hashlib.sha1(data[i:i+262144]).digest() for i in range(0,len(data),262144))}
            torrent = root/'sample.torrent'
            torrent.write_bytes(build_torrent.bencode({b'announce': (base+'/announce').encode(), b'announce-list': [[(base+'/denied').encode()],[(base+'/announce').encode()]], b'info': info}))
            common = ['--no-conf=true','--enable-dht=false','--enable-dht6=false','--enable-peer-exchange=false','--file-allocation=none','--check-integrity=true']
            log = root/'diagnostics.log'
            ps = root/'test.ps1'
            # The runtime accepts executable paths and arguments as data, even with spaces.
            def quoted(s): return "'" + str(s).replace("'", "''") + "'"
            args = common + [f'--dir={root / "download folder"}', '--seed-time=0', '--summary-interval=0', str(torrent)]
            ps.write_text("$ErrorActionPreference='Stop'\n. " + quoted(HERE/'get-game-diagnostics.ps1') + '\nInvoke-TorrentDownload ' + quoted(aria2) + ' @(' + ','.join(map(quoted,args)) + ') ' + quoted(log))
            with (root/'seed.log').open('w') as seed_log:
                proc = subprocess.Popen([aria2,*common,f'--dir={seed}',f'--listen-port={peer_port}','--max-upload-limit=64K','--seed-time=2',str(torrent)], stdout=seed_log, stderr=seed_log)
                try:
                    result = subprocess.run([pwsh,'-NoProfile','-File',str(ps)],text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=75)
                    print(result.stdout)
                    self.assertEqual(result.returncode,0)
                    self.assertEqual((root/'download folder/sample.bin').read_bytes(), data)
                    text = log.read_text()
                    self.assertIn('fixture tracker refused',text)
                    self.assertIn('Tracker asked: 127.0.0.1:',text)
                    self.assertIn('Tracker HTTP response: HTTP/1.0 200',text)
                    self.assertIn('Complete:1',text)
                    self.assertIn('Connected 127.0.0.1:',text)
                    self.assertIn('| seed |',text)
                    self.assertIn('complete:',text)
                    self.assertNotIn('token:',text)
                finally:
                    proc.terminate(); proc.wait(timeout=10); server.shutdown();server.server_close()

if __name__=='__main__': unittest.main()
