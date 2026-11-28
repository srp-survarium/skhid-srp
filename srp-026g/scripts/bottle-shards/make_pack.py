#!/usr/bin/env python3
"""Build bottle-shards@1 from the vodka bottle's mesh, for release/content.tsv.

--stock contains the bottle's files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json): stock's
props/kitchen/bottle_vodka and the v0.21 port's copy (the same mesh, its own
settings). --out is a new resource-porter pack directory. Never builds a
release patch.

A shot bottle (School's, the owner 2026-11-26: "there should be proper
shards and destruction of the bottle") breaks into SHARDS pieces, each a
model of its own, srp_bottle_vodka_shard<k>, beside each source model:

- the bottle's surface (49 vertices, 58 triangles: 17 cm flat sides) cut
  into triangles of at most STEP (2 cm) a side, each vertex's position, UV
  and three attribute vectors (normal, tangent, binormal as unsigned bytes)
  interpolated from its triangle's corners;
- those grouped into shards: the neck (above NECK) one, the rest each to the
  nearest of SEEDS (two rings of points on the body), with a fixed jitter a
  triangle so the edges are jagged;
- each triangle doubled with its winding reversed and its vectors negated,
  so a shard is seen from either side;
- its origin the centre of its bounds (where the lamp patch's flying body is,
  docs/notes/breakable_lamps.md "Loose pieces"), collision disabled.
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
cap_pack = load('cap_pack', 'skeleton-cap/make_pack.py')
parse, serialize, put = mesh_tools.parse, mesh_tools.serialize, mesh_tools.put

SOURCES = {
    'models/props/kitchen/bottle_vodka.model/': 'models/props/kitchen/srp_bottle_vodka_shard{}.model/',
    'models/ported/v021d/props/kitchen/bottle_vodka.model/':
        'models/ported/v021d/props/kitchen/srp_bottle_vodka_shard{}.model/',
}
SURFACE = 'dishes_02_s'
STEP = 0.02
# The bottle: a body of radius 3.8 cm from 0.5 to 18 cm, its shoulder to 21
# cm, its neck to 30.5 cm. The neck breaks off whole; the body into seven.
NECK = 0.2
RADIUS = 0.038
SEEDS = [(RADIUS * math.cos(math.radians(a)), y, RADIUS * math.sin(math.radians(a)))
         for y, angles in ((0.045, (20, 140, 260)), (0.125, (80, 200, 320))) for a in angles] + [(0.0, 0.19, 0.0)]
SHARDS = 1 + len(SEEDS)
JITTER = 0.008


class Cut:
    """How a surface breaks: its triangles cut to at most `step` a side,
    those above `neck` (None: none) one shard, the rest each the nearest of
    `seeds`' (jittered by up to `jitter` a triangle). The bottle's is the
    module's constants; other glass (kitchen-ware's jugs, Vector's capsule
    glass) makes its own, with seeds from far_seeds."""

    def __init__(self, seeds, neck=None, step=STEP, jitter=JITTER):
        self.seeds, self.neck, self.step, self.jitter = list(seeds), neck, step, jitter
        self.count = len(self.seeds) + (neck is not None)


BOTTLE = Cut(SEEDS, NECK)


def far_seeds(data, n, step, first):
    """`n` seeds spread over the mesh's surface: its small triangles'
    centres picked one by one, each the farthest from those picked, from
    the one nearest `first`."""
    centres = [[sum(p[0][a] for p in t) / 3 for a in range(3)] for t in subdivided(data, step)]
    picked = [min(centres, key=lambda c: math.dist(c, first))]
    nearest = [math.dist(c, picked[0]) for c in centres]
    while len(picked) < n:
        k = max(range(len(centres)), key=lambda i: nearest[i])
        picked.append(centres[k])
        nearest = [min(d, math.dist(c, centres[k])) for d, c in zip(nearest, centres)]
    return [tuple(c) for c in picked]


def jitter(k, size=JITTER):
    """A fixed value in -size..size for the k-th small triangle."""
    h = (k * 2654435761) & 0xFFFFFFFF
    return ((h >> 8) / float(1 << 24) * 2 - 1) * size


def vectors(record):
    """A vertex's three attribute vectors as floats (each byte b: b / 127.5 - 1) and their w bytes."""
    raw = record[12:24]
    return [[raw[4 * k + i] / 127.5 - 1 for i in range(3)] for k in range(3)], [raw[4 * k + 3] for k in range(3)]


def encode(vs, ws):
    out = bytearray()
    for v, w in zip(vs, ws):
        n = math.sqrt(sum(c * c for c in v)) or 1.0
        out += bytes(max(0, min(255, round((c / n + 1) * 127.5))) for c in v) + bytes([w])
    return bytes(out)


def vertex(position, uv, vs, ws):
    return struct.pack('<3f', *position) + encode(vs, ws) + struct.pack('<2e', *uv)


def subdivided(data, step=STEP):
    """The mesh's triangles cut to at most `step` a side: [(three vertices as
    (position, uv, vectors, w bytes))]."""
    records, indices = mesh_tools.mesh(data)
    corners = []
    for r in records:
        vs, ws = vectors(r)
        corners.append((struct.unpack_from('<3f', r), struct.unpack_from('<2e', r, 24), vs, ws))
    out = []
    for t in range(0, len(indices), 3):
        a, b, c = (corners[i] for i in indices[t:t + 3])
        n = max(1, math.ceil(max(math.dist(p[0], q[0]) for p, q in ((a, b), (b, c), (c, a))) / step))

        def at(i, j):
            u, v = i / n, j / n
            w = 1 - u - v
            lerp = lambda f: [w * x + u * y + v * z for x, y, z in zip(f(a), f(b), f(c))]
            vs = [[w * x + u * y + v * z for x, y, z in zip(a[2][k], b[2][k], c[2][k])] for k in range(3)]
            return lerp(lambda p: p[0]), lerp(lambda p: p[1]), vs, a[3]
        for i in range(n):
            for j in range(n - i):
                out.append((at(i, j), at(i + 1, j), at(i, j + 1)))
                if i + j < n - 1:
                    out.append((at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)))
    return out


def shard_of(k, triangle, cut=BOTTLE):
    """Which shard the k-th small triangle is: the neck's (the first, when
    the cut has one), else its nearest seed's (jittered)."""
    centre = [sum(p[0][a] for p in triangle) / 3 for a in range(3)]
    neck = cut.neck is not None
    if neck and centre[1] > cut.neck:
        return 0
    seeds = cut.seeds
    return neck + min(range(len(seeds)), key=lambda s: math.dist(centre, seeds[s]) + jitter(k * len(seeds) + s, cut.jitter))


def shards(data, cut=BOTTLE):
    """[(converted_model blob, its points, triangle count, centre)] of each shard, in shard order."""
    groups = [[] for _ in range(cut.count)]
    for k, triangle in enumerate(subdivided(data, cut.step)):
        groups[shard_of(k, triangle, cut)].append(triangle)
    out = []
    for group in groups:
        assert group, 'an empty shard'
        low, high = mesh_tools.bounds([p[0] for t in group for p in t])
        centre = [(lo + hi) / 2 for lo, hi in zip(low, high)]
        records, points, faces = [], [], []
        for side in (1, -1):              # the shard's face, then its back
            for triangle in group:
                corners = triangle if side == 1 else triangle[::-1]
                for position, uv, vs, ws in corners:
                    p = [x - o for x, o in zip(position, centre)]
                    records.append(vertex(p, uv, [[side * c for c in v] for v in vs], ws))
                    points.append(tuple(p))
                faces.append(len(faces))
        indices = list(range(len(records)))
        blob = (struct.pack('<III', 3, 4 + 28 * len(records), len(records)) + b''.join(records)
                + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices))
                + struct.pack('<%dH' % len(indices), *indices))
        out.append((blob, points, len(faces), centre))
    return out


def centres(stock):
    """Each shard's centre in the bottle's frame (stock's mesh)."""
    source = next(iter(SOURCES))
    return [c for *_, c in shards((stock / source / f'render/{SURFACE}/converted_model').read_bytes())]


def model(originals, target, blob, points, triangles):
    files = {}
    part = f'render/{SURFACE}/converted_model'
    cap_pack.surface_files(files, target, part, originals[f'render/{SURFACE}/export_properties'], blob, points, triangles)
    tree = parse(bytearray(originals['render/export_properties']))
    low, high = mesh_tools.bounds(points)
    put(tree, 'bounding_box/min', (7, low))
    put(tree, 'bounding_box/max', (7, high))
    files[target + 'render/export_properties'] = serialize(tree)
    settings = parse(bytearray(originals['settings']))
    present = {key for key, _ in settings[1]}
    for key in cap_pack.COLLISION:
        if key in present:
            put(settings, key, (mesh_tools.T_BOOL, 0))
        else:
            settings[1].append((key, (mesh_tools.T_BOOL, 0)))
    files[target + 'settings'] = serialize(settings)
    return files


def build(stock, hashes):
    files = {}
    parts = ['settings', 'render/export_properties', f'render/{SURFACE}/converted_model',
             f'render/{SURFACE}/export_properties']
    for source, target in SOURCES.items():
        originals = {part: (stock / (source + part)).read_bytes() for part in parts}
        for part, data in originals.items():
            assert hashlib.sha256(data).hexdigest() == hashes[source + part], source + part
        for k, (blob, points, triangles, _) in enumerate(shards(originals[f'render/{SURFACE}/converted_model'])):
            files |= model(originals, target.format(k), blob, points, triangles)
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
    pack = {'format_version': 1, 'id': 'bottle-shards', 'version': '1', 'target_build': '0.26g',
            'description': "The vodka bottle's shards, each a model of its own",
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g and the v0.21 port by scripts/bottle-shards/make_pack.py; '
                      "School's placement is breakable-bottles-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
