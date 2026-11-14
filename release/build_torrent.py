#!/usr/bin/env python3
"""The BitTorrent torrent of a revision's whole game folder: release/build-torrent's work (run
that, not this; srp-026g/docs/notes/torrent-downloads.md has the format).

    python3 -I build_torrent.py --out <skhid-rev<n>.torrent> --zip <players' ZIP>
        [--file <path in the folder> <source file> <its sha256>]...

The folder (the torrent's name, NAME) holds every --file at its path, then every file of the ZIP
at its path, which replaces a --file of the same path (the Skhid files over the original game's).
A --file is read once: its SHA-256 must be the one given, or nothing is written.

Hybrid v1 + v2 (BEP 52) at PIECE bytes a piece: every file starts a piece (a v1 pad file after
each one but the last), so a file's v2 hashes are its own, whatever is around it. The same files
make the same bytes: no date, no creator, keys and files in byte order.
"""
import argparse
import hashlib
import os
import sys
import zipfile

NAME = 'Survarium'
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


def build(files):
    """The torrent's bytes; `files` maps a path (`a/b/c`) to a function that opens its bytes."""
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
    torrent = {'announce': TRACKERS[0], 'announce-list': [[t] for t in TRACKERS], 'info': info,
               'piece layers': layers}
    return bencode(torrent), bencode(info)


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
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--out', required=True)
    parser.add_argument('--zip', required=True)
    parser.add_argument('--file', nargs=3, action='append', default=[], metavar=('PATH', 'SOURCE', 'SHA256'))
    args = parser.parse_args()
    if os.path.lexists(args.out):
        sys.exit(f'{args.out}: already there; never overwritten')
    files = {}
    for path, source, sha256 in args.file:
        if not safe(path) or path in files:
            sys.exit(f'--file {path}: not a path of the folder, or given twice')
        files[path] = Checked(path, source, sha256.lower())
    with zipfile.ZipFile(args.zip) as archive:
        for entry in archive.infolist():
            if entry.is_dir():
                continue
            if not safe(entry.filename):
                sys.exit(f'{args.zip}: {entry.filename} is not a path of the folder')
            if entry.filename in files:
                print(f"  {entry.filename}: the ZIP's, over the original")
            files[entry.filename] = lambda entry=entry: archive.open(entry)
        torrent, info = build(files)
    with open(args.out + '.part', 'xb') as out:
        out.write(torrent)
    os.rename(args.out + '.part', args.out)
    btmh = hashlib.sha256(info).hexdigest()
    print(f'{args.out}: {len(files)} files, {len(torrent)} bytes, piece {PIECE}')
    print(f'btih {hashlib.sha1(info).hexdigest()}')
    print(f'btmh 1220{btmh}')


if __name__ == '__main__':
    main()
