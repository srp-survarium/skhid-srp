"""The original linear blend in the native parallax blend programs.

Each of the 20 programs must be composed unchanged and differ from stock by one more
temporary and the four-instruction guard after the native weight (material constants in
cb1): a material whose strength is the original-linear selector blends by its painted
vertex alpha. No masked material may use parallax: these programs carry no masked branch.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from check_vertex_blend import chunks, instructions
from check_material_visibility import read as leaves

MARKER = struct.unpack('<I', struct.pack('<f', -21021))[0]
PREFIX = 'material.g_stage.effect.'


def check(porter, stock, patch, pack):
    manifest = json.loads((pack / 'pack.json').read_text())
    assert manifest['id'] == 'v021d-vertex-blend-parallax'
    with tempfile.TemporaryDirectory() as tmp:
        def read(db, path):
            out = Path(tmp) / 'shader'
            subprocess.run([str(porter), 'extract', '--db', str(db), '--path', path, '--output', str(out)],
                           check=True, stdout=subprocess.DEVNULL)
            return out.read_bytes()
        for op in manifest['operations']:
            path = op['path']
            assert op['op'] == 'replace_file' and path.rsplit('/', 1)[1][10] == '1', path
            native, added = read(stock, path), (pack / 'files' / path).read_bytes()
            assert hashlib.sha256(native).hexdigest() == op['base_sha256'], path
            assert read(patch, path) == added, path
            (old_header, old), (new_header, new) = chunks(native), chunks(added)
            assert old_header == new_header and old.keys() == new.keys(), path
            assert all(old[tag] == new[tag] for tag in old if tag != b'SHDR'), path
            (version, before), (same, after) = instructions(old[b'SHDR']), instructions(new[b'SHDR'])
            assert version == same, path
            weight = [(i, o) for i, o in enumerate(before) if o[0] == 0x08002038 and o[-3:] == [0x0020801a, 1, 14]]
            assert len(weight) == 1, path
            at, weight_op = weight[0]
            logs = [o for o in before[at - 5:at] if o[0] & 2047 == 47]
            assert len(logs) == 1 and logs[0][3:5] == [0x8010100a, 0x81], path
            temps = [(i, o) for i, o in enumerate(before) if o[0] & 2047 == 104]
            assert len(temps) == 1, path
            temps_at, scratch = temps[0][0], temps[0][1][1]
            assert after[at + 1:at + 5] == [
                [0x08000018, 0x00100012, scratch, 0x0020801a, 1, 14, 0x00004001, MARKER],
                [0x0304001f, 0x0010000a, scratch],
                [0x05000036, *weight_op[1:3], 0x0010100a, logs[0][5]],
                [0x01000015]], (path, 'guard differs')
            del after[at + 1:at + 5]
            assert after[temps_at] == [0x02000068, scratch + 1], path
            after[temps_at] = before[temps_at]
            assert before == after, (path, 'native instructions changed')
    source = json.loads((Path(__file__).resolve().parent / 'vertex-blend-inputs.json').read_text())
    paths = set(subprocess.check_output([str(porter), 'list', '--db', str(patch)], text=True).splitlines())
    for row in source['materials']:
        material = f"material_instances/{row['target_material']}.material"
        if row['mode'] == 'masked' and material in paths:
            assert leaves(porter, patch, material).get(PREFIX + 'use_parallax.value') is not True, (material, 'masked and parallax')
    print('Parallax blend: 20 programs, each the stock program with the original-linear guard; no masked parallax material')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('porter', 'stock', 'patch', 'pack'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    check(args.porter, args.stock, args.patch, args.pack)
