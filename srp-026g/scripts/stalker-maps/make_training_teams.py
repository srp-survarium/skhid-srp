#!/usr/bin/env python3
"""Training Camp in Team Deathmatch and Freeze Tag at Free for All's points (the owner, 2026-11-22:
"add tdm and freeze tag on stk2 training camp, there are already many spawn points, we should just
reuse them, all teams spawns everywhere randomly (there are no 'map sides')"; "not the same rules, the
same spawn points that already exist"). Writes ../content-edit/training-camp-teams-1.json.

usage: python3 -I make_training_teams.py --live <folder>

<folder> holds projects/v010b_cs_training_camp/day_8/client_project as every row of
release/content.tsv above the list leaves it (Free for All's project at the owner's 47 picks,
ffa-training-camp-picks-1; a build-content patch's file, or the build cache's).

The list:

- copies Free for All's project day_8 (the fence's wall, the trucks in its gateways, the burning
  campfires, no ammo crates) to day_teams, the two team modes' project;
- in the copy, each of the 47 Free for All points is a point of team 0 and one of team 1 at its
  place and facing, point ids 0-46 in each team: Team Deathmatch's own 12 points of each team
  (ids 0-11, most at the gateways, outside the fence) take the first 12 places, 35 more of each team are
  made like them; every point is Team Deathmatch's own kind (6 s protection, 1.2 m enemy and 0.8 m
  ally radius, priority 1), the game's own choice picks one of a team's free points at random;
  the 47 team-2 points go (a team mode reads only teams 0 and 1);
- plays Team Deathmatch's config (maps_dict 212, held out since stalker-tdm-hold-1) on day_teams
  and gives it its dictionary row back;
- makes Freeze Tag's config of it as freeze-tag-1 makes Rudnya's (mode 12, its icon and names, no
  ticket rule) under gameplay/game_modes/freeze/, maps_dict 450.
"""
import argparse
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

LIST = HERE.parent / 'content-edit' / 'training-camp-teams-1.json'
FFA = 'projects/v010b_cs_training_camp/day_8/'
TEAMS = 'projects/v010b_cs_training_camp/day_teams/'
PROJECT = TEAMS + 'client_project'
# A config's match_making_params.map_name: Team Deathmatch's project, the team modes' copy.
TDM_PROJECT, TEAMS_PROJECT = 'v010b_cs_training_camp/day_4', 'v010b_cs_training_camp/day_teams'
POINTS = 'server_objects/respawn_points'
MODES = 'gameplay/game_modes/'
TDM = 'v010b_cs_training_camp_level_03_day_4.options'
FREEZE = 'freeze/' + TDM
TDM_ID, FREEZE_ID = 212, 450
# Team Deathmatch's own points' full names, by team.
FULL_NAME = {0: 'logic/deathmatch/team1/respawn_point(%d)', 1: 'logic/deathmatch/team2/respawn_point(%d)'}


def vector(values):
    return [round(v, 6) for v in values]


def edits(live):
    project = bcw.loads((live / FFA / 'client_project').read_bytes())
    points = project['server_objects']['respawn_points']
    solo = sorted((e for e in points if e['team'] == 2), key=lambda e: e['point_id'])
    assert [e['point_id'] for e in solo] == list(range(len(solo))), 'team-2 ids not 0..n'
    own = {team: sorted((e for e in points if e['team'] == team), key=lambda e: e['point_id'])
           for team in (0, 1)}
    for team, entries in own.items():
        assert [e['point_id'] for e in entries] == list(range(len(entries))), f'team {team} ids not 0..n'
        assert len(entries) <= len(solo)
    out = {
        '_about': '',
        'copy': [
            {'files': FFA + 'client_project', 'from': FFA, 'to': TEAMS},
            {'files': MODES + TDM, 'from': MODES, 'to': MODES + 'freeze/'},
        ],
        'set': [
            {'file': MODES + FREEZE, 'path': 'mode/mode', 'from': 4, 'to': 12},
            {'file': MODES + FREEZE, 'path': 'mode/ui/icon', 'from': 2, 'to': 4},
        ],
        'remove_item': [{'file': MODES + FREEZE, 'path': 'mode/rules', 'value': 90, 'expected_count': 1}],
        # Both configs (the copy is made of the config as it is) name day_4; both play day_teams.
        'replace_string': [
            {'files': MODES + TDM, 'from': TDM_PROJECT, 'to': TEAMS_PROJECT},
            {'files': MODES + FREEZE, 'from': TDM_PROJECT, 'to': TEAMS_PROJECT},
            {'files': MODES + FREEZE, 'from': 'st_mode_04_name', 'to': 'st_mode_12_name'},
            {'files': MODES + FREEZE, 'from': 'st_mode_04_description', 'to': 'st_mode_12_description'},
        ],
        'set_entry': [],
        'add_entry': [],
        'remove_entry': [],
    }
    for team, entries in own.items():
        for entry, place in zip(entries, solo):
            out['set_entry'].append({
                'file': PROJECT, 'list': POINTS, 'where': {'team': team, 'point_id': entry['point_id']},
                'from': {'position': vector(entry['position']), 'rotation': vector(entry['rotation'])},
                'to': {'position': vector(place['position']), 'rotation': vector(place['rotation'])}})
        for place in solo[len(entries):]:
            i = place['point_id']
            out['add_entry'].append({
                'files': PROJECT, 'list': POINTS, 'after': {'team': team, 'point_id': i - 1},
                'like': {'team': team, 'point_id': 0}, 'expected_files': 1,
                'values': {'name': f'respawn_point({i})', 'full_name': FULL_NAME[team] % i, 'point_id': i,
                           'team': team, 'position': vector(place['position']),
                           'rotation': vector(place['rotation'])}})
    for place in solo:
        out['remove_entry'].append({'file': PROJECT, 'list': POINTS,
                                    'where': {'team': 2, 'point_id': place['point_id'], 'name': place['name']}})
    # The two rows after Free for All's (44): 212, then 450.
    for after, map_id, cfg in ((44, TDM_ID, TDM), (TDM_ID, FREEZE_ID, FREEZE)):
        out['add_entry'].append({
            'files': 'gameplay/db_static_dictionaries', 'list': 'maps_dict', 'after': {'id': after},
            'like': {'id': 44}, 'expected_files': 1, 'values': {'id': map_id, 'cfg_mode_name': cfg}})
    out['_about'] = (
        'Training Camp in Team Deathmatch and Freeze Tag, every team from the Free for All points (the owner, '
        '2026-11-22: "all teams spawns everywhere randomly (there are no \'map sides\')"; "not the same rules, the '
        'same spawn points that already exist"). projects/v010b_cs_training_camp/day_teams, a copy of Free for '
        'All\'s day_8 as every row above leaves it (the fence\'s wall, its trucks, the burning campfires, no ammo '
        f'crates): each of its {len(solo)} team-2 points (the owner\'s picks, ffa-training-camp-picks-1) is a point '
        f'of team 0 and one of team 1 at its place and facing, ids 0-{len(solo) - 1} in each team; Team '
        f'Deathmatch\'s own {len(own[0])} + {len(own[1])} (most at the gateways, outside the fence) take the first places, the others are '
        'made like them (Team Deathmatch\'s protection, radii and priority; the game\'s own choice of a team\'s '
        'free points), and the team-2 points go. Team Deathmatch\'s config (maps_dict 212, its row taken out by '
        'stalker-tdm-hold-1) plays day_teams and has its row again; Freeze Tag\'s, a copy of it as freeze-tag-1 '
        'makes Rudnya\'s (mode 12, ui.icon 4, its names, no ticket rule), is maps_dict 450. After artefact-hunt-1, '
        'whose rows count the dictionary, and the other team maps\' lists (freeze-tag-maps-1, artefact-hunt-maps-1), '
        'whose rows follow its last (scripts/stalker-maps/make_training_teams.py, docs/notes/stalker-ffa.md).')
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', type=Path, required=True)
    args = parser.parse_args()
    out = edits(args.live)
    lines = [f' "{key}": [\n' + ',\n'.join('  ' + json.dumps(e) for e in value) + '\n ]'
             if isinstance(value, list) else f' "{key}": {json.dumps(value)}' for key, value in out.items()]
    LIST.write_text('{\n' + ',\n'.join(lines) + '\n}\n', encoding='utf-8')
    print(f'{LIST.name}: {len(out["set_entry"])} points moved, {len(out["add_entry"]) - 2} added, '
          f'{len(out["remove_entry"])} team-2 points out')


if __name__ == '__main__':
    main()
