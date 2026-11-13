#!/usr/bin/env python3
"""Training Camp's crater boundary and 500 ms FFA protection, as a source edit.

--native is the stock level/04/level_border.model extracted with srp-game-data.
The outline is shared with the spawn planner; models are copied by build-content,
never assembled into a patch by this script. Only walker collision is added.
"""
import argparse
import csv
import json
import math
from pathlib import Path

from make_ffa_edits import tree

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


def outline():
    with (HERE / 'training-arena.tsv').open() as f:
        return [(float(r['x']), float(r['z'])) for r in csv.DictReader(f, delimiter='\t')]


def build(native):
    points = outline()
    original = tree((native / SOURCE / PART).read_bytes())['primitives']
    edits = {'_about': 'Training Camp: crater rim closed to walkers; forest is scenery. '
             '24 FFA points use 500 integer milliseconds (requires the optional-ms client hook); '
             'legacy seconds are zero. The original TDM points and their protection are unchanged.',
             'copy': [{'files': f'{SOURCE}/{p}', 'from': f'{SOURCE}/{p}', 'to': f'{MODEL}/{p}'} for p in FILES],
             'append_entry': [], 'remove_entry': [], 'set_entry': [], 'add_value': []}
    for i, (a, b) in enumerate(zip(points, points[1:] + points[:1])):
        dx, dz = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dz)
        if length <= 0:
            raise ValueError('duplicate crater vertices')
        # Primitive local +x is (cos(yaw), 0, sin(yaw)). Each wall overlaps
        # its neighbours by 0.75 m and spans y=-30..50, above every rim exit.
        primitive = {'type': 1, 'mtl': 0, 'node_name': f'|training_crater|walker_col|rim_{i:02}',
                     'position': [(a[0] + b[0]) / 2, 10.0, (a[1] + b[1]) / 2],
                     'rotation': [0.0, math.atan2(dz, dx), 0.0],
                     'scale': [length / 2 + 0.75, 40.0, 0.5]}
        edits['append_entry'].append({'file': f'{MODEL}/{PART}', 'list': 'primitives',
             'expected_count': len(original) + i, 'unique_key': 'node_name',
             'value': primitive, 'vectors': True})
    edits['remove_entry'] = [{'file': f'{MODEL}/{PART}', 'list': 'primitives',
                              'where': {'node_name': p['node_name']}} for p in original]
    # Collision libraries repeat at different placements; lib_name is not
    # an identity for this list. Clone its first native record and insert
    # after the last container, identified by both library and position.
    edits['add_entry'] = [{'files': PROJECT, 'list': 'collision_objects',
        'after': {'lib_name': 'resources/models/item/quest/container_closed.model/collision',
                  'position': [-20.82, -1.05, -9.82]},
        'like': {'lib_name': 'resources/models/ported/v010b/ported/cs/training_camp/training_camp_000.model/collision'},
        'values': {'rotation': [0.0, 0.0, 0.0], 'position': [0.0, 0.0, 0.0],
                  'cmask': 4, 'cgroup': 2, 'scale': [1.0, 1.0, 1.0],
                  'lib_name': f'resources/{MODEL}/walker_collision'}}]
    for i in range(24):
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
