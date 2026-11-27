#!/usr/bin/env python3
"""Build kitchen-ware@1 from the maps' small kitchen things, for release/content.tsv.

--stock contains the models' files at their archive paths (srp-game-data get
with the stock db and every listed patch; every path of stock.json). --out is
a new resource-porter pack directory. Never builds a release patch.

The owner (2026-11-27, "shoot the can"): a shot throws the maps' small
kitchen things off, and they are kicked about after, as the skeleton's cap
(docs/notes/breakable_lamps.md "Loose pieces"). Each source model makes,
beside it:

- WARE (tin cans, teapots, wash-stand tanks, a pan): srp_<name>_loose, the
  model whole, every surface and LOD moved so its origin is the centre of
  its LOD0 bounds (where the flying body is);
- CERAMIC (Rudnya's clay jugs and pot, the bottle's surface's material):
  srp_<name>_shard<k>, SHARDS shards of its LOD0 surface as the bottle's
  (scripts/bottle-shards: cut to 2 cm triangles, two-sided, each about its
  centre), its seeds spread over the surface (far_seeds); a jug's at the
  scale the maps place it (SCALE).

Bounds and counts are recomputed and collision is disabled in every model:
the map's own copy (its collision objects taken out by kitchen-ware-1.json)
is what a shot meets.
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
cap_pack = load('cap_pack', 'skeleton-cap/make_pack.py')
bottle = load('bottle_pack', 'bottle-shards/make_pack.py')
parse, serialize, put = mesh_tools.parse, mesh_tools.serialize, mesh_tools.put

# The models the maps place (kitchen-ware-1.json's), by their model name.
WARE = ['props/kitchen/can', 'ported/v021d/props/kitchen/can', 'ported/v011e/props/kitchen/can',
        'props/kitchen/teapot', 'ported/v021d/props/kitchen/teapot', 'ported/v011e/props/kitchen/teapot',
        'ported/v032a/props/kitchen/teapot', 'ported/v044a/props/kitchen/teapot',
        'props/kitchen/woter_tank', 'ported/v021d/props/kitchen/woter_tank', 'v069d_rudnya/props/kitchen/woter_tank',
        'ported/v011e/props/kitchen/pan_cooker_01']
CERAMIC = ['props/kitchen/pitcher_01', 'ported/v021d/props/kitchen/pitcher_01', 'v069d_rudnya/props/kitchen/pitcher_01',
           'props/kitchen/pot', 'ported/v021d/props/kitchen/pot']
SHARDS = 8
# The scale every placement of a ceramic thing has (Rudnya's jugs are all
# placed at 1.225): its shards are cut from the mesh at that scale, and
# placed unscaled (a loose piece's seat is rigid).
SCALE = {'props/kitchen/pitcher_01': 1.225, 'ported/v021d/props/kitchen/pitcher_01': 1.225,
         'v069d_rudnya/props/kitchen/pitcher_01': 1.225}


def renamed(name, suffix):
    folder, base = name.rsplit('/', 1)
    return f'{folder}/srp_{base}_{suffix}'


def loose_name(name):
    return renamed(name, 'loose')


def shard_names(name):
    return [renamed(name, f'shard{k}') for k in range(SHARDS)]


def model_dir(name):
    return f'models/{name}.model/'


def lod0(settings):
    """The LOD0 surfaces' names."""
    lods = dict(settings[1])['lod_hierrarchy'][1]
    return [name for lod, (_, table) in lods if lod == 'LOD0' for key, (_, names) in table if key == 'surfaces'
            for _, name in names]


def parts(settings):
    return ['settings', 'render/export_properties'] + [
        f'render/{s}/{part}' for s in lamp_pack.surfaces(settings) for part in ('converted_model', 'export_properties')]


def originals_of(stock, name, hashes):
    root = stock / model_dir(name)
    settings = parse(bytearray((root / 'settings').read_bytes()))
    files = {part: (root / part).read_bytes() for part in parts(settings)}
    for part, data in files.items():
        assert hashlib.sha256(data).hexdigest() == hashes[model_dir(name) + part], model_dir(name) + part
    return files


def points_of(data):
    vertices, _ = mesh_tools.mesh(data)
    return [struct.unpack_from('<3f', v) for v in vertices]


def centre(originals):
    """The centre of the model's LOD0 bounds, in its frame: a loose copy's origin."""
    settings = parse(bytearray(originals['settings']))
    low, high = mesh_tools.bounds([p for s in lod0(settings) for p in points_of(originals[f'render/{s}/converted_model'])])
    return [(lo + hi) / 2 for lo, hi in zip(low, high)]


def shape(originals):
    """Its flying body's capsule about its centre: along its LOD0 bounds'
    longest side (the axis), its radius the mean of the other two half
    sides, its half the rest of the longest's half: (half, radius, axis)."""
    settings = parse(bytearray(originals['settings']))
    low, high = mesh_tools.bounds([p for s in lod0(settings) for p in points_of(originals[f'render/{s}/converted_model'])])
    sides = [hi - lo for lo, hi in zip(low, high)]
    axis = max(range(3), key=lambda a: sides[a])
    radius = sum(s for a, s in enumerate(sides) if a != axis) / 4
    return round(max(0.0, sides[axis] / 2 - radius), 3), round(radius, 3), 'xyz'[axis]


def settings_of(original, keep=None):
    """The model's settings with collision off, and (with `keep`) its LODs
    and materials those surfaces' alone."""
    settings = parse(bytearray(original))
    if keep is not None:
        for key, (_, table) in settings[1]:
            if key == 'lod_hierrarchy':
                for lod, (_, entry) in table:
                    for field, (_, names) in entry:
                        if field == 'surfaces':
                            names[:] = [n for n in names if n[1] in keep]
            if key == 'material_settings':
                table[:] = [(name, value) for name, value in table if name in keep]
    present = {key for key, _ in settings[1]}
    for key in cap_pack.COLLISION:
        if key in present:
            put(settings, key, (mesh_tools.T_BOOL, 0))
        else:
            settings[1].append((key, (mesh_tools.T_BOOL, 0)))
    return serialize(settings)


def root_props(original, points):
    tree = parse(bytearray(original))
    low, high = mesh_tools.bounds(points)
    put(tree, 'bounding_box/min', (7, low))
    put(tree, 'bounding_box/max', (7, high))
    return serialize(tree)


def surface_files(files, target, part, original_props, blob, points, triangles):
    """A surface's mesh and its export properties with its new bounds and
    counts (skeleton-cap's, for properties that may have no bounding sphere:
    some ports' have none)."""
    files[target + part] = blob
    low, high = mesh_tools.bounds(points)
    middle = [(lo + hi) / 2 for lo, hi in zip(low, high)]
    radius = max(math.dist(p, middle) for p in points)
    tree = parse(bytearray(original_props))
    present = {key for key, _ in tree[1]}
    put(tree, 'bounding_box/min', (7, low))
    put(tree, 'bounding_box/max', (7, high))
    if 'bounding_sphere' in present:
        put(tree, 'bounding_sphere/center', (7, middle))
        put(tree, 'bounding_sphere/radius', (2, struct.unpack('<I', struct.pack('<f', radius))[0]))
    for field, value in [('vertices', len(points)), ('indices', triangles * 3), ('primitives', triangles)]:
        put(tree, 'source/' + field, (1, value))
    files[target + part.replace('/converted_model', '/export_properties')] = serialize(tree)


def every(points, uvs, triangles):
    return set(range(len(triangles)))


def loose_model(originals, target):
    """The model whole about its centre: every surface moved, collision off."""
    files, origin, all_points = {}, centre(originals), []
    settings = parse(bytearray(originals['settings']))
    for name in lamp_pack.surfaces(settings):
        part = f'render/{name}/converted_model'
        blob, points, triangles = cap_pack.keep(originals[part], every, origin)
        surface_files(files, target, part, originals[f'render/{name}/export_properties'], blob, points, triangles)
        if name in lod0(settings):
            all_points += points
    files[target + 'render/export_properties'] = root_props(originals['render/export_properties'], all_points)
    files[target + 'settings'] = settings_of(originals['settings'])
    return files


def scaled(data, scale):
    """A converted_model's mesh, every position times `scale` (its vectors as they are)."""
    out = bytearray(data)
    vertices, _ = mesh_tools.mesh(data)
    for k in range(len(vertices)):
        at = 12 + 28 * k
        struct.pack_into('<3f', out, at, *(c * scale for c in struct.unpack_from('<3f', out, at)))
    return bytes(out)


def ceramic_mesh(originals, name):
    """A ceramic thing's LOD0 mesh at the scale the maps place it."""
    return scaled(originals[f'render/{surface_of(originals)}/converted_model'], SCALE.get(name, 1.0))


def cut_of(data, count=SHARDS):
    """The surface's cut into `count` shards: seeds spread over it from its top."""
    top = max(p[1] for p in points_of(data))
    return bottle.Cut(bottle.far_seeds(data, count, bottle.STEP, (0.0, top, 0.0)))


def shard_models(originals, surface, targets, cut, data=None):
    """Each shard of `surface` (its LOD0's; `data` its mesh, when not the model's) as a model of
    `targets`, about its centre."""
    files = {}
    part = f'render/{surface}/converted_model'
    for target, (blob, points, triangles, _) in zip(targets, bottle.shards(data or originals[part], cut)):
        surface_files(files, target, part, originals[f'render/{surface}/export_properties'], blob, points, triangles)
        files[target + 'render/export_properties'] = root_props(originals['render/export_properties'], points)
        files[target + 'settings'] = settings_of(originals['settings'], keep={surface})
    return files


def surface_of(originals):
    """A ceramic model's one surface (its LOD0's)."""
    (name,) = lod0(parse(bytearray(originals['settings'])))
    return name


def shard_centres(stock, name, hashes):
    """Each shard's centre in the model's frame at its SCALE: where kitchen-ware-1.json places it
    (the placement unscaled)."""
    data = ceramic_mesh(originals_of(stock, name, hashes), name)
    return [c for *_, c in bottle.shards(data, cut_of(data))]


def build(stock, hashes):
    files = {}
    for name in WARE:
        files |= loose_model(originals_of(stock, name, hashes), model_dir(loose_name(name)))
    for name in CERAMIC:
        originals = originals_of(stock, name, hashes)
        data = ceramic_mesh(originals, name)
        files |= shard_models(originals, surface_of(originals), [model_dir(t) for t in shard_names(name)],
                              cut_of(data), data)
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
    pack = {'format_version': 1, 'id': 'kitchen-ware', 'version': '1', 'target_build': '0.26g',
            'description': "The maps' small kitchen things as loose models: whole ones about their centre, "
                           'and the clay jugs and pot as eight shards each',
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['Generated from stock 0.26g and the ports by scripts/kitchen-ware/make_pack.py; '
                      "the maps' placement is kitchen-ware-1.json."]}
    for path, data in files.items():
        dest = args.out / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
    data = (json.dumps(pack, indent=2) + '\n').encode()
    (args.out / 'pack.json').write_bytes(data)
    print(hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
