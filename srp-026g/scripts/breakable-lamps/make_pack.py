#!/usr/bin/env python3
"""Build breakable-lamps@1 from stock lamp meshes, for release/content.tsv.

--stock contains the two models at their archive paths (srp-game-data get).
--out is a new resource-porter pack directory. Never builds a release patch.
The unlit models supply the socket/cap and material; the glass is removed.
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

# Local y of the socket's lip / ceiling fitting's cap. Keep their existing
# vertices, normals and UVs. The source triangles below the lip are glass.
MODELS = {'lamp': -0.0871, 'plafon_lamp': -0.1591}
PARTS = ('settings', 'render/export_properties',
         'render/lamp_lights_s/converted_model', 'render/lamp_lights_s/export_properties')


def cropped(data, lip):
    vertices, indices = mesh_tools.mesh(data)
    points = [struct.unpack_from('<3f', v) for v in vertices]
    triangles = [indices[k:k + 3] for k in range(0, len(indices), 3)
                 if all(points[i][1] >= lip for i in indices[k:k + 3])]
    used = sorted({i for tri in triangles for i in tri})
    assert triangles and len(triangles) * 3 < len(indices)
    index = {old: new for new, old in enumerate(used)}
    out = [index[i] for tri in triangles for i in tri]
    blob = (struct.pack('<III', 3, 4 + 28 * len(used), len(used))
            + b''.join(vertices[i] for i in used)
            + struct.pack('<III', 4, 4 + 2 * len(out), len(out))
            + struct.pack('<%dH' % len(out), *out))
    return blob, [points[i] for i in used], len(triangles)


def build(stock, hashes):
    files = {}
    for name, lip in MODELS.items():
        source = f'models/props/light/{name}.model/'
        target = f'models/props/light/srp_{name}_broken.model/'
        originals = {part: (stock / (source + part)).read_bytes() for part in PARTS}
        for part, data in originals.items():
            assert hashlib.sha256(data).hexdigest() == hashes[source + part], source + part
        blob, points, triangles = cropped(originals[PARTS[2]], lip)
        low, high = mesh_tools.bounds(points)
        center = [(lo + hi) / 2 for lo, hi in zip(low, high)]
        radius = max(math.dist(p, center) for p in points)
        for part in (PARTS[1], PARTS[3]):
            tree = parse(bytearray(originals[part]))
            put(tree, 'bounding_box/min', (7, low))
            put(tree, 'bounding_box/max', (7, high))
            if part == PARTS[3]:
                put(tree, 'bounding_sphere/center', (7, center))
                put(tree, 'bounding_sphere/radius', (2, struct.unpack('<I', struct.pack('<f', radius))[0]))
                for field, value in [('vertices', len(points)), ('indices', triangles * 3), ('primitives', triangles)]:
                    put(tree, 'source/' + field, (1, value))
            files[target + part] = serialize(tree)
        settings = parse(bytearray(originals['settings']))
        for key in ('collision_enabled', 'walker_collision_enabled', 'collision_use_raytrace_for_walker'):
            put(settings, key, (0, 0))
        files[target + 'settings'] = serialize(settings)
        files[target + PARTS[2]] = blob
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
    pack = {'format_version': 1, 'id': 'breakable-lamps', 'version': '1', 'target_build': '0.26g',
            'description': 'Broken bulb sockets and ceiling lamp caps, without glass or emission',
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
