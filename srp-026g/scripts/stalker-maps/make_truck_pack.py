#!/usr/bin/env python3
"""make_truck_pack.py: Training Camp's GAZ-66 as a model of its own, the content pack
training-camp-truck (docs/notes/stalker-ffa.md, "The fence and its trucks").

usage: python3 -I make_truck_pack.py --pack <packs folder>/stalker-maps-6 --out <packs folder>/training-camp-truck-<version>

--pack  the stalker-maps@6 pack folder ($SKHID_PACKS_DIR/stalker-maps-6): every path of SOURCES is
        read from its files/ and must have the pinned sha256 (the live files are these).
--out   the new pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
        manifest's `pack` row (release/content.tsv). Never builds a release patch.

Training Camp is a level mesh: its one truck, the GAZ-66 standing in the south gateway, is baked into
it. Its picture is the whole of one render chunk (training_camp_104, the only surface of material
shader_0107, texture veh/veh_gaz66); its collision is spread over five of the level's collision
chunks, among the ground's and the gate's triangles. The pack's model, models/srp/training_camp_truck,
is that truck alone:

- render: chunk 104's vertices and triangles, all of them;
- collision: every triangle of those five chunks whose three corners are vertices of the picture
  (the level's collision was made of the same mesh; the two differ by a rounding of 2 micrometres
  at most, and the nearest other triangle is decimetres off), with its game material;
- both moved and turned into the truck's own frame: the origin on the ground under the middle of
  its four wheels, +z out of its nose, +y up out of its roof. Normals, tangents and binormals turn
  with it. A placement's rotation [0, a, 0] points the nose at (-sin a, 0, cos a).

The material instance and its textures are stalker-maps@6's and stay where they are.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import sys

HELPER = Path(__file__).resolve().parents[1] / 'lobby-bulb/make_pack.py'
spec = importlib.util.spec_from_file_location('bulb_mesh', HELPER)
tools = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tools)
parse, serialize, put, get = tools.parse, tools.serialize, tools.put, tools.get
T_BOOL, T_INT, T_FLOAT, T_NAMED, T_STR = tools.T_BOOL, tools.T_INT, tools.T_FLOAT, tools.T_NAMED, tools.T_STR

PACK_ID = 'training-camp-truck'
VERSION = '1'
MODEL = 'models/srp/training_camp_truck.model/'
LEVEL = 'models/ported/v010b/ported/cs/training_camp/training_camp_%03d.model/'
RENDER = LEVEL % 104
SURFACE = 'shader_0107_part_000'
COLLISION = (5, 6, 46, 54, 55)
SOURCES = {
    RENDER + 'settings': '1bf319d1c0b26f38def0ccbcf29ad33fb6ce08cbc8f2c57740f40996f5b15275',
    RENDER + 'render/export_properties': '0385ae93983f2193b6a48372e6e3a7eeaa6f530609ce0529f7cc1790af0aa182',
    RENDER + f'render/{SURFACE}/converted_model': '27038ef9a1a4741bd58594456452f4c4c7110982d8ffcdcc0bd823fce1d59085',
    RENDER + f'render/{SURFACE}/export_properties': 'ea35ef4ea2a3b6c24116b2d4ccb4dc62e245be084e2e9751ae3bd4235e6c7e38',
    LEVEL % 5 + 'settings': 'fc40b950b0d7f1d28a8b49120939912504c9a4600658f9914cf630e5b70b060c',
    LEVEL % 5 + 'collision/vertices': 'd11454d2ec6bbe05eb8e3e01179580208beaa264324119145a9a48c407b61a33',
    LEVEL % 5 + 'collision/indices': 'dcf34f1cee64b16d03d02e4480ced09ddb32816bc6f088ea20eda9edfe7b09d9',
    LEVEL % 5 + 'collision/face_data': '9e4d5d2f1151577b180685a38de3b1673cf783a3dfe41ca543c6f2871a48a147',
    LEVEL % 6 + 'settings': '0079c33d4661a0dbfeeebc1716e1823fd417ba1bcb8d3fc914c07b80007a55ef',
    LEVEL % 6 + 'collision/vertices': '5f388524754cbe78d6ea86e307ee9a9705402ae6c7c6bad299cc1f5f485eeb27',
    LEVEL % 6 + 'collision/indices': '0c899e66ff10efcc28a744924ef67998a5e63965c2d60ef757bdc3aacb2e9d9a',
    LEVEL % 6 + 'collision/face_data': '3c4b09f0ebb9e73335ee7d5fcb9a5f0f254aa3191480fe212603f693673c1066',
    LEVEL % 46 + 'settings': 'a1a58fd600bec38a8a1dcaafd3791e6a44b06d7e5312290e67921de546ee8a6c',
    LEVEL % 46 + 'collision/vertices': 'bf0a6c7bd0ac2499dff184d6814023ce551627e528862e2684ae72933e166da0',
    LEVEL % 46 + 'collision/indices': 'b92ad3a9577d2e7919a2ffd4bcb25917117dd397987cc719180952808a2448bd',
    LEVEL % 46 + 'collision/face_data': 'adf620d29a575e85e5369942f7ed7faeb0e6bee942f7fea9cbaabbd6a0857830',
    LEVEL % 54 + 'settings': '2f3383a110082bd3f9d775102ebfda92c2774ba1f31910a00b0c4053bd615a73',
    LEVEL % 54 + 'collision/vertices': '940d7fd1c62b15f9f109efc738a8e8e2cdf125708bc1b5d4e88bb9b97e4e90b4',
    LEVEL % 54 + 'collision/indices': '823c53a5333a704d5539ee4d03aa1425ac0d578e5535bfd96dccbfe1fcf4f573',
    LEVEL % 54 + 'collision/face_data': '9a18f8524cc65916a60bda1e3a3e8f5bb15e312a1af8431e264886e10a8b5e18',
    LEVEL % 55 + 'settings': 'fe874b9639a70ff7aeb973632c342f3266fcfe54247a6e8251c6bc6e68ce6bd0',
    LEVEL % 55 + 'collision/vertices': '723cbc3e472713fb47b9da47a8c049f10cf81be3b20a9604cc5cd23c4c01ca67',
    LEVEL % 55 + 'collision/indices': '8068e908785c5e6e99ad867e631c33ed5e1881dedb542d38d70df4b893ab2ed8',
    LEVEL % 55 + 'collision/face_data': '44607c00159b8c49d7eaa9e3a5333f69652cf835a7db537aa15e99e5fcda7898',
}

# The truck as the level has it (map frame), measured on its four wheels, each a connected part of
# 238 triangles: ORIGIN is the middle of their lowest points, UP the normal of the plane through
# those (the truck leans 1.9 degrees), NOSE the direction from the rear axle's middle to the front
# one's. frame() makes them orthonormal; standing() checks the result against the wheels.
ORIGIN = (23.574062, -0.997932, -30.134527)
UP = (-0.023570, 0.999465, -0.022659)
NOSE = (-0.297808, 0.014617, 0.954514)
# The middles of the wheels' flat undersides in the truck's frame: half the track, half the
# wheelbase. Each underside is within PATCH of its middle on the ground plane.
TRACK, WHEELBASE, PATCH = 1.08, 2.2, 0.55
# Chunk tags of the collision files.
VERTICES, INDICES, MATERIALS, FACES = 0x19, 0x1a, 0x1b, 0x1c
# A collision corner is a vertex of the picture when it is this near one, on every axis (metres).
SAME = 1e-4


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def unit(a):
    length = math.sqrt(dot(a, a))
    return tuple(x / length for x in a)


def frame():
    """The truck's axes in the map frame: (right, up, nose), orthonormal, right = up x nose (the
    map frame is left-handed with y up and z forward, so they are the model's +x, +y, +z)."""
    up = unit(UP)
    nose = unit(tuple(n - u * dot(NOSE, up) for n, u in zip(NOSE, up)))
    return cross(up, nose), up, nose


def to_truck(point, axes):
    """A map point in the truck's frame."""
    rel = tuple(p - o for p, o in zip(point, ORIGIN))
    return tuple(dot(rel, axis) for axis in axes)


def turned(direction, axes):
    return tuple(dot(direction, axis) for axis in axes)


def f32(values):
    """As the file holds them."""
    return struct.unpack('<%df' % len(values), struct.pack('<%df' % len(values), *values))


def chunk(data, at, tag):
    """(payload, the offset after it) of the chunk at `at`, which must be `tag`."""
    kind, size = struct.unpack_from('<II', data, at)
    assert kind == tag and at + 8 + size <= len(data), f'chunk {tag:#x} expected'
    return data[at + 8:at + 8 + size], at + 8 + size


def whole(data, tag):
    payload, end = chunk(data, 0, tag)
    assert end == len(data), 'bytes after the chunk'
    return payload


def collision_mesh(files, number):
    """A level collision chunk: (positions, triangles, each triangle's game material name)."""
    base = LEVEL % number + 'collision/'
    raw = whole(files[base + 'vertices'], VERTICES)
    positions = [struct.unpack_from('<3f', raw, k) for k in range(0, len(raw), 12)]
    raw = whole(files[base + 'indices'], INDICES)
    indices = struct.unpack('<%dI' % (len(raw) // 4), raw)
    triangles = [indices[k:k + 3] for k in range(0, len(indices), 3)]
    names, at = chunk(files[base + 'face_data'], 0, MATERIALS)
    count = struct.unpack_from('<H', names, 0)[0]
    names = [n.decode() for n in names[2:].split(b'\0')[:-1]]
    assert len(names) == count
    raw, end = chunk(files[base + 'face_data'], at, FACES)
    assert end == len(files[base + 'face_data']) and len(raw) == 2 * len(triangles)
    faces = struct.unpack('<%dH' % len(triangles), raw)
    settings = dict(get(parse(bytearray(files[LEVEL % number + 'settings'])), 'game_material_settings')[1])
    return positions, triangles, [(names[f], settings[names[f]]) for f in faces]


class Points:
    """The picture's vertices in the map frame, found by place."""

    def __init__(self):
        self.cells = {}

    @staticmethod
    def cell(point):
        return tuple(math.floor(c / SAME) for c in point)

    def add(self, point):
        self.cells.setdefault(self.cell(point), []).append(point)

    def has(self, point):
        x, y, z = self.cell(point)
        return any(all(abs(a - b) <= SAME for a, b in zip(point, other))
                   for i in (x - 1, x, x + 1) for j in (y - 1, y, y + 1) for k in (z - 1, z, z + 1)
                   for other in self.cells.get((i, j, k), ()))


def pack_normal(vector):
    """A unit vector as the vertex holds it: z, y, x bytes, 0..255 for -1..1."""
    return bytes(max(0, min(255, round((c + 1) * 127.5))) for c in reversed(vector))


def render(files, axes):
    """The picture in the truck's frame: (converted_model, positions, triangle count, its
    vertices where the level has them)."""
    vertices, indices = tools.mesh(files[RENDER + f'render/{SURFACE}/converted_model'])
    out, positions, level = [], [], Points()
    for vertex in vertices:
        point = struct.unpack_from('<3f', vertex, 0)
        level.add(point)
        record = bytearray(vertex)
        struct.pack_into('<3f', record, 0, *to_truck(point, axes))
        for at in (12, 16, 20):   # normal, tangent, binormal; the fourth byte of each stays
            vector = tuple(b / 127.5 - 1 for b in reversed(vertex[at:at + 3]))
            record[at:at + 3] = pack_normal(turned(vector, axes))
        out.append(bytes(record))
        positions.append(struct.unpack_from('<3f', record, 0))
    blob = (struct.pack('<III', 3, 4 + 28 * len(out), len(out)) + b''.join(out)
            + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices))
            + struct.pack('<%dH' % len(indices), *indices))
    return blob, positions, len(indices) // 3, level


def collision(files, axes, level):
    """The truck's triangles of the level's collision, in its frame: the three files and the
    game materials' settings."""
    points, index, triangles, materials, used = [], {}, [], [], {}
    for number in COLLISION:
        positions, chunk_triangles, chunk_materials = collision_mesh(files, number)
        for triangle, (name, setting) in zip(chunk_triangles, chunk_materials):
            corners = [positions[i] for i in triangle]
            if not all(level.has(c) for c in corners):
                continue
            assert used.setdefault(name, setting) == setting, f'{name}: two game materials'
            for corner in corners:
                if corner not in index:
                    index[corner] = len(points)
                    points.append(f32(to_truck(corner, axes)))
            triangles.append([index[c] for c in corners])
            materials.append(name)
    names = sorted(used)
    vertices = b''.join(struct.pack('<3f', *p) for p in points)
    indices = struct.pack('<%dI' % (3 * len(triangles)), *(i for t in triangles for i in t))
    dictionary = struct.pack('<H', len(names)) + b''.join(n.encode() + b'\0' for n in names)
    faces = struct.pack('<%dH' % len(materials), *(names.index(m) for m in materials))
    return {
        'vertices': struct.pack('<II', VERTICES, len(vertices)) + vertices,
        'indices': struct.pack('<II', INDICES, len(indices)) + indices,
        'face_data': (struct.pack('<II', MATERIALS, len(dictionary)) + dictionary
                      + struct.pack('<II', FACES, len(faces)) + faces),
    }, [(n, used[n]) for n in names], points, len(triangles)


def standing(positions):
    """The truck stands on its wheels at the origin: its lowest points (within 2 cm of the ground
    plane, none under it) are the four wheels' undersides, where the track and the wheelbase put
    them."""
    low = [p for p in positions if p[1] < 0.02]
    assert min(p[1] for p in positions) > -0.003, 'something under the wheels'
    wheels = [(x, z) for x in (-TRACK, TRACK) for z in (-WHEELBASE, WHEELBASE)]
    for wheel in wheels:
        assert any(math.dist((p[0], p[2]), wheel) < PATCH for p in low), f'no wheel at {wheel}'
    assert all(any(math.dist((p[0], p[2]), w) < PATCH for w in wheels) for p in low), 'not on its wheels'


def build(files):
    axes = frame()
    out = {}
    blob, positions, triangles, level = render(files, axes)
    standing(positions)
    out[MODEL + f'render/{SURFACE}/converted_model'] = blob
    low, high = tools.bounds(positions)
    centre = [(a + b) / 2 for a, b in zip(low, high)]
    radius = max(math.dist(p, centre) for p in positions)
    props = parse(bytearray(files[RENDER + f'render/{SURFACE}/export_properties']))
    assert (get(props, 'source/vertices')[1], get(props, 'source/primitives')[1]) == (len(positions), triangles)
    put(props, 'bounding_box/min', (7, low))
    put(props, 'bounding_box/max', (7, high))
    put(props, 'bounding_sphere/center', (7, centre))
    put(props, 'bounding_sphere/radius', (T_FLOAT, struct.unpack('<I', struct.pack('<f', radius))[0]))
    out[MODEL + f'render/{SURFACE}/export_properties'] = serialize(props)
    model = parse(bytearray(files[RENDER + 'render/export_properties']))
    put(model, 'bounding_box/min', (7, low))
    put(model, 'bounding_box/max', (7, high))
    out[MODEL + 'render/export_properties'] = serialize(model)

    parts, materials, points, solid = collision(files, axes, level)
    for name, data in parts.items():
        out[MODEL + 'collision/' + name] = data
    # The level chunk's settings (collision on, the walker by ray casts, one LOD0 surface of the
    # level's own material) with the game materials of the truck's collision triangles.
    settings = parse(bytearray(files[RENDER + 'settings']))
    assert 'game_material_settings' not in dict(settings[1])
    settings[1].append(('game_material_settings', (T_NAMED, materials)))
    out[MODEL + 'settings'] = serialize(settings)

    for path, data in out.items():   # read back: each config as the tree it was made from
        if path.endswith(('settings', 'export_properties')):
            assert serialize(parse(bytearray(data))) == data, path
    for path, data in files.items():   # the writer against the pack's own files: the same trees
        if path.endswith(('settings', 'export_properties')):
            assert parse(bytearray(serialize(parse(bytearray(data))))) == parse(bytearray(data)), path
    report = (f'{len(positions)} vertices, {triangles} triangles drawn; {len(points)} vertices, {solid} '
              f'triangles solid ({", ".join(n for n, _ in materials)}); '
              f'{[round(c, 3) for c in low]} .. {[round(c, 3) for c in high]}')
    return out, report


def sources(pack):
    """The pack's files the truck is made of, each with its pinned sha256."""
    files = {}
    for path, digest in SOURCES.items():
        data = (pack / 'files' / path).read_bytes()
        if sha256(data) != digest:
            raise ValueError(f"{path}: sha256 {sha256(data)}, not stalker-maps@6's {digest}")
        files[path] = data
    return files


DESCRIPTION = ("Training Camp's GAZ-66 truck as a model of its own (its picture and its collision cut out of the "
               "level mesh of stalker-maps@6, in the truck's own frame), for the trucks that close the fence's "
               "gateways in Free for All (training-camp-arena-1.json)")


def manifest(out):
    """pack.json's text. It lists operations, not contents: the files' hashes in its notes make its
    own sha256 (the pin of release/content.tsv) theirs too."""
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': DESCRIPTION,
        'operations': [{'op': 'add_file', 'path': path} for path in out],
        'notes': ['made by srp-026g/scripts/stalker-maps/make_truck_pack.py from stalker-maps@6: render chunk '
                  'training_camp_104 and collision chunks 005, 006, 046, 054 and 055; the material instance '
                  'shader_0107 and its textures are that pack\'s']
                 + [f'sha256 {sha256(data)} {path}' for path, data in out.items()],
    }
    return json.dumps(pack, ensure_ascii=False, indent=2) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--pack', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    try:
        files = sources(args.pack)
    except ValueError as e:
        sys.exit(str(e))
    out, report = build(files)
    for path, data in out.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(manifest(out), encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(report)
    print(f'{args.out}: {len(out)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{DESCRIPTION}')


if __name__ == '__main__':
    main()
