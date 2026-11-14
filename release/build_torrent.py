#!/usr/bin/env python3
"""The BitTorrent torrent of a revision's whole game folder: release/build-torrent's work (run
that, not this; srp-026g/docs/notes/torrent-downloads.md has the format).

    python3 -I build_torrent.py --out <skhid-rev<n>.torrent> --zip <players' ZIP>
        --zip-out <the ZIP with it> --revision <n> --key <private.pem> --public-key <signing.pub>
        [--openssl <program>] [--file <path in the folder> <source file> <its sha256>]...

The folder (the torrent's name, NAME) holds every --file at its path, then every file of the ZIP
at its path, which replaces a --file of the same path (the Skhid files over the original game's).
A --file is read once: its SHA-256 must be the one given, or nothing is written. --zip-out is the
ZIP with the torrent added as ENTRY (stored, not compressed: the web server reads it from there),
which the torrent itself leaves out: it cannot hold its own hash. A ZIP that has ENTRY already is
refused.

Hybrid v1 + v2 (BEP 52) at PIECE bytes a piece: every file starts a piece (a v1 pad file after
each one but the last), so a file's v2 hashes are its own, whatever is around it. The same files
make the same bytes: no date, no creator, keys and files in byte order (Ed25519 signatures are
deterministic too).

Signed: the top-level `skhid` dictionary (outside info, so the info hashes are not changed by it)
holds `revision` and `signature`, the Ed25519 signature by --key of signed(): DOMAIN, the revision
as one byte, the SHA-256 of the bencoded info. It must verify with --public-key (the committed
release/signing.pub), or nothing is written.
"""
import argparse
import base64
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile

NAME = 'Survarium'
# What the release's signature is of starts with this (then the revision, then SHA-256 of info).
DOMAIN = b'skhid torrent signature 1\0'
ENTRY = 'game/binaries/x86/skhid.torrent'
# The bootstrap torrent, in the ZIP too (--bootstrap): the updater seeds it (from copies it keeps of
# its two files) while it shares. Left out of the revision's torrent, as ENTRY.
BOOTSTRAP_ENTRY = 'game/binaries/x86/skhid-bootstrap.torrent'
PIECE = 4 << 20
BLOCK = 16 << 10
TRACKERS = (
    'udp://tracker.opentrackr.org:1337/announce',
    'http://tracker.opentrackr.org:1337/announce',
    'udp://open.stealth.si:80/announce',
    'udp://tracker.torrent.eu.org:451/announce',
    'udp://exodus.desync.com:6969/announce',
)
ZERO = bytes(32)


def bencode(value):
    if isinstance(value, int):
        return b'i%de' % value
    if isinstance(value, str):
        value = value.encode()
    if isinstance(value, bytes):
        return b'%d:%s' % (len(value), value)
    if isinstance(value, list):
        return b'l' + b''.join(map(bencode, value)) + b'e'
    if isinstance(value, dict):
        keys = sorted((k.encode() if isinstance(k, str) else k, v) for k, v in value.items())
        return b'd' + b''.join(bencode(k) + bencode(v) for k, v in keys) + b'e'
    raise TypeError(value)


def merkle(hashes, width, pad):
    """The root of `hashes` filled to `width` (a power of two) with `pad`."""
    layer = hashes + [pad] * (width - len(hashes))
    while len(layer) > 1:
        layer = [hashlib.sha256(layer[i] + layer[i + 1]).digest() for i in range(0, len(layer), 2)]
    return layer[0]


def pow2(n):
    return 1 << (n - 1).bit_length()


# The root of a piece of zero leaves: what fills a file's piece layer to a power of two.
PIECE_PAD = merkle([], PIECE // BLOCK, ZERO)


def hash_file(read, padded):
    """(size, v1 piece SHA-1s, v2 piece layer, v2 root) of `read`'s bytes; `padded`: its last v1
    piece is filled with zeros (the pad file after it)."""
    size, v1, layer, leaves = 0, [], [], []
    while True:
        piece = read(PIECE)
        if not piece:
            break
        size += len(piece)
        v1.append(hashlib.sha1(piece + bytes(PIECE - len(piece)) if padded else piece).digest())
        blocks = [hashlib.sha256(piece[i:i + BLOCK]).digest() for i in range(0, len(piece), BLOCK)]
        leaves = blocks
        layer.append(merkle(blocks, PIECE // BLOCK, ZERO))
        if len(piece) < PIECE:
            break
    if size == 0:
        sys.exit('an empty file cannot be in the torrent')
    if size <= PIECE:
        root = merkle(leaves, pow2(len(leaves)), ZERO)
    else:
        root = merkle(layer, pow2(len(layer)), PIECE_PAD)
    return size, v1, layer, root


def build(files, sign):
    """The torrent's bytes and its info's; `files` maps a path (`a/b/c`) to a function that opens
    its bytes, `sign` gives the `skhid` dictionary of the bencoded info."""
    tree, v1_files, pieces, layers = {}, [], [], {}
    order = sorted(files, key=lambda path: [part.encode() for part in path.split('/')])
    for index, path in enumerate(order):
        last = index == len(order) - 1
        with files[path]() as stream:
            size, v1, layer, root = hash_file(stream.read, not last)
        pieces += v1
        node = tree
        for part in path.split('/'):
            node = node.setdefault(part, {})
        node[''] = {'length': size, 'pieces root': root}
        if size > PIECE:
            layers[root] = b''.join(layer)
        v1_files.append({'length': size, 'path': path.split('/')})
        pad = -size % PIECE
        if pad and not last:
            v1_files.append({'attr': 'p', 'length': pad, 'path': ['.pad', str(pad)]})
    info = {'name': NAME, 'piece length': PIECE, 'meta version': 2, 'file tree': tree,
            'files': v1_files, 'pieces': b''.join(pieces)}
    raw = bencode(info)
    torrent = {'announce': TRACKERS[0], 'announce-list': [[t] for t in TRACKERS], 'info': info,
               'piece layers': layers}
    if sign:
        torrent['skhid'] = sign(raw)
    return bencode(torrent), raw


def decode(data, at=0):
    """(value, end) of the bencoded value at `at` (of our own files: no limits); keys as bytes."""
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


def info_bytes(data):
    """The bencoded info of a torrent file, as it has it."""
    start = data.index(b'4:infod') + 6
    return data[start:decode(data, start)[1]]


# The bootstrap (srp-026g/docs/notes/torrent-downloads.md, "Bootstrap").
BOOTSTRAP_HEAD = r"""@echo off
rem Skhid-get.bat: gets Skhid (Survarium 0.26g) into a Survarium folder beside
rem this file. Plain text: read it. It fetches the Skhid updater and 7zr.exe
rem by BitTorrent from anyone who shares them (the public trackers below),
rem else from the addresses below, checks every piece and file against the
rem hashes below, then runs the updater, which installs the rest, and starts
rem Skhid.bat. No administrator rights. Made by release/build-bootstrap.
set "SKHID_GET_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t = [IO.File]::ReadAllText($env:SKHID_GET_SCRIPT); iex $t.Substring($t.IndexOf('<#' + 'PS#>'))"
if errorlevel 1 pause
exit /b
<#PS#>
"""


def quoted(text):
    return "'" + text.replace("'", "''") + "'"


def bootstrap_script(torrent, rows, body, trackers):
    """Skhid-get.bat: the pins of `torrent` (the bootstrap torrent's bytes) and `rows` (name:
    (sha256, size, urls) of release/bootstrap.tsv, by the file's name), then `body`."""
    meta, _ = decode(torrent)
    info = meta[b'info']
    pieces, at, files = info[b'pieces'], 0, []
    for entry in info[b'files']:
        if entry.get(b'attr') == b'p':
            continue
        path = '/'.join(part.decode() for part in entry[b'path'])
        size = entry[b'length']
        count = -(-size // info[b'piece length'])
        name = path.rsplit('/', 1)[-1]
        if name not in rows:
            sys.exit(f'{name}: no row in the bootstrap list')
        sha256, row_size, urls = rows[name]
        if row_size != size:
            sys.exit(f'{name}: {row_size} bytes in the list, {size} in the torrent')
        hashes = [pieces[20 * i:20 * i + 20].hex() for i in range(at, at + count)]
        at += count
        files.append(f"    @{{ Path = {quoted(path)}; Size = {size}; Sha256 = {quoted(sha256)};\n"
                     f"        Pieces = @({', '.join(map(quoted, hashes))});\n"
                     f"        Urls = @({', '.join(map(quoted, urls))}) }}")
    pins = (f"$InfoHash = {quoted(hashlib.sha1(info_bytes(torrent)).hexdigest())}\n"
            f"$PieceLength = {info[b'piece length']}\n"
            f"$Trackers = @({', '.join(map(quoted, trackers))})\n"
            "$Files = @(\n" + ",\n".join(files) + "\n)\n")
    return (BOOTSTRAP_HEAD + pins + body).replace('\r\n', '\n').replace('\n', '\r\n').encode()


def bootstrap(argv):
    parser = argparse.ArgumentParser(prog='build_torrent.py bootstrap')
    parser.add_argument('--torrent', required=True, help='the bootstrap torrent: made with --file, else read')
    parser.add_argument('--file', nargs=2, action='append', default=[], metavar=('PATH', 'SOURCE'))
    parser.add_argument('--list', required=True, help='release/bootstrap.tsv')
    parser.add_argument('--body', required=True, help='release/bootstrap.ps1')
    parser.add_argument('--out', required=True, help='Skhid-get.bat')
    parser.add_argument('--tracker', action='append', help='another tracker than the UDP ones of TRACKERS')
    args = parser.parse_args(argv)
    rows = {}
    with open(args.list) as listed:
        for line in listed:
            if line.strip() and not line.startswith('#'):
                name, sha256, size, urls = line.rstrip('\r\n').split('\t')
                rows[name] = (sha256, int(size), urls.split(' '))
    if args.file:
        for path, source in args.file:
            name = path.rsplit('/', 1)[-1]
            with open(source, 'rb') as each:
                if name not in rows or hashlib.sha256(each.read()).hexdigest() != rows[name][0]:
                    sys.exit(f'{source}: not the file of the row {name} of {args.list}')
        torrent, _ = build({path: (lambda source=source: open(source, 'rb')) for path, source in args.file}, None)
        with open(args.torrent, 'wb') as out:
            out.write(torrent)
    with open(args.torrent, 'rb') as each:
        torrent = each.read()
    with open(args.body) as each:
        body = each.read()
    trackers = args.tracker or [t for t in TRACKERS if t.startswith('udp://')]
    script = bootstrap_script(torrent, rows, body, trackers)
    with open(args.out, 'wb') as out:
        out.write(script)
    print(f'{args.out}: {len(script)} bytes; the bootstrap torrent btih {hashlib.sha1(info_bytes(torrent)).hexdigest()}')


def signed(revision, info):
    """What the release's Ed25519 key signs: DOMAIN, the revision (one byte), SHA-256 of info."""
    return DOMAIN + bytes([revision]) + hashlib.sha256(info).digest()


# The public key's DER (RFC 8410) before its 32 bytes, for openssl.
ED25519_PUBLIC = bytes.fromhex('302a300506032b6570032100')


def signer(openssl, key, public_hex, revision, scratch):
    """`sign` of build: the release key's signature, checked against the committed public key."""
    public = bytes.fromhex(public_hex.strip())
    if len(public) != 32:
        sys.exit('the public key is not 64 hex digits (32 bytes)')
    pem = os.path.join(scratch, 'public.pem')
    with open(pem, 'w') as out:
        out.write('-----BEGIN PUBLIC KEY-----\n' + base64.b64encode(ED25519_PUBLIC + public).decode()
                  + '\n-----END PUBLIC KEY-----\n')

    def sign(info):
        message, sig = os.path.join(scratch, 'message'), os.path.join(scratch, 'signature')
        with open(message, 'wb') as out:
            out.write(signed(revision, info))
        subprocess.run([openssl, 'pkeyutl', '-sign', '-rawin', '-inkey', key, '-in', message, '-out', sig],
                       check=True, stdin=subprocess.DEVNULL)
        if subprocess.run([openssl, 'pkeyutl', '-verify', '-rawin', '-pubin', '-inkey', pem, '-in', message,
                           '-sigfile', sig], stdin=subprocess.DEVNULL, capture_output=True).returncode:
            sys.exit(f'the signature of {key} does not verify with the public key given: another key')
        with open(sig, 'rb') as signature:
            value = signature.read()
        if len(value) != 64:
            sys.exit(f'openssl gave a signature of {len(value)} bytes, not 64')
        return {'revision': revision, 'signature': value}
    return sign


def safe(path):
    parts = path.split('/')
    return all(p and p not in ('.', '..') and '\\' not in p and ':' not in p for p in parts)


class Checked:
    """A --file, read through: its SHA-256 must be `sha256` when it has been read whole."""

    def __init__(self, path, source, sha256):
        self.path, self.source, self.sha256 = path, source, sha256

    def __call__(self):
        return self

    def __enter__(self):
        self.file, self.hash = open(self.source, 'rb'), hashlib.sha256()
        return self

    def read(self, n):
        data = self.file.read(n)
        self.hash.update(data)
        return data

    def __exit__(self, kind, *_):
        self.file.close()
        if kind is None and self.hash.hexdigest() != self.sha256:
            sys.exit(f'{self.source} ({self.path}): sha256 {self.hash.hexdigest()}, not {self.sha256}')


def main():
    if sys.argv[1:2] == ['bootstrap']:
        return bootstrap(sys.argv[2:])
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--out', required=True)
    parser.add_argument('--zip', required=True)
    parser.add_argument('--zip-out', required=True)
    parser.add_argument('--revision', required=True, type=int)
    parser.add_argument('--key', required=True, help="the release's Ed25519 private key (PEM)")
    parser.add_argument('--public-key', required=True, help='its public key: 64 hex digits')
    parser.add_argument('--openssl', default='openssl')
    parser.add_argument('--file', nargs=3, action='append', default=[], metavar=('PATH', 'SOURCE', 'SHA256'))
    parser.add_argument('--bootstrap', help='release/bootstrap.torrent, added to the ZIP as BOOTSTRAP_ENTRY')
    args = parser.parse_args()
    if not 1 <= args.revision <= 255:
        sys.exit(f'--revision {args.revision}: a revision is 1 to 255')
    for need in (args.key, args.public_key):
        if not os.path.isfile(need):
            sys.exit(f'{need}: no such file (the release signs every torrent: release/make-signing-key)')
    for out in (args.out, args.zip_out):
        if os.path.lexists(out):
            sys.exit(f'{out}: already there; never overwritten')
    files = {}
    for path, source, sha256 in args.file:
        if not safe(path) or path in files:
            sys.exit(f'--file {path}: not a path of the folder, or given twice')
        files[path] = Checked(path, source, sha256.lower())
    with zipfile.ZipFile(args.zip) as archive:
        for entry in archive.infolist():
            if entry.is_dir():
                continue
            if not safe(entry.filename) or entry.filename in (ENTRY, BOOTSTRAP_ENTRY):
                sys.exit(f'{args.zip}: {entry.filename} is not a path of the folder, or the torrent')
            if entry.filename in files:
                print(f"  {entry.filename}: the ZIP's, over the original")
            files[entry.filename] = lambda entry=entry: archive.open(entry)
        with tempfile.TemporaryDirectory() as scratch:
            with open(args.public_key) as public:
                sign = signer(args.openssl, args.key, public.read(), args.revision, scratch)
            torrent, info = build(files, sign)
    with open(args.out + '.part', 'xb') as out:
        out.write(torrent)
    # The ZIP as it was, the torrent appended: its entry as make_client_026g.py makes them.
    shutil.copyfile(args.zip, args.zip_out + '.part')
    with zipfile.ZipFile(args.zip_out + '.part', 'a') as archive:
        entry = zipfile.ZipInfo(ENTRY)
        entry.create_system = 3
        entry.external_attr = 0o100644 << 16
        entry.compress_type = zipfile.ZIP_STORED
        archive.writestr(entry, torrent)
        if args.bootstrap:
            with open(args.bootstrap, 'rb') as bootstrap:
                data = bootstrap.read()
            entry = zipfile.ZipInfo(BOOTSTRAP_ENTRY)
            entry.create_system = 3
            entry.external_attr = 0o100644 << 16
            entry.compress_type = zipfile.ZIP_STORED
            archive.writestr(entry, data)
    os.rename(args.out + '.part', args.out)
    os.rename(args.zip_out + '.part', args.zip_out)
    btmh = hashlib.sha256(info).hexdigest()
    print(f'{args.out}: {len(files)} files, {len(torrent)} bytes, piece {PIECE}')
    print(f'btih {hashlib.sha1(info).hexdigest()}')
    print(f'btmh 1220{btmh}')
    print(f'signed for revision {args.revision}')


if __name__ == '__main__':
    main()
