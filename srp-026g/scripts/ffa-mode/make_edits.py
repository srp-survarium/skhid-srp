#!/usr/bin/env python3
"""Writes the edit list of Free for all (lobby mode 8, docs/notes/ffa.md): content-edit/ffa-1.json.

For each FFA map, a Team Deathmatch (mode 4) map of 0.26g in one of its weathers (levels 1-8) and
London (level_09, day; base patch 14):
- its config gameplay/game_modes/ffa/level_0N_<w>_8.options, a copy of TDM's
  gameplay/game_modes/level_0N_<w>_4.options as the rows above the list's leave it: mode 8, its own
  name (level_0N[_<w>]_8), ui.mode_name st_mode_08_name and ui.description st_mode_08_description
  (the loading screen's text; TDM's speaks of tickets), ui.icon 2 (TDM's skull, set where a TDM
  config had another), the rules without TDM's 90 (93 time, 91 respawn, 94 kd), and the new ints
  the server reads (ADDED): mode.parameters.kills_to_win (the kill target, as TDM's tickets_count,
  which stays) and mode.match_making_params.max_players (0.31's mode 6's); the same project,
  timings, music and texts otherwise. In a folder of their own, as Blackout's copies are: modes.tsv
  has a row for every config of gameplay/game_modes/ itself and is applied before a later list's
  copies exist;
- its maps_dict row (gameplay/db_static_dictionaries), appended: IDS in MAPS order;
- 0.31a2's mode-6 ("domination") respawn points added to the TDM project as team-2 entries, made
  like the project's first point with float radii: 0.31's position (put on the ground where
  checked.tsv nudged it), rotation and enemy_check_radius, ally_check_radius 1, PROTECTION seconds
  of invulnerability, a name of their own (ffa_<0.31 name>) and the next point ids. 0.26g's TDM
  tick never reads team 2's points; the FFA respawn patch uses only theirs. A team-2 point the
  project has already (level_02's) must read PROTECTION too: maps.tsv's spawn_protection_s is the
  two teams' points only (content_edit.py STATS 'maps'), so it keeps its stock 2.

The list's row follows every TDM retune in release/content.tsv: the copies are of the TDM configs as
every other row leaves them (a retune of modes.tsv or an edit of a _4 config reaches them at the
next build), the dictionary rows come after Blackout's (its list pins maps_dict's count), and the
points go into the projects as maps.tsv and the other lists leave them. A list after it that
changes a TDM config would not reach the copies: it goes above.

checked.tsv is the collision check of every 0.31 point on the 0.26g project it goes to, made by
check-points/ (its main.rs says how a point is kept, nudged or dropped); a dropped point is left out.
To make it again (the 0.31a2 archive part holding the mode-6 projects, the stock db and every patch
of release/patches.tsv, in order):

  cd check-points && cargo run --release -- --donor <v031a2>/resources.03.db \
      --db <resources.db> --patch <resources.db.patch_12> ... $(python3 ../make_edits.py --pairs) \
      > ../checked.tsv

Then, with the files the list reads (python3 make_edits.py --paths) as the manifest's rows above
its own leave them:

  python3 make_edits.py --live <folder>

writes ../content-edit/ffa-1.json; content_edit.py ffa-1.json <folder> out/ --check checks it.
Players' files (srp-game-data get <layers> --out live/ ...) are those when nothing the manifest
makes of them is unreleased; else release/build_content.py's work folder has them, in
edits/ffa-1.json/live (a build that stops at the list leaves them too: a stale list is refused, not
applied).
"""
import argparse
import csv
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
EDIT = HERE.parent / 'content-edit/content_edit.py'
OUT = HERE.parent / 'content-edit/ffa-1.json'
CHECKED = HERE / 'checked.tsv'

spec = importlib.util.spec_from_file_location('content_edit', EDIT)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

# (level, the configs' weather, the projects' weather, 0.31a2's weather); the FFA configs' maps_dict
# ids are IDS, in this order.
MAPS = [(level, config, project, donor)
        for level in range(1, 9)
        for config, project, donor in [('day', 'day', 'day'), ('evn', 'evening', 'evn'),
                                       ('prerain', 'prerain', 'prerain') if level == 8 else
                                       ('rain', 'rain', 'rain')]]
MAPS.append((9, 'day', 'day', 'day'))
# Free in maps_dict, an id a byte: 0.26g's own are 11, 45, 61-110, 114, 115 and 117-151; srp's are
# 152, 153, 166, 175 (the 0.21d maps), 181-204, 206-209 and 216-218 (London, Research, the 0.11e maps,
# Rudnya) and 219-248 (Blackout); Scarcity (#1083) takes 12-41. Levels 1-5 get 46-60, levels 6-8 and
# London 154-163.
IDS = [*range(46, 61), *range(154, 164)]
assert len(IDS) == len(MAPS)
TDM, FFA = 4, 8
# The FFA configs' folder under gameplay/game_modes/ (maps_dict's cfg_mode_name starts with it).
FOLDER = 'ffa/'
TDM_RULE = 90
# ui.icon: TDM's skull, which FFA shows too.
ICON = 2
RULES = [93, 91, TDM_RULE, 94]
# The mode's texts, by their key in a config's ui: TDM's st_mode_04_*, FFA's st_mode_08_*.
TEXTS = {'mode_name': 'st_mode_{:02}_name', 'description': 'st_mode_{:02}_description'}
ALLY_CHECK_RADIUS = 1.0
# Seconds of invulnerability at a team-2 point (the owner, 2026-11-11: "1.5 or 2 max"; 0.31's 3 was
# too long). A whole number: the game reads the key as an int, whatever its type says, and makes
# milliseconds of it (respawn_point_core's load, 0x2F4628: mov edx, [eax]; imul edx, edx, 1000),
# where it converts the two radii beside it by their type; a float 1.5 would be read as its bits.
PROTECTION = 2
assert isinstance(PROTECTION, int)
# New ints of the configs, for the server, by table: the kills that win a match (the server ends it
# there) and the most players it takes (0.31a2's mode 6's). No fewest: two play a duel.
ADDED = [('mode/parameters', 'kills_to_win', 30), ('mode/match_making_params', 'max_players', 10)]
DICTIONARY = 'gameplay/db_static_dictionaries'
POINTS = 'server_objects/respawn_points'
RADII = ('enemy_check_radius', 'ally_check_radius')


MODES = 'gameplay/game_modes/'


def config(level, weather, mode):
    return f'{MODES}{FOLDER if mode == FFA else ""}level_{level:02}_{weather}_{mode}.options'


def project(level, weather):
    return f'projects/level_{level:02}/{weather}_{TDM}/client_project'


def tree(buf, off=0):
    kind = c.rec(buf, off)[3]
    if kind == c.T_NAMED:
        return {c.name_at(buf, c.rec(buf, k)[1]).decode(): tree(buf, k) for k in c.children(buf, off)}
    if kind == c.T_INDEXED:
        return [tree(buf, k) for k in c.children(buf, off)]
    return c.value_of(buf, off)


def walked_types(buf, path):
    """The type of each value of the entry at `path`, by key."""
    off = c.find(buf, path)
    return {c.name_at(buf, c.rec(buf, k)[1]).decode(): c.rec(buf, k)[3] for k in c.children(buf, off)}


def checked_points():
    """checked.tsv's points that stay, by project, in 0.31's order: (name, position, row)."""
    with open(CHECKED, newline='') as f:
        rows = list(csv.DictReader(f, delimiter='\t'))
    by = {}
    for row in rows:
        verdict = row['verdict']
        if verdict == 'dropped':
            continue
        x, y, z = (float(row[k]) for k in ('x', 'y', 'z'))
        if verdict == 'nudged':
            y = float(row['ground'])
        elif verdict != 'kept':
            raise SystemExit(f'checked.tsv: verdict {verdict!r}')
        by.setdefault('projects/' + row['project'] + '/client_project', []).append((row['point'], [x, y, z], row))
    return by


def edits(live):
    def read(path):
        return bytearray((live / path).read_bytes())

    points = checked_points()
    dictionary = read(DICTIONARY)
    rows = tree(dictionary)['maps_dict']
    ids = {row['id'] for row in rows}
    names = {row['cfg_mode_name']: row['id'] for row in rows}
    out = {'_about': (
        f'Free for all (lobby mode {FFA}, docs/notes/ffa.md; generated by scripts/ffa-mode/make_edits.py, '
        'do not edit; after all TDM retunes in release/content.tsv): for each Team Deathmatch map of '
        'levels 1-8 in its three weathers and London (day), a config '
        f'gameplay/game_modes/{FOLDER}level_0N_<w>_{FFA}.options copied from TDM\'s _4 as the rows above '
        f'leave it (mode {FFA}, its name, st_mode_{FFA:02}_name and st_mode_{FFA:02}_description, rules 93, '
        '91, 94 without TDM\'s 90, and for the server '
        + ', '.join(f'{table.split("/")[-1]}.{key} {value}' for table, key, value in ADDED)
        + '; the same project), its maps_dict row '
        f'(ids {ranges(IDS)}), and 0.31a2\'s mode-6 respawn points added to the '
        f'TDM project as team-2 points (ally radius 1, {PROTECTION} s of invulnerability), those that pass '
        'the collision check of scripts/ffa-mode/checked.tsv (nudged onto the ground where it says). '
        '0.26g\'s TDM never reads team 2\'s points; the FFA respawn patch spawns at them only.'),
        'copy': [], 'set': [], 'replace_string': [], 'remove_item': [], 'add_value': [], 'append_entry': [],
        'add_entry': []}
    for index, (level, weather, project_weather, _) in enumerate(MAPS):
        tdm, ffa = config(level, weather, TDM), config(level, weather, FFA)
        mode = tree(read(tdm))['mode']
        name = mode['name']
        want = f'level_{level:02}_{TDM}' if weather == 'day' else f'level_{level:02}_{weather}_{TDM}'
        if (mode['mode'], mode['rules'], name) != (TDM, RULES, want):
            raise SystemExit(f'{tdm}: not the TDM config this list expects')
        for key, text in TEXTS.items():
            if mode['ui'][key] != text.format(TDM):
                raise SystemExit(f'{tdm}: ui.{key} is {mode["ui"][key]}, not {text.format(TDM)}')
        if mode['match_making_params']['map_name'] != f'level_{level:02}/{project_weather}_{TDM}':
            raise SystemExit(f'{tdm}: plays {mode["match_making_params"]["map_name"]}')
        out['copy'].append({'files': tdm, 'from': tdm, 'to': ffa})
        out['set'].append({'file': ffa, 'path': 'mode/mode', 'from': TDM, 'to': FFA})
        if mode['ui']['icon'] != ICON:
            out['set'].append({'file': ffa, 'path': 'mode/ui/icon', 'from': mode['ui']['icon'], 'to': ICON})
        out['replace_string'].append({'files': ffa, 'from': name, 'to': name[:-1] + str(FFA)})
        for text in TEXTS.values():
            out['replace_string'].append({'files': ffa, 'from': text.format(TDM), 'to': text.format(FFA)})
        out['remove_item'].append({'file': ffa, 'path': 'mode/rules', 'value': TDM_RULE})
        for table, key, _ in ADDED:
            if key in mode[table.split('/')[-1]]:
                raise SystemExit(f'{tdm}: has {table}/{key} already')

        map_id = IDS[index]
        if map_id in ids:
            raise SystemExit(f'maps_dict id {map_id} is taken')
        if tdm[len(MODES):] not in names:
            raise SystemExit(f'maps_dict has no {tdm[len(MODES):]}')
        # As Blackout's rows: appended, the count pinned (a map another list added meanwhile is refused).
        out['append_entry'].append({'file': DICTIONARY, 'list': 'maps_dict', 'expected_count': len(rows) + index,
                                    'unique_key': 'id', 'value': {'cfg_mode_name': ffa[len(MODES):], 'id': map_id}})

        path = project(level, project_weather)
        buf = read(path)
        entries = tree(buf)['server_objects']['respawn_points']
        if [e['point_id'] for e in entries] != list(range(len(entries))):
            raise SystemExit(f'{path}: point ids are not 0..{len(entries) - 1}')
        # The model: the first point whose radii are floats (some of London's are ints).
        like = next((i for i in range(len(entries))
                     if {walked_types(buf, f'{POINTS}[{i}]')[key] for key in RADII} == {c.T_FLOAT}), None)
        if like is None:
            raise SystemExit(f'{path}: no point has radii that are floats')
        taken = {e['name'] for e in entries} | {e['full_name'] for e in entries}
        # A team-2 point of the project's own (level_02's) is a free-for-all spawn too.
        for e in entries:
            if e['team'] == 2 and e['invulnerability_time'] != PROTECTION:
                raise SystemExit(f'{path}: its own team-2 point {e["name"]} has {e["invulnerability_time"]} s of '
                                 f'invulnerability, not {PROTECTION}: not the project as the rows above leave it '
                                 '(players have 6 until this list ships; maps.tsv no longer writes team 2)')
            if e['team'] not in (0, 1, 2):
                raise SystemExit(f'{path}: {e["name"]} is of team {e["team"]}')
        previous = len(entries) - 1
        for k, (donor, position, row) in enumerate(points.pop(path)):
            name = 'ffa_' + donor
            full_name = 'logic/modes/ffa/respawn/' + name
            if name in taken or full_name in taken:
                raise SystemExit(f'{path}: {name} is taken')
            point_id = len(entries) + k
            out['add_entry'].append({'files': path, 'list': POINTS, 'after': {'point_id': previous},
                                     'like': {'point_id': like}, 'values': {
                'name': name, 'full_name': full_name, 'point_id': point_id, 'team': 2,
                'position': position, 'rotation': [float(row[axis]) for axis in ('rx', 'ry', 'rz')],
                'enemy_check_radius': float(row['enemy_check_radius']),
                'ally_check_radius': ALLY_CHECK_RADIUS,
                'invulnerability_time': PROTECTION,
                'priority': 1, 'is_temporary': False}})
            previous = point_id
    for table, key, value in ADDED:
        out['add_value'].append({'files': [config(level, weather, FFA) for level, weather, _, _ in MAPS],
                                 'table': table, 'key': key, 'value': value})
    if points:
        raise SystemExit(f'checked.tsv: projects of no FFA map: {sorted(points)}')
    return out


def ranges(ids):
    """12, 13, 14, 20 as '12-14, 20'."""
    out = []
    for i in ids:
        if out and out[-1][1] == i - 1:
            out[-1][1] = i
        else:
            out.append([i, i])
    return ', '.join(str(a) if a == b else f'{a}-{b}' for a, b in out)


def write(edits, path):
    """One edit a line: a list of hundreds of points stays readable in a diff."""
    lines = ['{', f' "_about": {json.dumps(edits["_about"], ensure_ascii=False)},']
    keys = [k for k in edits if k != '_about']
    for i, key in enumerate(keys):
        items = [' ' * 2 + json.dumps(e, ensure_ascii=False) for e in edits[key]]
        lines.append(f' "{key}": [')
        lines.append(',\n'.join(items))
        lines.append(' ]' + (',' if i < len(keys) - 1 else ''))
    lines.append('}')
    path.write_text('\n'.join(lines) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    what = parser.add_mutually_exclusive_group(required=True)
    what.add_argument('--live', type=Path, help='the files of --paths at their game paths, as the rows above the list\'s leave them')
    what.add_argument('--pairs', action='store_true', help="print check-points' --pair arguments")
    what.add_argument('--paths', action='store_true', help='print the game paths --live needs')
    parser.add_argument('--out', type=Path, default=OUT)
    args = parser.parse_args()
    if args.pairs:
        print(' '.join(f'--pair level_{level:02}/{donor}_6=level_{level:02}/{weather}_{TDM}'
                       for level, _, weather, donor in MAPS))
    elif args.paths:
        print(' '.join([DICTIONARY] + [p for level, weather, project_weather, _ in MAPS
                                       for p in (config(level, weather, TDM), project(level, project_weather))]))
    else:
        write(edits(args.live), args.out)


if __name__ == '__main__':
    main()
