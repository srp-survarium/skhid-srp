#!/usr/bin/env python3
"""Generate ffa-stalker-1.json from checked FFA points and stalker-maps@6, (--second)
ffa-stalker-2.json from ffa-spawns-2.tsv and stalker-maps-2@2, or (--third) ffa-stalker-3.json from
ffa-spawns-3.tsv and stalker-maps@7 (Station; Depository held).

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
import sys
from collections.abc import Callable
from dataclasses import dataclass
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


# The lists after the first: each map with its own Free for All config on its own project (`day_8`,
# copied by ffa-projects-1.json) and the points of its table: the STALKER planner's (ffa-points)
# made twice as many by the spawn rule (check-points' to_cover --times 2), as the first list's maps
# have them after ffa-cover-maps-1. Their Team Deathmatch rows are held out of the dictionary by
# stalker-tdm-hold-1.json, which comes after them.
@dataclass(frozen=True)
class Later:
    out: Path
    points: Path
    pack: tuple
    maps: list                  # (key, TDM maps_dict id, Free for All id)
    held: list                  # maps whose points stay in the table, not offered
    count: dict                 # the rule's points a map (twice the planner's)
    excluded: dict              # (project as to_cover names it, point id) -> why it is left out
    crates: bool                # the TDM project has ammo crates: the copy loses them
    about: Callable             # (the points a map, planned only) -> the list's _about


RADIUS_2 = 20.0                 # ffa-radius-1: 20 m on every map
PROTECTION_MS_2 = 500           # ffa-protection-ms-1: 0.5 s on every map

SECOND = Later(
    out=HERE.parent / 'content-edit/ffa-stalker-2.json', points=HERE / 'ffa-spawns-2.tsv',
    pack=('stalker-maps-2', '2'),
    maps=[('cs_mp_firestation', 35, 39)],
    # Held out (the owner, 2026-11-16: "let's merge only single map for now which is fire station"): their
    # points stay in ffa-spawns-2.tsv and their ids stay theirs; move one up to maps to offer it.
    held=[('cs_mp_darkvalley', 33, 37), ('cs_mp_atp', 34, 38), ('cs_mp_sport_center', 36, 40)],
    count={key: 48 for key in ('cs_mp_firestation', 'cs_mp_darkvalley', 'cs_mp_atp', 'cs_mp_sport_center')},
    # Points the rule keeps where no spawn may stand (the owner, 2026-11-14: no spawn on what only a ladder
    # reaches). The points after one take the ids down.
    excluded={('v010b_cs_mp_atp/day_8', 14): 'the roof ladder clmbl#4 (111.1, -33.9; -2.5 -> 1.0) leads to, '
                                             '333 m2 only that ladder reaches on foot'},
    crates=False,
    about=lambda counts, planned: (
        'Free for All on Fire Station (pack stalker-maps-2; maps_dict 39, its Team Deathmatch row 35 held '
        'out by stalker-tdm-hold-1; Dark Valley, Vehicle Station and Sports Center held, the owner, 2026-11-16: '
        '"let\'s merge only single map for now which is fire station"). Generated by '
        'scripts/stalker-maps/make_ffa_edits.py --second from ffa-spawns-2.tsv. Its config is a copy of its '
        'Team Deathmatch config as mode 8 (rules 93/91/94, 30 kills) on its own project, day_8 (ffa-projects-1); '
        f'{counts["cs_mp_firestation"]} team-2 points by the spawn rule (cover, past the perimeter band; ffa.md), '
        f'{RADIUS_2:g} m enemy exclusion and {PROTECTION_MS_2} ms protection; the map has no ammo crates. '
        'TDM points unchanged. Requires the current FFA runtime.'))

# Station and Depository of stalker-maps@7 (the owner, 2026-11-21: spawns by the spawn rule, as Fire
# Station's were made): 31 and 32, the free ids under the STALKER maps' 33-44; Depository held.
THIRD = Later(
    out=HERE.parent / 'content-edit/ffa-stalker-3.json', points=HERE / 'ffa-spawns-3.tsv',
    pack=('stalker-maps', '7'),
    maps=[('soc_ug_station', 214, 31)],
    # Held out (the owner, 2026-11-21: "only station is fine"): its points stay in ffa-spawns-3.tsv and
    # its id stays its own; move it up to maps to offer it.
    held=[('soc_ug_depository', 215, 32)],
    # Station's walkable floor is 3.4 times Fire Station's: twice its points (ffa-points' STATION_COUNT).
    count={'soc_ug_station': 96, 'soc_ug_depository': 48},
    excluded={},
    crates=True,
    about=lambda counts, planned: (
        'Free for All on Station (pack stalker-maps@7; maps_dict 31, its Team Deathmatch row 214 held out by '
        'stalker-tdm-hold-1; the owner, 2026-11-21: spawns by the spawn rule, as Fire Station\'s; Depository held, '
        '"only station is fine"). Generated by scripts/stalker-maps/make_ffa_edits.py --third from '
        'ffa-spawns-3.tsv. Its config is a copy of its Team Deathmatch config as mode 8 (rules 93/91/94, '
        f'30 kills) on its own project, day_8 (ffa-projects-1); {counts["soc_ug_station"]} team-2 points '
        + ('planned, before the spawn rule' if planned else 'by the spawn rule (cover, past the perimeter band; ffa.md)')
        + f', {RADIUS_2:g} m enemy exclusion and {PROTECTION_MS_2} ms protection; every ammo crate of the copy '
        'removed whole, as no-ammo-crates-1 removes the other maps\'. TDM points unchanged. Requires the '
        'current FFA runtime.'))


def count_of(later, project_name):
    """A map's points in its table: its count less those left out."""
    key = project_name.split('/')[0].removeprefix('v010b_')
    return later.count[key] - sum(p == project_name.replace('/day_4', '/day_8') for p, _ in later.excluded)


def points_of(later, planned=False):
    """{project: [rows]} of the list's table, by point id: the planner's alone (half the count, as
    to_cover is given them: --planned) or the rule's."""
    by = {project(key): [] for key, _, _ in later.maps}
    held = {project(key) for key, _, _ in later.held}
    with later.points.open() as f:
        for row in csv.DictReader(f, delimiter='\t'):
            if row['project'] in held:
                continue
            if row['project'] not in by:
                raise ValueError(f"{row['project']}: not a map of {later.out.name}")
            for key in ('x', 'y', 'z', 'ry'):
                row[key] = float(row[key])
                if not math.isfinite(row[key]):
                    raise ValueError(f'non-finite {key}')
            row['point'] = int(row['point'])
            by[row['project']].append(row)
    for name, rows in by.items():
        n = later.count[name.split('/')[0].removeprefix('v010b_')] // 2 if planned else count_of(later, name)
        if [r['point'] for r in rows] != list(range(n)):
            raise ValueError(f'{name}: expected points 0..{n - 1} in order')
    return by


def table_of(later, planned, cover):
    """The list's table: the planner's points (ffa-points' table of the day_4 projects) with
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
        for key in later.excluded:
            rows.pop(key)
    head = ['project', 'point', 'x', 'y', 'z', 'ry', 'verdict', 'reason']
    lines = ['\t'.join(head)]
    order = [k for k, _, _ in sorted(later.maps + later.held, key=lambda m: m[1])]
    for proj in sorted({p for p, _ in rows}, key=lambda p: order.index(p.split('/')[0].removeprefix('v010b_'))):
        for point, (_, old) in enumerate(sorted(k for k in rows if k[0] == proj)):
            r = rows[(proj, old)]
            lines.append('\t'.join([proj.replace('/day_8', '/day_4'), str(point), r['x'], r['y'], r['z'], r['ry'], r['verdict'], r['reason']]))
    return '\n'.join(lines) + '\n'


def no_crates(buf, path):
    """The remove_entry rows that take every ammo crate out of the copy `path` of a TDM project
    (`buf`), as no-ammo-crates-1 takes them out of the other Free for All copies."""
    sys.path.insert(0, str(HERE.parent / 'ffa-mode'))
    import make_no_crates_edits as no_crates_edits
    tree_ = no_crates_edits.base.tree(buf)
    found, models = no_crates_edits.crates(tree_, path)
    if not found:
        raise ValueError(f'{path}: no ammo crate')
    if tree_.get('srp_lamps'):
        raise ValueError(f'{path}: breakable lamps would need their visual indices moved')
    out = [{'file': path, 'list': 'client_objects',
            'where': {'game_object_type': o['game_object_type'], 'full_name': o['full_name']}}
           for _, inside in found for o in inside]
    for _, v in models:
        out.append({'file': path, 'list': 'render_visuals',
                    'where': {'lib_name': v['lib_name'], 'position': no_crates_edits.at(v['position'])}})
        prefix = f'resources/models/{v["lib_name"]}.model/'
        collisions = [c for c in tree_['collision_objects'] if c['lib_name'].startswith(prefix)
                      and math.dist(c['position'], v['position']) < 0.0005]
        if not collisions:
            raise ValueError(f'{path}: {v["lib_name"]} has no collision')
        out += [{'file': path, 'list': 'collision_objects',
                 'where': {'lib_name': c['lib_name'], 'position': no_crates_edits.at(c['position'])}}
                for c in collisions]
    return out


def edits_of(later, pack, dictionary, planned=False):
    manifest = json.loads((pack / 'pack.json').read_text())
    if (manifest['id'], str(manifest['version'])) != later.pack:
        raise ValueError(f'recheck the collision and the points for a pack other than {"@".join(later.pack)}')
    rows = tree(dictionary.read_bytes())['maps_dict']
    used = {r['id'] for r in rows}
    names = {r['cfg_mode_name'] for r in rows}
    if len(used) != len(rows) or len(names) != len(rows):
        raise ValueError('input maps_dict has duplicate IDs or config names')
    checked = points_of(later, planned)
    counts = {key: len(checked[project(key)]) for key, _, _ in later.maps}
    out = {'_about': later.about(counts, planned),
           'copy': [], 'set': [], 'replace_string': [], 'remove_item': [], 'add_value': [], 'add_entry': []}
    if later.crates:
        out['remove_entry'] = []
    for key, tdm_id, ffa_id in later.maps:
        tdm = f'gameplay/game_modes/v010b_{key}_level_03_day_4.options'
        ffa = f'gameplay/game_modes/ffa/v010b_{key}_day_8.options'
        if not any(r['id'] == tdm_id and r['cfg_mode_name'] == tdm.removeprefix('gameplay/game_modes/') for r in rows):
            raise ValueError(f'missing STALKER TDM map {tdm_id}; compose {"@".join(later.pack)} first')
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
        if later.crates:
            out['remove_entry'] += no_crates(buf, path)
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
                    help="write the list's table (--second, --third) from ffa-points' table and to_cover's rows, and nothing else")
    ap.add_argument('--pack', type=Path)
    ap.add_argument('--dictionary', type=Path)
    which = ap.add_mutually_exclusive_group()
    which.add_argument('--second', action='store_true', help='the second list: ffa-stalker-2.json over stalker-maps-2@2')
    which.add_argument('--third', action='store_true', help='the third list: ffa-stalker-3.json over stalker-maps@7')
    ap.add_argument('--planned', action='store_true', help="--second, --third with the planner's points alone (to_cover's input)")
    ap.add_argument('--out', type=Path)
    ap.add_argument('--check', action='store_true')
    a = ap.parse_args()
    later = SECOND if a.second else THIRD if a.third else None
    if a.table:
        if not later:
            ap.error('--table needs --second or --third')
        later.points.write_text(table_of(later, *a.table))
        print(later.points)
        return
    if not (a.pack and a.dictionary):
        ap.error('--pack and --dictionary are required')
    a.out = a.out or (later.out if later else OUT)
    text = encoded(edits_of(later, a.pack, a.dictionary, a.planned) if later else edits(a.pack, a.dictionary))
    if a.check:
        if a.out.read_text() != text:
            raise SystemExit('FFA edit list differs from its sources')
        print('FFA edit list matches its sources')
    else:
        a.out.write_text(text)
        print(a.out)


if __name__ == '__main__':
    main()
