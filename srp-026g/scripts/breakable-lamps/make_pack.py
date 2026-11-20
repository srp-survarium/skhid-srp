#!/usr/bin/env python3
"""Build breakable-lamps@2 from stock lamp meshes, for release/content.tsv.

--stock contains the models' files at their archive paths (srp-game-data get;
every path of stock.json). --out is a new resource-porter pack directory.
Never builds a release patch.

Each broken model is its stock model without its glass: the other triangles
keep their vertices, normals, UVs and material, every LOD surface the same
way. Bounds and vertex/triangle counts are recomputed and model collision is
disabled (the intact model's stays in the map).
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct

HELPER = Path(__file__).resolve().parents[1] / 'lobby-bulb/make_pack.py'
spec = importlib.util.spec_from_file_location('bulb_mesh', HELPER)
mesh_tools = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mesh_tools)
parse, serialize, put = mesh_tools.parse, mesh_tools.serialize, mesh_tools.put


def below(y):
    """The bulb under its socket: every triangle with a corner below y."""
    def glass(points, uvs, triangles):
        return {k for k, tri in enumerate(triangles) if any(points[i][1] < y for i in tri)}
    return glass


def no_glass(points, uvs, triangles):
    """The unlit plafon_lamp is metal only (cap, rings, cage): its glass dome
    is a model of its own, plafon_lamp_on, and v0.11's plafon_lamp is this
    model plus that dome. Its broken model is all of it."""
    return set()


def parts_textured(u0, v0, u1, v1):
    """Fluorescent tubes: each tube (or broken tube end) is a connected part
    of its own, textured from a tube region of the lamp_school atlas: a part
    with a corner in [u0, u1] x [v0, v1] is glass, all of it."""
    def glass(points, uvs, triangles):
        parent = {}

        def root(x):
            while parent.setdefault(x, x) != x:
                parent[x] = parent[parent[x]]
                x = parent[x]
            return x
        weld = [tuple(round(c, 4) for c in p) for p in points]   # UV seams split vertices
        for tri in triangles:
            for i in tri[1:]:
                parent[root(weld[i])] = root(weld[tri[0]])
        tubes = {root(weld[i]) for i, (u, v) in enumerate(uvs) if u0 <= u <= u1 and v0 <= v <= v1}
        return {k for k, tri in enumerate(triangles) if root(weld[tri[0]]) in tubes}
    return glass


# lamp: the glass starts under the socket's last ring (y -0.0578).
# lamp_midle_0N: tubes from the atlas' top right block; _02/_03 also have
# broken tube ends of that texture. lamp_big_01: two tubes from the atlas'
# bottom strips (lamp_big_03 is that fixture without them).
TUBES_MIDLE = parts_textured(0.80, 0.0, 1.0, 0.36)
MODELS = {
    'lamp': below(-0.058),
    'plafon_lamp': no_glass,
    'lamp_midle_01': TUBES_MIDLE,
    'lamp_midle_02': TUBES_MIDLE,
    'lamp_midle_03': TUBES_MIDLE,
    'lamp_big_01': parts_textured(0.0, 0.825, 0.70, 0.945),
}
COLLISION = ('collision_enabled', 'walker_collision_enabled', 'collision_use_raytrace_for_walker')


def surfaces(settings):
    """The render surfaces of every LOD, from the model's settings."""
    lods = dict(settings[1])['lod_hierrarchy'][1]
    return [name for lod, (_, table) in lods if lod.startswith('LOD')
            for key, (_, names) in table if key == 'surfaces' for _, name in names]


def model_parts(stock, name):
    """Every stock file a broken model is made from, relative to its model."""
    settings = parse(bytearray((stock / f'models/props/light/{name}.model/settings').read_bytes()))
    return ['settings', 'render/export_properties'] + [
        f'render/{s}/{part}' for s in surfaces(settings) for part in ('converted_model', 'export_properties')]


def without(data, glass):
    """The mesh without its glass triangles: (blob, kept points, triangle count)."""
    vertices, indices = mesh_tools.mesh(data)
    points = [struct.unpack_from('<3f', v) for v in vertices]
    uvs = [struct.unpack_from('<2e', v, 24) for v in vertices]
    triangles = [indices[k:k + 3] for k in range(0, len(indices), 3)]
    dropped = glass(points, uvs, triangles)
    kept = [tri for k, tri in enumerate(triangles) if k not in dropped]
    assert kept, 'no body left'
    used = sorted({i for tri in kept for i in tri})
    index = {old: new for new, old in enumerate(used)}
    out = [index[i] for tri in kept for i in tri]
    blob = (struct.pack('<III', 3, 4 + 28 * len(used), len(used))
            + b''.join(vertices[i] for i in used)
            + struct.pack('<III', 4, 4 + 2 * len(out), len(out))
            + struct.pack('<%dH' % len(out), *out))
    return blob, [points[i] for i in used], len(kept)


def build(stock, hashes):
    files = {}
    for name, glass in MODELS.items():
        source = f'models/props/light/{name}.model/'
        target = f'models/props/light/srp_{name}_broken.model/'
        parts = model_parts(stock, name)
        originals = {part: (stock / (source + part)).read_bytes() for part in parts}
        for part, data in originals.items():
            assert hashlib.sha256(data).hexdigest() == hashes[source + part], source + part
        every = []
        for part in parts:
            if not part.endswith('/converted_model'):
                continue
            blob, points, triangles = without(originals[part], glass)
            files[target + part] = blob
            every += points
            low, high = mesh_tools.bounds(points)
            center = [(lo + hi) / 2 for lo, hi in zip(low, high)]
            radius = max(math.dist(p, center) for p in points)
            props = part.replace('/converted_model', '/export_properties')
            tree = parse(bytearray(originals[props]))
            put(tree, 'bounding_box/min', (7, low))
            put(tree, 'bounding_box/max', (7, high))
            put(tree, 'bounding_sphere/center', (7, center))
            put(tree, 'bounding_sphere/radius', (2, struct.unpack('<I', struct.pack('<f', radius))[0]))
            for field, value in [('vertices', len(points)), ('indices', triangles * 3), ('primitives', triangles)]:
                put(tree, 'source/' + field, (1, value))
            files[target + props] = serialize(tree)
        tree = parse(bytearray(originals['render/export_properties']))
        low, high = mesh_tools.bounds(every)
        put(tree, 'bounding_box/min', (7, low))
        put(tree, 'bounding_box/max', (7, high))
        files[target + 'render/export_properties'] = serialize(tree)
        settings = parse(bytearray(originals['settings']))
        present = {key for key, _ in settings[1]}
        for key in COLLISION:
            if key in present:
                put(settings, key, (mesh_tools.T_BOOL, 0))
            else:   # lamp_big_01's settings leave collision to its default (on)
                settings[1].append((key, (mesh_tools.T_BOOL, 0)))
        files[target + 'settings'] = serialize(settings)
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out already exists')
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    files = build(args.stock, hashes)
    pack = {'format_version': 1, 'id': 'breakable-lamps', 'version': '2', 'target_build': '0.26g',
            'description': 'Broken lamp fixtures: bulb sockets, ceiling lamp bodies and fluorescent '
                           'housings, without their glass',
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g by scripts/breakable-lamps/make_pack.py; '
                      'map placement and light bindings are breakable-lamps-1.json.']}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
