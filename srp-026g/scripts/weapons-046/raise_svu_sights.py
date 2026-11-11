#!/usr/bin/env python3
"""Version weapons-046 with the SVU's iron sights raised, its eye on them.

usage: raise_svu_sights.py --packs <packs folder> [--out <packs folder>]

0.46c2's SVU always carried a PSO-1 (its default scope), and its mesh has the iron sights folded
flat: the rear diopter on the receiver, the front sight on the muzzle brake. Aimed without the
scope the eye looked over them. 0.69d0 ships the same two sights as modules, folded ("off", the
vertices of 0.46c2's mesh exactly, at its locators ironsight_rear_point and ironsight_front_point)
and raised ("on": the same vertices turned a quarter about x round that locator), and puts the aim
locator 0.0994 m high, on the raised sights. This turns weapons-046@12's folded sights up the same
way (positions and their packed basis words), grows the boxes over them and puts the eye there.
Every other file is @12's.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content-edit'))
from content_edit import find, rec, set_vector  # noqa: E402

FROM, TO = '12', '13'
FROM_SHA256 = 'e98d272d4c4f934ad80d79cde14ca2b86ca770ddd592d3c1b57f484ff614ac89'
MODEL = 'files/models/weapons/sniper/svu_a/svu_a.skinned_model/render'
VERTEX = 36  # 0.26g skinned vertex: float3 position, u32 bone, three packed basis words, float2 UV
OFFSET_BONE = 1
# The rig's frame (the one skinned vertices are kept in): 0.69d0's locators turned a half about y,
# over the bone `offset`'s origin (0). Boxes: the folded sight's vertices, 0.5 mm wider.
SIGHTS = [
    {'surface': 'svu_a_handguard_s', 'hinge': (0.0, 0.060147904, -0.050372038),
     'box': ((-0.0094, 0.0514, -0.0586), (0.0094, 0.0702, -0.0031)), 'vertices': 393},
    {'surface': 'muzzle_svu_a_handguard_s', 'hinge': (0.0, 0.04796514, 0.42640373),
     'box': ((-0.0115, 0.0375, 0.4153), (0.0115, 0.0582, 0.4893)), 'vertices': 551},
]
AIM = {'from': (-0.0000025749, 0.093499035, 0.23069364), 'to': (-0.0000025749, 0.09936328, 0.23069364)}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raise_sight(data, sight):
    """The surface with the sight's vertices turned up: (x, y, z) -> (x, z, -y) about the hinge;
    the box of the turned vertices."""
    kind, size, n = struct.unpack_from('<3I', data)
    assert kind == 3 and size == 4 + n * VERTEX, 'not a 0.26g skinned surface'
    vertices = np.frombuffer(data, np.uint8, n * VERTEX, 12).reshape(n, VERTEX).copy()
    at = 8 + size
    kind, _, count = struct.unpack_from('<3I', data, at)
    assert kind == 4
    triangles = np.frombuffer(data, '<u2', count, at + 12).reshape(-1, 3)
    p = vertices[:, :12].copy().view('<f4').reshape(n, 3).astype(float)
    lo, hi = np.array(sight['box'])
    inside = np.all((p >= lo) & (p <= hi), axis=1)
    # Whole pieces only: a triangle's vertices are all the sight's or all not.
    parent = np.arange(n)

    def root(v):
        while parent[v] != v:
            parent[v] = parent[parent[v]]
            v = parent[v]
        return v
    for a, b, c in triangles:
        parent[root(b)] = root(a)
        parent[root(c)] = root(a)
    roots = np.array([root(v) for v in range(n)])
    outside = set(roots[~inside])
    chosen = np.array([r not in outside for r in roots])
    assert chosen.sum() == sight['vertices'], f"{sight['surface']}: {chosen.sum()} vertices in the box"
    assert (vertices[chosen, 12:16].copy().view('<u4') == OFFSET_BONE).all(), 'a sight vertex off the offset bone'
    hinge = np.array(sight['hinge'])
    q = p[chosen] - hinge
    turned = np.column_stack([q[:, 0], q[:, 2], -q[:, 1]]) + hinge
    vertices[chosen, :12] = turned.astype('<f4').view(np.uint8).reshape(-1, 12)
    for word in (16, 20, 24):  # bytes x, y, z, w; 127 is 0 (-b is 254 - b, as the porter's basis_to_rig)
        y = vertices[chosen, word + 1].astype(int)
        vertices[chosen, word + 1] = vertices[chosen, word + 2]
        vertices[chosen, word + 2] = np.clip(254 - y, 0, 254)
    out = bytearray(data)
    out[12:12 + n * VERTEX] = vertices.tobytes()
    return bytes(out), turned.min(0), turned.max(0)


def grow_box(buf, path, lo, hi):
    """The float3 pair at `path/min`, `path/max` over the box lo..hi too."""
    for end, pick in (('min', np.minimum), ('max', np.maximum)):
        key = f'{path}/{end}'
        old = struct.unpack_from('<3f', buf, rec(buf, find(buf, key))[0])
        new = pick(old, lo if end == 'min' else hi)
        set_vector(buf, key, old, [float(v) for v in new])


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--packs', type=Path, required=True)
    ap.add_argument('--out', type=Path)
    args = ap.parse_args()
    src = args.packs / f'weapons-046-{FROM}'
    dst = (args.out or args.packs) / f'weapons-046-{TO}'
    assert digest(src / 'pack.json') == FROM_SHA256, f'{src}: unexpected pack'
    assert not dst.exists(), f'{dst}: refusing to replace a pack'
    shutil.copytree(src, dst)
    model = bytearray((dst / MODEL / 'export_properties').read_bytes())
    for sight in SIGHTS:
        mesh = dst / MODEL / sight['surface'] / 'converted_model'
        data, lo, hi = raise_sight(mesh.read_bytes(), sight)
        mesh.write_bytes(data)
        props = dst / MODEL / sight['surface'] / 'export_properties'
        buf = bytearray(props.read_bytes())
        grow_box(buf, 'bounding_box', lo, hi)
        props.write_bytes(buf)
        grow_box(model, 'bounding_box', lo, hi)
    set_vector(model, 'locators[0]/position', AIM['from'], AIM['to'])
    (dst / MODEL / 'export_properties').write_bytes(model)
    pack = json.loads((src / 'pack.json').read_text())
    pack['version'] = TO
    pack['description'] += "; the SVU's iron sights raised, its eye on them"
    pack['notes'].append(
        "The SVU's iron sights: srp-026g/scripts/weapons-046/raise_svu_sights.py. 0.46c2's mesh has "
        "them folded (it always carried a PSO-1); turned up as 0.69d0's ironsight modules are, the "
        "aim locator 0.0994 m high (0.69d0's). No other content changes.")
    (dst / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    changed = sorted(str(p.relative_to(dst)) for p in dst.rglob('*')
                     if p.is_file() and digest(p) != digest(src / p.relative_to(dst)))
    expected = sorted(['pack.json', f'{MODEL}/export_properties']
                      + [f"{MODEL}/{s['surface']}/{f}" for s in SIGHTS for f in ('converted_model', 'export_properties')])
    assert changed == expected, changed
    print(f"weapons-046@{TO}\t{digest(dst / 'pack.json')}\t{len(changed) - 1} SVU files; other files identical")


if __name__ == '__main__':
    main()
