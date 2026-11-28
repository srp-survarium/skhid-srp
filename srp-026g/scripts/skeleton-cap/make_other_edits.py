#!/usr/bin/env python3
"""Author skeleton-cap-2.json: the sitting skeleton's cap flies off on every other map.

The owner (2026-11-27: the cap on every map, not Rudnya's alone). Each
remains/skeleton_sitting1 (stock's and the v0.11, v0.21, v0.32 and v0.42
ports' copies) of a project other than Rudnya's (skeleton-cap-1.json's) is a
fixture of the project's lamp table, as Rudnya's: after the project's last
visual the skeleton without its cap and the cap alone (pack skeleton-cap@2),
the row `skeleton_cap<k>` the cap's bounds, the skeleton drawn while intact,
the bare one while broken, the cap its loose piece. These tables are lamps'
(their break material glass, their leftover glass): the row says its own,
stone (the skeleton's collision) and leftover 'body' (no stone mark in the
cap's bounds while it is on, the skull's once it is off).

Vostok, Chemical Plant, the Fort and the three Londons (stock, v0.32,
v0.42), and v0.11's Vostok and Chemical Plant and v0.21's Vostok, Chemical
Plant and Fort. --projects holds `projects/<level>/<project>/client_project`
as the manifest leaves them before this list; --stock the skeleton's files
(make_pack.py's). The indices are those of the final render_visuals:
build-content's lamp check (breakable-lamps/make_edits.check_built) refuses
a build where a later list moved them.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS / 'breakable-lamps'))
import fixtures  # noqa: E402
import make_edits as lamps  # noqa: E402


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pack = load('cap_pack', 'skeleton-cap/make_pack.py')
first = load('cap_edits', 'skeleton-cap/make_edits.py')


def rows_of(path, data, centre, bounds):
    """One project's visuals placed and its rows by key, or None without a sitting skeleton."""
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    found = [(i, v) for i, v in enumerate(visuals) if lamps.model_name(v) in lamps.FIGURES]
    if not found:
        return None
    table = fixtures.table_of(path, root)
    placer = fixtures.Placer(path, visuals)
    rows = {}
    for k, (index, skeleton) in enumerate(found):
        bare, cap = lamps.FIGURES[lamps.model_name(skeleton)]
        at = lamps.world_bounds(skeleton, centre, centre)[0]
        broken = placer.place(skeleton, bare)
        loose = placer.place(skeleton, cap, at)
        lo, hi = lamps.world_bounds(dict(skeleton, position=at), *bounds)
        rows[f'skeleton_cap{k}'] = fixtures.row(
            lo, hi, at, at, break_material=first.STONE, effect_material=first.BULLET, leftover='body',
            visuals={'intact0': {'index': index, 'state': 0}, 'broken0': {'index': broken, 'state': 1}},
            lights={}, loose={'cap0': {'index': loose}})
    fixtures.check_keys(path, table, rows)
    return placer.entries, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    centre, bounds = pack.cap_centre(args.stock), first.cap_bounds(args.stock)
    placed, found, every = {}, {}, []
    for path, data in fixtures.projects(args.projects):
        every.append(path)
        if path.split('/')[1] in first.LEVELS:
            continue
        made = rows_of(path, data, centre, bounds)
        if made is None:
            continue
        placed[path] = made[0]
        found[path] = made[1]
        print(f'{path}: {len(made[1])} skeletons')
    edits = {'_about': "The sitting skeleton's cap flies off when shot on every map but Rudnya (skeleton-cap-1's; "
             'the owner, 2026-11-27): after the project\'s last visual the skeleton\'s bare model and its cap (pack '
             "skeleton-cap@2), and a row of the project's lamp table, the cap's bounds, stone and leftover body of its "
             "own, whose loose piece is the cap (docs/notes/breakable_lamps.md). Authored by "
             'scripts/skeleton-cap/make_other_edits.py over the projects as the manifest leaves them; indices refer '
             'to the final render_visuals.',
             'add_entry': fixtures.compact(placed, every), 'add_value': fixtures.grouped(found)}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
