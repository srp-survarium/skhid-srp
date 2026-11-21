#!/usr/bin/env python3
"""London (v0.32)'s edit lists: the live London's maps switched to 0.32a4's own projects (pack london-v032,
projects v032a_level_09/<weather>_<mode>), with the owner's edits of London on them.

    python3 make_edits.py --live <folder> --pack <pack files>

<folder> holds what players have of London (projects/level_09/*/client_project, gameplay/game_modes/*level_09*):
the owner's placements are read there. <pack files> is the pack's `files` folder: the new projects as the pack
makes them (the `from` values; no row above these lists edits their spawns, crates or lamps).

Writes, in the order of their rows in release/content.tsv:
- london-v032-spawns-1.json: each project's 20 team spawns where the owner placed them (london-1.json, the map
  editor; the rain and evening copies of the day, london-weathers), by full_name: the donor's names are the live
  London's, the same 20 in every project;
- ffa-london-v032-1.json: Free for All's own project v032a_level_09/day_8, a copy of day_4 as the list above leaves
  it, with the live Free for All's 50 spawns (the owner's picks, ffa-london-picks-1, with every field the FFA lists
  gave them) and without ammo crates (make_no_crates_edits' rule);
- london-v032-1.json: the switch: every London config (the ten modes, Blackout's and Free for All's) names its
  v032a project in mode.match_making_params.map_name.
"""
import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'ffa-mode'))
import make_edits as base  # noqa: E402
import make_no_crates_edits as crates_rule  # noqa: E402

OUT = HERE.parent / 'content-edit'
OLD, NEW = 'level_09', 'v032a_level_09'
PROJECTS = [f'{w}_{m}' for w in ('day',) for m in (2, 3, 4, 5)] + [f'{w}_{m}' for w in ('rain', 'evening') for m in (2, 3, 4)]
POINTS = base.POINTS
CONFIGS = {f'gameplay/game_modes/level_09_{w}_{m}.options': f'{"evening" if w == "evn" else w}_{m}'
           for w, ms in (('day', (2, 3, 4, 5)), ('rain', (2, 3, 4)), ('evn', (2, 3, 4))) for m in ms}
CONFIGS['gameplay/game_modes/blackout/level_09_day_2.options'] = 'day_2'
CONFIGS['gameplay/game_modes/ffa/level_09_day_8.options'] = 'day_8'


def project(root, level, name):
    return base.tree((root / f'projects/{level}/{name}/client_project').read_bytes())


def path_of(level, name):
    return f'projects/{level}/{name}/client_project'


def at(v):
    return [round(x, 6) for x in v]


def spawns(live, pack):
    out, removed = [], []
    for name in PROJECTS:
        mine = {p['full_name']: p for p in project(live, OLD, name)['server_objects']['respawn_points']}
        theirs = {p['full_name']: p for p in project(pack, NEW, name)['server_objects']['respawn_points']}
        if set(mine) != set(theirs) or len(mine) != 20:
            raise ValueError(f'{name}: the live and the donor spawns are not the same 20 names')
        for full_name in sorted(mine):
            p, d = mine[full_name], theirs[full_name]
            out.append({'file': path_of(NEW, name), 'list': POINTS, 'where': {'full_name': full_name},
                        'from': {'position': at(d['position']), 'rotation': at(d['rotation'])},
                        'to': {'position': at(p['position']), 'rotation': at(p['rotation'])}})
        # london-1's other half: the spawn protection zones around the donor's own spawn places (an
        # effect_zone and its folder's inv_collision) go where the live project has none of the name
        live_zones = {o['full_name'] for o in project(live, OLD, name)['client_objects']
                      if o.get('game_object_type') == 'effect_zone'}
        objects = project(pack, NEW, name)['client_objects']
        for zone in objects:
            if zone.get('game_object_type') != 'effect_zone' or zone['full_name'] in live_zones:
                continue
            folder = zone['full_name'].rsplit('/', 1)[0]
            shape = [o for o in objects if o['full_name'] == f'{folder}/inv_collision'
                     and o.get('game_object_type') == 'collision_geometry']
            if len(shape) != 1:
                raise ValueError(f'{name}: {zone["full_name"]} has no one inv_collision')
            for o in (zone, shape[0]):
                removed.append({'file': path_of(NEW, name), 'list': 'client_objects',
                                'where': {'full_name': o['full_name'], 'game_object_type': o['game_object_type']}})
    about = ("London (v0.32)'s team spawns where the owner placed them on London (the owner, 2026-10-25, in the map "
             'editor: london-1.json; the rain and evening as their day, london-weathers): in each of the ten '
             'v032a_level_09 projects its 20 points by full_name (0.32a4 names them as the live London), position '
             'and rotation as players have them; nothing else of a point changes. The spawn protection zones '
             "around 0.32a4's own spawn places (each effect_zone with its inv_collision; the live London has none "
             'since london-1) are removed. None is moved to fit the new collision: scripts/ffa-mode/check-points '
             'report checks each where it stands (docs/notes/london-v032.md). Generated by '
             'scripts/london-v032/make_edits.py.')
    return {'_about': about, 'set_entry': out, 'remove_entry': removed}


def ffa(live, pack):
    day_4 = project(pack, NEW, 'day_4')
    target = path_of(NEW, 'day_8')
    played = [p for p in project(live, OLD, 'day_8')['server_objects']['respawn_points'] if p['team'] == 2]
    if len(played) != 50:
        raise ValueError(f'the live Free for All has {len(played)} spawns, not the owner\'s 50')
    teams = day_4['server_objects']['respawn_points']
    ids = [p['point_id'] for p in teams]
    if len(set(ids)) != len(ids) or set(ids) & {p['point_id'] for p in played}:
        raise ValueError(f'{target}: point ids of the teams and of the Free for All are not distinct')
    model = teams[0]
    adds, values_added, after = [], [], {'point_id': max(ids)}
    for k, p in enumerate(sorted(played, key=lambda p: p['point_id'])):
        values = {key: v for key, v in p.items() if key in model and not isinstance(v, dict)}
        adds.append({'files': target, 'list': POINTS, 'after': after, 'like': {'point_id': model['point_id']},
                     'values': values})
        # keys a team point has not (the FFA lists' add_value): at the point's index after the adds
        for key in sorted(set(p) - set(model)):
            values_added.append({'files': [target], 'table': f'{POINTS}[{len(teams) + k}]', 'key': key, 'value': p[key]})
        after = {'point_id': p['point_id']}
    removed, models = [], []
    found, models = crates_rule.crates(day_4, target)
    if not found:
        raise ValueError(f'{target}: no ammo crate')
    for _, inside in found:
        for o in inside:
            removed.append({'file': target, 'list': 'client_objects',
                            'where': {'game_object_type': o['game_object_type'], 'full_name': o['full_name']}})
    for _, v in models:
        removed.append({'file': target, 'list': 'render_visuals',
                        'where': {'lib_name': v['lib_name'], 'position': crates_rule.at(v['position'])}})
        prefix = f'resources/models/{v["lib_name"]}.model/'
        for c in day_4['collision_objects']:
            if c['lib_name'].startswith(prefix) and sum((a - b) ** 2 for a, b in zip(c['position'], v['position'])) < 2.5e-7:
                removed.append({'file': target, 'list': 'collision_objects',
                                'where': {'lib_name': c['lib_name'], 'position': crates_rule.at(c['position'])}})
    about = ("London (v0.32)'s Free for All on its own project (as every Free for All map, ffa-projects-1): "
             'v032a_level_09/day_8, a copy of day_4 as london-v032-spawns-1 leaves it; the live Free for All\'s 50 '
             "spawns (team 2: the owner's picks of 2026-11-16, ffa-london-picks-1, with the radii and the 500 ms "
             'of protection the FFA lists gave them), each with every value players have; its ammo crates removed '
             f'whole as make_no_crates_edits removes one ({len(found)} crates). None is moved to fit the new '
             'collision: scripts/ffa-mode/check-points report checks each where it stands (docs/notes/london-v032.md). Generated by '
             'scripts/london-v032/make_edits.py.')
    return {'_about': about,
            'copy': [{'files': path_of(NEW, 'day_4'), 'from': f'projects/{NEW}/day_4/', 'to': f'projects/{NEW}/day_8/'}],
            'add_entry': adds, 'add_value': values_added, 'remove_entry': removed}


def switch(live):
    out = []
    for config, name in sorted(CONFIGS.items()):
        have = base.tree((live / config).read_bytes())['mode']['match_making_params']['map_name']
        if have != f'{OLD}/{name}':
            raise ValueError(f'{config}: map_name {have}, not {OLD}/{name}')
        out.append({'files': config, 'from': have, 'to': f'{NEW}/{name}'})
    about = ('London is London (v0.32): each of its configs (the ten modes of day, rain and evening, Blackout\'s and '
             'Free for All\'s) plays 0.32a4\'s own project of its weather and mode (pack london-v032, '
             'v032a_level_09/<weather>_<mode>) in mode.match_making_params.map_name. Its map ids, level 9, names '
             'and loading pictures stay; the old projects level_09/* are no config\'s any more. Generated by '
             'scripts/london-v032/make_edits.py.')
    return {'_about': about, 'replace_string': out}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--live', type=Path, required=True)
    ap.add_argument('--pack', type=Path, required=True)
    a = ap.parse_args()
    for name, edits in (('london-v032-spawns-1.json', spawns(a.live, a.pack)), ('ffa-london-v032-1.json', ffa(a.live, a.pack)),
                        ('london-v032-1.json', switch(a.live))):
        base.write(edits, OUT / name)
        print(OUT / name, sum(len(v) for k, v in edits.items() if isinstance(v, list)))


if __name__ == '__main__':
    main()
