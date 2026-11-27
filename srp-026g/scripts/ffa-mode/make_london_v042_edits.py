#!/usr/bin/env python3
"""London (v0.42) in Free for All, at the owner's picks (2026-11-27: his 20 `where` places on
v044a_level_09/day_4, map 292; private/spawn-examples/london-v044-2026-11-27.tsv).

    python3 make_london_v042_edits.py --live <folder>

<folder> holds, at their game paths, gameplay/db_static_dictionaries, London (v0.42)'s three Team
Deathmatch configs (gameplay/game_modes/v044a_level_09_<w>_4.options) and their projects
(projects/v044a_level_09/<weather>_4/client_project) as every row above the list leaves them. Writes
content-edit/ffa-london-v042-1.json: in each weather, as every Free for All map has it (ffa.md, "Rules"):

- its own project v044a_level_09/<weather>_8, a copy of the Team Deathmatch project <weather>_4
  (ffa-projects-1's rule), without ammo crates (make_no_crates_edits' rule, the breakable lamps'
  visual indices after a removed model following);
- its config gameplay/game_modes/ffa/v044a_level_09_<w>_8.options, a copy of the Team Deathmatch
  config as mode 8 (ffa-1's rule, as ffa-stalker-3 makes Station's): rules 93, 91, 94 without 90,
  st_mode_08_name and _description, 30 kills to win, the mode's icon 5 (ffa-icon-1's), its
  map_name the copy; its maps_dict row (MAPS's ids: new, given to no other map);
- the owner's picks as team-2 points after the teams' (london-v042-ffa-picks.tsv, check-points picks
  --add --shift 2.5 on the three <weather>_4 projects: the weathers share the day's ground, the rows
  are the same in each), copies of the first team point as solo spawns: 20 m enemy exclusion,
  500 ms protection (invulnerability_time 0, invulnerability_time_ms 500), no zones.

The Team Deathmatch configs and projects are not changed. The list is the manifest's last row on
these files: every list that edits London (v0.42)'s team projects (kitchen-ware-1, skeleton-cap-2)
is above it, so the copies are as Team Deathmatch has them.
"""
import argparse
import csv
import math
from pathlib import Path

import make_edits as base
import make_no_crates_edits as crates_rule

HERE = Path(__file__).resolve().parent
PICKS = HERE / 'london-v042-ffa-picks.tsv'
OUTPUT = HERE.parent / 'content-edit/ffa-london-v042-1.json'
LEVEL = 'v044a_level_09'
# (the configs' weather, the projects' weather, the new maps_dict id). 503-505: past Research's
# second layout (500-502), no map's before (ids.tsv, the dictionary, the edit lists).
MAPS = [('day', 'day', 503), ('rain', 'rain', 504), ('evn', 'evening', 505)]
TDM_RULE = 90
KILLS_TO_WIN = 30
ICON_TDM, ICON_FFA = 2, 5
PREFIX = 'ffa_london_v042_'
SOLO = {'enemy_check_radius': 20., 'ally_check_radius': 1., 'invulnerability_time': 0}
PROTECTION_MS = 500


def config(w, mode):
    folder = 'ffa/' if mode == 8 else ''
    return f'gameplay/game_modes/{folder}{LEVEL}_{w}_{mode}.options'


def project(weather, mode):
    return f'projects/{LEVEL}/{weather}_{mode}/client_project'


def picks():
    """The checked picks by Team Deathmatch project: [(point id, position, rotation y)] in id order,
    the same in every weather."""
    by = {}
    with PICKS.open(newline='') as stream:
        for row in csv.DictReader(stream, delimiter='\t'):
            if row['verdict'] != 'added':
                raise ValueError(f'{PICKS.name}: point {row["point"]} is not a checked new point')
            values = [float(row[k]) for k in ('x', 'y', 'z', 'ry')]
            if not all(math.isfinite(v) for v in values):
                raise ValueError(f'{PICKS.name}: point {row["point"]} is not finite')
            by.setdefault(row['project'], []).append((int(row['point']), values[:3], values[3]))
    if set(by) != {f'{LEVEL}/{weather}_4' for _, weather, _ in MAPS}:
        raise ValueError(f'{PICKS.name}: not exactly the three Team Deathmatch projects')
    first = next(iter(by.values()))
    if any(rows != first for rows in by.values()):
        raise ValueError(f'{PICKS.name}: the weathers give other places')
    return {project: sorted(rows) for project, rows in by.items()}


def no_crates(tree, path):
    """make_no_crates_edits' removal of every ammo crate of the copy `path` (its tree `tree`)."""
    found, models = crates_rule.crates(tree, path)
    if not found:
        raise ValueError(f'{path}: no ammo crate')
    removed, lamps = [], []
    for _, inside in found:
        for o in inside:
            removed.append({'file': path, 'list': 'client_objects',
                            'where': {'game_object_type': o['game_object_type'], 'full_name': o['full_name']}})
    gone = sorted(i for i, _ in models)
    for lamp, row in tree.get('srp_lamps', {}).get('lamps', {}).items():
        for key, visual in row['visuals'].items():
            index = visual['index']
            if index in gone:
                raise ValueError(f'{path}: lamp {lamp} {key} is a crate\'s model')
            below = sum(i < index for i in gone)
            if below:
                lamps.append({'file': path, 'path': f'srp_lamps/lamps/{lamp}/visuals/{key}/index',
                              'from': index, 'to': index - below})
    for _, v in models:
        removed.append({'file': path, 'list': 'render_visuals',
                        'where': {'lib_name': v['lib_name'], 'position': crates_rule.at(v['position'])}})
        prefix = f'resources/models/{v["lib_name"]}.model/'
        collisions = [c for c in tree['collision_objects'] if c['lib_name'].startswith(prefix)
                      and math.dist(c['position'], v['position']) < 0.0005]
        if not collisions:
            raise ValueError(f'{path}: {v["lib_name"]} has no collision')
        removed += [{'file': path, 'list': 'collision_objects',
                     'where': {'lib_name': c['lib_name'], 'position': crates_rule.at(c['position'])}}
                    for c in collisions]
    return removed, lamps, len(found)


def edits(live):
    def read(path):
        return base.tree((live / path).read_bytes())
    dictionary = read(base.DICTIONARY)['maps_dict']
    taken = {e['id'] for e in dictionary}
    names = {e['cfg_mode_name'] for e in dictionary}
    out = {'copy': [], 'set': [], 'replace_string': [], 'remove_item': [], 'add_value': [], 'add_entry': [],
           'remove_entry': []}
    by = picks()
    after = max(taken)
    crates_said = []
    for w, weather, map_id in MAPS:
        tdm, ffa = config(w, 4), config(w, 8)
        tdm_project, ffa_project = project(weather, 4), project(weather, 8)
        mode = read(tdm)['mode']
        if map_id in taken or ffa.removeprefix('gameplay/game_modes/') in names:
            raise ValueError(f'{ffa}: id {map_id} or its config is in the dictionary already')
        if (mode['mode'], mode['ui']['icon'], mode['match_making_params']['map_name']) != (4, ICON_TDM, f'{LEVEL}/{weather}_4'):
            raise ValueError(f'{tdm}: not the Team Deathmatch config of {LEVEL}/{weather}_4')
        if TDM_RULE not in mode['rules'] or 'kills_to_win' in mode['parameters']:
            raise ValueError(f'{tdm}: rules {mode["rules"]}, parameters {sorted(mode["parameters"])}')
        out['copy'] += [{'files': tdm, 'from': tdm, 'to': ffa},
                        {'files': tdm_project, 'from': f'projects/{LEVEL}/{weather}_4/',
                         'to': f'projects/{LEVEL}/{weather}_8/'}]
        out['set'] += [{'file': ffa, 'path': 'mode/mode', 'from': 4, 'to': 8},
                       {'file': ffa, 'path': 'mode/ui/icon', 'from': ICON_TDM, 'to': ICON_FFA}]
        out['replace_string'] += [
            {'files': ffa, 'from': mode['name'], 'to': mode['name'].removesuffix('_4') + '_8'},
            {'files': ffa, 'from': 'st_mode_04_name', 'to': 'st_mode_08_name'},
            {'files': ffa, 'from': 'st_mode_04_description', 'to': 'st_mode_08_description'},
            {'files': ffa, 'from': f'{LEVEL}/{weather}_4', 'to': f'{LEVEL}/{weather}_8'}]
        out['remove_item'].append({'file': ffa, 'path': 'mode/rules', 'value': TDM_RULE})
        out['add_value'].append({'files': [ffa], 'table': 'mode/parameters', 'key': 'kills_to_win',
                                 'value': KILLS_TO_WIN})
        out['add_entry'].append({'files': base.DICTIONARY, 'list': 'maps_dict', 'after': {'id': after},
                                 'like': {'id': after}, 'values': {'id': map_id, 'cfg_mode_name': ffa.removeprefix('gameplay/game_modes/')}})
        after = map_id
        # The team points stay; the picks follow the last as solo spawns.
        tree = read(tdm_project)
        teams = tree['server_objects']['respawn_points']
        ids = [p['point_id'] for p in teams]
        if any(p['team'] == 2 for p in teams) or len(set(ids)) != len(ids) or ids[-1] != max(ids):
            raise ValueError(f'{tdm_project}: a solo spawn already, or point ids not distinct, the highest last')
        model = teams[0]
        if (model['ally_zones']['collision_geometries'] or model['enemy_zones']['collision_geometries']
                or 'invulnerability_time_ms' in model):
            raise ValueError(f'{tdm_project}: point {model["point_id"]} is no template (zones, ms protection)')
        rows = by[f'{LEVEL}/{weather}_4']
        if [i for i, _, _ in rows] != list(range(max(ids) + 1, max(ids) + 1 + len(rows))):
            raise ValueError(f'{PICKS.name}: the new points are not numbered after {tdm_project}\'s last')
        for k, (i, position, yaw) in enumerate(rows):
            name = f'{PREFIX}{i}'
            out['add_entry'].append({'files': ffa_project, 'list': base.POINTS, 'after': {'point_id': i - 1},
                                     'like': {'point_id': model['point_id']}, 'expected_files': 1, 'values': {
                                         'name': name, 'full_name': 'logic/modes/ffa/respawn/' + name, 'point_id': i,
                                         'team': 2, 'position': position, 'rotation': [0., yaw, 0.], **SOLO}})
            out['add_value'].append({'files': [ffa_project], 'table': f'{base.POINTS}[{len(teams) + k}]',
                                     'key': 'invulnerability_time_ms', 'value': PROTECTION_MS})
        removed, lamps, count = no_crates(tree, ffa_project)
        out['remove_entry'] += removed
        out['set'] += lamps
        crates_said.append(f'{weather} {count}')
    points = len(next(iter(by.values())))
    out['_about'] = (
        f'London (v0.42) in Free for All at the owner\'s picks (2026-11-27: his {points} `where` places on '
        f'{LEVEL}/day_4), in its three weathers, as every Free for All map: its own projects {LEVEL}/<weather>_8, '
        'copies of the Team Deathmatch projects (ffa-projects-1\'s rule) without their ammo crates (make_no_crates_edits\' '
        'rule; crates: ' + ', '.join(crates_said) + '), and its configs gameplay/game_modes/ffa/'
        f'{LEVEL}_<w>_8.options, copies of the Team Deathmatch configs as mode 8 (rules 93, 91, 94, {KILLS_TO_WIN} kills, '
        f'the mode\'s icon {ICON_FFA}), maps_dict ' + ', '.join(f'{i} ({weather})' for _, weather, i in MAPS) + f'; {points} '
        'team-2 points after the teams\', each on its ground (check-points picks --add --shift 2.5 on the Team '
        'Deathmatch projects, the same places in every weather), facing as he faced, 20 m enemy exclusion and 500 ms '
        'protection. Team Deathmatch unchanged. Generated by scripts/ffa-mode/make_london_v042_edits.py from '
        'london-v042-ffa-picks.tsv.')
    return {k: v for k, v in out.items() if v}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', type=Path, required=True)
    args = parser.parse_args()
    base.write(edits(args.live), OUTPUT)
    print(OUTPUT)


if __name__ == '__main__':
    main()
