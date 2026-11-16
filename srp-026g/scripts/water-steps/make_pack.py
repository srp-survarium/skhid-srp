#!/usr/bin/env python3
"""make_pack.py: every map's drawn water as collision only a step's water look meets, the content
pack water-steps and its edit list (docs/notes/footsteps.md, "Wading").

usage: python3 -I make_pack.py --extract --archives <game folder> --vostok-port <binary> --inputs <folder>
       python3 -I make_pack.py --inputs <folder> --out <packs folder>/water-steps-<version> [--edits <file>]
       python3 -I make_pack.py --manifest release/content.tsv

--extract   gathers the inputs: from the released game's archives (resources.db and its patches, the
            newest copy of each path) reads every project (projects/<level>/<name>/client_project)
            and the settings of every model it draws; puts the projects that draw water, the
            settings of their water models and the water surfaces at their archive paths under
            --inputs, and prints their sha256 lines (inputs.sha256 here pins them). The released
            game's files are the manifest's inputs and the rows above this list as they stand: no
            row after rev61's changes a project's render visuals or collision objects.
--inputs    the gathered files; each must have its inputs.sha256 hash, and every file of the list
            must be there. A project's model whose settings are not in the list draws no water.
--out       the new pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
            manifest's `pack` row (release/content.tsv). Its pack.json must be PACK_JSON's.
--edits     also writes the edit list that places the models (content-edit/water-steps-1.json).
--manifest  makes nothing: the manifest has this script's pack, and its edit list after Free for
            All's copies (it edits those projects as they are).

What water is: a surface a project draws (render_visuals, the model's LOD0 surfaces) whose material
is a water one (material_instances/water/*, and the ported builds' ported/<build>/water/*). The
generic rule, no list of maps: every live project that draws water gets one model of the triangles
its water draws, placed as the project places them (scale, then position; a turned water visual
stops the script), facing up or down within UPRIGHT of vertical (the walls of a box of water are
not a surface), each once (two drawn sides are one).

The model, models/srp/water/<level>[_<n>].model, is a `collision` mesh alone of game material
deep_water (32), placed at the origin as a collision object of cgroup and cmask 1000h: no object,
body or ray of the game has that bit either way, so bullets, blasts, grenades, batteries, walkers,
dead bodies, the use ray and a spot's ray pass as before (a spot's ray meets cgroup & 80Ah), and
the physics world's box (groups & 0Ah, bullet_physics_world::add 0x758B0) is as it was. Only
patch 153's water look (vostok/step.rs, Look::Water) meets it. Projects of a level that draw the
same water share a model.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
HERE = Path(__file__).resolve().parent


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


tools = module('bulb_mesh', SCRIPTS / 'lobby-bulb/make_pack.py')
# The edit lists' own tool: the list is tried with what build-content applies it with.
content_edit = module('content_edit_for_water', SCRIPTS / 'content-edit/content_edit.py')
parse, serialize, get = tools.parse, tools.serialize, tools.get
T_BOOL, T_INT, T_NAMED, T_STR = tools.T_BOOL, tools.T_INT, tools.T_NAMED, tools.T_STR

PACK_ID = 'water-steps'
VERSION = '1'
# The pack.json this script makes: the pin of the manifest's row (release/content.tsv).
PACK_JSON = 'f333969fa38f1bc3de63c6fb050f6f34709ac898cded75db3857d105b1f310de'
EDITS = 'water-steps-1.json'
# The row the list comes after: Free for All's projects are copies of Team Deathmatch's it makes.
COPIES = 'ffa-projects-1.json'
PINS = HERE / 'inputs.sha256'
PROJECT = re.compile(r'projects/([^/]+)/[^/]+/client_project$')
WATER = re.compile(r'(ported/[^/]+/)?water/')
# The game material of the model's faces (game_materials/game.materials).
MATERIAL = ('deep_water', 32)
FACE = 'default'
CGROUP = CMASK = 0x1000
# A surface: a triangle whose normal is within 60 degrees of straight up or down.
UPRIGHT = 0.5
DRAWN_VERTICES, DRAWN_INDICES = 3, 4
VERTICES, INDICES, MATERIALS, FACES = 0x19, 0x1a, 0x1b, 0x1c
MOST_TRIANGLES = 64801


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def value(tree, path):
    try:
        return get(tree, path)[1]
    except KeyError:
        return None


def entries(tree, path):
    """The entries of an indexed table, each as a {key: value} of its own keys."""
    found = value(tree, path) or []
    return [{name: kid[1] for name, kid in entry[1]} for entry in found]


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


def model_path(lib_name):
    return f'models/{lib_name}.model'


def water_surfaces(settings):
    """The LOD0 surfaces of a model's settings whose material is water: [(surface, material)]."""
    lod0 = value(settings, 'lod_hierrarchy/LOD0/surfaces') or []
    found = []
    for _, surface in lod0:
        material = value(settings, f'material_settings/{surface}/material_name')
        if material and WATER.match(material):
            found.append((surface, material))
    return found


class Inputs:
    """The gathered files, each read once and checked against its pin."""

    def __init__(self, folder, pins):
        self.folder, self.pins = folder, pins

    def has(self, path):
        return path in self.pins

    def file(self, path):
        data = (self.folder / path).read_bytes()
        if self.pins.get(path) != sha256(data):
            raise SystemExit(f'{path}: sha256 {sha256(data)}, inputs.sha256 has {self.pins.get(path)}')
        return data

    def config(self, path):
        return parse(bytearray(self.file(path)))


def project_water(inputs, project_path, needed=None):
    """The water a project draws: [(model, surface, material, placed triangles)]. `needed` collects
    the water's files (for --extract)."""
    project = inputs.config(project_path)
    water, settings_of = [], {}
    for visual in entries(project, 'render_visuals'):
        lib_name = visual.get('lib_name')
        if not lib_name:
            continue
        model = model_path(lib_name)
        if model not in settings_of:
            settings_of[model] = inputs.config(model + '/settings') if inputs.has(model + '/settings') else None
        settings = settings_of[model]
        if settings is None:
            continue
        for surface, material in water_surfaces(settings):
            path = f'{model}/render/{surface}/converted_model'
            if needed is not None:
                needed += [project_path, model + '/settings', path]
            if visual.get('rotation', [0.0, 0.0, 0.0]) != [0.0, 0.0, 0.0]:
                raise SystemExit(f'{project_path}: {lib_name} draws water turned; no rule for that')
            scale = visual.get('scale', [1.0, 1.0, 1.0])
            position = visual.get('position', [0.0, 0.0, 0.0])
            placed = [tuple(tuple(p[k] * scale[k] + position[k] for k in range(3)) for p in triangle)
                      for triangle in drawn_triangles(inputs.file(path))]
            water.append((lib_name, surface, material, placed))
    return water


def normal(triangle):
    a, b, c = triangle
    u = [b[k] - a[k] for k in range(3)]
    v = [c[k] - a[k] for k in range(3)]
    return (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])


def surface_triangles(water):
    """The triangles of a project's water that are a surface, each once, in a stable order."""
    kept, seen = [], set()
    for _, _, _, placed in water:
        for triangle in placed:
            triangle = tuple(tuple(struct.unpack('<3f', struct.pack('<3f', *p))) for p in triangle)
            n = normal(triangle)
            length = math.sqrt(sum(x * x for x in n))
            if length == 0 or abs(n[1]) < UPRIGHT * length:
                continue
            if frozenset(triangle) in seen:
                continue
            seen.add(frozenset(triangle))
            kept.append(triangle)
    return kept


def settings():
    """A model of collision alone, as stock's bushes with one are set, its one face deep_water."""
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


def collision_files(model, triangles):
    points, index, faces = [], {}, []
    for triangle in triangles:
        for corner in triangle:
            if corner not in index:
                index[corner] = len(points)
                points.append(corner)
        faces.append(tuple(index[corner] for corner in triangle))
    assert 0 < len(faces) <= MOST_TRIANGLES, f'{model}: {len(faces)} triangles'
    vertices = b''.join(struct.pack('<3f', *p) for p in points)
    indices = struct.pack('<%dI' % (3 * len(faces)), *(i for t in faces for i in t))
    dictionary = struct.pack('<H', 1) + FACE.encode() + b'\0'
    materials = bytes(2 * len(faces))
    return {
        model + '/collision/vertices': struct.pack('<II', VERTICES, len(vertices)) + vertices,
        model + '/collision/indices': struct.pack('<II', INDICES, len(indices)) + indices,
        model + '/collision/face_data': (struct.pack('<II', MATERIALS, len(dictionary)) + dictionary
                                         + struct.pack('<II', FACES, len(materials)) + materials),
        model + '/settings': settings(),
    }


def anchor(project, project_path):
    """The lib_name of the project's first collision object no other has: the edit's `after` and
    `like` (the new object's position, rotation and scale are set, not copied)."""
    names = [entry.get('lib_name') for entry in entries(project, 'collision_objects')]
    for name in names:
        if name and names.count(name) == 1:
            return name
    raise SystemExit(f'{project_path}: no collision object with a lib_name of its own')


def placement(model):
    return {'lib_name': f'resources/{model}/collision', 'cgroup': CGROUP, 'cmask': CMASK,
            'position': [0.0, 0.0, 0.0], 'rotation': [0.0, 0.0, 0.0], 'scale': [1.0, 1.0, 1.0]}


def addition(project_path, model, after):
    return {'files': project_path, 'list': 'collision_objects', 'after': {'lib_name': after},
            'like': {'lib_name': after}, 'values': placement(model), 'expected_files': 1}


def place(inputs, edit):
    """The edit applied as build-content applies it: the project read back with that one object
    more, after its anchor, the others as they were."""
    data = bytearray(inputs.file(edit['files']))
    before = get(parse(bytearray(data)), 'collision_objects')[1]
    at = content_edit.add_entry(data, edit['list'], edit['after'], edit['like'], edit['values'])
    after = get(parse(data), 'collision_objects')[1]
    assert at is not None and after[:at] + after[at + 1:] == before, f"{edit['files']}: not placed"
    added = {key: entry[1] for key, entry in after[at][1]}
    assert all(added[key] == want for key, want in edit['values'].items()), f"{edit['files']}: not as placed"


DESCRIPTION = ("every map's drawn water as collision a step's water look meets (exe patch 153): "
               "models/srp/water/<level>, a collision mesh alone of game material deep_water, the triangles a project "
               "draws with a water material, placed as it draws them; cgroup and cmask 1000h, which nothing else of the "
               f"game meets; placed by {EDITS}")


def manifest(out):
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': DESCRIPTION,
        'operations': [{'op': 'add_file', 'path': path} for path in out],
        'notes': ['made by srp-026g/scripts/water-steps/make_pack.py from the released game (inputs.sha256)']
                 + [f'sha256 {sha256(data)} {path}' for path, data in out.items()],
    }
    return json.dumps(pack, ensure_ascii=False, indent=2) + '\n'


def edit_list(edits):
    return {
        '_about': ("A step in water wades (the owner, 2026-11-16, of Rudnya's pond: \"I want them there\"): each "
                   f"project that draws water gets models/srp/water/<level> of pack {PACK_ID}@{VERSION} (its drawn "
                   "water, game material deep_water) as a collision object at the origin, cgroup and cmask 1000h: "
                   "patch 153's water look alone meets it (docs/notes/footsteps.md, \"Wading\"). After Free for "
                   f"All's copies ({COPIES}): their projects are edited as they are. expected_files: a project the "
                   "entry cannot be placed in stops the build. Made by scripts/water-steps/make_pack.py --edits."),
        'add_entry': edits,
    }


def read_pins():
    pins = {}
    for line in PINS.read_text(encoding='utf-8').splitlines():
        if line:
            digest, path = line.split('  ', 1)
            pins[path] = digest
    return pins


def extract(archives, binary, folder):
    """The newest copy of every input under `folder`, then the pins' lines."""
    layers = sorted(archives.glob('resources.db.patch_*'), key=lambda p: int(p.name.rsplit('_', 1)[1]))
    layers = [archives / 'resources.db'] + layers
    newest = {}
    for layer in layers:
        listed = subprocess.run([binary, 'list', '--db', str(layer)], check=True, capture_output=True, text=True)
        for path in listed.stdout.splitlines():
            newest[path.lower()] = (path, layer)

    def fetch(path):
        found = newest.get(path.lower())
        if found is None:
            raise FileNotFoundError(path)
        target = folder / path
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run([binary, 'extract', '--db', str(found[1]), '--path', found[0], '--output', str(target)],
                           check=True, capture_output=True)
        return target.read_bytes()

    class Fetching(Inputs):
        def has(self, path):
            return path.lower() in newest

        def file(self, path):
            return fetch(path)

    projects = sorted(path for path, _ in newest.values() if PROJECT.match(path))
    needed = []
    for project_path in projects:
        project_water(Fetching(folder, {}), project_path, needed)
    for path in sorted(set(needed)):
        print(f'{sha256((folder / path).read_bytes())}  {path}')


def make(inputs, out_folder, edits_path):
    projects = sorted(path for path in inputs.pins if PROJECT.match(path))
    shared, out, edits = {}, {}, []
    for project_path in projects:
        water = project_water(inputs, project_path)
        if not water:
            continue
        triangles = surface_triangles(water)
        if not triangles:
            raise SystemExit(f'{project_path}: draws water, none of it a surface')
        level = PROJECT.match(project_path).group(1)
        key = (level, tuple(triangles))
        if key not in shared:
            count = sum(1 for other in shared if other[0] == level)
            model = f'models/srp/water/{level}' + (f'_{count + 1}' if count else '') + '.model'
            shared[key] = model
            out.update(collision_files(model, triangles))
            area = sum(math.sqrt(sum(x * x for x in normal(t))) / 2 for t in triangles)
            low = [min(p[k] for t in triangles for p in t) for k in range(3)]
            high = [max(p[k] for t in triangles for p in t) for k in range(3)]
            drawn = ', '.join(sorted({f'{lib}/{surface} ({material})' for lib, surface, material, _ in water}))
            print(f'{model}: {len(triangles)} triangles, {area:.0f} m2, {[round(c, 2) for c in low]} .. '
                  f'{[round(c, 2) for c in high]}; {drawn}')
        model = shared[key]
        edit = addition(project_path, model, anchor(inputs.config(project_path), project_path))
        place(inputs, edit)
        edits.append(edit)
    for path, data in out.items():
        target = out_folder / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (out_folder / 'pack.json').write_text(manifest(out), encoding='utf-8')
    digest = sha256((out_folder / 'pack.json').read_bytes())
    print(f'{out_folder}: {len(out)} files for {len(edits)} projects, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{DESCRIPTION}')
    if edits_path:
        edits_path.write_text(json.dumps(edit_list(edits), ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(f'{edits_path}: {len(edits)} collision objects')
    if digest != PACK_JSON:
        sys.exit(f'pack.json has sha256 {digest}, PACK_JSON pins {PACK_JSON or "nothing"}')


def manifest_problems(manifest_path):
    lines = manifest_path.read_text(encoding='utf-8').splitlines()
    listed = [tuple(line.split('\t')[:3]) for line in lines if line and not line.startswith('#')]
    problems = []
    packs = {name.split('@')[0]: (name, source) for kind, name, source in listed if kind == 'pack'}
    if packs.get(PACK_ID) != (f'{PACK_ID}@{VERSION}', PACK_JSON):
        problems.append(f'no pack row {PACK_ID}@{VERSION} {PACK_JSON}')
    edits = [name for kind, name, _ in listed if kind == 'edit']
    if EDITS not in edits or COPIES not in edits or edits.index(EDITS) < edits.index(COPIES):
        problems.append(f'no edit row {EDITS} after {COPIES}')
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--extract', action='store_true')
    parser.add_argument('--archives', type=Path)
    parser.add_argument('--vostok-port')
    parser.add_argument('--inputs', type=Path)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--edits', type=Path)
    parser.add_argument('--manifest', type=Path)
    args = parser.parse_args()
    if args.manifest:
        problems = manifest_problems(args.manifest)
        if problems:
            sys.exit(f'{args.manifest}: ' + '; '.join(problems))
        print(f'{args.manifest}: {PACK_ID}@{VERSION}, {EDITS} after {COPIES}')
        return
    if args.extract:
        if not (args.archives and args.vostok_port and args.inputs):
            parser.error('--extract takes --archives, --vostok-port and --inputs')
        extract(args.archives, args.vostok_port, args.inputs)
        return
    if not (args.inputs and args.out):
        parser.error('--inputs and --out, --extract, or --manifest')
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    pins = read_pins()
    missing = [path for path in pins if not (args.inputs / path).exists()]
    if missing:
        sys.exit(f'{args.inputs}: {len(missing)} inputs missing, first {missing[0]}')
    make(Inputs(args.inputs, pins), args.out, args.edits)


if __name__ == '__main__':
    main()
