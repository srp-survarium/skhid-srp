#!/usr/bin/env python3
"""make.py: Freeze Tag and Artifact Hunt on every map a team mode is played on (the owner,
2026-11-21: "make sure freeze tag is playable on all non FFA-only maps", "let's add artifact hunt
to all other non FFA-only maps too"; docs/notes/freeze-tag.md and artefact_hunt.md, "Maps").

usage: python3 -I make.py --base <folder> --mapdata <folder> [--write <content-edit folder>]
           [--pack <packs>/artefact-hunt-maps-<v>]

--base   the files as a build of release/content.tsv without these lists and their pack leaves them, at their game
         paths (`srp-game-data get` of a build of the manifest): gameplay/db_static_dictionaries, the
         maps' configs, their Team Deathmatch projects and the stock levels' Artifact Hunt projects.
--mapdata the `<project>.mapdata` files of the Team Deathmatch projects of the maps Artifact Hunt lacks
         (`mapdata-dump --db <stock> --patch ... --project v021d_level_01/day_4 ...`): their floors.
--write  writes freeze-tag-maps-1.json and artefact-hunt-maps-1.json there, and the new maps' ids to ids.tsv.
--pack   writes the pack of Artifact Hunt's new configs (a new folder).

A team map is a level and weather that maps_dict gives a config of mode 2 to 5 whose project the data
has, but a STALKER port's (a v010b_ project: Free for All's maps alone, and Training Camp, whose Team
Deathmatch and Freeze Tag are training-camp-teams-1's, on its Free for All points). For each:

- Freeze Tag (mode 12), when it has none: a copy of the map's Team Deathmatch config, as
  freeze-tag-1.json makes Rudnya's (its project and team spawns, mode 12, the mode's icon and names,
  no ticket rule), under gameplay/game_modes/freeze/, its maps_dict row of ids.tsv.
- Artifact Hunt (mode 13), when it has none, from the Artifact Hunt of the stock level the map is a
  version of (its project vNNNx_level_MM; artefact-hunt-1's, 0.46c2's points, which 0.69d has the same;
  the owner, 2026-11-23, of the battery spawners and containers taken before: "not where batteries
  are", "there should be spawns for them in original data somewhere"): the artefact's spawn points
  and the two bases' boxes in the map's frame (frame_offset: Mamayev Kurgan v0.21 lies 7.277 m lower),
  each checked on the map's floors (floors.py, make_pack's fit report): a box's foot on the floor
  under it, a point or box that does not fit there at the nearest place it does, said. A base is the
  stock maps' box (item/storage_device/box_atlant8c, its render visual and collision as
  artefact-hunt-1's projects have them; the owner, 2026-11-23: "you should use exactly the same prop
  for storing artifact"), its use shape the box itself. A map no stock level's Artifact Hunt fits
  (none now) gets none. The project is a copy of the map's Team Deathmatch project with
  the box (its respawn points, but one a box stands on; each team's base checked to be on its own
  team's side); the config, in the pack, has the stock Artifact Hunt config's rules,
  parameters, announcer and item and the map's own ui. Its maps_dict row of ids.tsv.
  A map whose Artifact Hunt plays a project other than its Team Deathmatch project's `_ah` copy
  (London: artefact-hunt-1 copied level_09's, London plays v0.32's) gets the same, named after
  its project, and its maps_dict row is pointed at it (its id kept).

ids.tsv (level, weather, mode, id, the config it was given for) keeps every id these lists ever gave:
rooms, drafts and recordings name a map by it, so it never moves. A map it has keeps its id; a new one
takes the next id of its mode past the mode's highest there (FT_FIRST_ID, AH_FIRST_ID for the first),
over any id taken (any row of ids.tsv, the base's maps_dict, another edit list's maps_dict rows): Freeze
Tag past 449 goes on at 451 (450 is Training Camp's), Artifact Hunt past 399 at Freeze Tag's next free.
A map that is gone keeps its row, its id given to no other.

Both lists run after artefact-hunt-1.json and before client-dictionary-1.json, as Freeze Tag's and
Artifact Hunt's own. Their maps_dict rows go after the dictionary's last row (artefact-hunt-1's), each
after the one before (add_entry: no row count to keep in step when a pack adds rows).
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
sys.path.insert(0, str(HERE.parent / 'artefact-hunt'))
import bcw  # noqa: E402

_spec = importlib.util.spec_from_file_location('ah_make_pack', HERE.parent / 'artefact-hunt' / 'make_pack.py')
ah = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ah)
research = ah.research

FT_EDITS, AH_EDITS = 'freeze-tag-maps-1.json', 'artefact-hunt-maps-1.json'
PACK_ID, VERSION = 'artefact-hunt-maps', '4'
FT_MODE, AH_MODE, TDM = 12, 13, 4
FT_ICON = 4
# The first ids of a mode ids.tsv has none of: past Artifact Hunt's 300-326, over a byte (exe patch
# 154); Freeze Tag's apart, with room for Artifact Hunt's to grow.
AH_FIRST_ID, FT_FIRST_ID = 327, 400
IDS = HERE / 'ids.tsv'
IDS_HEADER = ('# The maps_dict id of each (level, weather, mode) these lists ever gave, never moved nor given again\n'
              '# (make.py appends a new map\'s; a map that is gone keeps its row). Tabs: level weather mode id config\n')
EDITS_DIR = HERE.parent / 'content-edit'
# The stock Artifact Hunt config the new ones take the mode's values of.
AH_TEMPLATE = 'artefact_hunt/level_01_day_ah.options'
# A ported map's project names the level it is a version of: the donor's own level number, the same place
# in every build (v021d_level_03 and v011e_level_03 are Vostok, v069d_level_01 Rudnya, v044a_level_09 London).
PORTED = re.compile(r'^v[0-9]{3}[a-z]_(level_[0-9]{2})/')
# frame_offset: the share of the stock map's models that must stand at one offset in the ported map
# (the fewest of the maps, Fort v0.21: 40 %), and the size of the cells the offsets are counted in (m).
FRAME_SHARE, FRAME_CELL = 0.3, 0.05
MODES_DIR = 'gameplay/game_modes/'
# The STALKER ports' projects: no team map of these lists (Training Camp's are training-camp-teams-1's).
STALKER = 'v010b_'


def load(base, path):
    return bcw.loads((base / path).read_bytes())


def project_file(project):
    return f'projects/{project}/client_project'


def team_maps(base):
    """{(level, weather): {mode: (id, cfg, config)}} of the maps_dict configs whose project the data
    has, the STALKER ports' left out; mode is mode.mode, Blackout's variant left out (its own copy of
    a battery config)."""
    maps = {}
    dictionary = load(base, 'gameplay/db_static_dictionaries')['maps_dict']
    for entry in dictionary:
        path = base / (MODES_DIR + entry['cfg_mode_name'])
        if not path.exists():
            continue
        mode = bcw.loads(path.read_bytes())['mode']
        making = mode.get('match_making_params', {})
        project = making.get('map_name', '')
        if 'srp_variant' in mode or project.startswith(STALKER) or not (base / project_file(project)).exists():
            continue
        key = (making['map_id'], making['map_weather_id'])
        maps.setdefault(key, {})[mode['mode']] = (entry['id'], entry['cfg_mode_name'], mode)
    team = {key: modes for key, modes in maps.items() if set(modes) & {2, 3, 4, 5}}
    return dictionary, team


def order(key):
    """A map's place: by level, its weathers day, evening, rain (then the others)."""
    level, weather = key
    return (level, {1: 0, 3: 1, 2: 2}.get(weather, weather))


class Ids:
    """ids.tsv: the kept ids, and the next free id of a mode for a new map."""

    def __init__(self, taken):
        self.rows = []
        if IDS.exists():
            for line in IDS.read_text(encoding='utf-8').splitlines():
                if line and not line.startswith('#'):
                    level, weather, mode, map_id, cfg = line.split('\t')
                    self.rows.append((int(level), int(weather), int(mode), int(map_id), cfg))
        self.kept = {row[:3]: row[3] for row in self.rows}
        mine = [row[3] for row in self.rows]
        if len(self.kept) != len(mine) or len(set(mine)) != len(mine) or taken & set(mine):
            sys.exit(f'{IDS}: a map twice, an id twice, or ids maps_dict or another list has: {sorted(taken & set(mine))}')
        self.used = taken | set(mine)
        self.new = []

    def of(self, key, mode, cfg):
        if (*key, mode) in self.kept:
            return self.kept[(*key, mode)]
        mine = [row[3] for row in self.rows if row[2] == mode]
        map_id = max(mine) + 1 if mine else {FT_MODE: FT_FIRST_ID, AH_MODE: AH_FIRST_ID}[mode]
        while map_id in self.used:
            map_id += 1
        row = (*key, mode, map_id, cfg)
        self.rows.append(row)
        self.new.append(row)
        self.kept[row[:3]] = map_id
        self.used.add(map_id)
        return map_id

    def write(self):
        IDS.write_text(IDS_HEADER + ''.join('\t'.join(map(str, row)) + '\n' for row in self.rows), encoding='utf-8')


def listed_ids():
    """The maps_dict ids the other edit lists give."""
    ids = set()
    for path in EDITS_DIR.glob('*.json'):
        if path.name in (FT_EDITS, AH_EDITS):
            continue
        for ops in json.loads(path.read_text(encoding='utf-8')).values():
            for op in ops if isinstance(ops, list) else []:
                if isinstance(op, dict) and op.get('list') == 'maps_dict' and 'id' in op.get('values', {}):
                    ids.add(op['values']['id'])
    return ids


def spans(ids):
    """400-449, 451: the ids as runs."""
    out = []
    for map_id in sorted(ids):
        if out and out[-1][1] == map_id - 1:
            out[-1][1] = map_id
        else:
            out.append([map_id, map_id])
    return ', '.join(str(a) if a == b else f'{a}-{b}' for a, b in out)


def dictionary_rows(edits, anchor, rows):
    """maps_dict rows of `rows`, the first after the row of id `anchor`, each made like it."""
    for map_id, cfg, _ in rows:
        edits['add_entry'].append({'files': 'gameplay/db_static_dictionaries', 'list': 'maps_dict',
                                   'after': {'id': anchor}, 'like': {'id': anchor}, 'expected_files': 1,
                                   'values': {'cfg_mode_name': cfg, 'id': map_id}})
        anchor = map_id


# ---- Freeze Tag ----------------------------------------------------------------------------

def freeze_edits(team, anchor, ids):
    edits = {'_about': '', 'copy': [], 'set': [], 'remove_item': [], 'replace_string': [], 'add_entry': []}
    rows = []
    for key in sorted(team, key=order):
        modes = team[key]
        if FT_MODE in modes:
            continue
        if TDM not in modes:
            sys.exit(f'level {key[0]} weather {key[1]}: a team map without a Team Deathmatch config')
        _, cfg, tdm = modes[TDM]
        if tdm['rules'] != [93, 91, 90, 94] or tdm['ui']['icon'] != 2:
            sys.exit(f'{cfg}: not the Team Deathmatch config freeze-tag-1 copies')
        name = cfg.rsplit('/', 1)[-1]
        target = f'{MODES_DIR}freeze/{name}'
        edits['copy'].append({'files': MODES_DIR + cfg, 'from': MODES_DIR + cfg.rsplit(name, 1)[0],
                              'to': f'{MODES_DIR}freeze/'})
        edits['set'] += [{'file': target, 'path': 'mode/mode', 'from': TDM, 'to': FT_MODE},
                         {'file': target, 'path': 'mode/ui/icon', 'from': 2, 'to': FT_ICON}]
        edits['remove_item'].append({'file': target, 'path': 'mode/rules', 'value': 90, 'expected_count': 1})
        edits['replace_string'] += [{'files': target, 'from': 'st_mode_04_name', 'to': 'st_mode_12_name'},
                                    {'files': target, 'from': 'st_mode_04_description', 'to': 'st_mode_12_description'}]
        rows.append((ids.of(key, FT_MODE, f'freeze/{name}'), f'freeze/{name}', tdm['match_making_params']['map_name']))
    dictionary_rows(edits, anchor, rows)
    edits['_about'] = (
        f'Freeze Tag (mode 12, docs/notes/freeze-tag.md) on every team map but Rudnya day (freeze-tag-1.json), '
        f'{len(rows)} maps in their weathers: each a copy of the map\'s Team Deathmatch config under '
        'gameplay/game_modes/freeze/, as freeze-tag-1 makes Rudnya\'s (its project and team spawns, mode 12, '
        f'ui.icon 4, no ticket rule), maps_dict {spans(r[0] for r in rows)}. Generated by '
        'scripts/mode-maps/make.py, after artefact-hunt-1.json (its rows after that list\'s last).')
    return edits, rows


# ---- Artifact Hunt -------------------------------------------------------------------------

def stock_hunt(team, source):
    """(project, mode) of the stock level's Artifact Hunt (artefact-hunt-1's) in the weather of the ported
    map's project `source` (PORTED), or None."""
    match = PORTED.match(source)
    if not match:
        return None
    project = f"{match[1]}/{source.split('/')[1].rsplit('_', 1)[0]}_ah"
    found = [modes[AH_MODE][2] for modes in team.values()
             if AH_MODE in modes and modes[AH_MODE][2]['match_making_params']['map_name'] == project]
    return (project, found[0]) if found else None


def models(project):
    """{a render visual's model name (its last part): its places}."""
    out = {}
    for entry in project['render_visuals']:
        name = (entry.get('visual_name') or entry['lib_name']).removeprefix('resources/models/').split('.model')[0]
        out.setdefault(name.rsplit('/', 1)[-1], []).append(entry['position'])
    return out


def frame_offset(stock, ported):
    """Where the stock map's frame lies in the ported map's: the offset (x, y, z) most of the stock map's
    models stand at in the ported map under the same name (Mamayev Kurgan v0.21 is the stock one 7.277 m
    lower, the others are in the same frame: a mean offset under half a cell is none), or None
    when fewer than FRAME_SHARE of them agree (not the same map)."""
    theirs, ours = models(stock), models(ported)
    votes = {}
    for name, places in theirs.items():
        for p in places:
            for q in ours.get(name, []):
                cell = tuple(round((q[i] - p[i]) / FRAME_CELL) for i in range(3))
                votes.setdefault(cell, []).append([q[i] - p[i] for i in range(3)])
    if not votes:
        return None
    agree = max(votes.values(), key=len)
    if len(agree) < FRAME_SHARE * sum(len(places) for places in theirs.values()):
        return None
    mean = [sum(d[i] for d in agree) / len(agree) for i in range(3)]
    return tuple(round(v, 3) if abs(v) >= FRAME_CELL / 2 else 0.0 for v in mean)


def base_at(team, foot, rotation, box):
    """A team's base: the stock maps' box (the owner, 2026-11-23: "you should use exactly the same prop
    for storing artifact"), its foot at `foot`, turned `rotation`; its use shape the box itself, as large
    as make_pack's fit report takes it (BOX_HALF, BOX_HEIGHT)."""
    visual, solids = box
    place = {'position': foot, 'rotation': rotation, 'scale': (1.0, 1.0, 1.0)}
    axes = research.rotation(rotation)
    centre = tuple(ah.f32(v) for v in (foot[0], foot[1] + ah.BOX_HEIGHT / 2, foot[2]))
    return {'team': team, 'position': centre, 'model': {'model_name': ah.BOX, **place},
            'props': [dict(visual, **place)], 'solids': [dict(solid, **place) for solid in solids],
            'shapes': [{'type': 1, 'position': centre, 'axis_x': tuple(ah.f32(v) for v in axes[0]),
                        'axis_y': tuple(ah.f32(v) for v in axes[1]), 'axis_z': tuple(ah.f32(v) for v in axes[2]),
                        'half': tuple(ah.f32(v) for v in (ah.BOX_HALF[0], ah.BOX_HEIGHT / 2, ah.BOX_HALF[1]))}]}


def box_place(world, at, rotation, where, said):
    """A box's foot on the ported map: on the highest floor under it (make_pack's fit rule), or, where
    the box does not fit there (box_fit: a wall through it, no floor), at the nearest place it fits
    within 10 m, said."""
    def snapped(point):
        top = ah.foot_floor(world, point, rotation)
        return point if top is None else (point[0], ah.f32(top), point[2])
    foot = snapped(at)
    wrong = ah.box_fit(world, foot, rotation)
    if wrong is None:
        return foot
    near = world.place(foot, reach=10.0, clear=0.7, low=0.75, high=1.0, flat=0.6)
    moved = near and snapped(tuple(ah.f32(v) for v in near[1]))
    if not moved or ah.box_fit(world, moved, rotation):
        sys.exit(f'{where}: the base at {ah.spot(at)}: {wrong}; no place within 10 m fits the box')
    said.append(f'{where}: the base at {ah.spot(at)}: {wrong}; at the nearest place it fits, {ah.spot(moved)}, '
                f'{math.dist(at, moved):.2f} m away')
    return moved


def original_data(stock, stock_project, offset, world, box, tdm, where, said):
    """The artefact's spawn points and the two bases of the stock level's Artifact Hunt (artefact-hunt-1:
    0.46c2's, and 0.69d's the same) on the ported map, moved by `offset` (frame_offset), each checked on
    the ported map's floors (floors.py): a base's box as box_place puts it, a spawn point that does not
    fit (lying_fit, in a box) at the fit report's nearest fitting place (make_pack's proposal)."""
    hunt = stock['artefact_hunt']
    boxes = [e for e in stock_project['render_visuals'] if e.get('visual_name') == ah.BOX]
    bases = []
    for base in hunt['bases']:
        entry = min(boxes, key=lambda e: math.dist(e['position'], base['position']))
        if math.dist(entry['position'], base['position']) > 3:
            sys.exit(f"{where}: the stock project has no box within 3 m of team {base['team']}'s base")
        rotation = tuple(entry['rotation'])
        at = tuple(ah.f32(entry['position'][i] + offset[i]) for i in range(3))
        bases.append(base_at(base['team'], box_place(world, at, rotation, where, said), rotation, box))
    check_sides(bases, tdm, where)
    boxed = [{'team': b['team'], 'box': {'position': b['model']['position'], 'rotation': b['model']['rotation']}}
             for b in bases]
    spawners = []
    for index, spawner in enumerate(hunt['spawners']):
        at = tuple(ah.f32(spawner['position'][i] + offset[i]) for i in range(3))
        team = ah.in_box(at, boxed, 0.0, 0.0, -0.05)
        wrong = f"in team {team}'s box" if team is not None else ah.lying_fit(world, at)
        if wrong:
            text, place = ah.proposal(world, 'spawner', at)
            if place is None:
                sys.exit(f'{where}: artefact spawn point {index} at {ah.spot(at)}: {wrong}; {text}')
            said.append(f'{where}: artefact spawn point {index} at {ah.spot(at)}: {wrong}; at the {text}')
            at = tuple(ah.f32(v) for v in place)
        spawners.append({'position': at, 'rotation': tuple(spawner['rotation'])})
    return {'spawners': spawners, 'bases': bases}


def check_sides(bases, tdm, where):
    """The project's spawns are Team Deathmatch's: each team's base on its own side of them."""
    sides = []
    for team in (0, 1):
        points = [p['position'] for p in tdm['server_objects']['respawn_points'] if p['team'] == team]
        sides.append([sum(p[i] for p in points) / len(points) for i in range(3)])
    if [b['team'] for b in bases] != [0, 1] or math.dist(bases[0]['position'], bases[1]['position']) < 1:
        sys.exit(f'{where}: not two bases, one a team, a metre apart')
    for base in bases:
        own, other = (math.dist(base['position'], sides[t]) for t in (base['team'], 1 - base['team']))
        if own >= other:
            sys.exit(f"{where}: team {base['team']}'s base is nearer the other team's spawns")


def stock_box(base):
    """The artefact's box as the stock maps' Artifact Hunt projects have it (artefact-hunt-1): its render
    visual and its collision objects, of level_01's day project."""
    project = load(base, project_file('level_01/day_ah'))
    visual = next(e for e in project['render_visuals'] if e.get('visual_name') == ah.BOX)
    solids = [e for e in project['collision_objects'] if e['lib_name'].startswith(f'resources/models/{ah.BOX}.model/')
              and math.dist(e['position'], visual['position']) < 1e-3]
    if len(solids) != 2:
        sys.exit(f'level_01/day_ah: the box {ah.BOX} without its two collision objects')
    return visual, solids


def under_boxes(data, spawns):
    """The Team Deathmatch respawn points a base's box stands on (within a body's radius of its
    footprint, up to its top): a body spawned there would be in the box (Rudnya v0.69's point 387 is
    where its Batteries container stands). Their full names."""
    out = []
    for point in spawns:
        for base in data['bases']:
            at, axes = base['model']['position'], research.rotation(base['model']['rotation'])
            offset = [point['position'][i] - at[i] for i in range(3)]
            u, w = (sum(offset[i] * axes[k][i] for i in range(3)) for k in (0, 2))
            if (abs(u) < ah.BOX_HALF[0] + ah.floors.RADIUS and abs(w) < ah.BOX_HALF[1] + ah.floors.RADIUS
                    and -1.0 < offset[1] < ah.BOX_HEIGHT):
                out.append(point['full_name'])
    return out


def like_of(entries, entry, keys, where):
    """An entry of the project to make a new one like, no other the same in `keys`, and the values
    to set: one with `entry`'s keys, else (the ports' render visuals have no `visual_name`, the game
    draws `lib_name`) one with them but a `visual_name` that is `entry`'s `lib_name`."""
    values = {k: (ah.vector(v) if isinstance(v, tuple) else v) for k, v in entry.items()}
    same = [e for e in entries if set(e) == set(entry)]
    if not same and entry.get('visual_name') == entry.get('lib_name'):
        same = [e for e in entries if set(e) == set(entry) - {'visual_name'}]
        values.pop('visual_name')
    keys = tuple(k for k in keys if k in values)
    return ah.unique(same, keys, where), values


def hunt_project_edits(edits, source, target, tdm, data, where):
    file = project_file(target)
    edits['copy'].append({'files': project_file(source), 'from': f'projects/{source}/', 'to': f'projects/{target}/'})
    name = 'visual_name' if all('visual_name' in e for e in tdm['render_visuals']) else 'lib_name'
    last_visual, _ = ah.last(tdm['render_visuals'], (name, 'position', 'rotation', 'scale'), where)
    last_solid, _ = ah.last(tdm['collision_objects'], ('lib_name', 'position', 'rotation', 'cgroup', 'cmask'), where)
    for base in data['bases']:
        for entry in base['props']:
            like, values = like_of(tdm['render_visuals'], entry, ('visual_name', 'lib_name', 'position'), where)
            edits['add_entry'].append({'files': file, 'list': 'render_visuals', 'after': last_visual,
                                       'expected_files': 1, 'like': like, 'values': values})
        for entry in base['solids']:
            like, values = like_of([e for e in tdm['collision_objects'] if '#' not in e['lib_name']], entry,
                                   ('lib_name', 'position', 'cgroup', 'cmask'), where)
            edits['add_entry'].append({'files': file, 'list': 'collision_objects', 'after': last_solid,
                                       'expected_files': 1, 'like': like, 'values': values})


def hunt_config(template, tdm, name, project, data):
    config = copy.deepcopy(template)
    mode = config['mode']
    mode['name'] = name
    mode['ui'].update({key: tdm['ui'][key] for key in ('loading_music', 'loading_screen_id', 'map_name')})
    mode['match_making_params'].update({key: tdm['match_making_params'][key] for key in ('map_weather_id', 'map_id')})
    mode['match_making_params']['map_name'] = project
    mode['artefact_hunt']['spawners'] = [dict(s) for s in data['spawners']]
    mode['artefact_hunt']['bases'] = [{'team': b['team'], 'position': b['position'], 'shapes': b['shapes']}
                                      for b in data['bases']]
    return config


def hunt(base, team, anchor, ids, mapdata):
    edits = {'_about': '', 'copy': [], 'add_entry': [], 'remove_entry': [], 'replace_string': [], 'add_value': []}
    template = load(base, MODES_DIR + AH_TEMPLATE)
    files, rows, moved, said = {}, [], [], []
    box = stock_box(base)
    for key in sorted(team, key=order):
        modes = team[key]
        _, tdm_cfg, tdm = modes[TDM]
        source = tdm['match_making_params']['map_name']
        if not source.endswith('_4'):
            sys.exit(f'{tdm_cfg}: its project {source} is no <weather>_4')
        target = source[:-2] + '_ah'
        name = tdm_cfg.rsplit('/', 1)[-1][:-len('_4.options')] + '_ah'
        old = None
        if AH_MODE in modes:
            old_id, old_cfg, old_mode = modes[AH_MODE]
            if old_mode['match_making_params']['map_name'] == target:
                continue
            # A map whose Team Deathmatch project changed since its Artifact Hunt was made (London,
            # now v0.32's): its id plays a new config on the new project.
            old = (old_id, old_cfg)
            name = target.replace('/', '_')
        tdm_project = load(base, project_file(source))
        stock = stock_hunt(team, source)
        offset = stock and frame_offset(load(base, project_file(stock[0])), tdm_project)
        if not offset:
            said.append(f'{tdm_cfg}: no stock level\'s Artifact Hunt in its frame, no Artifact Hunt')
            continue
        path = mapdata / (source.replace('/', '_') + '.mapdata')
        if not path.exists():
            sys.exit(f'--mapdata: no {path.name} (mapdata-dump --project {source})')
        floors = ah.floors.load(path)
        hunt_data = stock[1]['artefact_hunt']
        world = ah.floors.World(floors, [tuple(p['position'][i] + offset[i] for i in range(3))
                                         for p in hunt_data['spawners'] + hunt_data['bases']]
                                + [s[0] for s in floors['spawns']])
        data = original_data(stock[1], load(base, project_file(stock[0])), offset, world, box, tdm_project, target, said)
        boxed = under_boxes(data, tdm_project['server_objects']['respawn_points'])
        for full_name in boxed:
            edits['remove_entry'].append({'file': project_file(target), 'list': 'server_objects/respawn_points',
                                          'where': {'full_name': full_name}})
            said.append(f'{target}: the respawn point {full_name} is under a base\'s box: taken out')
        config = hunt_config(template, tdm, name, target, data)
        research.check_config_types(config, template, target)
        files[f'{MODES_DIR}artefact_hunt/{name}.options'] = bcw.dump(config)
        hunt_project_edits(edits, source, target, tdm_project, data, target)
        cfg = f'artefact_hunt/{name}.options'
        if 'srp_build' in tdm:
            # The map's build as its Team Deathmatch config says it (modes.tsv's `build`): a ported
            # prefix no build of srp's is named by (v032a, v044a: the Londons) needs it, else the
            # server refuses the game data (survarium maps::Build::of).
            edits['add_value'].append({'files': [MODES_DIR + cfg], 'table': 'mode', 'key': 'srp_build',
                                       'value': tdm['srp_build']})
        if old:
            edits['replace_string'].append({'files': 'gameplay/db_static_dictionaries', 'from': old[1], 'to': cfg})
            moved.append((old[0], cfg, target))
        else:
            rows.append((ids.of(key, AH_MODE, cfg), cfg, target))
        said.append(f'{(old or rows[-1])[0]} {name}: project {target} of {source}, the points of {stock[0]}'
                    + (f' moved by {offset}' if any(offset) else '') + (f' (was {old[1]})' if old else '') + ': '
                    f"{len(data['spawners'])} artefact spawn points; bases "
                    + '; '.join(f"team {b['team']} {ah.spot(b['model']['position'])}" for b in data['bases']))
    dictionary_rows(edits, anchor, rows)
    edits['_about'] = (
        f'Artifact Hunt (mode 13, docs/notes/artefact_hunt.md, "Maps") on the {len(rows)} team maps artefact-hunt-1 '
        'has not, each a version of a stock level (its project vNNNx_level_MM): the artefact\'s spawn points and '
        'the two bases of that level\'s Artifact Hunt (artefact-hunt-1\'s, 0.46c2\'s; 0.69d has the same), in the '
        'map\'s frame, each checked on its floors: projects/<level>/<weather>_ah/, a copy of the map\'s Team '
        'Deathmatch project with the two bases\' box (its model, collision and walker collision); '
        f'maps_dict {spans(r[0] for r in rows)} for the configs of the pack {PACK_ID}'
        + (f'; and the same for the maps whose Team Deathmatch project is no longer the one artefact-hunt-1 copied '
           f'(London, v0.32\'s now): their ids ' + ', '.join(str(m[0]) for m in moved) + ' play the pack\'s configs '
           'of the new projects' if moved else '')
        + '; a config of a map whose Team Deathmatch config names its build (srp_build: the Londons, v032 and '
        'v042) names it too'
        + f'. Generated by scripts/mode-maps/make.py, after {FT_EDITS} (its rows after that list\'s last).')
    return edits, files, rows, moved, said


def dump_edits(edits, path):
    path.write_text(json.dumps(edits, ensure_ascii=False, separators=(',', ':')).replace('},{', '},\n{') + '\n',
                    encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--base', type=Path, required=True)
    parser.add_argument('--write', type=Path)
    parser.add_argument('--pack', type=Path)
    parser.add_argument('--mapdata', type=Path, required=True)
    args = parser.parse_args()
    dictionary, team = team_maps(args.base)
    taken = {entry['id'] for entry in dictionary}
    ids = Ids(taken | listed_ids())
    last = dictionary[-1]['id']
    ft, ft_rows = freeze_edits(team, last, ids)
    ah_edits, files, ah_rows, ah_moved, said = hunt(args.base, team, ft_rows[-1][0] if ft_rows else last, ids,
                                                       args.mapdata)
    new = [row[0] for row in ft_rows + ah_rows]
    if taken & set(new) or len(set(new)) != len(new):
        sys.exit(f'maps_dict ids taken or twice: {sorted(taken & set(new))}, {len(new) - len(set(new))} twice')
    print(f'{len(team)} team maps; Freeze Tag {len(ft_rows)} new:')
    print('\n'.join(f'  {map_id} {cfg} ({project})' for map_id, cfg, project in ft_rows))
    print(f'Artifact Hunt {len(ah_rows)} new, {len(ah_moved)} moved to a new project:')
    print('\n'.join('  ' + line for line in said))
    print(f'ids.tsv: {len(ids.new)} new: ' + ', '.join(f'{row[3]} {row[4]}' for row in ids.new))
    if args.write:
        ids.write()
        dump_edits(ft, args.write / FT_EDITS)
        dump_edits(ah_edits, args.write / AH_EDITS)
    if args.pack:
        if args.pack.exists():
            sys.exit(f'{args.pack}: already there')
        pack = {
            'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
            'description': f'Artifact Hunt (mode 13) on the {len(ah_rows)} team maps 0.46c2 did not have it on '
                           f'(maps {spans(r[0] for r in ah_rows)})'
                           + (f' and on the new projects of maps ' + ', '.join(str(m[0]) for m in ah_moved)
                              + ' (their Team Deathmatch project changed)' if ah_moved else '')
                           + ', each a version of a stock level, with that level\'s spawn points and bases (artefact-hunt-1\'s, '
                           f'0.46c2\'s) in its frame: their configs (maps_dict rows and projects: {AH_EDITS})',
            'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
            'notes': ['made by srp-026g/scripts/mode-maps/make.py from a build of the manifest before it'] + said,
        }
        for path, data in files.items():
            target = args.pack / 'files' / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        (args.pack / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        digest = hashlib.sha256((args.pack / 'pack.json').read_bytes()).hexdigest()
        print(f'manifest row:\npack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
