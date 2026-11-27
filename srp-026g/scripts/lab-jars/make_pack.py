#!/usr/bin/env python3
"""Build lab-jars@1 from Vector Laboratory's racks of glass jars, for release/content.tsv.

--stock contains the model's files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json). --out is
a new resource-porter pack directory. Never builds a release patch.

Vector's racks (props/lab/chemical_flask, five a project; the owner,
2026-11-27: "I expected those one") hold ten glass jars each, 36 cm across
and 1.06 m tall, on rods: the surface windows_broken_s, ten closed parts. A
shot breaks one jar (lab-jars-1.json, a fixture of the map's lamp table each,
docs/notes/breakable_lamps.md "Vector's jars"):

- srp_chemical_flask_frame: the rack without its glass (the surface out of
  its settings, the rest as it is), which the map's racks draw instead;
- srp_chemical_flask_jar<k>: jar k's glass alone, about its centre;
- srp_chemical_flask_jar<k>_broken: what is left of it, its bottom up to a
  jagged rim 6 to 22 cm high (its triangles cut to 4 cm, those under the rim
  kept);
- srp_chemical_flask_jar<k>_glass<j>: the rest of it, the glass over the
  rim, as SHARDS two-sided shards (scripts/bottle-shards' cut, jagged by up
  to 4 cm): strips around the jar, some 22 cm wide and 90 tall, the lid's
  share with each, each about its centre. `shard_info`
  gives each one's centre in the rack's frame, its outward normal there (from
  the jar's axis) and its long side: they fly as a capsule's (kind
  'pane_shard', the owner, 2026-11-27: "jar racks don't have shards").

- srp_chemical_flask_collision: the rack's collision without its jars' ten
  glass capsules (its primitives of coll_glass, `mtl` 0), its settings the
  rack's, which Vector's
  racks' collision objects take instead (the owner, 2026-11-27: "glass
  bullet holes are left" "even though glass was destroyed": a jar's glass
  capsule, kept, reached 18 cm over the jar, past its fixture's bounds). A
  bullet passes a jar, breaking it, to what is behind.

Jars are numbered by their centres, x then z. Collision is disabled in the
rendered models; the walkers' collision is the rack's.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import struct
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ware = load('kitchen_ware', 'kitchen-ware/make_pack.py')
bottle = ware.bottle
mesh_tools = ware.mesh_tools
parse = ware.parse

RACK = 'props/lab/chemical_flask'
FRAME = 'props/lab/srp_chemical_flask_frame'
COLLISION = 'props/lab/srp_chemical_flask_collision'
PRIMITIVES = 'collision/exported_primitives'
# the jars' glass capsules: the rack's primitives of its mtl_list's first, coll_glass
GLASS_MTL = 0
GLASS = 'windows_broken_s'
JARS = 10
# the stump: its rim's height over the jar's bottom, a step's jitter about
# it around the jar (RIM_STEPS sectors), its triangles' size
RIM = 0.14
RIM_JITTER = 0.08
RIM_STEPS = 14
STEP = 0.04
SHARDS = 5
# the glass over the rim is cut coarser: it flies off (a few cm of the rim's
# jag lost between it and the stump), and a mesh has at most 65536 corners
OVER_STEP = 0.1
SHARD_JITTER = 0.04


def jar_names():
    return [f'props/lab/srp_chemical_flask_jar{k}' for k in range(JARS)]


def broken_names():
    return [name + '_broken' for name in jar_names()]


def shard_names(k):
    return [f'{jar_names()[k]}_glass{j}' for j in range(SHARDS)]


def glass_of(originals):
    return originals[f'render/{GLASS}/converted_model']


def parts(data):
    """The glass's closed parts (triangles sharing corners, to the
    millimetre), each a set of triangle numbers, ordered by their centres (x,
    then z)."""
    vertices, indices = mesh_tools.mesh(data)
    points = [tuple(round(c, 3) for c in struct.unpack_from('<3f', v)) for v in vertices]
    parent = {}

    def root(p):
        while parent.setdefault(p, p) != p:
            parent[p] = parent[parent[p]]
            p = parent[p]
        return p
    triangles = [indices[k:k + 3] for k in range(0, len(indices), 3)]
    for a, b, c in triangles:
        parent[root(points[a])] = root(points[b])
        parent[root(points[c])] = root(points[b])
    groups = {}
    for k, t in enumerate(triangles):
        groups.setdefault(root(points[t[0]]), set()).add(k)
    found = []
    for group in groups.values():
        low, high = mesh_tools.bounds([points[i] for k in group for i in triangles[k]])
        found.append(([(lo + hi) / 2 for lo, hi in zip(low, high)], (low, high), group))
    found.sort(key=lambda f: (round(f[0][0], 1), round(f[0][2], 1)))
    assert len(found) == JARS, f'{len(found)} parts, not {JARS} jars'
    return found


def jars(stock, hashes):
    """Each jar's (centre, bounds) in the rack's frame: where lab-jars-1.json
    places it and its fixture's bounds."""
    data = glass_of(ware.originals_of(stock, RACK, hashes))
    return [(centre, bounds) for centre, bounds, _ in parts(data)]


def rim(k, angle):
    """The stump's rim height at `angle` about jar k's axis: a jitter a sector, between the sectors' straight."""
    at = angle / (2 * math.pi) * RIM_STEPS
    i = math.floor(at)
    f = at - i
    h = [RIM + bottle.jitter(k * RIM_STEPS + (j % RIM_STEPS), RIM_JITTER) for j in (i, i + 1)]
    return h[0] * (1 - f) + h[1] * f


def split(data, k, centre, low, step=STEP):
    """Jar k's triangles (`data` its glass alone), cut to `step`: (those under its jagged rim, those over it)."""
    under, over = [], []
    for triangle in bottle.subdivided(data, step):
        mid = [sum(p[0][a] for p in triangle) / 3 for a in range(3)]
        angle = math.atan2(mid[2] - centre[2], mid[0] - centre[0]) % (2 * math.pi)
        (under if mid[1] - low[1] < rim(k, angle) else over).append(triangle)
    assert under and over, 'a jar not cut'
    return under, over


def blob_of(triangles, origin):
    """A converted_model of `triangles` (as `bottle.subdivided` gives them) about `origin`: (blob, points, count)."""
    records, points = [], []
    for triangle in triangles:
        for position, uv, vs, ws in triangle:
            p = [x - o for x, o in zip(position, origin)]
            records.append(bottle.vertex(p, uv, vs, ws))
            points.append(tuple(p))
    indices = list(range(len(records)))
    blob = (struct.pack('<III', 3, 4 + 28 * len(records), len(records)) + b''.join(records)
            + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices))
            + struct.pack('<%dH' % len(indices), *indices))
    return blob, points, len(triangles)


def stump(data, k, centre, low):
    """Jar k's bottom under its jagged rim, about its centre: (blob, points, triangle count)."""
    return blob_of(split(data, k, centre, low)[0], centre)


def cut(over, centre):
    """The glass over the rim (a converted_model) cut into SHARDS strips
    around the jar (its centre `centre`): the seeds around its wall at one
    height, so the nearest is the nearest about the axis (seeds spread over
    it as a pane's made rings, the jar's lid with its top)."""
    radius = max(math.hypot(p[0] - centre[0], p[2] - centre[2]) for p in ware.points_of(over))
    seeds = [(centre[0] + radius * math.cos(2 * math.pi * (j + 0.25) / SHARDS), centre[1],
              centre[2] + radius * math.sin(2 * math.pi * (j + 0.25) / SHARDS)) for j in range(SHARDS)]
    return bottle.Cut(seeds, step=0.2, jitter=SHARD_JITTER)


def jar_glass(data, group):
    """Jar `group`'s glass alone, in the rack's frame."""
    return ware.cap_pack.keep(data, lambda p, u, t: group, [0, 0, 0])[0]


def over_of(data, k, centre, low, group):
    return blob_of(split(jar_glass(data, group), k, centre, low, OVER_STEP)[1], [0, 0, 0])[0]


def shard_info(stock, hashes):
    """Each jar's shards' (centre, out, along) in the rack's frame: where
    lab-jars-1.json places them, the jar's level outward normal there and
    their long side, rounded as the rows keep them."""
    data = glass_of(ware.originals_of(stock, RACK, hashes))
    out = []
    for k, (centre, (low, _), group) in enumerate(parts(data)):
        over = over_of(data, k, centre, low, group)
        pieces = []
        for _, points, _, c in bottle.shards(over, cut(over, centre)):
            dx, dz = c[0] - centre[0], c[2] - centre[2]
            length = math.hypot(dx, dz)
            assert length > 0.05, 'a shard on the axis'
            pieces.append((c, [round(dx / length, 4), 0.0, round(dz / length, 4)],
                           [round(a, 4) for a in principal(points)]))
        out.append(pieces)
    return out


def principal(points):
    """The unit direction of the points' greatest spread (about their mean)."""
    n = len(points)
    mean = [sum(p[a] for p in points) / n for a in range(3)]
    c = [[sum((p[i] - mean[i]) * (p[j] - mean[j]) for p in points) for j in range(3)] for i in range(3)]
    v = [1.0, 1.0, 1.0]
    for _ in range(100):
        w = [sum(c[i][j] * v[j] for j in range(3)) for i in range(3)]
        length = math.sqrt(sum(x * x for x in w))
        assert length > 0, 'no spread'
        v = [x / length for x in w]
    return v


def frame(originals, target):
    """The rack without its glass: its bounds the rack's (the jars stand
    inside its rods; its rods' surface has 32-byte vertices, which the mesh
    tools do not read)."""
    settings = parse(bytearray(originals['settings']))
    kept = [s for s in ware.lamp_pack.surfaces(settings) if s != GLASS]
    files = {}
    for name in kept:
        for part in ('converted_model', 'export_properties'):
            files[target + f'render/{name}/{part}'] = originals[f'render/{name}/{part}']
    files[target + 'render/export_properties'] = originals['render/export_properties']
    files[target + 'settings'] = ware.settings_of(originals['settings'], keep=set(kept))
    return files


def without_glass(original):
    """The rack's collision primitives without its jars' glass: (file, kept, dropped)."""
    tree = parse(bytearray(original))
    table = dict(tree[1])
    assert table['mtl_list'][1][GLASS_MTL][1] == 'coll_glass', table['mtl_list']
    _, items = table['primitives']
    kept = [item for item in items if dict(item[1])['mtl'][1] != GLASS_MTL]
    dropped = len(items) - len(kept)
    items[:] = kept
    return ware.serialize(tree), len(kept), dropped


def collision(stock, hashes, target):
    path = ware.model_dir(RACK) + PRIMITIVES
    original = (stock / path).read_bytes()
    assert hashlib.sha256(original).hexdigest() == hashes[path], path
    data, _, dropped = without_glass(original)
    assert dropped == JARS, f'{dropped} glass primitives, not {JARS}'
    settings = (stock / ware.model_dir(RACK) / 'settings').read_bytes()
    return {target + PRIMITIVES: data, target + 'settings': settings}


def build(stock, hashes):
    originals = ware.originals_of(stock, RACK, hashes)
    files = frame(originals, ware.model_dir(FRAME)) | collision(stock, hashes, ware.model_dir(COLLISION))
    data = glass_of(originals)
    part = f'render/{GLASS}/converted_model'
    props = originals[f'render/{GLASS}/export_properties']
    for k, ((centre, (low, _), group), whole, broken) in enumerate(zip(parts(data), jar_names(), broken_names())):
        blob, points, triangles = ware.cap_pack.keep(data, lambda p, u, t, group=group: group, centre)
        piece = stump(jar_glass(data, group), k, centre, low)
        for target, (blob, points, triangles) in ((ware.model_dir(whole), (blob, points, triangles)),
                                                  (ware.model_dir(broken), piece)):
            ware.surface_files(files, target, part, props, blob, points, triangles)
            files[target + 'render/export_properties'] = ware.root_props(originals['render/export_properties'], points)
            files[target + 'settings'] = ware.settings_of(originals['settings'], keep={GLASS})
        over = over_of(data, k, centre, low, group)
        files |= ware.shard_models(originals, GLASS, [ware.model_dir(t) for t in shard_names(k)], cut(over, centre),
                                   data=over)
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
    pack = {'format_version': 1, 'id': 'lab-jars', 'version': '1', 'target_build': '0.26g',
            'description': "Vector Laboratory's rack of jars without its glass, each of its ten jars alone, "
                           'what a broken one leaves and its five shards',
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g by scripts/lab-jars/make_pack.py; '
                      "Vector's placement is lab-jars-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
