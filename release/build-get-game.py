#!/usr/bin/env python3
"""Build a standalone, revision-pinned Windows full-game torrent downloader."""
import argparse
import base64
import hashlib
from pathlib import Path
import build_torrent


def build(torrent, revision):
    meta, end = build_torrent.decode(torrent)
    if end != len(torrent) or meta.get(b'skhid', {}).get(b'revision') != revision:
        raise ValueError('torrent must be the released revision named by --revision')
    here = Path(__file__).resolve().parent
    sources = (here / 'get-game.cmd').read_text() + (here / 'get-game.ps1').read_text()
    trackers = [url.decode('ascii') for tier in meta[b'announce-list'] for url in tier]
    display = '\n'.join("    Write-Host '  " + url.replace("'", "''") + "'" for url in trackers)
    values = {
        '@@REVISION@@': str(revision),
        '@@TORRENT_BASE64@@': base64.encodebytes(torrent).decode().rstrip(),
        '@@TORRENT_SHA256@@': hashlib.sha256(torrent).hexdigest(),
        '@@TRACKER_DISPLAY@@': display,
        '@@DIAGNOSTICS@@': (here / 'get-game-diagnostics.ps1').read_text().rstrip(),
    }
    for key, value in values.items():
        sources = sources.replace(key, value)
    return sources.replace('\n', '\r\n').encode('ascii')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--torrent', type=Path, required=True)
    p.add_argument('--revision', type=int, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    args.out.write_bytes(build(args.torrent.read_bytes(), args.revision))
    print(args.out)
