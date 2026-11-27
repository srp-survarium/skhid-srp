#!/usr/bin/env python3
"""Build vector-windows@1 from Vector Laboratory's glazed partitions, for release/content.tsv.

--stock contains the model's files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json). --out is
a new resource-porter pack directory. Never builds a release patch.

Vector's glass partitions (the owner, 2026-11-27, of the boiler room's
metal-framed windows: break them as the capsules) are no model of their own:
they are the surface glass_s (tiled/window/glass) of the level's building
level/07/base_a, a render visual at the origin of every Vector project, with
its glass's collision in the building's (collision object
models/level/07/base_a.model/collision, its material coll_glass). Its
vertical glass stands in bays of the partitions' frames (the boiler room's
at z -27.5 and x 68.1, the back wall's high windows at z -31.5, two more),
each glass part two-sided (a quad and its twin 2 cm off); its flat glass
(tabletops, the floor's) breaks nothing. Bays: the vertical parts of a plane
(their thin side within PLANE of each other) whose horizontal extents meet
(within GAP), ordered by plane (x, then z) and along it.

- srp_base_a_frame: the building with glass_s keeping its flat glass alone
  (every other surface as it is, the root's bounds as they are), which the
  map's building draws instead (vector-windows-1.json);
- srp_base_a_pane<k>: bay k's glass alone, about its centre;
- srp_base_a_pane<k>_broken: what is left of it, each part's glass along its
  frame, in from its edges 1 to 7 cm, jagged (its triangles cut to 6 cm,
  those near an edge kept; a part and its twin alike).

A broken bay leaves no pieces: Vector's projects have the client's most
already (the capsules'); the game's glass burst is its break. Collision is
disabled in all: the building's collision object keeps the glass's (as a
lamp's).
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

BUILDING = 'level/07/base_a'
FRAME = 'level/07/srp_base_a_frame'
GLASS = 'glass_s'
# a part is vertical when it rises VERTICAL and is at most THIN across;
# bays' planes within PLANE, their parts within GAP along them
VERTICAL = 0.1
THIN = 0.05
PLANE = 0.05
GAP = 0.015
# the broken bay: its triangles' size, the glass left along an edge (EDGE,
# jittered by EDGE_JITTER every EDGE_STEP along it)
STEP = 0.06
EDGE = 0.04
EDGE_JITTER = 0.03
EDGE_STEP = 0.07


def pane_name(k):
    return f'level/07/srp_base_a_pane{k}'


def broken_name(k):
    return pane_name(k) + '_broken'


def glass_of(originals):
    return originals[f'render/{GLASS}/converted_model']


class Part:
    """A closed part of the glass: its triangle numbers and bounds."""

    def __init__(self, triangles, low, high):
        self.triangles, self.low, self.high = triangles, low, high

    def size(self):
        return [hi - lo for lo, hi in zip(self.low, self.high)]

    def vertical(self):
        size = self.size()
        return size[1] >= VERTICAL and min(size[0], size[2]) <= THIN

    def axis(self):
        """The axis across it (0: x, 2: z)."""
        size = self.size()
        return 0 if size[0] <= size[2] else 2


class Bay:
    """Vertical glass parts in one plane meeting along it: one fixture."""

    def __init__(self, axis, parts):
        self.axis, self.parts = axis, parts
        self.low = [min(p.low[a] for p in parts) for a in range(3)]
        self.high = [max(p.high[a] for p in parts) for a in range(3)]
        self.centre = [(lo + hi) / 2 for lo, hi in zip(self.low, self.high)]
        self.triangles = set().union(*(p.triangles for p in parts))


def parts_of(data):
    """The glass's closed parts (triangles sharing corners, to the millimetre)."""
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
    parts = []
    for group in groups.values():
        low, high = mesh_tools.bounds([points[i] for k in group for i in triangles[k]])
        parts.append(Part(group, low, high))
    return parts


def bays(data):
    """The glass's bays, in order, and the triangle numbers of its flat glass."""
    parts = parts_of(data)
    flat = set().union(*(p.triangles for p in parts if not p.vertical()))
    found = []
    for axis in (0, 2):
        along = 2 - axis
        standing = sorted((p for p in parts if p.vertical() and p.axis() == axis),
                          key=lambda p: (p.low[axis] + p.high[axis]) / 2)
        planes, last = [], None
        for p in standing:
            at = (p.low[axis] + p.high[axis]) / 2
            if last is None or at - last > PLANE:
                planes.append([])
            planes[-1].append(p)
            last = at
        for plane in planes:
            plane.sort(key=lambda p: (p.low[along], p.low[1]))
            reach = None
            for p in plane:
                if reach is None or p.low[along] > reach + GAP:
                    found.append([p])
                    reach = p.high[along]
                else:
                    found[-1].append(p)
                    reach = max(reach, p.high[along])
    return [Bay(group[0].axis(), group) for group in found], flat


def kept_edge(seed, s):
    """The glass left along an edge at `s` along it: EDGE, jittered, between steps straight."""
    at = s / EDGE_STEP
    i = math.floor(at)
    f = at - i
    h = [EDGE + bottle.jitter(seed * 7919 + (j & 0xFFFF), EDGE_JITTER) for j in (i, i + 1)]
    return h[0] * (1 - f) + h[1] * f


def remains(data, bay):
    """The broken bay: its parts' glass along their frames, about the bay's centre: (blob, points, triangle count)."""
    along = 2 - bay.axis
    records, points = [], []
    count = 0
    for part in bay.parts:
        piece = ware.cap_pack.keep(data, lambda p, u, t, part=part: part.triangles, [0, 0, 0])[0]
        u0, u1, v0, v1 = part.low[along], part.high[along], part.low[1], part.high[1]
        # a part and its twin alike: the seed of its place in the frame, to the 5 cm
        base = round(u0 / 0.05) * 131 + round(v0 / 0.05) * 7
        for triangle in bottle.subdivided(piece, STEP):
            mid = [sum(p[0][a] for p in triangle) / 3 for a in range(3)]
            u, v = mid[along], mid[1]
            edges = [(u - u0, 0, v - v0), (u1 - u, 1, v - v0), (v - v0, 2, u - u0), (v1 - v, 3, u - u0)]
            d, edge, s = min(edges)
            if d >= kept_edge(base + edge, s):
                continue
            for position, uv, vs, ws in triangle:
                p = [x - o for x, o in zip(position, bay.centre)]
                records.append(bottle.vertex(p, uv, vs, ws))
                points.append(tuple(p))
            count += 1
    assert count, 'nothing left of a bay'
    indices = list(range(len(records)))
    blob = (struct.pack('<III', 3, 4 + 28 * len(records), len(records)) + b''.join(records)
            + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices))
            + struct.pack('<%dH' % len(indices), *indices))
    return blob, points, count


def found(stock, hashes):
    """Each bay's (centre, low, high, axis) in the building's frame (the map's: it stands at the origin)."""
    data = glass_of(ware.originals_of(stock, BUILDING, hashes))
    return [(bay.centre, bay.low, bay.high, bay.axis) for bay in bays(data)[0]]


def frame(originals, flat, target):
    """The building with its glass's flat parts alone."""
    settings = parse(bytearray(originals['settings']))
    files = {}
    for name in ware.lamp_pack.surfaces(settings):
        if name == GLASS:
            continue
        for part in ('converted_model', 'export_properties'):
            files[target + f'render/{name}/{part}'] = originals[f'render/{name}/{part}']
    part = f'render/{GLASS}/converted_model'
    blob, points, triangles = ware.cap_pack.keep(glass_of(originals), lambda p, u, t: flat, [0, 0, 0])
    ware.surface_files(files, target, part, originals[f'render/{GLASS}/export_properties'], blob, points, triangles)
    files[target + 'render/export_properties'] = originals['render/export_properties']
    files[target + 'settings'] = ware.settings_of(originals['settings'])
    return files


def build(stock, hashes):
    originals = ware.originals_of(stock, BUILDING, hashes)
    data = glass_of(originals)
    found_bays, flat = bays(data)
    files = frame(originals, flat, ware.model_dir(FRAME))
    part = f'render/{GLASS}/converted_model'
    props = originals[f'render/{GLASS}/export_properties']
    for k, bay in enumerate(found_bays):
        whole = ware.cap_pack.keep(data, lambda p, u, t, bay=bay: bay.triangles, bay.centre)
        for target, (blob, points, triangles) in ((ware.model_dir(pane_name(k)), whole),
                                                  (ware.model_dir(broken_name(k)), remains(data, bay))):
            ware.surface_files(files, target, part, props, blob, points, triangles)
            files[target + 'render/export_properties'] = ware.root_props(originals['render/export_properties'], points)
            files[target + 'settings'] = ware.settings_of(originals['settings'], keep={GLASS})
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
    pack = {'format_version': 1, 'id': 'vector-windows', 'version': '1', 'target_build': '0.26g',
            'description': "Vector Laboratory's building level/07/base_a with its flat glass alone, each bay of its "
                           "glazed partitions alone and what a broken one leaves",
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g by scripts/vector-windows/make_pack.py; '
                      "Vector's placement is vector-windows-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
