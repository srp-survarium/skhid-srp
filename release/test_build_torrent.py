#!/usr/bin/env python3
"""build_torrent.py on small files and a throwaway key; no network. Needs the openssl program
(nix shell nixpkgs#openssl nixpkgs#python3 -c python3 -I release/test_build_torrent.py). With
libtorrent importable (python3Packages.libtorrent-rasterbar), it also reads the torrent and checks
the files."""
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import zipfile

SCRIPT = Path(__file__).with_name('build_torrent.py')
sys.path.insert(0, str(SCRIPT.parent))
import build_torrent as T  # noqa: E402


def decode(data, at=0):
    """(value, end) of the bencoded value at `at`: dict keys as bytes."""
    kind = data[at:at + 1]
    if kind == b'i':
        end = data.index(b'e', at)
        return int(data[at + 1:end]), end + 1
    if kind in (b'l', b'd'):
        at, items = at + 1, []
        while data[at:at + 1] != b'e':
            item, at = decode(data, at)
            items.append(item)
        if kind == b'l':
            return items, at + 1
        return dict(zip(items[::2], items[1::2])), at + 1
    colon = data.index(b':', at)
    end = colon + 1 + int(data[at:colon])
    return data[colon + 1:end], end


def make_key(folder, name):
    """A throwaway Ed25519 key: (its PEM file, its public key's file of 64 hex digits)."""
    key, public = folder / f'{name}.pem', folder / f'{name}.pub'
    subprocess.run(['openssl', 'genpkey', '-algorithm', 'ed25519', '-out', str(key)], check=True)
    der = subprocess.run(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'],
                         check=True, capture_output=True).stdout
    public.write_text(der[-32:].hex() + '\n')
    return key, public


@unittest.skipUnless(shutil.which('openssl'), 'the openssl program is needed')
class Build(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.key, self.public = make_key(self.root, 'key')
        self.folder = self.root / 'Survarium'
        # Sizes around a block and a piece, and a file of several pieces.
        self.bytes = {
            'game/binaries/x86/survarium.exe': b'original exe',
            'game/binaries/x86/bugtrap.dll': b'original dll',
            'game/resources.db': bytes(range(256)) * (T.PIECE * 2 // 256 + 77),
            'game/resources.db.patch_12': b'p' * T.BLOCK,
            'game/resources.db.patch_14': b'q' * (T.PIECE + 1),
        }
        self.zip_bytes = {
            'game/binaries/x86/bugtrap.dll': b'skhid dll',
            'game/binaries/x86/survarium_skhid.exe': b'x' * (T.BLOCK + 5),
            'Skhid.bat': b'@echo off\r\n',
            'game.txt': b'sorts after game/ by part, before it by bytes',
        }
        self.sources = {}
        for path, data in self.bytes.items():
            source = self.root / 'in' / path.replace('/', '_')
            source.parent.mkdir(exist_ok=True)
            source.write_bytes(data)
            self.sources[path] = source
        self.zip = self.root / 'files.zip'
        with zipfile.ZipFile(self.zip, 'x') as out:
            for path, data in self.zip_bytes.items():
                out.writestr(path, data)

    def run_build(self, out, sha=None, zip=None, public=None):
        command = [sys.executable, '-I', str(SCRIPT), '--out', str(out), '--zip', str(zip or self.zip),
                   '--zip-out', str(out) + '.zip', '--revision', '59', '--key', str(self.key),
                   '--public-key', str(public or self.public)]
        for path, source in self.sources.items():
            command += ['--file', path, str(source), sha or hashlib.sha256(self.bytes[path]).hexdigest()]
        return subprocess.run(command, capture_output=True, text=True)

    def expected(self):
        return {**self.bytes, **self.zip_bytes}

    def test_layout_hashes_and_determinism(self):
        first, second = self.root / 'a.torrent', self.root / 'b.torrent'
        for out in (first, second):
            done = self.run_build(out)
            self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(Path(f'{first}.zip').read_bytes(), Path(f'{second}.zip').read_bytes())
        # The ZIP with it: every entry as before, and the torrent, stored.
        with zipfile.ZipFile(f'{first}.zip') as archive:
            self.assertEqual(archive.namelist(), [*self.zip_bytes, T.ENTRY])
            for path, data in self.zip_bytes.items():
                self.assertEqual(archive.read(path), data)
            self.assertEqual(archive.read(T.ENTRY), first.read_bytes())
            self.assertEqual(archive.getinfo(T.ENTRY).compress_type, zipfile.ZIP_STORED)
        self.assertIn('bugtrap.dll: the ZIP', done.stdout)
        data = first.read_bytes()
        torrent, end = decode(data)
        self.assertEqual(end, len(data))
        info = torrent[b'info']
        self.assertEqual(info[b'name'], b'Survarium')
        self.assertEqual(info[b'piece length'], T.PIECE)
        self.assertEqual(info[b'meta version'], 2)
        self.assertEqual(torrent[b'announce-list'], [[t.encode()] for t in T.TRACKERS])
        # Signed: the revision and the signature of DOMAIN, it, SHA-256 of info.
        self.assertEqual(sorted(torrent), [b'announce', b'announce-list', b'info', b'piece layers', b'skhid'])
        self.assertEqual(torrent[b'skhid'][b'revision'], 59)
        raw = data[data.index(b'4:infod') + 6:]
        raw = raw[:decode(raw)[1]]
        message = self.root / 'message'
        message.write_bytes(b'skhid torrent signature 1\0' + bytes([59]) + hashlib.sha256(raw).digest())
        (self.root / 'sig').write_bytes(torrent[b'skhid'][b'signature'])
        pem = self.root / 'public.pem'
        subprocess.run(['openssl', 'pkey', '-in', str(self.key), '-pubout', '-out', str(pem)], check=True)
        verify = subprocess.run(['openssl', 'pkeyutl', '-verify', '-rawin', '-pubin', '-inkey', str(pem),
                                 '-in', str(message), '-sigfile', str(self.root / 'sig')], capture_output=True)
        self.assertEqual(verify.returncode, 0, verify.stdout + verify.stderr)
        expected = self.expected()
        # v1: the files in order of their parts, a pad after each but the last.
        order = sorted(expected, key=lambda p: [x.encode() for x in p.split('/')])
        self.assertGreater(order.index('game.txt'), order.index('game/resources.db.patch_14'))
        stream, listed = b'', []
        for entry in info[b'files']:
            path = b'/'.join(entry[b'path']).decode()
            if entry.get(b'attr') == b'p':
                self.assertEqual(len(stream) % T.PIECE + entry[b'length'], T.PIECE)
                stream += bytes(entry[b'length'])
                continue
            self.assertEqual(len(stream) % T.PIECE, 0, path)
            listed.append(path)
            stream += expected[path]
        self.assertEqual(listed, order)
        pieces = [stream[i:i + T.PIECE] for i in range(0, len(stream), T.PIECE)]
        self.assertEqual(info[b'pieces'], b''.join(hashlib.sha1(p).digest() for p in pieces))
        # v2: each file's root from its own bytes; piece layers of files over a piece.
        for path, content in expected.items():
            node = info[b'file tree']
            for part in path.split('/'):
                node = node[part.encode()]
            leaf = node[b'']
            self.assertEqual(leaf[b'length'], len(content))
            self.assertEqual(leaf[b'pieces root'], reference_root(content))
            if len(content) > T.PIECE:
                layer = torrent[b'piece layers'][leaf[b'pieces root']]
                self.assertEqual(len(layer), 32 * -(-len(content) // T.PIECE))
        self.assertEqual(len(torrent[b'piece layers']), 2)
        lt_check(self, first, self.folder, expected)

    def test_wrong_sha256_writes_nothing(self):
        out = self.root / 'c.torrent'
        done = self.run_build(out, sha='0' * 64)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('not ' + '0' * 64, done.stderr)
        self.assertFalse(out.exists())
        self.assertFalse(Path(str(out) + '.part').exists())
        self.assertFalse(Path(str(out) + '.zip').exists())

    def test_a_key_that_is_not_the_public_keys_writes_nothing(self):
        _, other = make_key(self.root, 'other')
        out = self.root / 'f.torrent'
        done = self.run_build(out, public=other)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('does not verify', done.stderr)
        self.assertFalse(out.exists())
        self.assertFalse(Path(str(out) + '.zip').exists())

    def test_a_zip_with_a_torrent_is_refused(self):
        done = self.run_build(self.root / 'd.torrent')
        self.assertEqual(done.returncode, 0, done.stderr)
        again = self.run_build(self.root / 'e.torrent', zip=self.root / 'd.torrent.zip')
        self.assertNotEqual(again.returncode, 0)
        self.assertIn('or the torrent', again.stderr)


def reference_root(content):
    """BEP 52's root, the plain way: 16 KiB leaves, zero leaves to a power of two."""
    leaves = [hashlib.sha256(content[i:i + T.BLOCK]).digest() for i in range(0, len(content), T.BLOCK)]
    width = 1
    while width < len(leaves):
        width *= 2
    layer = leaves + [bytes(32)] * (width - len(leaves))
    while len(layer) > 1:
        layer = [hashlib.sha256(layer[i] + layer[i + 1]).digest() for i in range(0, len(layer), 2)]
    return layer[0]


def lt_check(test, torrent, folder, expected):
    """libtorrent loads it as hybrid and finds every piece of the folder good."""
    try:
        import libtorrent as lt
    except ImportError:
        print('skip libtorrent check (not importable)', file=sys.stderr)
        return
    for path, data in expected.items():
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        (folder / path).write_bytes(data)
    # Without its trackers: nothing leaves this machine (the info hashes are of info alone).
    without = lt.bdecode(torrent.read_bytes())
    del without[b'announce'], without[b'announce-list']
    info = lt.torrent_info(without)
    hashes = info.info_hashes()
    test.assertTrue(hashes.has_v1() and hashes.has_v2())
    raw = torrent.read_bytes()
    start = raw.index(b'4:infod') + 6
    _, end = decode(raw, start)
    test.assertEqual(str(hashes.v1), hashlib.sha1(raw[start:end]).hexdigest())
    test.assertEqual(str(hashes.v2), hashlib.sha256(raw[start:end]).hexdigest())
    session = lt.session({'listen_interfaces': '127.0.0.1:0', 'enable_dht': False,
                          'enable_lsd': False, 'enable_upnp': False, 'enable_natpmp': False})
    handle = session.add_torrent({'ti': info, 'save_path': str(folder.parent)})
    for _ in range(200):
        status = handle.status()
        if status.state not in (lt.torrent_status.checking_files, lt.torrent_status.checking_resume_data) \
                and status.num_pieces == info.num_pieces():
            break
        time.sleep(0.05)
    test.assertEqual(handle.status().num_pieces, info.num_pieces())
    test.assertTrue(handle.status().is_seeding)


if __name__ == '__main__':
    unittest.main()
