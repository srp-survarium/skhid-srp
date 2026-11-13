#!/usr/bin/env python3
"""Generate ffa-stalker-1.json from checked FFA points and stalker-maps@6.

--pack is the source pack folder (pack.json and files/). --dictionary is the
composed gameplay/db_static_dictionaries, used to refuse occupied FFA IDs.
The edit is applied after the map pack and the existing FFA content edits.
It requires the current FFA runtime (mode 8, two seconds of protection).
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
# Points per map: 24 distributed ones (Training Camp's moved perimeter ones in their place),
# then Training Camp's on its raised islands (training-islands.tsv), which take the next IDs.
COUNTS = {'cs_mp_bath': 24, 'cs_mp_military_1': 24, 'cs_training_camp': 29}
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
        for i, a in enumerate(rows):
            for b in rows[:i]:
                if spacing(a, b) < 12 - 0.001:
                    raise ValueError(f'{name}: points less than 12 m apart')
    return by


def islands():
    with (HERE / 'training-islands.tsv').open() as f:
        return [{k: v if k == 'island' else float(v) for k, v in r.items()}
                for r in csv.DictReader(f, delimiter='\t')]


def on_island(p):
    return any(i['x_min'] <= p['x'] <= i['x_max'] and i['z_min'] <= p['z'] <= i['z_max']
               and i['y_min'] <= p['y'] <= i['y_max'] for i in islands())


def spacing(a, b):
    """Horizontal, or in 3D when either stands on a Training Camp island (the owner, 2026-11-13)."""
    if a['project'] == project('cs_training_camp') and (on_island(a) or on_island(b)):
        return math.dist((a['x'], a['y'], a['z']), (b['x'], b['y'], b['z']))
    return math.hypot(a['x'] - b['x'], a['z'] - b['z'])


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
           '24 collision-checked team-2 points per map; Training Camp moves 7 perimeter ones to its centre and adds 5 on its raised islands, '
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
        for table, name, value in [('parameters', 'kills_to_win', 30),
                                   ('match_making_params', 'max_players', 10)]:
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
    ap.add_argument('--pack', type=Path, required=True)
    ap.add_argument('--dictionary', type=Path, required=True)
    ap.add_argument('--out', type=Path, default=OUT)
    ap.add_argument('--check', action='store_true')
    a = ap.parse_args()
    text = encoded(edits(a.pack, a.dictionary))
    if a.check:
        if a.out.read_text() != text:
            raise SystemExit('FFA edit list differs from its sources')
        print('FFA edit list matches its sources')
    else:
        a.out.write_text(text)
        print(a.out)


if __name__ == '__main__':
    main()
