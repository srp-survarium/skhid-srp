#!/usr/bin/env python3
"""Build skeleton-cap@1 from the sitting skeleton's meshes, for release/content.tsv.

--stock contains the models' files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json): stock's
remains/skeleton_sitting1 and the v0.21 port's copy (the same meshes, its own
materials). --out is a new resource-porter pack directory. Never builds a
release patch.

The skeleton's cap (Rudnya's, the owner 2026-11-26: "shooting this helmet
should make it fly off") is the part of its cloth surface (rag) textured
from the atlas' top right block, in every LOD. Each source model makes two:

- srp_skeleton_sitting1_bare: the skeleton without its cap (its bones as
  they are, the cloth without the cap part);
- srp_skeleton_sitting1_cap: the cap alone, its origin the centre of its
  LOD0 bounds (where the lamp patch's flying body is, docs/notes/
  breakable_lamps.md "Loose pieces").

Bounds and vertex/triangle counts are recomputed and model collision is
disabled: the intact skeleton stays in the map and keeps its own.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


mesh_tools = load('bulb_mesh', 'lobby-bulb/make_pack.py')
lamp_pack = load('lamp_pack', 'breakable-lamps/make_pack.py')
parse, serialize, put = mesh_tools.parse, mesh_tools.serialize, mesh_tools.put

# The cap: the rag part with a corner in the atlas' top right block (LOD0
# 250 triangles, uv 0.75..0.99 x 0.01..0.25; LOD1 108, LOD2 70). The
# cloth's other parts (a shirt, trousers) are textured from u < 0.75.
CAP = lamp_pack.parts_textured(0.75, 0.0, 1.0, 0.26)
SOURCES = {
    'models/character/human/static/remains/skeleton_sitting1.model/':
        'models/character/human/static/remains/srp_skeleton_sitting1_{}.model/',
    'models/ported/v021d/character/human/static/remains/skeleton_sitting1.model/':
        'models/ported/v021d/character/human/static/remains/srp_skeleton_sitting1_{}.model/',
}
CLOTH = ('rag_s', 'lod1_rag_s', 'lod2_rag_s')
COLLISION = ('collision_enabled', 'walker_collision_enabled', 'collision_use_raytrace_for_walker')


def surfaces(settings):
    return lamp_pack.surfaces(settings)


def parts(settings):
    """Every source file a model is made from, relative to its model."""
    return ['settings', 'render/export_properties'] + [
        f'render/{s}/{part}' for s in surfaces(settings) for part in ('converted_model', 'export_properties')]


def keep(data, picked, offset):
    """The mesh's triangles `picked` chooses, moved by `offset`: (blob, points, triangle count)."""
    vertices, indices = mesh_tools.mesh(data)
    points = [struct.unpack_from('<3f', v) for v in vertices]
    uvs = [struct.unpack_from('<2e', v, 24) for v in vertices]
    triangles = [indices[k:k + 3] for k in range(0, len(indices), 3)]
    chosen = picked(points, uvs, triangles)
    kept = [tri for k, tri in enumerate(triangles) if k in chosen]
    assert kept, 'nothing kept'
    used = sorted({i for tri in kept for i in tri})
    index = {old: new for new, old in enumerate(used)}
    out = [index[i] for tri in kept for i in tri]
    moved = []
    for i in used:
        v = bytearray(vertices[i])
        p = [c - o for c, o in zip(points[i], offset)]
        struct.pack_into('<3f', v, 0, *p)
        moved.append((bytes(v), tuple(p)))
    blob = (struct.pack('<III', 3, 4 + 28 * len(used), len(used))
            + b''.join(v for v, _ in moved)
            + struct.pack('<III', 4, 4 + 2 * len(out), len(out))
            + struct.pack('<%dH' % len(out), *out))
    return blob, [p for _, p in moved], len(kept)


def cap_part(points, uvs, triangles):
    return CAP(points, uvs, triangles)


def body_part(points, uvs, triangles):
    return set(range(len(triangles))) - CAP(points, uvs, triangles)


def cap_centre(stock, source=next(iter(SOURCES))):
    """The cap's LOD0 bounds' centre in the skeleton's frame: the cap model's origin."""
    data = (stock / source / 'render/rag_s/converted_model').read_bytes()
    _, points, _ = keep(data, cap_part, (0.0, 0.0, 0.0))
    low, high = mesh_tools.bounds(points)
    return [(lo + hi) / 2 for lo, hi in zip(low, high)]


def surface_files(files, target, part, original_props, blob, points, triangles):
    files[target + part] = blob
    low, high = mesh_tools.bounds(points)
    center = [(lo + hi) / 2 for lo, hi in zip(low, high)]
    radius = max(math.dist(p, center) for p in points)
    tree = parse(bytearray(original_props))
    put(tree, 'bounding_box/min', (7, low))
    put(tree, 'bounding_box/max', (7, high))
    put(tree, 'bounding_sphere/center', (7, center))
    put(tree, 'bounding_sphere/radius', (2, struct.unpack('<I', struct.pack('<f', radius))[0]))
    for field, value in [('vertices', len(points)), ('indices', triangles * 3), ('primitives', triangles)]:
        put(tree, 'source/' + field, (1, value))
    files[target + part.replace('/converted_model', '/export_properties')] = serialize(tree)


def model(originals, target, cloth_only, picked, offset):
    """One model's files: the cloth's `picked` triangles moved by `offset`,
    and (unless `cloth_only`) the bones as they are."""
    files = {}
    settings = parse(bytearray(originals['settings']))
    every = []
    for name in surfaces(settings):
        part = f'render/{name}/converted_model'
        props = originals[f'render/{name}/export_properties']
        if name in CLOTH:
            blob, points, triangles = keep(originals[part], picked, offset)
            surface_files(files, target, part, props, blob, points, triangles)
            every += points
        elif not cloth_only:
            files[target + part] = originals[part]
            files[target + f'render/{name}/export_properties'] = props
            vertices, _ = mesh_tools.mesh(originals[part])
            every += [struct.unpack_from('<3f', v) for v in vertices]
    tree = parse(bytearray(originals['render/export_properties']))
    low, high = mesh_tools.bounds(every)
    put(tree, 'bounding_box/min', (7, low))
    put(tree, 'bounding_box/max', (7, high))
    files[target + 'render/export_properties'] = serialize(tree)
    if cloth_only:
        # its LODs and materials: the cloth's alone
        for key, (_, table) in settings[1]:
            if key == 'lod_hierrarchy':
                for lod, (_, entry) in table:
                    for field, (_, names) in entry:
                        if field == 'surfaces':
                            names[:] = [n for n in names if n[1] in CLOTH]
            if key == 'material_settings':
                table[:] = [(name, value) for name, value in table if name in CLOTH]
    present = {key for key, _ in settings[1]}
    for key in COLLISION:
        if key in present:
            put(settings, key, (mesh_tools.T_BOOL, 0))
        else:
            settings[1].append((key, (mesh_tools.T_BOOL, 0)))
    files[target + 'settings'] = serialize(settings)
    return files


def build(stock, hashes):
    files = {}
    centre = cap_centre(stock)
    for source, target in SOURCES.items():
        settings = parse(bytearray((stock / source / 'settings').read_bytes()))
        originals = {part: (stock / (source + part)).read_bytes() for part in parts(settings)}
        for part, data in originals.items():
            assert hashlib.sha256(data).hexdigest() == hashes[source + part], source + part
        assert cap_centre(stock, source) == centre, f'{source}: its cap is not stock\'s'
        files |= model(originals, target.format('bare'), False, body_part, (0.0, 0.0, 0.0))
        files |= model(originals, target.format('cap'), True, cap_part, centre)
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('--out already exists')
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    files = build(args.stock, hashes)
    pack = {'format_version': 1, 'id': 'skeleton-cap', 'version': '1', 'target_build': '0.26g',
            'description': "The sitting skeleton's cap as a model of its own, and the skeleton without it",
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g and the v0.21 port by scripts/skeleton-cap/make_pack.py; '
                      "Rudnya's placement and the lamp table are skeleton-cap-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
