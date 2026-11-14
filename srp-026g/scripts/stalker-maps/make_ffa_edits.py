#!/usr/bin/env python3
"""Generate ffa-stalker-1.json from checked FFA points and stalker-maps@6, or
(--second) ffa-stalker-2.json from ffa-spawns-2.tsv and stalker-maps-2@1.

--pack is the source pack folder (pack.json and files/). --dictionary is the
composed gameplay/db_static_dictionaries, used to refuse occupied FFA IDs.
The edit is applied after the map pack and the existing FFA content edits.
It requires the current FFA runtime (mode 8; the first list two seconds of
protection, the second 500 ms, the key the client hook of revision 57 reads).
"""
import argparse
import csv
import importlib.util
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
EDIT = HERE.parent / 'content-edit/content_edit.py'
OUT = HERE.parent / 'content-edit/ffa-stalker-1.json'
spec = importlib.util.spec_from_file_location('content_edit', EDIT)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

MAPS = [('cs_mp_bath', 210, 42), ('cs_mp_military_1', 211, 43), ('cs_training_camp', 212, 44)]
# Points per map, the rows of ffa-spawns.tsv in id order: 24 distributed ones; Training Camp's are
# on its raised floors (islands, the towers' stairs, cabins) but for GROUND on its base's ground
# (docs/notes/stalker-ffa.md).
COUNTS = {'cs_mp_bath': 24, 'cs_mp_military_1': 24, 'cs_training_camp': 26}
TRAINING = 'cs_training_camp'
GROUND = 2
# Metres between two points of a map: on the ground plane, and Training Camp's in 3D.
# Training Camp's ground is at about y = -0.8: a point under this height stands on it.
GROUND_BELOW = 1.0
ON_GROUND = 'ground inside the fence'
DICTIONARY = 'gameplay/db_static_dictionaries'
POINTS = 'server_objects/respawn_points'
FFA = 8
PROTECTION = 2


def tree(buf, off=0):
    kind = c.rec(buf, off)[3]
    if kind == c.T_NAMED:
        return {c.name_at(buf, c.rec(buf, k)[1]).decode(): tree(buf, k) for k in c.children(buf, off)}
    if kind == c.T_INDEXED:
        return [tree(buf, k) for k in c.children(buf, off)]
    return c.value_of(buf, off)


def project(key):
    return f'v010b_{key}/day_4'


def ffa_copy(path):
    """The free-for-all's own copy of a TDM project (`day_4` -> `day_8`, ffa-projects-1.json, before
    the FFA lists): the points go there, TDM's project stays as it is."""
    if not path.endswith('/day_4/client_project'):
        raise ValueError(f'{path}: not a TDM project')
    return path.removesuffix('/day_4/client_project') + '/day_8/client_project'


def in_pack(pack, path):
    """The pack's file an edited path starts as: an FFA copy is its TDM project as the pack has it."""
    if path.endswith('/day_8/client_project'):
        path = path.removesuffix('/day_8/client_project') + '/day_4/client_project'
    return pack / 'files' / path


def points():
    by = {project(key): [] for key, _, _ in MAPS}
    with (HERE / 'ffa-spawns.tsv').open() as f:
        for row in csv.DictReader(f, delimiter='\t'):
            if row['project'] not in by or row['verdict'] != 'kept':
                raise ValueError('unexpected project or unchecked spawn')
            for key in ('x', 'y', 'z', 'rx', 'ry', 'rz', 'enemy_check_radius'):
                row[key] = float(row[key])
                if not math.isfinite(row[key]):
                    raise ValueError(f'non-finite {key}')
            if int(row['invulnerability_time']) != PROTECTION or row['enemy_check_radius'] != 30:
                raise ValueError('FFA points require two seconds of protection and a 30 m enemy radius')
            by[row['project']].append(row)
    for key, count in COUNTS.items():
        name, rows = project(key), by[project(key)]
        if len(rows) != count or len({r['point'] for r in rows}) != count:
            raise ValueError(f'{name}: expected {count} uniquely named points')
    stands = [stands_on(row) for row in by[project(TRAINING)]]
    if stands.count(ON_GROUND) != GROUND or None in stands:
        raise ValueError(f'{project(TRAINING)}: every point but {GROUND} on the ground stands on a raised floor')
    return by


def table(name):
    with (HERE / name).open() as f:
        return [{k: v if k in ('island', 'place', 'kind') else float(v) for k, v in row.items()}
                for row in csv.DictReader(f, delimiter='\t')]


def stands_on(p):
    """What a Training Camp point stands on, as the planner names it (ffa-points arena.rs): a cabin,
    else an island, else a tower's stairs (training-islands.tsv, training-towers.tsv), the ground,
    or None on any other floor (a tower's roof, a catwalk, the platform over the hub)."""
    def on(kind):
        return next((f'{"stairs of tower" if kind == "stairs" else kind} {r["place"]}' for r in table('training-towers.tsv')
                     if r['kind'] == kind and r['y_min'] <= p['y'] <= r['y_max']
                     and math.hypot(p['x'] - r['x'], p['z'] - r['z']) <= r['radius']), None)
    island = next((f'raised island {i["island"]}' for i in table('training-islands.tsv')
                   if i['x_min'] <= p['x'] <= i['x_max'] and i['z_min'] <= p['z'] <= i['z_max']
                   and i['y_min'] <= p['y'] <= i['y_max']), None)
    return on('cabin') or island or on('stairs') or (ON_GROUND if p['y'] < GROUND_BELOW else None)


def edits(pack, dictionary):
    manifest = json.loads((pack / 'pack.json').read_text())
    if (manifest['id'], str(manifest['version'])) != ('stalker-maps', '6'):
        raise ValueError('recheck the collision and source assumptions for a pack other than stalker-maps@6')
    rows = tree(dictionary.read_bytes())['maps_dict']
    used = {r['id'] for r in rows}
    names = {r['cfg_mode_name'] for r in rows}
    if len(used) != len(rows):
        raise ValueError('input maps_dict has duplicate IDs')
    if len(names) != len(rows):
        raise ValueError('input maps_dict has duplicate config names')
    checked = points()
    out = {'_about': 'FFA for Experimental Pool, Military Warehouses and Training Camp over '
           'stalker-maps@6; mode 8, maps_dict 42-44. Generated by scripts/stalker-maps/make_ffa_edits.py. '
           f'24 collision-checked team-2 points per map; Training Camp has {COUNTS[TRAINING]} inside its base\'s fence, 8 m apart '
           f'in 3D: {COUNTS[TRAINING] - GROUND} on its raised floors (islands, the towers\' stairs, cabins) and {GROUND} on the ground, '
           '2 s protection, enemy radius 30 m; '
           'TDM points unchanged. Requires the current FFA runtime. Apply after other FFA lists.',
           'copy': [], 'set': [], 'replace_string': [], 'remove_item': [], 'add_value': [], 'add_entry': []}
    for key, tdm_id, ffa_id in MAPS:
        tdm = f'gameplay/game_modes/v010b_{key}_level_03_day_4.options'
        ffa = f'gameplay/game_modes/ffa/v010b_{key}_day_8.options'
        if not any(r['id'] == tdm_id and r['cfg_mode_name'] == tdm.removeprefix('gameplay/game_modes/')
                   for r in rows):
            raise ValueError(f'missing STALKER TDM map {tdm_id}; compose stalker-maps@6 first')
        if ffa_id in used or ffa.removeprefix('gameplay/game_modes/') in names:
            raise ValueError(f'FFA map {ffa_id} or config is already present')
        mode = tree((pack / 'files' / tdm).read_bytes())['mode']
        if (mode['mode'], mode['rules'], mode['match_making_params']['map_name']) != (
                4, [93, 91, 90, 94], project(key)):
            raise ValueError(f'{tdm}: unexpected source config')
        out['copy'].append({'files': tdm, 'from': tdm, 'to': ffa})
        out['set'].append({'file': ffa, 'path': 'mode/mode', 'from': 4, 'to': FFA})
        for old, new in [(mode['name'], f'v010b_{key}_8'), ('st_mode_04_name', 'st_mode_08_name'),
                         ('st_mode_04_description', 'st_mode_08_description')]:
            out['replace_string'].append({'files': ffa, 'from': old, 'to': new})
        out['remove_item'].append({'file': ffa, 'path': 'mode/rules', 'value': 90})
        for table, name, value in [('parameters', 'kills_to_win', 30)]:
            if name in mode[table]:
                raise ValueError(f'{tdm}: {name} already exists')
            out['add_value'].append({'files': [ffa], 'table': 'mode/' + table, 'key': name, 'value': value})
        out['add_entry'].append({'files': DICTIONARY, 'list': 'maps_dict', 'after': {'id': tdm_id},
                                 'like': {'id': tdm_id}, 'values': {'id': ffa_id,
                                 'cfg_mode_name': ffa.removeprefix('gameplay/game_modes/')}})
        source = f'projects/{project(key)}/client_project'
        path = ffa_copy(source)
        buf = (pack / 'files' / source).read_bytes()
        existing = tree(buf)['server_objects']['respawn_points']
        if [(r['team'], r['point_id']) for r in existing] != [(team, i) for team in (0, 1) for i in range(12)]:
            raise ValueError(f'{path}: changed TDM spawn layout; recheck before regenerating')
        if any(existing[0][kind]['collision_geometries'] for kind in ('enemy_zones', 'ally_zones')):
            raise ValueError(f'{path}: template has team-base zones; cannot clone it for FFA')
        for radius in ('enemy_check_radius', 'ally_check_radius'):
            if c.rec(buf, c.find(buf, f'{POINTS}[0]/{radius}'))[3] != c.T_FLOAT:
                raise ValueError(f'{path}: source radius is not a float')
        previous = {'team': 1, 'point_id': 11}
        for i, row in enumerate(checked[project(key)]):
            name = 'ffa_stalker_' + row['point']
            out['add_entry'].append({'files': path, 'list': POINTS, 'after': previous,
                                     'like': {'team': 0, 'point_id': 0}, 'values': {
                'name': name, 'full_name': 'logic/modes/ffa/respawn/' + name, 'point_id': i, 'team': 2,
                'position': [row[k] for k in ('x', 'y', 'z')],
                'rotation': [row[k] for k in ('rx', 'ry', 'rz')],
                'enemy_check_radius': row['enemy_check_radius'], 'ally_check_radius': 1.0,
                'invulnerability_time': PROTECTION, 'priority': 1, 'is_temporary': 0}})
            previous = {'team': 2, 'point_id': i}
    return out


# The second list: Dark Valley, Vehicle Station, Fire Station and Sports Center (pack stalker-maps-2@1,
# maps_dict 33-36 held out of the dictionary by stalker-tdm-hold-1.json), each with its own Free for
# All config on its own project (`day_8`, copied by ffa-projects-1.json) and the points of
# ffa-spawns-2.tsv: the STALKER planner's 24 (ffa-points) made twice as many by the spawn rule
# (check-points' to_cover --times 2), as the first list's maps have them after ffa-cover-maps-1.
OUT_2 = HERE.parent / 'content-edit/ffa-stalker-2.json'
POINTS_2 = HERE / 'ffa-spawns-2.tsv'
PACK_2 = ('stalker-maps-2', '1')
MAPS_2 = [('cs_mp_darkvalley', 33, 37), ('cs_mp_atp', 34, 38), ('cs_mp_firestation', 35, 39), ('cs_mp_sport_center', 36, 40)]
# 24 planned, twice as many by the rule: 48 a map, less the points left out (EXCLUDED_2).
COUNT_2 = 48
# Points the rule keeps where no spawn may stand (the owner, 2026-11-14: no spawn on what only a ladder
# reaches): (project as to_cover names it, point id) -> why. The points after one take the ids down.
EXCLUDED_2 = {('v010b_cs_mp_atp/day_8', 14): 'the roof ladder clmbl#4 (111.1, -33.9; -2.5 -> 1.0) leads to, '
                                            '333 m2 only that ladder reaches on foot'}
RADIUS_2 = 20.0                 # ffa-radius-1: 20 m on every map
PROTECTION_MS_2 = 500           # ffa-protection-ms-1: 0.5 s on every map


def count_2(project_name):
    """A map's points in ffa-spawns-2.tsv: COUNT_2 less those left out."""
    return COUNT_2 - sum(p == project_name.replace('/day_4', '/day_8') for p, _ in EXCLUDED_2)


def points_2(count=None):
    """{project: [rows]} of ffa-spawns-2.tsv, by point id: `count` a map (the planner's 24 alone, as
    to_cover is given them: --planned)."""
    by = {project(key): [] for key, _, _ in MAPS_2}
    with POINTS_2.open() as f:
        for row in csv.DictReader(f, delimiter='\t'):
            if row['project'] not in by:
                raise ValueError(f"{row['project']}: not a map of the second list")
            for key in ('x', 'y', 'z', 'ry'):
                row[key] = float(row[key])
                if not math.isfinite(row[key]):
                    raise ValueError(f'non-finite {key}')
            row['point'] = int(row['point'])
            by[row['project']].append(row)
    for name, rows in by.items():
        n = count or count_2(name)
        if [r['point'] for r in rows] != list(range(n)):
            raise ValueError(f'{name}: expected points 0..{n - 1} in order')
    return by


def table_2(planned, cover):
    """ffa-spawns-2.tsv's rows: the planner's points (ffa-points' table of the four day_4 projects) with
    to_cover's rows over them (`replaced` and `turned` take the point's place, `added` are new points),
    on day_8 as to_cover names the projects; a point to_cover keeps stays as planned."""
    rows = {}
    with Path(planned).open() as f:
        for r in csv.DictReader(f, delimiter='\t'):
            point = int(r['point'].removeprefix('ported_'))
            rows[(r['project'].replace('/day_4', '/day_8'), point)] = dict(
                x=r['x'], y=r['y'], z=r['z'], ry=r['ry'], verdict='planned', reason=r['reason'])
    with Path(cover).open() as f:
        for r in csv.DictReader(f, delimiter='\t'):
            key = (r['project'], int(r['point']))
            if r['verdict'] not in ('replaced', 'turned', 'added') or (r['verdict'] == 'added') == (key in rows):
                raise ValueError(f'{key}: to_cover says {r["verdict"]}')
            rows[key] = dict(x=r['x'], y=r['y'], z=r['z'], ry=r['ry'], verdict=r['verdict'], reason=r['reason'])
    if any(v['verdict'] != 'planned' for v in rows.values()):   # the rule's table, not the planner's alone
        for key in EXCLUDED_2:
            rows.pop(key)
    head = ['project', 'point', 'x', 'y', 'z', 'ry', 'verdict', 'reason']
    lines = ['\t'.join(head)]
    order = [k for k, _, _ in MAPS_2]
    for proj in sorted({p for p, _ in rows}, key=lambda p: order.index(p.split('/')[0].removeprefix('v010b_'))):
        for point, (_, old) in enumerate(sorted(k for k in rows if k[0] == proj)):
            r = rows[(proj, old)]
            lines.append('\t'.join([proj.replace('/day_8', '/day_4'), str(point), r['x'], r['y'], r['z'], r['ry'], r['verdict'], r['reason']]))
    return '\n'.join(lines) + '\n'


def edits_2(pack, dictionary, count=None):
    manifest = json.loads((pack / 'pack.json').read_text())
    if (manifest['id'], str(manifest['version'])) != PACK_2:
        raise ValueError('recheck the collision and the points for a pack other than stalker-maps-2@1')
    rows = tree(dictionary.read_bytes())['maps_dict']
    used = {r['id'] for r in rows}
    names = {r['cfg_mode_name'] for r in rows}
    if len(used) != len(rows) or len(names) != len(rows):
        raise ValueError('input maps_dict has duplicate IDs or config names')
    checked = points_2(count)
    points = (f'{count} team-2 points a map' if count else
              f'{COUNT_2} team-2 points a map (Vehicle Station 47: none on the roof only its ladder clmbl#4 reaches)')
    out = {'_about': 'Free for All on Dark Valley, Vehicle Station, Fire Station and Sports Center (pack stalker-maps-2@1; '
           'maps_dict 37-40, their Team Deathmatch rows 33-36 held out by stalker-tdm-hold-1). Generated by '
           'scripts/stalker-maps/make_ffa_edits.py --second from ffa-spawns-2.tsv. Each map\'s config is a copy of its '
           'Team Deathmatch config as mode 8 (rules 93/91/94, 30 kills) on its own project, day_8 (ffa-projects-1); '
           f'{points} by the spawn rule (cover, past the perimeter band; ffa.md), '
           f'{RADIUS_2:g} m enemy exclusion and {PROTECTION_MS_2} ms protection; the maps have no ammo crates. '
           'TDM points unchanged. Requires the current FFA runtime.',
           'copy': [], 'set': [], 'replace_string': [], 'remove_item': [], 'add_value': [], 'add_entry': []}
    for key, tdm_id, ffa_id in MAPS_2:
        tdm = f'gameplay/game_modes/v010b_{key}_level_03_day_4.options'
        ffa = f'gameplay/game_modes/ffa/v010b_{key}_day_8.options'
        if not any(r['id'] == tdm_id and r['cfg_mode_name'] == tdm.removeprefix('gameplay/game_modes/') for r in rows):
            raise ValueError(f'missing STALKER TDM map {tdm_id}; compose stalker-maps-2@1 first')
        if ffa_id in used or ffa.removeprefix('gameplay/game_modes/') in names:
            raise ValueError(f'FFA map {ffa_id} or config is already present')
        mode = tree((pack / 'files' / tdm).read_bytes())['mode']
        if (mode['mode'], mode['rules'], mode['match_making_params']['map_name']) != (4, [93, 91, 90, 94], project(key)):
            raise ValueError(f'{tdm}: unexpected source config')
        out['copy'].append({'files': tdm, 'from': tdm, 'to': ffa})
        out['set'].append({'file': ffa, 'path': 'mode/mode', 'from': 4, 'to': FFA})
        for old, new in [(mode['name'], f'v010b_{key}_8'), ('st_mode_04_name', 'st_mode_08_name'),
                         ('st_mode_04_description', 'st_mode_08_description'),
                         (project(key), ffa_copy(f'projects/{project(key)}/client_project')
                          .removeprefix('projects/').removesuffix('/client_project'))]:
            out['replace_string'].append({'files': ffa, 'from': old, 'to': new})
        out['remove_item'].append({'file': ffa, 'path': 'mode/rules', 'value': 90})
        if 'kills_to_win' in mode['parameters']:
            raise ValueError(f'{tdm}: kills_to_win already exists')
        out['add_value'].append({'files': [ffa], 'table': 'mode/parameters', 'key': 'kills_to_win', 'value': 30})
        out['add_entry'].append({'files': DICTIONARY, 'list': 'maps_dict', 'after': {'id': tdm_id},
                                 'like': {'id': tdm_id}, 'values': {'id': ffa_id,
                                 'cfg_mode_name': ffa.removeprefix('gameplay/game_modes/')}})
        source = f'projects/{project(key)}/client_project'
        path = ffa_copy(source)
        buf = (pack / 'files' / source).read_bytes()
        existing = tree(buf)['server_objects']['respawn_points']
        teams = [sum(r['team'] == t for r in existing) for t in (0, 1)]
        if [(r['team'], r['point_id']) for r in existing] != [(t, i) for t in (0, 1) for i in range(teams[t])]:
            raise ValueError(f'{path}: unexpected TDM spawn layout')
        if any(existing[0][kind]['collision_geometries'] for kind in ('enemy_zones', 'ally_zones')):
            raise ValueError(f'{path}: template has team-base zones; cannot clone it for FFA')
        for radius in ('enemy_check_radius', 'ally_check_radius'):
            if c.rec(buf, c.find(buf, f'{POINTS}[0]/{radius}'))[3] != c.T_FLOAT:
                raise ValueError(f'{path}: source radius is not a float')
        previous = {'team': 1, 'point_id': teams[1] - 1}
        for row in checked[project(key)]:
            i = row['point']
            name = f'ffa_stalker_{i:02d}'
            out['add_entry'].append({'files': path, 'list': POINTS, 'after': previous,
                                     'like': {'team': 0, 'point_id': 0}, 'values': {
                'name': name, 'full_name': 'logic/modes/ffa/respawn/' + name, 'point_id': i, 'team': 2,
                'position': [row[k] for k in ('x', 'y', 'z')], 'rotation': [0.0, row['ry'], 0.0],
                'enemy_check_radius': RADIUS_2, 'ally_check_radius': 1.0,
                'invulnerability_time': 0, 'priority': 1, 'is_temporary': 0}})
            out['add_value'].append({'files': [path], 'table': f'{POINTS}[{len(existing) + i}]',
                                     'key': 'invulnerability_time_ms', 'value': PROTECTION_MS_2})
            previous = {'team': 2, 'point_id': i}
    return out


def encoded(edits):
    lines = ['{', ' "_about": ' + json.dumps(edits['_about']) + ',']
    keys = [k for k in edits if k != '_about']
    for i, key in enumerate(keys):
        lines += [' ' + json.dumps(key) + ': [',
                  ',\n'.join('  ' + json.dumps(e) for e in edits[key]),
                  ' ]' + (',' if i + 1 < len(keys) else '')]
    return '\n'.join(lines + ['}']) + '\n'


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--table', nargs=2, metavar=('PLANNED', 'COVER'),
                    help="write ffa-spawns-2.tsv from ffa-points' table and to_cover's rows, and nothing else")
    ap.add_argument('--pack', type=Path)
    ap.add_argument('--dictionary', type=Path)
    ap.add_argument('--second', action='store_true', help='the second list: ffa-stalker-2.json over stalker-maps-2@1')
    ap.add_argument('--planned', type=int, metavar='N', help="--second with the planner's N points a map alone (to_cover's input)")
    ap.add_argument('--out', type=Path)
    ap.add_argument('--check', action='store_true')
    a = ap.parse_args()
    if a.table:
        POINTS_2.write_text(table_2(*a.table))
        print(POINTS_2)
        return
    if not (a.pack and a.dictionary):
        ap.error('--pack and --dictionary are required')
    a.out = a.out or (OUT_2 if a.second else OUT)
    text = encoded(edits_2(a.pack, a.dictionary, a.planned) if a.second else edits(a.pack, a.dictionary))
    if a.check:
        if a.out.read_text() != text:
            raise SystemExit('FFA edit list differs from its sources')
        print('FFA edit list matches its sources')
    else:
        a.out.write_text(text)
        print(a.out)


if __name__ == '__main__':
    main()
