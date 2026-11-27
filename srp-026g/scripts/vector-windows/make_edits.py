#!/usr/bin/env python3
"""Author vector-windows-1.json: Vector Laboratory's glazed partitions break bay by bay.

The owner (2026-11-27, of the boiler room's metal-framed glass: break it as
the capsules): Vector's building level/07/base_a (a render visual at the
origin of each of its projects) draws pack vector-windows@1's
srp_base_a_frame instead (the building with its flat glass alone: its
visual's names set), and each bay of its partitions' glass is a fixture of
its project's lamp table, `pane<k>` (the pack's bay k): the bay's bounds, 3 cm
off its plane and 1 cm along it (what a shot or a blast must reach),
srp_base_a_pane<k> drawn while intact and its _broken remains while broken,
both at the bay's centre, placed after the project's last visual; the glass
line the bay's height through its middle. No pieces: the game's glass burst
is its break. Its glass's collision stays (the building's, leftover glass,
as a lamp's: no mark on it).

--projects holds `projects/<level>/<project>/client_project` as the manifest
leaves them before this list; --stock the building's files (make_pack.py's).
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

spec = importlib.util.spec_from_file_location('vector_windows', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)

LEVELS = ('level_07',)
ACROSS = 0.03
ALONG = 0.01


def rows_of(path, data, bays):
    root = lamps.tree(data)
    visuals = root.get('render_visuals', [])
    buildings = [(i, v) for i, v in enumerate(visuals) if lamps.model_name(v) == pack.BUILDING]
    if not buildings:
        return None
    (index, building), = buildings
    table = fixtures.table_of(path, root)
    placer = fixtures.Placer(path, visuals)
    names = [{'file': path, 'path': f'render_visuals[{index}]/{key}', 'from': pack.BUILDING, 'to': pack.FRAME}
             for key in fixtures.names(building)]
    # the list sets the names before it adds entries: they are placed as the frame
    v = building | {key: pack.FRAME for key in fixtures.names(building)}
    rows = {}
    for k, (centre, low, high, axis) in enumerate(bays):
        at = lamps.world_bounds(v, centre, centre)[0]
        intact = placer.place(v, pack.pane_name(k), at)
        broken = placer.place(v, pack.broken_name(k), at)
        margin = [ACROSS if a == axis else ALONG for a in range(3)]
        lo, hi = lamps.world_bounds(v, [x - m for x, m in zip(low, margin)], [x + m for x, m in zip(high, margin)])
        line = [[centre[0], low[1] + 0.05, centre[2]], [centre[0], high[1] - 0.05, centre[2]]]
        start, end = (lamps.world_bounds(v, p, p)[0] for p in line)
        rows[f'pane{k}'] = fixtures.row(lo, hi, start, end, visuals={'intact0': {'index': intact, 'state': 0},
                                                                     'broken0': {'index': broken, 'state': 1}},
                                        lights={})
    fixtures.check_keys(path, table, rows)
    return names, placer.entries, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    hashes = json.loads(Path(__file__).with_name('stock.json').read_text())
    bays = pack.found(args.stock, hashes)
    names, placed, found, every = [], {}, {}, []
    for path, data in fixtures.projects(args.projects, LEVELS):
        every.append(path)
        made = rows_of(path, data, bays)
        if made is None:
            continue
        names += made[0]
        placed[path] = made[1]
        found[path] = made[2]
        print(f'{path}: {len(made[2])} bays')
    edits = {'_about': "Vector Laboratory's glazed partitions break bay by bay (the owner, 2026-11-27): its building "
             '(level/07/base_a) draws its copy with its flat glass alone (pack vector-windows@1) and each bay of its '
             "partitions' glass is a row of its project's lamp table, the bay drawn while intact and the glass left "
             "in its frame once broken, placed after the project's last visual (docs/notes/breakable_lamps.md "
             "\"Vector's windows\"). Authored by scripts/vector-windows/make_edits.py over the projects as the "
             'manifest leaves them; indices refer to the final render_visuals.',
             'set_string': names, 'add_entry': fixtures.compact(placed, every), 'add_value': fixtures.grouped(found)}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
