#!/usr/bin/env python3
"""Training Camp's Free for All arena as a source edit: the walker wall behind its base's fence, a
truck in each of the fence's gateways, 500 ms spawn protection.

--native is the stock level/04/level_border.model extracted with srp-game-data.
training-fence.tsv, the fence's posts in order, is shared with the spawn planner. The wall follows
it post to post, gateways included, WALL_BEHIND metres outside: a player meets the fence, and in a
gateway the truck, never the wall. training-trucks.tsv places the trucks: the model of the pack
training-camp-truck (make_truck_pack.py), drawn and solid, at each row's position and rotation.
Models are copied by build-content, never assembled into a patch by this script.
"""
import argparse
import csv
import json
import math
from pathlib import Path

from make_ffa_edits import COUNTS, tree

HERE = Path(__file__).resolve().parent
# Free for All's own copy of the TDM project (ffa-projects-1.json); TDM's stays as it is.
TDM_PROJECT = 'projects/v010b_cs_training_camp/day_4/client_project'
PROJECT = 'projects/v010b_cs_training_camp/day_8/client_project'
SOURCE = 'models/level/04/level_border.model'
MODEL = 'models/srp/training_camp_border.model'
PART = 'walker_collision/exported_primitives'
FILES = ['settings', 'render/export_properties',
         'render/donotwalkhere_sg/export_properties',
         'render/donotwalkhere_sg/converted_model', PART]
LEVEL = 'ported/v010b/ported/cs/training_camp/training_camp_%03d'
# The level's own truck (the whole of render chunk 104) and the pack's model of it.
LEVEL_TRUCK = LEVEL % 104
TRUCK = 'srp/training_camp_truck'
# A wall is a box 1 m thick; its middle runs this far outside the posts' line, so its inner face is
# 0.1 m behind the fence. Each overlaps its neighbours by WALL_OVERLAP and spans y = -30..50.
WALL_HALF = 0.5
WALL_BEHIND = WALL_HALF + 0.1
WALL_OVERLAP = 0.75


def outline():
    """The fence's posts, counter-clockwise seen from above (+x right, +z up)."""
    with (HERE / 'training-fence.tsv').open() as f:
        posts = [(float(r['x']), float(r['z'])) for r in csv.DictReader(f, delimiter='\t')]
    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(posts, posts[1:] + posts[:1])) / 2
    if area <= 0:
        raise ValueError('training-fence.tsv: the posts go counter-clockwise')
    return posts


def trucks():
    """(gateway, position, rotation) of each truck."""
    with (HERE / 'training-trucks.tsv').open() as f:
        return [(r['gateway'], [float(r[k]) for k in ('x', 'y', 'z')], [float(r[k]) for k in ('rx', 'ry', 'rz')])
                for r in csv.DictReader(f, delimiter='\t')]


def behind(posts):
    """The outline WALL_BEHIND outside the posts' line: each side moved out along its normal, the
    corners where neighbouring sides then meet."""
    sides = []
    for a, b in zip(posts, posts[1:] + posts[:1]):
        dx, dz = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dz)
        if length <= 0:
            raise ValueError('duplicate fence posts')
        out = (dz / length * WALL_BEHIND, -dx / length * WALL_BEHIND)   # outward of a counter-clockwise outline
        sides.append(((a[0] + out[0], a[1] + out[1]), (dx, dz)))
    corners = []
    for (p, d), (q, e) in zip(sides[-1:] + sides[:-1], sides):
        turn = d[0] * e[1] - d[1] * e[0]
        if abs(turn) < 1e-9 * math.hypot(*d) * math.hypot(*e):   # a straight run: the side's own start
            corners.append(q)
            continue
        t = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / turn
        corners.append((p[0] + d[0] * t, p[1] + d[1] * t))
    return corners


def border_wall(native, model, points, node):
    """The edits that make `model` a copy of the stock level border whose walker collision is a wall
    along the closed outline `points`: one box a side, WALL_HALF * 2 thick, overlapping its neighbours
    by WALL_OVERLAP and spanning y = -30..50; `node` the boxes' node names' prefix."""
    original = tree((native / SOURCE / PART).read_bytes())['primitives']
    edits = {'copy': [{'files': f'{SOURCE}/{p}', 'from': f'{SOURCE}/{p}', 'to': f'{model}/{p}'} for p in FILES],
             'append_entry': []}
    for i, (a, b) in enumerate(zip(points, points[1:] + points[:1])):
        dx, dz = b[0] - a[0], b[1] - a[1]
        # Primitive local +x is (cos(yaw), 0, sin(yaw)).
        primitive = {'type': 1, 'mtl': 0, 'node_name': f'{node}{i:02}',
                     'position': [(a[0] + b[0]) / 2, 10.0, (a[1] + b[1]) / 2],
                     'rotation': [0.0, math.atan2(dz, dx), 0.0],
                     'scale': [math.hypot(dx, dz) / 2 + WALL_OVERLAP, 40.0, WALL_HALF]}
        edits['append_entry'].append({'file': f'{model}/{PART}', 'list': 'primitives',
             'expected_count': len(original) + i, 'unique_key': 'node_name',
             'value': primitive, 'vectors': True})
    edits['remove_entry'] = [{'file': f'{model}/{PART}', 'list': 'primitives',
                              'where': {'node_name': p['node_name']}} for p in original]
    return edits


def build(native):
    count = COUNTS['cs_training_camp']
    placed = trucks()
    walls = border_wall(native, MODEL, behind(outline()), '|training_fence|walker_col|side_')
    edits = {'_about': "Training Camp: its base's fence closed to walkers by a wall behind it, gateways included; "
             f'a truck (pack training-camp-truck) in each of the {len(placed)} gateways; the meadow and the crater '
             f'round the base are scenery. {count} FFA points use 500 integer milliseconds (requires the '
             'optional-ms client hook); legacy seconds are zero. The original TDM points and their protection are '
             'unchanged. Generated by scripts/stalker-maps/make_training_arena.py from training-fence.tsv and '
             'training-trucks.tsv.',
             'copy': walls['copy'], 'append_entry': walls['append_entry'], 'remove_entry': walls['remove_entry'],
             'set_entry': [], 'add_value': []}
    # Collision libraries repeat at different placements; lib_name is not
    # an identity for this list. Clone its first native record and insert
    # after the last container, identified by both library and position.
    wall = f'resources/{MODEL}/walker_collision'
    solid = f'resources/models/{TRUCK}.model/collision'
    edits['add_entry'] = [{'files': PROJECT, 'list': 'collision_objects',
        'after': {'lib_name': 'resources/models/item/quest/container_closed.model/collision',
                  'position': [-20.82, -1.05, -9.82]},
        'like': {'lib_name': f'resources/models/{LEVEL % 0}.model/collision'},
        'values': {'rotation': [0.0, 0.0, 0.0], 'position': [0.0, 0.0, 0.0],
                  'cmask': 4, 'cgroup': 2, 'scale': [1.0, 1.0, 1.0],
                  'lib_name': wall}}]
    # A truck is the level's own twice over: drawn like its render chunk, after it, and solid like
    # a level collision chunk (walkers and bullets), after the wall. Each follows the one before.
    drawn_after, solid_after = {'lib_name': LEVEL_TRUCK}, {'lib_name': wall}
    for _, position, rotation in placed:
        edits['add_entry'].append({'files': PROJECT, 'list': 'render_visuals',
            'after': drawn_after, 'like': {'lib_name': LEVEL_TRUCK},
            'values': {'lib_name': TRUCK, 'visual_name': TRUCK, 'position': position, 'rotation': rotation}})
        edits['add_entry'].append({'files': PROJECT, 'list': 'collision_objects',
            'after': solid_after, 'like': {'lib_name': f'resources/models/{LEVEL % 0}.model/collision'},
            'values': {'lib_name': solid, 'position': position, 'rotation': rotation}})
        drawn_after = {'lib_name': TRUCK, 'position': position}
        solid_after = {'lib_name': solid, 'position': position}
    for i in range(count):
        edits['set_entry'].append({'file': PROJECT, 'list': 'server_objects/respawn_points',
            'where': {'team': 2, 'point_id': i},
            'from': {'invulnerability_time': 2}, 'to': {'invulnerability_time': 0}})
        edits['add_value'].append({'files': [PROJECT],
            'table': f'server_objects/respawn_points[{24 + i}]',
            'key': 'invulnerability_time_ms', 'value': 500})
    return edits


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--native', type=Path, required=True)
    ap.add_argument('--out', type=Path, default=HERE.parent / 'content-edit/training-camp-arena-1.json')
    args = ap.parse_args()
    args.out.write_text(json.dumps(build(args.native), indent=2) + '\n')
    print(args.out)
