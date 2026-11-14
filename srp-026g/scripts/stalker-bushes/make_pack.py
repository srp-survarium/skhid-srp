#!/usr/bin/env python3
"""make_pack.py: the bushes of the STALKER maps as collision a foot step meets, the content pack
stalker-bushes (docs/notes/stalker-maps.md, "Bushes rustle").

usage: python3 -I make_pack.py --pack <packs folder>/stalker-maps-6 --out <packs folder>/stalker-bushes-<version>
                               [--edits <file>]

--pack   the stalker-maps@6 pack folder ($SKHID_PACKS_DIR/stalker-maps-6). Each map's inputs (its
         Team Deathmatch project, the settings and material of every chunk of its level, the
         leaves' meshes, the level's collision) must have the sha256 pinned in MAPS.
--out    the new pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv). Never builds a release patch.
--edits  also writes the edit list that places the models (content-edit/stalker-bushes-1.json).

What a step is in 0.26g: step_manager::on_step (0x248AD0) casts a ray from 1 m above the foot
straight down, 2 m long, at the collision objects of group 8 whose mask has 10h or 20h, and plays
the step sound of the pair (the foot's material, the material of the triangle met). The stock
bushes that rustle (Tarakanovsky Fort's and Vector's, models/flora/vine/bush_*) have a `collision`
mesh of game material foliage_deciduous, which the ray meets before the ground. The ported levels
have no collision for their leaves: resource-porter drops X-Ray's passable triangles.

The pack gives each map one model, models/srp/<level>_bushes, a `collision` mesh alone of game
material foliage_deciduous (44):

- its triangles are the map's leaves as they are drawn: the triangles of the level's chunks whose
  material is foliage (base material `flora`) with a texture of LEAVES (branches and needles; bark
  and stumps are solid in the level's own collision), a leaf's two drawn sides once;
- only what a step can meet: a triangle is kept when a point of it (tried every SAMPLE) is above
  a floor of the level's collision (a triangle within FLOOR_SLOPE of level: the ground, a roof, a
  branch's top) by REACH at most, and is cut off at REACH above the highest such floor. The leaves
  over a walker's head and the crowns above their branches are not in it;
- in the map's frame (the level's chunks stand at the origin, unturned).

The edit list places it as a collision object of cgroup 8 and cmask 20h, which no object of the
game has: the step's ray (group 30h, mask 8) meets it; bullets, blasts and the lobby's and the
warm-up's cameras (group 10h), grenades (818h), batteries (0A18h), the use ray (114h) and the
walkers (mask 3) do not. A spot's ray (group 0FFFFh) does: it goes on through it as through
anything hittable that is not walkable.
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
parse, serialize, get = tools.parse, tools.serialize, tools.get
T_BOOL, T_INT, T_NAMED, T_STR = tools.T_BOOL, tools.T_INT, tools.T_NAMED, tools.T_STR

PACK_ID = 'stalker-bushes'
VERSION = '1'
SOURCE_PACK = 'stalker-maps@6'
LEVELS = 'ported/v010b/ported/cs/'
# (level, project, sha256 of its inputs: digest())
MAPS = (
    ('bath', 'v010b_cs_mp_bath',
     'b18fc0201cdc3eb24fcadb12e47a963bc63621239a7f1de37ab92cb8e3bff3f1'),
    ('military_1', 'v010b_cs_mp_military_1',
     '1b130eb1e3d1864fc51377402194d112a8d5bb2360bff91ef98bc7ed70086a46'),
    ('training_camp', 'v010b_cs_training_camp',
     '94d29ef23a8e5cc1ff7091e2086069fd4bcbf3953d66ee2eb8fc2385a55adf80'),
)
# The foliage textures of the levels (the file name of texture_diffuse): what a walker goes
# through, and what stands in the level's own collision. A foliage texture in neither is refused.
LEAVES = {'trees_vetkagreen1', 'trees_vetkagreen3', 'trees_vetkagreen5', 'trees_vetkabig', 'trees_vetkasux2',
          'trees_sosna', 'trees_sosna_sux', 'trees_dub', 'trees_elka'}
SOLID = {'trees_bark_a_01', 'trees_bark_a_02', 'trees_bark_sosna_01', 'trees_bark_sosna_02', 'trees_bark_sux_mox',
         'trees_dub_bark', 'trees_spil', 'wood_collect'}
# The game material of stock's bushes (game_materials/game.materials; models/flora/vine/bush_big).
MATERIAL = ('foliage_deciduous', 44)
FACE = 'default'
# How far above a floor a step meets a leaf, metres: on_step's ray starts 1 m above the foot
# (0x248B25), the foot bone 0.2 m above the floor at most.
REACH = 1.2
# A triangle's points are tried this far apart at most, metres.
SAMPLE = 0.5
# A floor: a collision triangle whose normal is within 60 degrees of straight up.
FLOOR_SLOPE = 0.5
# The collision object: hittable (8), tried by a ray whose group has 20h (the docstring).
CGROUP, CMASK = 8, 0x20
# The floors' grid, metres.
CELL = 1.0
# Chunk tags: a drawn mesh's vertices and triangles, a collision's.
DRAWN_VERTICES, DRAWN_INDICES = 3, 4
VERTICES, INDICES, MATERIALS, FACES = 0x19, 0x1a, 0x1b, 0x1c
# The stock collision meshes go up to 64,801 triangles (models/level/04/wagons_crash).
MOST_TRIANGLES = 64801


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Pack:
    """The source pack's files, each read once; digest() is the pin of what was read."""

    def __init__(self, folder):
        self.folder = folder
        self.read = {}

    def file(self, path):
        if path not in self.read:
            self.read[path] = (self.folder / 'files' / path).read_bytes()
        return self.read[path]

    def config(self, path):
        return parse(bytearray(self.file(path)))

    def digest(self):
        lines = ''.join(f'{sha256(data)}  {path}\n' for path, data in sorted(self.read.items()))
        return sha256(lines.encode())


def value(tree, path):
    """The value at a/b/c of a config's tree, None when it has no such key."""
    try:
        return get(tree, path)[1]
    except KeyError:
        return None


def chunk(data, at, tag):
    """(payload, the offset after it) of the chunk at `at`, which must be `tag`."""
    kind, size = struct.unpack_from('<II', data, at)
    assert kind == tag and at + 8 + size <= len(data), f'chunk {tag:#x} expected'
    return data[at + 8:at + 8 + size], at + 8 + size


def whole(data, tag):
    payload, end = chunk(data, 0, tag)
    assert end == len(data), 'bytes after the chunk'
    return payload


def at_origin(entry):
    """A project entry that stands at the origin, unturned and unscaled."""
    return (value(entry, 'position') == [0.0, 0.0, 0.0] and value(entry, 'rotation') == [0.0, 0.0, 0.0]
            and value(entry, 'scale') == [1.0, 1.0, 1.0])


def collision_triangles(pack, lib_name):
    """The triangles (three points each) of a level chunk's collision."""
    base = lib_name.removeprefix('resources/') + '/'
    raw = whole(pack.file(base + 'vertices'), VERTICES)
    points = [struct.unpack_from('<3f', raw, k) for k in range(0, len(raw), 12)]
    raw = whole(pack.file(base + 'indices'), INDICES)
    indices = struct.unpack('<%dI' % (len(raw) // 4), raw)
    return [(points[indices[k]], points[indices[k + 1]], points[indices[k + 2]]) for k in range(0, len(indices), 3)]


def drawn_triangles(data):
    """The triangles (three points each) of a drawn surface (converted_model)."""
    tag, size, count = struct.unpack_from('<III', data, 0)
    assert tag == DRAWN_VERTICES and count and (size - 4) % count == 0, 'not a drawn mesh'
    stride = (size - 4) // count
    points = [struct.unpack_from('<3f', data, 12 + stride * k) for k in range(count)]
    at = 8 + size
    tag, size, count = struct.unpack_from('<III', data, at)
    assert tag == DRAWN_INDICES and size == 4 + 2 * count and at + 8 + size == len(data), 'not a triangle list'
    indices = struct.unpack_from('<%dH' % count, data, at + 12)
    return [(points[indices[k]], points[indices[k + 1]], points[indices[k + 2]]) for k in range(0, count, 3)]


class Floors:
    """The level's floors: its collision triangles within FLOOR_SLOPE of level, by the cells of a
    grid over x and z."""

    def __init__(self):
        self.cells = {}
        self.low = [math.inf] * 3
        self.high = [-math.inf] * 3

    def add(self, triangle):
        a, b, c = triangle
        for point in triangle:              # the box of all the collision, floors or not
            for k in range(3):
                self.low[k] = min(self.low[k], point[k])
                self.high[k] = max(self.high[k], point[k])
        nx = (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1])
        ny = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
        nz = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        length = math.sqrt(nx * nx + ny * ny + nz * nz)
        # d: twice the triangle's area seen from above; a floor has some.
        d = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2])
        if length == 0 or abs(ny) < FLOOR_SLOPE * length or d == 0:
            return
        floor = (a, b, c, d, min(a[1], b[1], c[1]), max(a[1], b[1], c[1]))
        for i in range(math.floor(min(a[0], b[0], c[0]) / CELL), math.floor(max(a[0], b[0], c[0]) / CELL) + 1):
            for j in range(math.floor(min(a[2], b[2], c[2]) / CELL), math.floor(max(a[2], b[2], c[2]) / CELL) + 1):
                self.cells.setdefault((i, j), []).append(floor)

    def under(self, point):
        """The highest floor under `point` within REACH of it, or None."""
        x, y, z = point
        best = None
        for a, b, c, d, low, high in self.cells.get((math.floor(x / CELL), math.floor(z / CELL)), ()):
            if low >= y or high < y - REACH:
                continue
            u = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / d
            v = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / d
            w = 1 - u - v
            if u < 0 or v < 0 or w < 0:
                continue
            height = u * a[1] + v * b[1] + w * c[1]
            if y - REACH <= height < y and (best is None or height > best):
                best = height
        return best


def samples(triangle):
    """Points of a triangle no further apart than SAMPLE, its corners among them."""
    a, b, c = triangle
    n = max(1, math.ceil(max(math.dist(a, b), math.dist(b, c), math.dist(c, a)) / SAMPLE))
    for i in range(n + 1):
        for j in range(n + 1 - i):
            k = n - i - j
            yield tuple((i * a[m] + j * b[m] + k * c[m]) / n for m in range(3))


def below(triangle, top):
    """The part of a triangle at or under the height `top`: 0, 1 or 2 triangles."""
    inside = [p for p in triangle if p[1] <= top]
    if len(inside) == 3:
        return [triangle]
    if not inside:
        return []
    polygon = []
    for k in range(3):
        p, q = triangle[k], triangle[(k + 1) % 3]
        if p[1] <= top:
            polygon.append(p)
        if (p[1] <= top) != (q[1] <= top):
            t = (top - p[1]) / (q[1] - p[1])
            polygon.append((p[0] + t * (q[0] - p[0]), top, p[2] + t * (q[2] - p[2])))
    return [(polygon[0], polygon[k], polygon[k + 1]) for k in range(1, len(polygon) - 1)]


def f32(point):
    """A point as the file holds it."""
    return struct.unpack('<3f', struct.pack('<3f', *point))


def area(triangle):
    a, b, c = triangle
    u = [b[k] - a[k] for k in range(3)]
    v = [c[k] - a[k] for k in range(3)]
    n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
    return math.sqrt(sum(x * x for x in n)) / 2


def level_chunks(project, level, table):
    """The entries of a project's table (render_visuals, collision_objects) that are the level's own
    chunks, each with its model's path; they stand at the origin."""
    solid = table == 'collision_objects'
    prefix, suffix = (('resources/models/', '.model/collision') if solid else ('', ''))
    prefix += f'{LEVELS}{level}/{level}_'
    out = []
    for entry in get(project, table)[1]:
        name = value(entry, 'lib_name')
        if name.startswith(prefix) and name.endswith(suffix) and name[len(prefix):len(name) - len(suffix)].isdigit():
            assert at_origin(entry), f'{name}: not at the origin'
            assert solid or value(entry, 'visual_name') == name, f'{name}: another visual'
            out.append((entry, 'models/' + name.removeprefix('resources/models/').removesuffix(suffix) + '.model'))
    assert out, f'{table}: no chunk of {level}'
    return out


def leaves(pack, project, level):
    """The level's leaves: the triangles of its drawn chunks whose material is foliage with a
    texture of LEAVES; and the triangles drawn of each such texture."""
    triangles, drawn = [], {}
    for _, model in level_chunks(project, level, 'render_visuals'):
        settings = pack.config(model + '/settings')
        for surface in (s[1] for s in get(settings, 'lod_hierrarchy/LOD0/surfaces')[1]):
            name = value(settings, f'material_settings/{surface}/material_name')
            if not name:
                continue
            material = pack.config(f'material_instances/{name}.material')
            if value(material, 'material/instance/base_material') != 'flora':
                continue
            texture = value(material, 'material/g_stage/effect/texture_diffuse/value').rsplit('/', 1)[-1]
            if texture in SOLID:
                continue
            assert texture in LEAVES, f'{model}: foliage of texture {texture}: leaves or solid?'
            mesh = drawn_triangles(pack.file(f'{model}/render/{surface}/converted_model'))
            triangles += mesh
            drawn[texture] = drawn.get(texture, 0) + len(mesh)
    return triangles, drawn


def bushes(pack, level, project_name):
    """A map's model: its files, the lib_names of the level's first and last collision chunks (the
    edit list's `like` and `after`) and a report."""
    project = pack.config(f'projects/{project_name}/day_4/client_project')
    floors = Floors()
    solid = level_chunks(project, level, 'collision_objects')
    for entry, _ in solid:
        for triangle in collision_triangles(pack, value(entry, 'lib_name')):
            floors.add(triangle)
    drawn, by_texture = leaves(pack, project, level)

    points, index, triangles, kept, seen = [], {}, [], 0, set()
    for triangle in drawn:
        # The level draws a leaf's two sides as two triangles; the ray meets either side of one.
        if frozenset(triangle) in seen:
            continue
        seen.add(frozenset(triangle))
        tops = [t for t in map(floors.under, samples(triangle)) if t is not None]
        if not tops:
            continue
        kept += 1
        for part in below(triangle, max(tops) + REACH):
            part = tuple(f32(p) for p in part)
            if area(part) == 0:
                continue
            for corner in part:
                if corner not in index:
                    index[corner] = len(points)
                    points.append(corner)
            triangles.append(tuple(index[corner] for corner in part))
    assert 0 < len(triangles) <= MOST_TRIANGLES, f'{level}: {len(triangles)} triangles'
    low, high = tools.bounds(points)
    # Inside the level's collision: the physics world's box, which the level's collision makes
    # (bullet_physics_world::add 0x758E1, groups 8 and 2), stays as it is.
    assert all(floors.low[k] <= low[k] and high[k] <= floors.high[k] for k in range(3)), f'{level}: out of the level'

    vertices = b''.join(struct.pack('<3f', *p) for p in points)
    indices = struct.pack('<%dI' % (3 * len(triangles)), *(i for t in triangles for i in t))
    dictionary = struct.pack('<H', 1) + FACE.encode() + b'\0'
    faces = bytes(2 * len(triangles))
    model = f'models/srp/{level}_bushes.model/'
    files = {
        model + 'collision/vertices': struct.pack('<II', VERTICES, len(vertices)) + vertices,
        model + 'collision/indices': struct.pack('<II', INDICES, len(indices)) + indices,
        model + 'collision/face_data': (struct.pack('<II', MATERIALS, len(dictionary)) + dictionary
                                        + struct.pack('<II', FACES, len(faces)) + faces),
        model + 'settings': settings(),
    }
    spread = sum(area(tuple(points[i] for i in t)) for t in triangles)
    report = (f'{level}: {len(drawn)} leaf triangles drawn ('
              + ', '.join(f'{t} {n}' for t, n in sorted(by_texture.items()))
              + f'), {len(seen)} leaves, {kept} within a step of a floor; '
              f'{len(points)} vertices, {len(triangles)} triangles, {spread:.0f} m2; '
              f'{[round(c, 2) for c in low]} .. {[round(c, 2) for c in high]}')
    return files, value(solid[0][0], 'lib_name'), value(solid[-1][0], 'lib_name'), report


def settings():
    """A model of collision alone, as stock's bushes with one are set (models/flora/vine/bush_big)
    without their surfaces: every triangle of game material MATERIAL."""
    tree = (T_NAMED, [
        ('collision_enabled', (T_BOOL, 1)),
        ('walker_collision_enabled', (T_BOOL, 0)),
        ('collision_use_raytrace_for_walker', (T_BOOL, 0)),
        ('calculate_ao', (T_BOOL, 0)),
        ('lod_hierrarchy', (T_NAMED, [(name, (T_NAMED, [])) for name in ('unassigned', 'LOD0', 'LOD1', 'LOD2')])),
        ('material_settings', (T_NAMED, [])),
        ('game_material_settings', (T_NAMED, [(FACE, (T_NAMED, [
            ('game_material_id', (T_INT, MATERIAL[1])),
            ('game_material_name', (T_STR, MATERIAL[0])),
        ]))])),
        ('lod_switching', (T_NAMED, [('default_params', (T_BOOL, 1)), ('type', (T_INT, 0))])),
    ])
    data = serialize(tree)
    assert serialize(parse(bytearray(data))) == data
    return data


def edit_list(placed):
    """The edit list: each map's model as a collision object of its Team Deathmatch project, after
    the level's own chunks and placed as they are (level_chunks: at the origin, unturned)."""
    return {
        '_about': ("The STALKER maps' bushes rustle (the owner, 2026-11-14: \"bushes should have rustling sound just "
                   "like on normal survarium maps\"): each map's leaves within a step's reach of a floor, "
                   f"models/srp/<level>_bushes of pack {PACK_ID}@{VERSION} (game material {MATERIAL[0]}, as stock's "
                   "bushes), a collision object of its Team Deathmatch project, which Free for All's project is a "
                   f"copy of (ffa-projects-1, after this list). cgroup {CGROUP}, cmask {CMASK}: the step's ray (group "
                   "30h, mask 8) meets it and plays the foliage step; bullets, grenades, cameras and walkers do not "
                   "(docs/notes/stalker-maps.md, \"Bushes rustle\"). Made by scripts/stalker-bushes/make_pack.py "
                   "--edits."),
        'add_entry': [{
            'files': f'projects/{project}/day_4/client_project',
            'list': 'collision_objects',
            'after': {'lib_name': last},
            'like': {'lib_name': first},
            'values': {'lib_name': f'resources/models/srp/{level}_bushes.model/collision', 'cgroup': CGROUP,
                       'cmask': CMASK},
        } for level, project, first, last in placed],
    }


DESCRIPTION = ("the bushes of Experimental Pool, Military Warehouses and Training Camp as collision a foot step "
               f"meets: models/srp/<level>_bushes, a collision mesh alone of game material {MATERIAL[0]} (stock's "
               f"bushes'), the leaves {SOURCE_PACK} draws (the level chunks' foliage surfaces of branch and needle "
               f"textures) within {REACH:g} m above a floor of the level's collision, cut off there; placed by "
               "stalker-bushes-1.json")


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
        'notes': [f'made by srp-026g/scripts/stalker-bushes/make_pack.py from {SOURCE_PACK}']
                 + [f'sha256 {sha256(data)} {path}' for path, data in out.items()],
    }
    return json.dumps(pack, ensure_ascii=False, indent=2) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--pack', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--edits', type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    out, placed, stale = {}, [], []
    for level, project, pin in MAPS:
        pack = Pack(args.pack)
        files, first, last, report = bushes(pack, level, project)
        if pack.digest() != pin:
            stale.append(f"{level}: its inputs' sha256 is {pack.digest()}, not {SOURCE_PACK}'s {pin}")
        out.update(files)
        placed.append((level, project, first, last))
        print(report)
    if stale:
        sys.exit('\n'.join(stale))
    for path, data in out.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(manifest(out), encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(out)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{DESCRIPTION}')
    if args.edits:
        args.edits.write_text(json.dumps(edit_list(placed), ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(f'{args.edits}: {len(placed)} collision objects')


if __name__ == '__main__':
    main()
