#!/usr/bin/env python3
"""Skhid-get.bat's PowerShell against a UDP tracker, BitTorrent peers and an HTTP mirror of this
machine (127.0.0.1 only; nothing leaves it). Needs pwsh (PowerShell 7; Windows 10's PowerShell 5.1 is
not tested here). Start-Process, netsh and the Defender cmdlets are stubs (STUBS): the elevated part
runs in the same process, nothing is started.

    nix shell nixpkgs#python3 nixpkgs#powershell -c python3 -I release/test_bootstrap.py
"""
import hashlib
import http.server
import os
from pathlib import Path
import re
import shutil
import socket
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest

HERE = Path(__file__).parent
SCRIPT = HERE / 'build_torrent.py'
BODY = HERE / 'bootstrap.ps1'
NETWORK_SETUP = HERE.parent / 'srp-026g/client-patches/Skhid-network-setup.cmd'
PIECE = 4 << 20
# SKHID_TEST_UAC=declined: the player says no to the elevation; SKHID_TEST_DEFENDER=off:
# Defender's cmdlets fail. The elevated command runs in a pwsh of its own (it ends with exit).
ELEVATED = r'''
function global:netsh { Write-Host ('netsh' + (-join ($args | ForEach-Object { "`t$_" }))); $global:LASTEXITCODE = 0 }
function global:Get-MpPreference { if ($env:SKHID_TEST_DEFENDER -eq 'off') { throw 'Defender is off' }; [pscustomobject]@{ ExclusionPath = @() } }
function global:Add-MpPreference($ExclusionPath) { Write-Host "excluded: $ExclusionPath" }
'''
STUBS = r'''
$global:Elevated = @'
''' + ELEVATED + r'''
'@
function global:Start-Process($FilePath, $ArgumentList, $Verb, $WindowStyle, $WorkingDirectory, [switch]$Wait, [switch]$PassThru) {
    if ($Verb -ne 'RunAs') { Write-Host "started: $FilePath $ArgumentList"; return [pscustomobject]@{ ExitCode = 0 } }
    if ($env:SKHID_TEST_UAC -eq 'declined') { throw 'The operation was canceled by the user.' }
    if ($ArgumentList -notcontains 'Bypass') { throw 'not -ExecutionPolicy Bypass' }
    $command = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($ArgumentList[-1]))
    pwsh -NoProfile -NonInteractive -Command ($global:Elevated + "`n" + $command) | Write-Host
    [pscustomobject]@{ ExitCode = $LASTEXITCODE }
}
'''
# A file of two pieces (so a padded last piece) first, two small ones after.
FILES = {
    'game/binaries/x86/7zr.exe': bytes(range(256)) * (PIECE // 256) + b'tail of 7zr',
    'game/binaries/x86/skhid_torrent.dll': b'MZ its libtorrent' * 700,
    'game/binaries/x86/updater_skhid.exe': b'MZ the updater' * 1000,
}


def network_setup_commands(x86):
    """The netsh commands of Skhid-network-setup.cmd's :rule for every rule it adds, as arguments."""
    text = NETWORK_SETUP.read_text()
    programs = re.search(r'set "PROGRAMS=(.*)"', text)[1].split()
    sharer = re.search(r'set "SHARER=(.*)"', text)[1]
    rule = re.search(r'\n:rule\n(.*?)\nexit /b', text, re.S)[1]
    name = re.search(r'set "NAME=(.*)"', rule)[1]
    lines = re.findall(r'call :run(?:_quiet)? netsh (.*)', rule)
    out = []
    for program, direction, protocol in [(p, 'out', t) for p in programs for t in ('TCP', 'UDP')] + [(sharer, 'in', 'TCP')]:
        def expand(line):
            line = line.replace('%NAME%', name).replace('%X86%\\', x86 + os.sep)
            return line.replace('%~1', program).replace('%2', direction).replace('%3', protocol)
        for line in lines:
            out.append([word.replace('"', '') for word in re.findall(r'(?:[^\s"]|"[^"]*")+', expand(line))])
    return out


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
    """BEP 3: the handshake, a bitfield of every piece, unchoke, then blocks of `data`. `haves`: a
    bitfield of none, then a have of each piece. `stall`: never unchokes, a keep-alive a second."""
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, info_hash, data, pieces, haves=False, stall=False):
        self.info_hash, self.data, self.pieces, self.served = info_hash, data, pieces, 0
        self.haves, self.stall = haves, stall
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
        if server.stall:
            bits = bytes([255]) * (-(-server.pieces // 8))
            self.request.sendall(struct.pack('>IB', 1 + len(bits), 5) + bits)
            try:
                while True:
                    self.request.sendall(bytes(4))
                    time.sleep(1)
            except OSError:
                return
        if server.haves:
            bits = bytes(-(-server.pieces // 8))
            haves = b''.join(struct.pack('>IBI', 5, 4, i) for i in range(server.pieces))
            self.request.sendall(struct.pack('>IB', 1 + len(bits), 5) + bits + haves + struct.pack('>IB', 1, 1))
        else:
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


class Script(unittest.TestCase):
    """The script's making (build_torrent.py bootstrap), no pwsh: release/test.sh runs this."""

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

    def test_the_bat_is_crlf_and_pins_the_files(self):
        text = self.out.read_bytes()
        self.assertNotIn(b'\n', text.replace(b'\r\n', b''))
        for data in FILES.values():
            self.assertIn(hashlib.sha256(data).hexdigest().encode(), text)
        self.assertIn(self.info_hash.hex().encode(), text)
        self.assertIn(b"$InfoHash = '" + self.info_hash.hex().encode() + b"'", text)


@unittest.skipUnless(shutil.which('pwsh'), 'pwsh is needed')
class Bootstrap(Script):
    def run_script(self, **env):
        # As the .bat runs it: its text from the marker on.
        env = dict(os.environ, SKHID_GET_SCRIPT=str(self.out), **env)
        command = STUBS + "$t = [IO.File]::ReadAllText($env:SKHID_GET_SCRIPT); iex $t.Substring($t.IndexOf('<#' + 'PS#>'))"
        return subprocess.run(['pwsh', '-NoProfile', '-NonInteractive', '-Command', command], env=env,
                              capture_output=True, text=True, timeout=300)

    def installed(self):
        return {p: (self.out.parent / 'Survarium' / p).read_bytes() for p in FILES
                if (self.out.parent / 'Survarium' / p).exists()}

    def test_from_a_peer(self):
        peer = Peer(self.info_hash, stream(), self.pieces)
        self.tracker.peers = [peer.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(self.installed(), FILES)
        self.assertEqual(self.tracker.announced, [self.info_hash.hex()])
        self.assertGreater(peer.served, 0)
        root = self.out.parent / 'Survarium'
        x86 = root / 'game' / 'binaries' / 'x86'
        self.assertIn(f'started: {x86 / "updater_skhid.exe"} check torrent', done.stdout)
        self.assertIn(f'started: {root / "Skhid.bat"}', done.stdout)
        # Run again: the updater is there; nothing is fetched.
        served = peer.served
        self.assertEqual(self.run_script().returncode, 0)
        self.assertEqual(peer.served, served)

    def test_elevated_adds_the_network_setup_rules_and_the_defender_exclusion(self):
        done = self.run_script()
        root = self.out.parent / 'Survarium'
        netsh = [line.split('\t')[1:] for line in done.stdout.splitlines() if line.startswith('netsh\t')]
        self.assertEqual(netsh, network_setup_commands(str(root / 'game' / 'binaries' / 'x86')))
        x86 = root / 'game' / 'binaries' / 'x86'
        # Skhid-defender-setup.cmd's exclusion, and its marker: its --undo removes it.
        self.assertIn(f'excluded: {x86}\n', done.stdout)
        self.assertIn('Firewall rules: added.', done.stdout)
        self.assertIn('Defender exclusion of game\\binaries\\x86: added.', done.stdout)
        self.assertEqual((root / 'Skhid-network-setup.defender').read_text(encoding='utf-8-sig'), str(x86))

    def test_defender_off_keeps_the_firewall_rules_and_says_which_failed(self):
        for path, data in FILES.items():
            self.mirror.files[f"a/{path.rsplit('/', 1)[1]}"] = data
        done = self.run_script(SKHID_TEST_DEFENDER='off')
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('netsh', done.stdout)
        self.assertIn('Firewall rules: added.', done.stdout)
        self.assertIn('No Defender exclusion of game\\binaries\\x86', done.stdout)
        self.assertEqual(self.installed(), FILES)

    def test_uac_declined_says_so_and_goes_on(self):
        for path, data in FILES.items():
            self.mirror.files[f"a/{path.rsplit('/', 1)[1]}"] = data
        done = self.run_script(SKHID_TEST_UAC='declined')
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('No firewall rules or Defender exclusion: Windows may block the game.', done.stdout)
        self.assertNotIn('netsh', done.stdout)
        self.assertEqual(self.installed(), FILES)

    def test_a_lying_peer_is_dropped_for_the_next(self):
        bad = bytearray(stream())
        bad[100] ^= 1
        liar, honest = Peer(self.info_hash, bytes(bad), self.pieces), Peer(self.info_hash, stream(), self.pieces)
        self.tracker.peers = [liar.server_address, honest.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('not the pinned one', done.stdout)
        self.assertEqual(self.installed(), FILES)

    def test_a_bitfield_of_none_then_haves(self):
        peer = Peer(self.info_hash, stream(), self.pieces, haves=True)
        self.tracker.peers = [peer.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(self.installed(), FILES)
        self.assertGreater(peer.served, 0)

    def test_a_peer_that_never_gives_a_piece_is_left_for_the_addresses(self):
        staller = Peer(self.info_hash, stream(), self.pieces, stall=True)
        self.tracker.peers = [staller.server_address]
        for path, data in FILES.items():
            self.mirror.files[f"a/{path.rsplit('/', 1)[1]}"] = data
        started = time.monotonic()
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertIn('no piece in time', done.stdout)
        self.assertEqual(self.installed(), FILES)
        self.assertLess(time.monotonic() - started, 120)

    def test_a_folder_with_brackets(self):
        folder = self.root / 'get [1]'
        folder.mkdir()
        shutil.copy(self.out, folder / 'Skhid-get.bat')
        self.out = folder / 'Skhid-get.bat'
        peer = Peer(self.info_hash, stream(), self.pieces)
        self.tracker.peers = [peer.server_address]
        done = self.run_script()
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(self.installed(), FILES)
        self.assertIn('is there already', self.run_script().stdout)

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
