#!/usr/bin/env python3
"""Free for All spawns by the spawn rule on every map but Rudnya, twice as many (the owner, 2026-11-14:
"there should be a rule that spawns are inside buildings or close to two walls or some cover", "and
none at the perimeter", "let's make other maps have x2 spawns too"; docs/notes/ffa.md, "Cover").

cover-maps.tsv is the reviewed output of check-points' to_cover binary (its doc says how a spawn is
kept, turned, replaced or added): one row a changed or new spawn, `point` its point id, `verdict`
`replaced` (a new position and yaw), `turned` (its place kept, a new yaw) or `added` (a new point).
Run on the projects as players would have them (the stock db, every patch of release/patches.tsv and
a build of the manifest's rows above this list's over them), from the checkout's root:

  srp-026g/scripts/ffa-mode/check-points: cargo run --release --bin to_cover -- --db <resources.db> \\
      --patch <resources.db.patch_12> ... --patch <the build's patch> --plots <folder> --times 2 \\
      --project level_02/day_8 ... --project level_09/day_8 --below level_09/day_8=14.5 \\
      --project v010b_cs_mp_bath/day_8@-48.13,-22.87,34.12,53.13 \\
      --project v010b_cs_mp_military_1/day_8@-127.47,-71.12,115.78,95.38 \\
      --project v010b_cs_training_camp/day_8@-47.19,-83.47,82.81,91.53 \\
      --fence v010b_cs_training_camp/day_8=srp-026g/scripts/stalker-maps/training-fence.tsv \\
      --hold v010b_cs_training_camp/day_8 --below v010b_cs_training_camp/day_8=1 \\
      > srp-026g/scripts/ffa-mode/cover-maps.tsv

(the STALKER maps with their reviewed play area, as make_center_edits.py; none of London's above its
rotunda's second storey; Training Camp's 26 spawns as they are, the owner's arena, and its new ones
on the ground inside its fence, the area measured there). Then

  python3 make_cover_edits.py --live <folder>

with the projects at their game paths as the rows above the list leave them, writes
../content-edit/ffa-cover-maps-1.json: each replaced or turned spawn, matched by its point id, name
and team 2, gets the new position and rotation [0, yaw, 0] (a turned one the rotation only; its id,
name, radii and protection stay); each added one is a new solo spawn after the project's last point,
its point id after the highest, a copy of its first solo spawn (20 m exclusion, 500 ms protection, no
team zones) at the row's place, named ffa_cover_<point id>. No distance between spawns is kept (the owner, 2026-11-14: "spawns can be
as close as possible": the respawn's 20 m enemy exclusion keeps players apart). Only the free for
all's own projects (`<weather>_8`) are edited; Team Deathmatch's are never named. After every Free
for All list: its new points copy the 500 ms ffa-protection-ms-1 gave the others, and the lists
before it address points by index.
"""
import argparse
import csv
import math
from pathlib import Path

import make_center_edits as center
import make_edits as base

POINTS_TSV = base.HERE / 'cover-maps.tsv'
OUTPUT = base.HERE.parent / 'content-edit/ffa-cover-maps-1.json'
VERDICTS = ('replaced', 'turned', 'added')
# Rudnya has its own, from the owner's picks (ffa-rudnya-picks-1).
NOT_HERE = ('projects/level_01/',)


def changed():
    """cover-maps.tsv's rows by project: {point id: (verdict, position, yaw)}."""
    with POINTS_TSV.open(newline='') as stream:
        rows = list(csv.DictReader(stream, delimiter='\t'))
    out = {}
    for row in rows:
        if row['verdict'] not in VERDICTS:
            raise ValueError(f'{row["project"]} {row["point"]}: not a checked change')
        values = [float(row[k]) for k in ('x', 'y', 'z', 'ry')]
        if not all(math.isfinite(v) for v in values):
            raise ValueError(f'{row["project"]} {row["point"]}: not finite')
        path = center.path_of(row['project'])
        if path.startswith(NOT_HERE):
            raise ValueError(f'{row["project"]}: not this list\'s map')
        points = out.setdefault(path, {})
        point = int(row['point'])
        if point in points:
            raise ValueError(f'{row["project"]}: point {point} changed twice')
        points[point] = (row['verdict'], values[:3], values[3])
    return out


def template(buf, solo):
    """The project's first solo spawn that a new one can copy: no team zones, float radii, 500 ms."""
    for i in sorted(solo):
        model = solo[i]
        types = base.walked_types(buf, f"{base.POINTS}[{i}]")
        if (not any(model[z]['collision_geometries'] for z in ('ally_zones', 'enemy_zones'))
                and all(types[k] == base.c.T_FLOAT for k in base.RADII)
                and model.get('invulnerability_time_ms') == 500 and model['invulnerability_time'] == 0):
            return model
    raise ValueError('no solo spawn to copy (no zones, float radii, 500 ms)')


def edits(live):
    out = {'_about': 'Free for All spawns by the spawn rule, twice as many (the owner, 2026-11-14: "there should '
           'be a rule that spawns are inside buildings or close to two walls or some cover", "and none at the '
           'perimeter", "let\'s make other maps have x2 spawns too"): on every map but Rudnya, each team-2 spawn '
           'that fails the rule (cover, past the perimeter band) or the spawn checks (standing floor, body clear, '
           'harmful zones 2 m away) moves to a place that passes them, one that passes but faces a wall is '
           'turned, and new points that pass them, copies of a solo spawn (20 m exclusion, 500 ms protection), '
           'double the count; the places chosen so the inner area has places to spawn everywhere. Training Camp\'s '
           '26 stay as they are and its 26 new ones stand on the ground inside its fence. No distance between '
           'spawns is kept ("spawns can be as close as possible"). Generated by scripts/ffa-mode/'
           'make_cover_edits.py from cover-maps.tsv (check-points to_cover). The free for all\'s own projects '
           'only; the last Free for All list (its new points copy the 500 ms ffa-protection-ms-1 gave the others).',
           'set_entry': [], 'add_entry': []}
    for path, points in changed().items():
        buf = (live / path).read_bytes()
        entries = base.tree(buf)['server_objects']['respawn_points']
        solo = {e['point_id']: e for e in entries if e['team'] == 2}
        new = sorted(i for i, (verdict, _, _) in points.items() if verdict == 'added')
        missing = set(points) - set(new) - set(solo)
        if missing:
            raise ValueError(f'{path}: no solo spawn {sorted(missing)}')
        # Point ids are a team's (the STALKER maps' teams each count from 0): new ones follow the highest.
        top = max(e['point_id'] for e in entries)
        if new != list(range(top + 1, top + 1 + len(new))):
            raise ValueError(f'{path}: new points are not numbered after the highest, {top}')
        if new and entries[-1]['team'] != 2:
            raise ValueError(f'{path}: the last point is no solo spawn')
        for i in sorted(set(points) - set(new)):
            entry = solo[i]
            verdict, position, yaw = points[i]
            keys = ('position', 'rotation') if verdict == 'replaced' else ('rotation',)
            to = {'position': position, 'rotation': [0., yaw, 0.]}
            out['set_entry'].append({'file': path, 'list': base.POINTS,
                'where': {k: entry[k] for k in ('point_id', 'name', 'team')},
                'from': {k: [round(v, 6) for v in entry[k]] for k in keys},
                'to': {k: to[k] for k in keys}})
        if new:
            model = template(buf, solo)
        for k, i in enumerate(new):
            _, position, yaw = points[i]
            name = f'ffa_cover_{i}'
            after = entries[-1]['point_id'] if k == 0 else i - 1
            out['add_entry'].append({'files': path, 'list': base.POINTS, 'after': {'point_id': after, 'team': 2},
                'like': {'point_id': model['point_id'], 'team': 2}, 'expected_files': 1, 'values': {
                    'name': name, 'full_name': 'logic/modes/ffa/respawn/' + name, 'point_id': i, 'team': 2,
                    'position': position, 'rotation': [0., yaw, 0.], 'enemy_check_radius': 20.,
                    'ally_check_radius': 1., 'invulnerability_time': 0, 'invulnerability_time_ms': 500}})
    return {k: v for k, v in out.items() if v}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--live', type=Path, required=True)
    args = parser.parse_args()
    base.write(edits(args.live), OUTPUT)
    print(OUTPUT)


if __name__ == '__main__':
    main()
