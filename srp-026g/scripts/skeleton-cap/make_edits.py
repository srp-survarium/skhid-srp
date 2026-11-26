#!/usr/bin/env python3
"""Author skeleton-cap-1.json: Rudnya's sitting skeleton's cap is shot off.

The owner (2026-11-26, Rudnya's skeleton at 11.9, 18.7, 38.6: "shooting
this helmet should make it fly off", "similar to caps flying off dead ppl").
Each Rudnya project (level_01, and v021d_level_01, the v0.21 port) gets, after
its remains/skeleton_sitting1:

- the skeleton without its cap (pack skeleton-cap@1's bare model), placed as
  it;
- the cap alone, placed as it but at the cap's centre (the cap model's
  origin, make_pack.cap_centre);
- a lamp table (the client's patch 81, docs/notes/breakable_lamps.md) of one
  fixture: the cap's bounds; the skeleton drawn while intact, the bare one
  while broken; the cap its loose piece, which flies off as a corpse's
  headwear is shot off. Its break plays bullet x stone (the skeleton's
  collision, whose decals inside the cap's bounds are not drawn) at the cap.

Rudnya has no other lamp table. --projects holds
`projects/<level>/<project>/client_project` as the manifest leaves them
before this list; --stock the skeleton's files (make_pack.py's). The
indices are those of the final render_visuals: build-content's lamp check
(breakable-lamps/make_edits.check_built) refuses a build where a later list
moved them.
"""
import argparse
import importlib.util
import json
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


lamps = load('lamp_edits', 'breakable-lamps/make_edits.py')
pack = load('cap_pack', 'skeleton-cap/make_pack.py')

LEVELS = ('level_01', 'v021d_level_01')
# The skeleton's collision (game_material_settings: stone) and the bullet.
STONE, BULLET = 21, 7


def cap_bounds(stock):
    """The cap model's bounds about its origin (its LOD0's)."""
    data = (stock / next(iter(pack.SOURCES)) / 'render/rag_s/converted_model').read_bytes()
    _, points, _ = pack.keep(data, pack.cap_part, pack.cap_centre(stock))
    return pack.mesh_tools.bounds(points)


def edits_of(path, data, centre, bounds):
    """One project's add_entry rows and its lamp table."""
    root = lamps.tree(data)
    if 'srp_lamps' in root:
        raise ValueError(f'{path}: a lamp table already')
    visuals = root.get('render_visuals', [])
    found = [(i, v) for i, v in enumerate(visuals) if v.get('visual_name') in lamps.FIGURES]
    if len(found) != 1:
        raise ValueError(f'{path}: {len(found)} sitting skeletons')
    index, skeleton = found[0]
    bare, cap = lamps.FIGURES[skeleton['visual_name']]
    at = lamps.world_bounds(skeleton, centre, centre)[0]
    where = {k: skeleton[k] for k in ('visual_name', 'position', 'rotation', 'scale')}
    entries = [
        {'files': path, 'list': 'render_visuals', 'after': where, 'like': where,
         'values': {'visual_name': bare, 'lib_name': bare}},
        {'files': path, 'list': 'render_visuals', 'after': dict(where, visual_name=bare), 'like': where,
         'values': {'visual_name': cap, 'lib_name': cap, 'position': at}},
    ]
    placed = dict(skeleton, position=at)
    lo, hi = lamps.world_bounds(placed, *bounds)
    row = dict(zip(('min_x', 'min_y', 'min_z', 'max_x', 'max_y', 'max_z'), lo + hi))
    row |= dict(zip(('glass_from_x', 'glass_from_y', 'glass_from_z', 'glass_to_x', 'glass_to_y', 'glass_to_z'), at + at))
    row |= {'visuals': {'intact0': {'index': index, 'state': 0}, 'broken0': {'index': index + 1, 'state': 1}},
            'lights': {}, 'loose': {'cap0': {'index': index + 2}}}
    table = {'version': 3, 'break_material': STONE, 'effect_material': BULLET, 'lamps': {'cap0': row}}
    return entries, table


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    centre, bounds = pack.cap_centre(args.stock), cap_bounds(args.stock)
    entries, alike = [], {}
    for level in LEVELS:
        for p in sorted((args.projects / 'projects' / level).glob('*/client_project')):
            path = str(p.relative_to(args.projects))
            rows, table = edits_of(path, p.read_bytes(), centre, bounds)
            entries += rows
            alike.setdefault(json.dumps(table, sort_keys=True), []).append(path)
            print(f'{path}: skeleton at {table["lamps"]["cap0"]["visuals"]["intact0"]["index"]}')
    edits = {'_about': "Rudnya's sitting skeleton's cap flies off when shot (the owner, 2026-11-26): after the "
             'skeleton its bare model and its cap (pack skeleton-cap@1), and a lamp table of one fixture, the '
             "cap's bounds, whose loose piece is the cap (docs/notes/breakable_lamps.md). Authored by "
             'scripts/skeleton-cap/make_edits.py over the projects as the manifest leaves them; indices refer to '
             'the final render_visuals.',
             'add_entry': entries,
             'add_value': [{'files': files, 'table': '', 'key': 'srp_lamps', 'value': json.loads(spec)}
                           for spec, files in sorted(alike.items(), key=lambda kv: kv[1])]}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
