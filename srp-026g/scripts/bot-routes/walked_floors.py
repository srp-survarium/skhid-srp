"""A route table with every point put on a floor the players stood on (play 17's table, `bots/routes.rs`).

Reads (private files; this script holds no data):
  the route table  bot-routes.json, format 2 (`routes`: level, mode, team, points [x, z], optional heights)
  the verdicts     a TSV written by the ignored test `route_points_stand_where_the_players_stood` (`bots/routes.rs`),
                   a row a route point:
                   row point level mode x z height samples verdict to_x to_y to_z support
                   `row` and `point` index the table's `routes` and the route's `points`; `verdict` is
                     thin       fewer than 10 samples within 3 m of the point: not judged, kept;
                     walked     a sample stood on the side of the floor the server puts the point on: kept;
                     move       no sample did, a floor of the point's level within 1 m has 3 or more samples on its side:
                                the point goes there (its x, z and height);
                     move_wide  the same, found within 2.5 m (any level): the point goes there;
                     drop       no floor within 2.5 m has 3 samples on its side: the point is dropped.
and writes the table with the `move` and `drop` verdicts applied, and a list of every change with the counts per map.
The reasons and numbers: `docs/notes/player_bots_mapdata.md`, "Route points on floors the players walked".

usage: walked_floors.py <table.json> <verdicts.tsv> <out table.json> <changes.txt>
"""
import collections
import csv
import json
import sys

MAPS = {1: 'Rudnya', 2: 'School', 3: 'Vostok Radar Station', 4: 'Chemical Plant', 5: 'Tarakanovsky Fort',
        6: 'Mamayev Kurgan', 7: 'Vector Laboratory', 8: 'Cologne'}
CHANGES = ('move', 'move_wide', 'drop')


def read_verdicts(path):
    """{row: {point: verdict row}} of the TSV."""
    verdicts = collections.defaultdict(dict)
    with open(path, newline='') as f:
        for r in csv.DictReader(f, delimiter='\t'):
            verdicts[int(r['row'])][int(r['point'])] = r
    return verdicts


def apply(doc, verdicts):
    """The table `doc` (changed in place) with the verdicts applied, and the list of changes as lines.

    A moved point keeps its place in its route and gets the floor's height; a route that had no heights and has a moved point
    gets them blank (null) for the other points. A route is left as it was where no point changes."""
    lines = []
    per_map = collections.defaultdict(lambda: {'moved': 0, 'dropped': 0, 'touched': 0, 'rows': 0})
    for i, row in enumerate(doc['routes']):
        name = MAPS[int(row['level'][-2:])]
        per_map[name]['rows'] += 1
        changes = {k: v for k, v in verdicts.get(i, {}).items() if v['verdict'] in CHANGES}
        if not changes:
            continue
        per_map[name]['touched'] += 1
        heights = list(row.get('heights') or [None] * len(row['points']))
        points, kept = [], []
        for k, (x, z) in enumerate(row['points']):
            v = changes.get(k)
            where = f"row {i} {name} mode {row['mode']} team {row['team']} point {k}: ({x}, {z}) h={heights[k]}"
            if v is None:
                points.append([x, z])
                kept.append(heights[k])
            elif v['verdict'] == 'drop':
                per_map[name]['dropped'] += 1
                lines.append(f"{where} DROPPED: no floor within 2.5 m that a player stood on "
                             f"({v['samples']} samples within 3 m, none on the nearest floor's side)")
            else:
                to_x, to_y, to_z = float(v['to_x']), float(v['to_y']), float(v['to_z'])
                points.append([round(to_x, 2), round(to_z, 2)])
                kept.append(round(to_y, 2))
                per_map[name]['moved'] += 1
                reach = '2.5 m' if v['verdict'] == 'move_wide' else '1 m'
                lines.append(f"{where} -> ({to_x:.2f}, {to_z:.2f}) h={to_y:.2f}: floor within {reach}, "
                             f"{v['support']} samples on its side (nearest floor 0 of {v['samples']})")
        row['points'] = points
        if any(h is not None for h in kept):
            row['heights'] = kept
        else:
            row.pop('heights', None)
    return lines, per_map


def report(lines, per_map):
    head = ['map: points moved / dropped, routes touched, of routes']
    for name, c in sorted(per_map.items()):
        head.append(f"  {name}: {c['moved']} moved, {c['dropped']} dropped, {c['touched']} routes touched of {c['rows']}")
    head.append(f"total: {sum(c['moved'] for c in per_map.values())} moved, "
                f"{sum(c['dropped'] for c in per_map.values())} dropped")
    return '\n'.join(head + [''] + lines) + '\n'


def main(argv):
    if len(argv) != 4:
        sys.exit(__doc__)
    table, verdicts, out, changes = argv
    with open(table) as f:
        doc = json.load(f)
    lines, per_map = apply(doc, read_verdicts(verdicts))
    doc['source'] = doc.get('source', '') + '; points checked against the floors the human samples stood on'
    with open(out, 'w', newline='\n') as f:
        f.write(json.dumps(doc, indent=1))
    text = report(lines, per_map)
    with open(changes, 'w', newline='\n') as f:
        f.write(text)
    print(text)


if __name__ == '__main__':
    main(sys.argv[1:])
