#!/usr/bin/env python3
"""Skhid-get.bat's PowerShell against a UDP tracker, BitTorrent peers and an HTTP mirror of this
machine (127.0.0.1 only; nothing leaves it). Needs pwsh (PowerShell 7; Windows PowerShell 2 is
not tested here):

    nix shell nixpkgs#python3 nixpkgs#powershell -c python3 -I release/test_bootstrap.py
"""
import hashlib
import http.server
import os
from pathlib import Path
import shutil
import socket
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import unittest

HERE = Path(__file__).parent
SCRIPT = HERE / 'build_torrent.py'
BODY = HERE / 'bootstrap.ps1'
PIECE = 4 << 20
# A file of two pieces (so a padded last piece) first, a small one last.
FILES = {
    'game/binaries/x86/7zr.exe': bytes(range(256)) * (PIECE // 256) + b'tail of 7zr',
    'game/binaries/x86/updater_skhid.exe': b'MZ the updater' * 1000,
}


def stream():
    """The torrent's v1 bytes: each file, then zeros to its piece's end, but the last file."""
    out = b''
    for i, data in enumerate(FILES.values()):
        out += data
        if i < len(FILES) - 1:
            out += bytes(-len(data) % PIECE)
    return out


class Tracker(threading.Thread):
    """BEP 15: a connect, then an announce answered with `peers` (address, port)."""

    def __init__(self):
        super().__init__(daemon=True)
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind(('127.0.0.1', 0))
        self.port, self.peers, self.announced = self.socket.getsockname()[1], [], []

    def run(self):
        while True:
            data, sender = self.socket.recvfrom(2048)
            action, tid = struct.unpack('>II', data[8:16])
            if action == 0 and data[:8] == bytes.fromhex('0000041727101980'):
                self.socket.sendto(struct.pack('>II', 0, tid) + b'connidcd', sender)
            elif action == 1 and data[:8] == b'connidcd' and len(data) == 98:
                self.announced.append(data[16:36].hex())
                peers = b''.join(socket.inet_aton(a) + struct.pack('>H', p) for a, p in self.peers)
                self.socket.sendto(struct.pack('>IIIII', 1, tid, 1800, 0, len(self.peers)) + peers, sender)


class Peer(socketserver.ThreadingTCPServer):
    """BEP 3: the handshake, a bitfield of every piece, unchoke, then blocks of `data`."""
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, info_hash, data, pieces):
        self.info_hash, self.data, self.pieces, self.served = info_hash, data, pieces, 0
        super().__init__(('127.0.0.1', 0), PeerHandler)
        threading.Thread(target=self.serve_forever, daemon=True).start()


class PeerHandler(socketserver.BaseRequestHandler):
    def read(self, n):
        out = b''
        while len(out) < n:
            part = self.request.recv(n - len(out))
            if not part:
                raise EOFError
            out += part
        return out

    def handle(self):
        server = self.server
        hello = self.read(68)
        if hello[28:48] != server.info_hash:
            return
        self.request.sendall(hello[:20] + bytes(8) + server.info_hash + b'-PY0001-' + bytes(12))
        bits = bytes([255]) * (-(-server.pieces // 8))
        self.request.sendall(struct.pack('>IB', 1 + len(bits), 5) + bits + struct.pack('>IB', 1, 1))
        try:
            while True:
                length, = struct.unpack('>I', self.read(4))
                message = self.read(length)
                if message[:1] == b'\x06':
                    index, begin, size = struct.unpack('>III', message[1:])
                    at = index * PIECE + begin
                    block = server.data[at:at + size]
                    server.served += 1
                    self.request.sendall(struct.pack('>IBII', 9 + len(block), 7, index, begin) + block)
        except (EOFError, ConnectionError):
            pass


class Mirror(http.server.ThreadingHTTPServer):
    def __init__(self, files):
        self.files = files
        super().__init__(('127.0.0.1', 0), MirrorHandler)
        threading.Thread(target=self.serve_forever, daemon=True).start()


class MirrorHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        data = self.server.files.get(self.path.lstrip('/'))
        if data is None:
            self.send_response(404)
            self.end_headers()
            return
        self.send_response(200)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)


@unittest.skipUnless(shutil.which('pwsh'), 'pwsh is needed')
class Bootstrap(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.tracker = Tracker()
        self.tracker.start()
        self.mirror = Mirror({})
        sources = []
        for path, data in FILES.items():
            source = self.root / 'in' / path.rsplit('/', 1)[1]
            source.parent.mkdir(exist_ok=True)
            source.write_bytes(data)
            sources += ['--file', path, str(source)]
        port = self.mirror.server_address[1]
        rows = ''.join(f"{p.rsplit('/', 1)[1]}\t{hashlib.sha256(d).hexdigest()}\t{len(d)}\t"
                       f"http://127.0.0.1:{port}/a/{p.rsplit('/', 1)[1]} http://127.0.0.1:{port}/b/{p.rsplit('/', 1)[1]}\n"
                       for p, d in FILES.items())
        (self.root / 'bootstrap.tsv').write_text('# name\tsha256\tsize\turls\n' + rows)
        self.out = self.root / 'get' / 'Skhid-get.bat'
        self.out.parent.mkdir()
        done = subprocess.run([sys.executable, '-I', str(SCRIPT), 'bootstrap', '--torrent', str(self.root / 'b.torrent'),
                               *sources, '--list', str(self.root / 'bootstrap.tsv'), '--body', str(BODY),
                               '--out', str(self.out), '--tracker', f'udp://127.0.0.1:{self.tracker.port}/announce'],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        raw = (self.root / 'b.torrent').read_bytes()
        start = raw.index(b'4:infod') + 6
        sys.path.insert(0, str(HERE))
        import build_torrent
        self.info_hash = hashlib.sha1(build_torrent.info_bytes(raw)).digest()
        self.pieces = -(-len(stream()) // PIECE)

    def run_script(self):
        # As the .bat runs it: its text from the marker on.
        env = dict(os.environ, SKHID_GET_SCRIPT=str(self.out), SKHID_GET_NO_START='1')
        command = "$t = [IO.File]::ReadAllText($env:SKHID_GET_SCRIPT); iex $t.Substring($t.IndexOf('<#' + 'PS#>'))"
        return subprocess.run(['pwsh', '-NoProfile', '-NonInteractive', '-Command', command], env=env,
                              capture_output=True, text=True, timeout=300)

    def installed(self):
        return {p: (self.out.parent / 'Survarium' / p).read_bytes() for p in FILES
                if (self.out.parent / 'Survarium' / p).exists()}

    def test_the_bat_is_crlf_and_pins_the_files(self):
        text = self.out.read_bytes()
        self.assertNotIn(b'\n', text.replace(b'\r\n', b''))
        for data in FILES.values():
            self.assertIn(hashlib.sha256(data).hexdigest().encode(), text)
        self.assertIn(self.info_hash.hex().encode(), text)

    def test_from_a_peer(self):
        peer = Peer(self.info_hash, stream(), self.pieces)
        self.tracker.peers = [peer.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(self.installed(), FILES)
        self.assertEqual(self.tracker.announced, [self.info_hash.hex()])
        self.assertGreater(peer.served, 0)
        # Run again: the updater is there; nothing is fetched.
        served = peer.served
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(peer.served, served)

    def test_a_lying_peer_is_dropped_for_the_next(self):
        bad = bytearray(stream())
        bad[100] ^= 1
        liar, honest = Peer(self.info_hash, bytes(bad), self.pieces), Peer(self.info_hash, stream(), self.pieces)
        self.tracker.peers = [liar.server_address, honest.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('not the pinned one', done.stdout)
        self.assertEqual(self.installed(), FILES)

    def test_no_peer_then_the_addresses_in_their_order(self):
        # The first address of each file gives another file, the second the right one.
        for path, data in FILES.items():
            name = path.rsplit('/', 1)[1]
            self.mirror.files[f'a/{name}'] = b'not it'
            self.mirror.files[f'b/{name}'] = data
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('not the pinned file', done.stdout)
        self.assertEqual(self.installed(), FILES)

    def test_nothing_gives_it_says_so_and_writes_nothing(self):
        done = self.run_script()
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('no peer and no address gave it', done.stdout + done.stderr)
        self.assertEqual(self.installed(), {})


if __name__ == '__main__':
    unittest.main()
