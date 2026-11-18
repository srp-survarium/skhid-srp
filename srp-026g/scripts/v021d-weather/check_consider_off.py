"""The absent alpha-tested blend programs whose cutoff ignores the blend.

Each added path must be absent from stock, be composed unchanged, and equal the
consider-on program of the same key in the blend pack except for one instruction:
the blend-aware alpha `mad a, w, a, 1` becomes the base alpha `add a, a, 1`.
"""
import argparse
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile

from check_vertex_blend import chunks, instructions

KEY = re.compile(r'^(_[03]___[01]{3}1_.*_[01]1[01])0([01]____[01]{5}____[012]{3}__________[01]__)$')
ONE = struct.unpack('<I', struct.pack('<f', 1.0))[0]


def check(porter, stock, patch, pack, blend_pack):
    manifest = json.loads((pack / 'pack.json').read_text())
    paths = [op['path'] for op in manifest['operations']]
    assert manifest['id'] == 'v021d-vertex-blend-consider-off' and len(set(paths)) == len(paths) == 12
    assert all(op == {'op': 'add_file', 'path': op['path']} for op in manifest['operations'])
    listed = subprocess.run([str(porter), 'list', '--db', str(stock), '--prefix', 'shaders/sm_4_0/gbuffer_pass.ps/'],
                            check=True, capture_output=True, text=True).stdout.split()
    assert not set(paths) & set(listed), 'a stock program would be replaced'
    with tempfile.TemporaryDirectory() as tmp:
        for path in paths:
            folder, key = path.rsplit('/', 1)
            match = KEY.fullmatch(key)
            assert match, path
            source = (blend_pack / 'files' / folder / (match[1] + '1' + match[2])).read_bytes()
            added = (pack / 'files' / path).read_bytes()
            composed = Path(tmp) / 'shader'
            subprocess.run([str(porter), 'extract', '--db', str(patch), '--path', path, '--output', str(composed)],
                           check=True, stdout=subprocess.DEVNULL)
            assert composed.read_bytes() == added, path
            (old_header, old), (new_header, new) = chunks(source), chunks(added)
            assert old_header == new_header and old.keys() == new.keys(), path
            assert all(old[tag] == new[tag] for tag in old if tag != b'SHDR'), path
            (version, before), (same, after) = instructions(old[b'SHDR']), instructions(new[b'SHDR'])
            assert version == same and len(before) == len(after), path
            changed = [i for i, (a, b) in enumerate(zip(before, after)) if a != b]
            assert len(changed) == 1, path
            mix, base = before[changed[0]], after[changed[0]]
            target = mix[1:3]
            alpha = [0x0010000a | (((target[0] >> 4) & 15).bit_length() - 1) << 4, target[1]]
            assert mix[0] == 0x09000032 and mix[5:7] == alpha and mix[7:] == [0x00004001, ONE], path
            assert base == [0x07000000, *target, *alpha, 0x00004001, ONE], path
            assert after[changed[0] + 3][0] & 2047 == 13, path
    print(f'Consider-off closure: {len(paths)} added programs, each one instruction from its blend-aware source')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'stock', 'patch', 'pack', 'blend-pack'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    check(args.porter, args.stock, args.patch, args.pack, args.blend_pack)
