#!/usr/bin/env python3
"""Author lab-capsule-glass-1.json: Vector Laboratory's capsules' glass shatters.

The owner (2026-11-27): each props/chemicalflaskcentre/lab_flask of Vector
(level_07, its four capsules on the lower floor) and each of their doors
(lab_flaskdoor, the tube's other half: "only one wall, even if I shoot
another") is a fixture of its project's lamp table, `capsule<k>` and
`door<k>`: the glass's bounds (what a shot or a blast must reach), the model
drawn while intact and pack lab-capsule-glass@1's srp_<model>_broken (its
frame without the glass) placed as it while broken, its loose pieces the
glass's 16 shards (kind 'pane_shard', each at its centre, with its glass's
outward normal `out_*` and its long side `along_*` in the model's frame)
placed after the project's last visual.
Its break is the table's (bullet x glass, the game's shards and crash along
the glass's height, at its middle), its glass's collision stays (leftover
glass, as a lamp's: no mark on it, intact or broken).

--projects holds `projects/<level>/<project>/client_project` as the manifest
leaves them before this list; --stock the capsule's files (make_pack.py's).
Indices are those of the final render_visuals (build-content's lamp check
refuses a build where a later list moved them).
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

spec = importlib.util.spec_from_file_location('lab_capsule', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)

LEVELS = ('level_07',)


KEYS = {pack.CAPSULE.model: 'capsule', pack.DOOR.model: 'door'}


def pane_shard(index, out, along):
    piece = {'index': index, 'kind': 'pane_shard'}
    piece |= {f'out_{a}': v for a, v in zip('xyz', out)}
    piece |= {f'along_{a}': v for a, v in zip('xyz', along)}
    return piece


def rows_of(path, data, info, bounds):
    """`info` and `bounds` by model: its shards' (centre, out, along), its glass's bounds."""
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    found = [(i, v) for i, v in enumerate(visuals) if lamps.model_name(v) in KEYS]
    if not found:
        return None
    table = fixtures.table_of(path, root)
    placer = fixtures.Placer(path, visuals)
    rows = {}
    count = dict.fromkeys(KEYS.values(), 0)
    for index, v in found:
        glass = next(g for g in pack.GLASSES if g.model == lamps.model_name(v))
        (x0, y0, z0), (x1, y1, z1) = bounds[glass.model]
        # the glass's height through its middle, where the break plays
        line = [[(x0 + x1) / 2, y0 + 0.05, (z0 + z1) / 2], [(x0 + x1) / 2, y1 - 0.05, (z0 + z1) / 2]]
        broken = placer.place(v, glass.broken)
        loose = {f'glass{j}': pane_shard(placer.place(v, shard, lamps.world_bounds(v, c, c)[0]), out, along)
                 for j, (shard, (c, out, along)) in enumerate(zip(glass.shard_names, info[glass.model]))}
        lo, hi = lamps.world_bounds(v, *bounds[glass.model])
        start, end = (lamps.world_bounds(v, p, p)[0] for p in line)
        key = KEYS[glass.model]
        rows[f'{key}{count[key]}'] = fixtures.row(lo, hi, start, end,
                                                  visuals={'intact0': {'index': index, 'state': 0},
                                                           'broken0': {'index': broken, 'state': 1}},
                                                  lights={}, loose=loose)
        count[key] += 1
    fixtures.check_keys(path, table, rows)
    return placer.entries, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    info = {g.model: pack.shard_info(args.stock, hashes, g) for g in pack.GLASSES}
    bounds = {g.model: pack.glass_bounds(args.stock, hashes, g) for g in pack.GLASSES}
    placed, found, every = {}, {}, []
    for path, data in fixtures.projects(args.projects, LEVELS):
        every.append(path)
        made = rows_of(path, data, info, bounds)
        if made is None:
            continue
        placed[path] = made[0]
        found[path] = made[1]
        print(f'{path}: {len(made[1])} capsules and doors')
    edits = {'_about': "Vector Laboratory's glass capsules break (the owner, 2026-11-27): each lab_flask and each "
             "lab_flaskdoor a row of its project's lamp table, the glass's bounds, the model without its glass (pack "
             "lab-capsule-glass@1) drawn once broken and the glass's 16 shards its loose pieces, placed after the "
             "project's last visual (docs/notes/breakable_lamps.md). Authored by scripts/lab-capsule/make_edits.py "
             'over the projects as the manifest leaves them; indices refer to the final render_visuals.',
             'add_entry': fixtures.compact(placed, every), 'add_value': fixtures.grouped(found)}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
