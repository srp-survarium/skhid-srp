#!/usr/bin/env python3
"""make_pack.py: the content pack that puts decals under the STK2 loner's hood, as a stock vest
puts them under its straps (2026-11-26; ../../docs/notes/stk2-outfits.md, Decals).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/stk2-hood-decals-<version>

--live  the released files at their game paths: stk2-loner@15's `files/` (the pack as players
        have it), each file this reads checked by its sha256.
--out   the pack's folder (a new one): pack.json and files/, for the manifest's `pack` row
        (release/content.tsv), composed after stk2-loner.

Why. A decal is baked into the textures of its piece's first surface (`part_name`). The baker
(0x1307B0) draws that surface alone into a depth map along the decal's box, from 0.75 m outside
the clicked point, and the bake (bake_decal_accumulate.ps) paints only the texels nearest to that
camera: the first layer of the surface the decal's box meets. A piece's `additional_models` are
not in that map: a decal goes on the garment under them and they cover it, as a stock vest's
straps, pouches and fur, or the exoskeletons' back unit. The loner's jacket (1873) has its hood,
lying on the upper back, in the jacket's `torso` surface: a decal put on the upper back was
painted onto the hood, over everything.

What the pack does, in each of the jacket's three models (the sleeves of `data`/`data_full`,
`data_short`, `data_long`):

- the hood (one island of the map: 149 triangles, 89 vertices, every one of its 27 seam points
  joined to the jacket) becomes a surface of its own, `hood`, in the jacket's material, and an
  additional model of the jacket in all four tables of the item: drawn as before, never baked;
- in `torso` the hood's triangles stay as a lining under it: the same triangles and texels, each
  vertex moved into the hood by min(LINING_DEPTH, its distance from the seam) along LINING_IN
  (`lining_step`), so the lining is closed with the jacket at the seam and lies under the hood
  everywhere else. A decal's box now meets the lining there, and the hood covers it. The lining's
  texels are the hood's, which no other triangle of `torso` uses, and the hood draws the jacket's
  own unbaked map: what the bake paints on them shows nowhere;
- nothing else: the other islands, every vertex's skin, normal and UV, the maps and the other
  files are the live ones. The camouflage's `_aoac` mask (stk2-camo) reads the same UVs; the hood,
  an additional model, now wears none, as a stock vest's straps.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'content-edit'))
import content_edit  # noqa: E402

PACK_ID = 'stk2-hood-decals'
VERSION = '1'
REQUIRES = ['stk2-loner']
FOLDER = 'ported/stk2_loner_026'
MATERIAL = f'{FOLDER}/torso'
ITEM = 'gameplay/items/armour/torso/stk2_loner_026_torso.options'
TABLES = ('data', 'data_full', 'data_short', 'data_long')
# The jacket's three models (the tables' base_model) and the sha256 of their released torso
# surface and its export_properties (stk2-loner@15).
MODELS = {
    f'{FOLDER}/stk2_loner_026_torso': (
        '0962c786caf5e3e53103223c3c59187c03e29aaaa97a7ac93ac63b1f26e67002',
        '3c6f13b726ac677cba83f4db27c3c0e0d67b51ec1f9d454c7a980183915b8393'),
    f'{FOLDER}/stk2_loner_026_torso_s2': (
        'f5399f7abe2dd4d8429d52f89a199ff161dd174d75e1eab6eeb674c362ee833f',
        '5f4cbd6bf6aacf35a4142f581af3e9f23fd1c3a6d333e7a6365c2aa813830a9b'),
    f'{FOLDER}/stk2_loner_026_torso_s3': (
        '1a687f1120261ea8d1a8f22809246681466b427084e591994951d1889aafc41a',
        'fe4fbf78d6e0d6cdea2453530b6537a9903a9c3aefb5ed133960a8646c6d734b'),
}
ITEM_SHA256 = '9ae6abc7b0cf58c5180b900cbc64811c1f1e20d911b28c9389e654d294b9eaf9'
SETTINGS_SHA256 = 'bcfed807347e6200a43e8d00718826845c31af69790786fb7bd78bf9bd489343'
HOOD_TRIANGLES, HOOD_VERTICES, HOOD_SEAM = 149, 89, 27
LINING_DEPTH = 0.025
VERTEX_CHUNK, INDEX_CHUNK = 3, 4
NORMAL_AT = 28


class Surface:
    """A converted_model: a vertex chunk (id 3, its size, the count, the vertices) and an index
    chunk (id 4, its size, the count, u16 indices), nothing after them."""

    def __init__(self, data):
        tag, size, count = struct.unpack_from('<III', data, 0)
        if tag != VERTEX_CHUNK or not count or (size - 4) % count:
            raise ValueError('not a converted_model vertex chunk')
        self.stride = (size - 4) // count
        at = 12 + self.stride * count
        self.vertices = [bytearray(data[12 + i * self.stride:12 + (i + 1) * self.stride]) for i in range(count)]
        tag, size, n = struct.unpack_from('<III', data, at)
        if tag != INDEX_CHUNK or size != 4 + 2 * n or n % 3 or at + 12 + 2 * n != len(data):
            raise ValueError('not a converted_model index chunk')
        index = struct.unpack_from('<%dH' % n, data, at + 12)
        self.triangles = [tuple(index[i:i + 3]) for i in range(0, n, 3)]

    def copy(self, vertices, triangles):
        out = Surface.__new__(Surface)
        out.stride = self.stride
        out.vertices = [bytearray(v) for v in vertices]
        out.triangles = list(triangles)
        return out

    def bytes(self):
        out = bytearray(struct.pack('<III', VERTEX_CHUNK, 4 + self.stride * len(self.vertices), len(self.vertices)))
        for vertex in self.vertices:
            out += vertex
        index = [i for t in self.triangles for i in t]
        out += struct.pack('<III', INDEX_CHUNK, 4 + 2 * len(index), len(index))
        out += struct.pack('<%dH' % len(index), *index)
        return bytes(out)

    def position(self, i):
        return struct.unpack_from('<3f', self.vertices[i], 0)

    def normal(self, i):
        n = [c / 127.5 - 1 for c in self.vertices[i][NORMAL_AT:NORMAL_AT + 3][::-1]]
        length = math.sqrt(sum(c * c for c in n)) or 1.0
        return [c / length for c in n]

    def islands(self):
        """The triangles grouped by shared vertices: the islands of the map."""
        parent = list(range(len(self.vertices)))

        def root(a):
            while parent[a] != a:
                parent[a] = parent[parent[a]]
                a = parent[a]
            return a

        for a, b, c in self.triangles:
            for x, y in ((a, b), (b, c)):
                rx, ry = root(x), root(y)
                if rx != ry:
                    parent[rx] = ry
        groups = {}
        for t, tri in enumerate(self.triangles):
            groups.setdefault(root(tri[0]), []).append(t)
        return list(groups.values())


def is_hood(surface, island):
    """The hood: the island above the shoulders (y > 1.4 m) that reaches farthest back
    (z < -0.17 m; the jacket's back is at -z)."""
    points = [surface.position(i) for t in island for i in surface.triangles[t]]
    return min(p[1] for p in points) > 1.4 and min(p[2] for p in points) < -0.17


def lining_step(position, normal, seam_distance):
    """Where a hood vertex goes in the lining: into the hood along -normal by
    min(LINING_DEPTH, its distance from the seam)."""
    step = min(LINING_DEPTH, seam_distance)
    return [position[k] - normal[k] * step for k in range(3)]


def split(surface):
    """(the jacket's surface with the lining, the hood's own surface, the hood's bounds)."""
    hoods = [island for island in surface.islands() if is_hood(surface, island)]
    if len(hoods) != 1:
        raise SystemExit(f'{len(hoods)} islands look like the hood, one expected')
    hood = hoods[0]
    in_hood = set(hood)
    vertices = sorted({i for t in hood for i in surface.triangles[t]})
    key = lambda p: tuple(round(c, 5) for c in p)  # noqa: E731
    elsewhere = {key(surface.position(i)) for t, tri in enumerate(surface.triangles) if t not in in_hood for i in tri}
    seam = [surface.position(i) for i in vertices if key(surface.position(i)) in elsewhere]
    counts = (len(hood), len(vertices), len(seam))
    if counts != (HOOD_TRIANGLES, HOOD_VERTICES, HOOD_SEAM):
        raise SystemExit(f'the hood has {counts} triangles, vertices, seam points; '
                         f'{(HOOD_TRIANGLES, HOOD_VERTICES, HOOD_SEAM)} expected')
    renumber = {v: n for n, v in enumerate(vertices)}
    own = surface.copy([surface.vertices[v] for v in vertices],
                       [tuple(renumber[i] for i in surface.triangles[t]) for t in hood])
    jacket = surface.copy(surface.vertices, surface.triangles)
    for v in vertices:
        p = surface.position(v)
        struct.pack_into('<3f', jacket.vertices[v], 0,
                         *lining_step(p, surface.normal(v), min(math.dist(p, q) for q in seam)))
    points = [surface.position(v) for v in vertices]
    low = [min(p[k] for p in points) for k in range(3)]
    high = [max(p[k] for p in points) for k in range(3)]
    return jacket, own, (low, high)


def hood_properties(torso_properties, triangles, vertices, bounds):
    """The hood's export_properties: the torso's, its bounds and counts the hood's."""
    buf = bytearray(torso_properties)
    old = {path: value for path, _, value in content_edit.walk(buf)}
    low, high = bounds
    centre = [(a + b) / 2 for a, b in zip(low, high)]
    radius = math.dist(low, high) / 2
    for path, to in (('bounding_box/min', low), ('bounding_box/max', high), ('bounding_sphere/pos', centre)):
        frm = struct.unpack('<3f', old['/' + path])
        content_edit.set_vector(buf, path, frm, to)
    numbers = {'bounding_sphere/radius': radius, 'source/primitives': triangles,
               'source/indices': 3 * triangles, 'source/vertices': vertices}
    for path, to in numbers.items():
        offset = content_edit.find(buf, path)
        _, _, _, kind, _ = content_edit.rec(buf, offset)
        content_edit.set_number(buf, path, content_edit.as_number(kind, old['/' + path]), to)
    return bytes(buf)


def read(live, path, sha):
    data = (live / path).read_bytes()
    if hashlib.sha256(data).hexdigest() != sha:
        sys.exit(f'{path} is not the released file')
    return data


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--live', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out} exists')
    files, operations = {}, []
    for model, (sha, properties_sha) in MODELS.items():
        root = f'models/{model}.skinned_model'
        read(a.live, f'{root}/settings', SETTINGS_SHA256)
        torso_path = f'{root}/render/torso/converted_model'
        data = read(a.live, torso_path, sha)
        surface = Surface(data)
        if surface.bytes() != data:
            sys.exit(f'{torso_path}: not read back byte for byte')
        jacket, hood, bounds = split(surface)
        files[torso_path] = jacket.bytes()
        operations.append({'op': 'replace_file', 'path': torso_path,
                           'base_sha256': hashlib.sha256(data).hexdigest()})
        files[f'{root}/render/hood/converted_model'] = hood.bytes()
        files[f'{root}/render/hood/export_properties'] = hood_properties(
            read(a.live, f'{root}/render/torso/export_properties', properties_sha), len(hood.triangles), len(hood.vertices), bounds)
        operations += [{'op': 'add_file', 'path': f'{root}/render/hood/{name}'}
                       for name in ('converted_model', 'export_properties')]
        operations += [
            {'op': 'config_append', 'path': f'{root}/settings', 'array_path': 'lod_hierrarchy.LOD0.surfaces',
             'values': ['hood']},
            {'op': 'config_set', 'path': f'{root}/settings', 'values': {
                'material_settings.hood': {'table': {'material_name': MATERIAL, 'used_in_portals': False}}}},
        ]
    item = content_edit.walk(bytearray(read(a.live, ITEM, ITEM_SHA256)))
    bases = {path: value for path, kind, value in item if kind == content_edit.T_STR}
    for table in TABLES:
        model = bases[f'/{table}/base_model'].decode()
        if model not in MODELS:
            sys.exit(f'{ITEM}: {table} wears {model}, not one of the three models')
        operations.append({'op': 'config_append', 'path': ITEM, 'array_path': f'{table}.additional_models',
                           'values': [{'table': {'part_name': 'hood', 'base_model': model}}]})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'requires': REQUIRES,
        'description': 'decals under the STK2 loner\'s hood (1873): the hood is a surface of its own and an '
                       'additional model of the jacket, never baked; the jacket\'s torso surface keeps its '
                       'triangles as a lining under it, so a decal on the upper back is painted on the lining '
                       'and the hood covers it, as a stock vest\'s straps',
        'operations': operations,
    }
    for path, data in files.items():
        target = a.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (a.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(f'{a.out}: {len(files)} files, {len(operations)} operations')


if __name__ == '__main__':
    main()
