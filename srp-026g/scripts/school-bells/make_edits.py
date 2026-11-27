#!/usr/bin/env python3
"""Author school-bells-1.json: School's wall bells ring when shot.

The owner (2026-11-27: "should be directional and just once or every n
seconds"): each props/electric/bell_school of School and School v0.21 (four
a project, on the corridors' walls) is a row of its project's lamp table,
`bell<k>`, of reaction 'ring': a shot or a blast that reaches its bounds
rings it, a point sound at the bell (pack school-bell@1's collection, one
take at random), at most once every RING_EVERY seconds; it never breaks and
the history keeps nothing of it (docs/notes/breakable_lamps.md "Bells"). Its
glass line is its bounds' middle, where the ring plays. The bells have no
collision: a bullet passes them by, to the wall behind, its ray reaching
the bounds (the clip at its first hit stops at the wall).

--projects holds `projects/<level>/<project>/client_project` as the manifest
leaves them before this list; --stock the bell's export_properties (stock's
and the v0.21 port's).
"""
import argparse
import json
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS / 'breakable-lamps'))
import fixtures  # noqa: E402
import make_edits as lamps  # noqa: E402

BELLS = ('props/electric/bell_school', 'ported/v021d/props/electric/bell_school')
# A ring's takes last 3 to 5 s (their corridor's tail): one at a time.
RING_EVERY = 4.0
# A bell is 30 cm across and 8 deep: its bounds 2 cm larger, a shot at its edge rings it.
MARGIN = 0.02


def bounds_of(stock, name):
    b = (stock / f'models/{name}.model/render/export_properties').read_bytes()
    return [lamps.c.value_of(b, lamps.c.find(b, 'bounding_box/' + key)) for key in ('min', 'max')]


def rows_of(path, data, bounds):
    root = lamps.tree(data)
    bells = [v for v in root.get('render_visuals', []) if lamps.model_name(v) in BELLS]
    if not bells:
        return None
    table = fixtures.table_of(path, root)
    rows = {}
    for k, v in enumerate(bells):
        lo, hi = lamps.world_bounds(v, *bounds[lamps.model_name(v)])
        lo, hi = [a - MARGIN for a in lo], [a + MARGIN for a in hi]
        middle = [(a + b) / 2 for a, b in zip(lo, hi)]
        rows[f'bell{k}'] = fixtures.row(lo, hi, middle, middle, reaction='ring', ring_every=RING_EVERY,
                                        visuals={}, lights={})
    fixtures.check_keys(path, table, rows)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    bounds = {name: bounds_of(args.stock, name) for name in BELLS}
    found = {}
    for path, data in fixtures.projects(args.projects):
        rows = rows_of(path, data, bounds)
        if rows is not None:
            found[path] = rows
            print(f'{path}: {len(rows)} bells')
    edits = {'_about': "School's wall bells ring when shot or caught in a blast (the owner, 2026-11-27): each "
             "bell_school a row of its project's lamp table of reaction ring, every 4 s at most, its ring pack "
             'school-bell@1\'s collection played at the bell (docs/notes/breakable_lamps.md "Bells"). Authored by '
             'scripts/school-bells/make_edits.py over the projects as the manifest leaves them.',
             'add_value': fixtures.grouped(found)}
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
