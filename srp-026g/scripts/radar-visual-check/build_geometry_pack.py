#!/usr/bin/env python3
"""Build the Radar render-seam content pack from pinned published source meshes.

This creates a content pack for release/build-content, never a resources.db patch.
Collision, indices, vertex bases and UVs retain their source bytes.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def repair(raw, spec):
    if sha(raw) != spec['sha256']:
        raise ValueError(f"{spec['file']}: source mesh differs from reviewed input")
    kind, size, count = struct.unpack_from('<III', raw)
    if kind != 3 or size != 4 + 28 * count:
        raise ValueError('expected native 28-byte static vertices')
    out = bytearray(raw)
    seen = set()
    for move in spec['moves']:
        index = move['index']
        if index in seen or not 0 <= index < count:
            raise ValueError('duplicate or out-of-range vertex')
        seen.add(index)
        at = 12 + 28 * index
        if raw[at:at + 12] != struct.pack('<3f', *move['from']):
            raise ValueError(f'vertex {index}: unexpected source position')
        out[at:at + 12] = struct.pack('<3f', *move['to'])
    # Retain the original model bounds: neither seam repair reaches an extremum.
    before = [struct.unpack_from('<3f', raw, 12 + 28 * i) for i in range(count)]
    after = [struct.unpack_from('<3f', out, 12 + 28 * i) for i in range(count)]
    bounds = lambda points: [(min(p[a] for p in points), max(p[a] for p in points)) for a in range(3)]
    if bounds(before) != bounds(after):
        raise ValueError('repair requires an explicit bounding-box update')
    if out[8 + size:] != raw[8 + size:]:
        raise ValueError('index/chunk data changed')
    for i in range(count):
        at = 12 + 28 * i
        if out[at + 12:at + 28] != raw[at + 12:at + 28]:
            raise ValueError('vertex basis or UV changed')
    return bytes(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--porter', type=Path, required=True)
    parser.add_argument('--patch22', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    recipe = json.loads((HERE / 'geometry.json').read_text())
    made, operations = {}, []
    with tempfile.TemporaryDirectory(prefix='radar-geometry-') as temp:
        for number, spec in enumerate(recipe['meshes']):
            source = Path(temp) / str(number)
            subprocess.run([str(args.porter), 'extract', '--db', str(args.patch22),
                            '--path', spec['file'], '--output', str(source)],
                           check=True, capture_output=True)
            raw = source.read_bytes()
            made[spec['file']] = repair(raw, spec)
            operations.append({'op': 'replace_file', 'path': spec['file'], 'base_sha256': sha(raw)})
    manifest = {'format_version': 1, 'id': 'radar-v011e-seams', 'version': '1',
                'target_build': '0.26g', 'operations': operations,
                'notes': [recipe['why'], 'Render positions only; collision and scene placement are unchanged.']}
    args.out.mkdir(parents=True, exist_ok=False)
    for path, raw in made.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    raw = (json.dumps(manifest, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(raw)
    print(f"radar-v011e-seams@1: {len(made)} meshes; pack.json {sha(raw)}")


if __name__ == '__main__':
    main()
