#!/usr/bin/env python3
"""London (v0.42)'s battery mode (mode 2) on London's own spread of batteries.

usage: python3 -I make_battery_edits.py <live folder> <out.json>

<live folder> holds the game's files at their game paths: London (v0.32)'s rain_2 project (the spread)
and London (v0.42)'s day_2, rain_2 and evening_2 projects, as players have them.

0.44a5 has no mode 2: the port makes it from the charger mode's project (mode 3) without its stations,
so its batteries spawn from the charger mode's 24 points, four piles of six. London's mode 2 (0.31a2's,
0.32a4's, the old London's: the same 40 points) spreads them over the map. Its points go to v0.42 but
those whose floor the 0.42 rework moved or filled (check-points `picks` on both maps' collision: kept on
v0.32, dropped on v0.42), so 32 remain.
"""
import json
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'content-edit'))
import content_edit as ce  # noqa: E402

SPREAD = 'projects/v032a_level_09/rain_2/client_project'
PROJECTS = ['projects/v044a_level_09/%s_2/client_project' % w for w in ('day', 'rain', 'evening')]
LIST = 'server_objects/victory_item_spawners'
KEYS = ('name', 'full_name', 'position', 'rotation', 'spawned_item/position', 'spawned_item/rotation')
# London point_id: why v0.42 has no place for it (check-points picks, v0.32 kept it on its ground).
MOVED = {
    2: 'the ground is 0.96 m below',
    9: 'no floor reached from the spawns',
    11: 'collision 0.32 m from it',
    23: 'the ground is 0.96 m below',
    25: 'the ground is 1.30 m below (v0.32: collision 0.94 m above)',
    27: 'the ground is 0.75 m below',
    33: 'collision 0.32 m from it',
    39: 'the ground is 1.03 m below (v0.32: collision 1.32 m above)',
}


def spawners(path):
    b = bytearray(open(path, 'rb').read())
    out = []
    for e in ce.children(b, ce.find(b, LIST)):
        d = {}
        for k in KEYS + ('point_id',):
            v = ce.value_of(b, ce.find(b, k, e))
            d[k] = v.decode() if isinstance(v, (bytes, bytearray)) else v
        out.append(d)
    return sorted(out, key=lambda d: d['point_id'])


def numbers(v):
    return [round(x, 6) for x in v] if isinstance(v, (list, tuple)) else v


def main():
    live, out = sys.argv[1:3]
    spread = [p for p in spawners(os.path.join(live, SPREAD)) if p['point_id'] not in MOVED]
    edits = {'set_entry': [], 'add_entry': []}
    for f in PROJECTS:
        have = spawners(os.path.join(live, f))
        assert len(have) == 24 and [h['point_id'] for h in have] == list(range(24)), f
        for h, p in zip(have, spread):
            edits['set_entry'].append({
                'file': f, 'list': LIST, 'where': {'point_id': h['point_id']},
                'from': {k: numbers(h[k]) for k in ('position', 'spawned_item/position')},
                'to': {k: numbers(p[k]) for k in KEYS}})
        for n, p in enumerate(spread[len(have):], start=len(have)):
            values = {k: numbers(p[k]) for k in KEYS}
            values['point_id'] = n
            edits['add_entry'].append({
                'files': f, 'list': LIST, 'after': {'point_id': n - 1}, 'like': {'point_id': 0},
                'expected_files': 1, 'values': values})
    about = __doc__.split('\n\n')[2].replace('\n', ' ')
    json.dump({'_about': about, **edits}, open(out, 'w'), indent=1)
    print('%d points a project, %d left out' % (len(spread), len(MOVED)))


if __name__ == '__main__':
    main()
