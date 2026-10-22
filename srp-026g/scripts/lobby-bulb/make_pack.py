#!/usr/bin/env python3
"""make_pack.py: the content pack of the lobby's bench bulb as a model of its own (2026-10-22;
../content-edit/README.md, lobby-scene-2.json).

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/lobby-bulb-<version>

--stock  the stock resources.db's files at their game paths (no Skhid patch carries them;
         `srp-game-data get --db <resources.db> --out <folder> <path>...`): every path of STOCK.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

The bulb over the lobby's workbench is baked into the level model `level/lobby/electric1`: its
socket is in the surface `electric_s` (material level/lobby/electric) and its glass in `wp_locker_s`
(level/lobby/glass_clone, which draws see-through). No model of the db is that bulb with that glass
(`props/light/lamp` is the same shape in one opaque surface). The pack's model,
`models/level/lobby/bench_bulb.model`, is those two surfaces' triangles of the bench bulb alone (every
triangle whose corners are all within 0.2 m of (4.85, 3.25, 2.92)), the vertices as they are (normals,
tangents, UVs) moved by -(4.920, 3.320, 2.921), the socket's top, where the bench bulb's drop wire
enters it: the model's origin is that point and the bulb keeps the bench one's tilt (50.7 degrees from
the vertical towards -x). Its settings and export properties are the stock ones' with the counts and
bounds of the new geometry; no collision (`collision_enabled` false, as `props/light/lamp`'s).
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import zlib

PACK_ID = 'lobby-bulb'
VERSION = '1'
MODEL = 'models/level/lobby/bench_bulb.model/'
E1 = 'models/level/lobby/electric1.model/'
LAMP = 'models/props/light/lamp.model/'
SURFACES = ('electric_s', 'wp_locker_s')   # the socket, the glass
STOCK = {
    E1 + 'render/electric_s/converted_model': '1e21b95f117c40d3721da0cfd93d111e132c97f06375563e51403cb728a1c730',
    E1 + 'render/electric_s/export_properties': 'b0fc9e5b6b4be4c7fec0429e428c5ae7e1a7a2b7231476243f81215acd761665',
    E1 + 'render/wp_locker_s/converted_model': '036272087efcd0c5d943f1e9e5d5c52f545c7e06cc8a72cfee4cf48b30f943ec',
    E1 + 'render/wp_locker_s/export_properties': '3923f5b8cc7e391586866d6d7b01c9af604d9f535b3fb8cb11923ced21398138',
    E1 + 'render/export_properties': 'a82d126410054c0ffb18efdd67d78898c6f811cee9912dab7513e95dd8be168b',
    E1 + 'settings': '341036cadfc7d7bef76c95533c319fa627212eaffda4fef3d4ae1676d0f5055a',
    LAMP + 'settings': '241981991d59d67f01b568dc4e252f0df59641a4f1e22580d37942c9fac3116b',
}
BULB = (4.85, 3.25, 2.92)   # the bench bulb's middle; its triangles are within 0.2 m of it
ORIGIN = (4.920, 3.320, 2.921)   # the socket's top, where the drop wire enters it

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5
REC = struct.Struct('<QQIHH')


def sha256(data):
    return hashlib.sha256(data).hexdigest()


# A binary config as a tree: a value is (type, data); a named table's data a list of (name, value),
# an indexed one's a list of values, a string's its text, a vector's its floats, a number's or a
# boolean's the record's raw u64.
def parse(buf, off=0):
    data, _, _, t, cnt = REC.unpack_from(buf, off)
    if t in (T_NAMED, T_INDEXED):
        kids = []
        for k in range(cnt):
            c = data + 24 * k
            v = parse(buf, c)
            if t == T_NAMED:
                i = REC.unpack_from(buf, c)[1]
                kids.append((bytes(buf[i:buf.index(b'\0', i)]).decode(), v))
            else:
                kids.append(v)
        return (t, kids)
    if t == T_STR:
        return (t, bytes(buf[data:data + cnt - 1]).decode())
    if t > T_STR:
        return (t, list(struct.unpack_from('<%df' % (t - 4), buf, data)))
    return (t, data)


def serialize(root):
    """The tree as a config: the records (a table's children side by side, the tables in the order
    they are reached), then the names, strings and vectors; a named table's children sorted by CRC,
    as the game's lookup needs."""
    recs, heap, strings = [], bytearray(), {}

    def text(s):
        if s not in strings:
            strings[s] = len(heap)
            heap.extend(s.encode() + b'\0')
        return strings[s]

    slots = [[root, None]]   # each: [value, name], its record's place = its index
    fixups = []              # (slot, kind, what): offsets into the heap, resolved after the records
    queue = [0]
    first = {}
    while queue:
        at = queue.pop(0)
        t, v = slots[at][0]
        if t in (T_NAMED, T_INDEXED):
            kids = v
            if t == T_NAMED:
                kids = sorted(v, key=lambda kv: zlib.crc32(kv[0].encode()) & 0xffffffff)
            first[at] = len(slots)
            for kid in kids:
                if t == T_NAMED:
                    slots.append([kid[1], kid[0]])
                else:
                    slots.append([kid, None])
                queue.append(len(slots) - 1)
    heap_base = 24 * len(slots)
    out = bytearray()
    for at, (value, name) in enumerate(slots):
        t, v = value
        name_off = heap_base + text(name) if name is not None else 0
        crc = zlib.crc32(name.encode()) & 0xffffffff if name is not None else 0
        if t in (T_NAMED, T_INDEXED):
            data, cnt = 24 * first[at], len(v)
        elif t == T_STR:
            data, cnt = heap_base + text(v), len(v.encode()) + 1
        elif t > T_STR:
            data, cnt = heap_base + len(heap), 4 * len(v)
            heap.extend(struct.pack('<%df' % len(v), *v))
        else:
            data, cnt = v, 4
        out += REC.pack(data, name_off, crc, t, cnt)
    return bytes(out + heap)


def get(tree, path):
    for part in path.split('/'):
        tree = dict(tree[1])[part]
    return tree


def put(tree, path, value):
    *head, last = path.split('/')
    for part in head:
        tree = dict(tree[1])[part]
    for k, (name, _) in enumerate(tree[1]):
        if name == last:
            tree[1][k] = (name, value)
            return
    raise KeyError(path)


def mesh(data):
    """(vertices as 28-byte records, triangle indices) of a converted_model."""
    tag, size, n = struct.unpack_from('<III', data, 0)
    assert tag == 3 and size == 4 + 28 * n, 'not a mesh of 28-byte vertices'
    verts = [data[12 + 28 * k:12 + 28 * (k + 1)] for k in range(n)]
    at = 8 + size
    tag, size, m = struct.unpack_from('<III', data, at)
    assert tag == 4 and size == 4 + 2 * m and at + 8 + size == len(data), 'not a triangle list'
    return verts, list(struct.unpack_from('<%dH' % m, data, at + 12))


def bulb_part(data):
    """The bench bulb's triangles of a surface, moved to the model's origin."""
    verts, idx = mesh(data)
    pos = [struct.unpack_from('<3f', v, 0) for v in verts]
    near = [all(abs(p[k] - BULB[k]) < 0.2 for k in range(3)) for p in pos]
    tris = [idx[k:k + 3] for k in range(0, len(idx), 3) if all(near[i] for i in idx[k:k + 3])]
    used = sorted({i for tri in tris for i in tri})
    new = {old: k for k, old in enumerate(used)}
    out_verts = []
    for i in used:
        p = [pos[i][k] - ORIGIN[k] for k in range(3)]
        out_verts.append(struct.pack('<3f', *p) + verts[i][12:])
    out_idx = [new[i] for tri in tris for i in tri]
    points = [struct.unpack_from('<3f', v, 0) for v in out_verts]
    blob = (struct.pack('<III', 3, 4 + 28 * len(out_verts), len(out_verts)) + b''.join(out_verts)
            + struct.pack('<III', 4, 4 + 2 * len(out_idx), len(out_idx)) + struct.pack('<%dH' % len(out_idx), *out_idx))
    return blob, points, len(tris)


def bounds(points):
    lo = [min(p[k] for p in points) for k in range(3)]
    hi = [max(p[k] for p in points) for k in range(3)]
    return lo, hi


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    stock = {}
    for path, sha in STOCK.items():
        data = (args.stock / path).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{path}: sha256 {sha256(data)}, not the stock file\'s {sha}')
        stock[path] = data

    files, every = {}, []
    for s in SURFACES:
        blob, points, tris = bulb_part(stock[E1 + f'render/{s}/converted_model'])
        files[MODEL + f'render/{s}/converted_model'] = blob
        every += points
        props = parse(stock[E1 + f'render/{s}/export_properties'])
        lo, hi = bounds(points)
        centre = [(lo[k] + hi[k]) / 2 for k in range(3)]
        radius = max(math.dist(p, centre) for p in points)
        put(props, 'bounding_box/min', (7, lo))
        put(props, 'bounding_box/max', (7, hi))
        put(props, 'bounding_sphere/center', (7, centre))
        put(props, 'bounding_sphere/radius', (T_FLOAT, struct.unpack('<I', struct.pack('<f', radius))[0]))
        put(props, 'source/vertices', (T_INT, len(points)))
        put(props, 'source/primitives', (T_INT, tris))
        put(props, 'source/indices', (T_INT, 3 * tris))
        files[MODEL + f'render/{s}/export_properties'] = serialize(props)
        print(f'{s}: {len(points)} vertices, {tris} triangles, {lo} .. {hi}')
    model = parse(stock[E1 + 'render/export_properties'])
    lo, hi = bounds(every)
    put(model, 'bounding_box/min', (7, lo))
    put(model, 'bounding_box/max', (7, hi))
    files[MODEL + 'render/export_properties'] = serialize(model)

    # the lamp's settings (no collision, LOD0 only) with the two surfaces and electric1's materials
    settings = parse(stock[LAMP + 'settings'])
    e1 = dict(parse(stock[E1 + 'settings'])[1])['material_settings']
    put(settings, 'lod_hierrarchy/LOD0/surfaces', (T_INDEXED, [(T_STR, s) for s in SURFACES]))
    mats = []
    for s in SURFACES:
        name = dict(get(e1, s)[1])['material_name']
        mats.append((s, (T_NAMED, [('material_name', name), ('used_in_portals', (T_BOOL, 0))])))
    put(settings, 'material_settings', (T_NAMED, mats))
    files[MODEL + 'settings'] = serialize(settings)

    for path, data in files.items():   # read back: each config as the tree it was made from
        if not path.endswith('converted_model'):
            assert serialize(parse(data)) == data, path
    for path in STOCK:   # the writer against the game's own files: the same trees
        if not path.endswith('converted_model'):
            assert parse(serialize(parse(stock[path]))) == parse(stock[path]), path
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the lobby\'s bench bulb as a model of its own (its socket and see-through glass from '
                       'level/lobby/electric1), for the second bulb of lobby-scene-2.json',
        'operations': [{'op': 'add_file', 'path': path} for path in files],
        'notes': ['made by srp-026g/scripts/lobby-bulb/make_pack.py from the stock resources.db\'s '
                  'level/lobby/electric1 and props/light/lamp'],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
