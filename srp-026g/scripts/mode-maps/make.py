#!/usr/bin/env python3
"""make.py: Freeze Tag and Artifact Hunt on every map a team mode is played on (the owner,
2026-11-21: "make sure freeze tag is playable on all non FFA-only maps", "let's add artifact hunt
to all other non FFA-only maps too"; docs/notes/freeze-tag.md and artefact_hunt.md, "Maps").

usage: python3 -I make.py --base <folder> [--write <content-edit folder>] [--pack <packs>/artefact-hunt-maps-<v>]

--base   the files as a build of release/content.tsv without these lists and their pack leaves them, at their game
         paths (`srp-game-data get` of a build of the manifest): gameplay/db_static_dictionaries, the
         maps' configs, the battery and Team Deathmatch projects of the maps Artifact Hunt lacks.
--write  writes freeze-tag-maps-1.json and artefact-hunt-maps-1.json there.
--pack   writes the pack of Artifact Hunt's new configs (a new folder).

A team map is a level and weather that maps_dict gives a config of mode 2 to 5 whose project the data
has, but a STALKER port's (a v010b_ project: Free for All's maps alone, and Training Camp, whose Team
Deathmatch and Freeze Tag are training-camp-teams-1's, on its Free for All points). For each:

- Freeze Tag (mode 12), when it has none: a copy of the map's Team Deathmatch config, as
  freeze-tag-1.json makes Rudnya's (its project and team spawns, mode 12, the mode's icon and names,
  no ticket rule), under gameplay/game_modes/freeze/, its maps_dict row from FT_FIRST_ID.
- Artifact Hunt (mode 13), when it has none, from the map's own battery mode (0.46c2 had the mode on
  the nine stock levels alone, which artefact-hunt-1 has): the artefact's spawn points are the
  battery spawners' items (lifted by the item's drop_lift, as a dropped one lies), each team's base
  its battery container: its use shapes (the container's collision geometries, in world space) and
  its props (the container's models and collision, as the battery project has them). The project is
  a copy of the map's Team Deathmatch project with those props (its respawn points, each team's
  base checked to be on its own team's side); the config, in the pack, has the stock Artifact Hunt config's rules,
  parameters, announcer and item and the map's own ui. Its maps_dict row from AH_FIRST_ID.
  A map whose Artifact Hunt plays a project other than its Team Deathmatch project's `_ah` copy
  (London: artefact-hunt-1 copied level_09's, London plays v0.32's) gets the same, named after
  its project, and its maps_dict row is pointed at it (its id kept).

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
PACK_ID, VERSION = 'artefact-hunt-maps', '1'
FT_MODE, AH_MODE, TDM, BATTERIES, RESEARCH = 12, 13, 4, 2, 5
FT_ICON = 4
# The ids: past Artifact Hunt's 300-326, over a byte (exe patch 154); Freeze Tag's apart, with room
# for Artifact Hunt's to grow.
AH_FIRST_ID, FT_FIRST_ID = 327, 400
# The stock Artifact Hunt config the new ones take the mode's values of.
AH_TEMPLATE = 'artefact_hunt/level_01_day_ah.options'
# A container's props: the battery project's own entries within this of its model's place (m).
PROP_REACH = 1.2
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


def dictionary_rows(edits, anchor, rows):
    """maps_dict rows of `rows`, the first after the row of id `anchor`, each made like it."""
    for map_id, cfg, _ in rows:
        edits['add_entry'].append({'files': 'gameplay/db_static_dictionaries', 'list': 'maps_dict',
                                   'after': {'id': anchor}, 'like': {'id': anchor}, 'expected_files': 1,
                                   'values': {'cfg_mode_name': cfg, 'id': map_id}})
        anchor = map_id


# ---- Freeze Tag ----------------------------------------------------------------------------

def freeze_edits(team, anchor):
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
        rows.append((FT_FIRST_ID + len(rows), f'freeze/{name}', tdm['match_making_params']['map_name']))
    dictionary_rows(edits, anchor, rows)
    edits['_about'] = (
        f'Freeze Tag (mode 12, docs/notes/freeze-tag.md) on every team map but Rudnya day (freeze-tag-1.json), '
        f'{len(rows)} maps in their weathers: each a copy of the map\'s Team Deathmatch config under '
        'gameplay/game_modes/freeze/, as freeze-tag-1 makes Rudnya\'s (its project and team spawns, mode 12, '
        f'ui.icon 4, no ticket rule), maps_dict {FT_FIRST_ID}-{FT_FIRST_ID + len(rows) - 1}. Generated by '
        'scripts/mode-maps/make.py, after artefact-hunt-1.json (its rows after that list\'s last).')
    return edits, rows


# ---- Artifact Hunt -------------------------------------------------------------------------

def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def shapes_of(geometry):
    """A collision_geometry's meshes in world space (research.zone_shapes, with the object's own
    rotation: rows are axes, v * M, so a mesh's centre is its position times the object's matrix
    over the object's position, its axes its matrix times the object's)."""
    if any(abs(s - 1) > 1e-6 for s in geometry['scale']):
        sys.exit(f"{geometry['full_name']}: a scaled geometry")
    outer = research.rotation(geometry['rotation'])
    origin = geometry['position']
    shapes = []
    for mesh in geometry['meshes']:
        if mesh['type'] not in (0, 1, 2):
            sys.exit(f"{geometry['full_name']}: mesh type {mesh['type']}")
        axes = mul(research.rotation(mesh['rotation']), outer)
        centre = [origin[j] + sum(mesh['position'][i] * outer[i][j] for i in range(3)) for j in range(3)]
        shapes.append({'type': mesh['type'], 'position': tuple(ah.f32(v) for v in centre),
                       'axis_x': tuple(ah.f32(v) for v in axes[0]), 'axis_y': tuple(ah.f32(v) for v in axes[1]),
                       'axis_z': tuple(ah.f32(v) for v in axes[2]), 'half': tuple(ah.f32(v) for v in mesh['scale'])})
    return shapes


def entry_key(entry):
    return json.dumps(entry, sort_keys=True, default=str)


def battery_data(battery, tdm, lift, where):
    """The artefact's spawn points, the two bases (shapes and props) of a battery project."""
    server = battery['server_objects']
    spawners = [{'position': tuple(ah.f32(v + (lift if i == 1 else 0)) for i, v in enumerate(s['spawned_item']['position'])),
                 'rotation': tuple(s['spawned_item']['rotation'])} for s in server['victory_item_spawners']]
    if not 1 <= len(spawners) <= 64:
        sys.exit(f'{where}: {len(spawners)} battery spawners (1 to 64)')
    objects = {o.get('full_name'): o for o in battery['client_objects']}
    own = {key: {entry_key(e) for e in tdm[key]} for key in ('render_visuals', 'collision_objects')}
    visuals = [e for e in battery['render_visuals'] if entry_key(e) not in own['render_visuals']]
    solids = [e for e in battery['collision_objects'] if entry_key(e) not in own['collision_objects']]
    bases = []
    for container in sorted(server['victory_items_containers'], key=lambda c: c['team']):
        shapes = []
        for link in container['collision_geometries']:
            geometry = objects.get(link['name'])
            if geometry is None or geometry.get('game_object_type') != 'collision_geometry':
                sys.exit(f"{where}: {container['full_name']} names no geometry {link['name']}")
            shapes += shapes_of(geometry)
        if not 1 <= len(shapes) <= 8:
            sys.exit(f"{where}: {container['full_name']}: {len(shapes)} shapes (1 to 8)")
        model = container['visual_geometry']
        props = [e for e in visuals if math.dist(e['position'], model['position']) <= PROP_REACH]
        if not any(e['visual_name'] == model['model_name'] for e in props):
            sys.exit(f"{where}: {container['full_name']}: no render visual of {model['model_name']} at its place")
        prop_solids = [s for s in solids if any(math.dist(s['position'], e['position']) < 1e-3 and
                                                s['lib_name'].startswith(f"resources/models/{e['visual_name']}.model/")
                                                for e in props)]
        centre = tuple(ah.f32(sum(s['position'][i] for s in shapes) / len(shapes)) for i in range(3))
        bases.append({'team': container['team'], 'position': centre, 'shapes': shapes,
                      'props': props, 'solids': prop_solids})
    if [b['team'] for b in bases] != [0, 1] or math.dist(bases[0]['position'], bases[1]['position']) < 1:
        sys.exit(f'{where}: not two bases, one a team, a metre apart')
    # The project's spawns are Team Deathmatch's: each team's base on its own side of them.
    sides = []
    for team in (0, 1):
        points = [p['position'] for p in tdm['server_objects']['respawn_points'] if p['team'] == team]
        sides.append([sum(p[i] for p in points) / len(points) for i in range(3)])
    for base in bases:
        own, other = (math.dist(base['position'], sides[t]) for t in (base['team'], 1 - base['team']))
        if own >= other:
            sys.exit(f"{where}: team {base['team']}'s base is nearer the other team's spawns")
    return {'spawners': spawners, 'bases': bases}


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


def hunt(base, team, anchor):
    edits = {'_about': '', 'copy': [], 'add_entry': [], 'replace_string': []}
    template = load(base, MODES_DIR + AH_TEMPLATE)
    lift = template['mode']['artefact_hunt']['item']['drop_lift']
    files, rows, moved, said = {}, [], [], []
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
            # now v0.32's): its id plays a new config on the new project, of its battery mode.
            old = (old_id, old_cfg)
            name = target.replace('/', '_')
        if BATTERIES not in modes:
            said.append(f'{tdm_cfg}: no battery mode, no Artifact Hunt')
            continue
        battery_project = modes[BATTERIES][2]['match_making_params']['map_name']
        tdm_project = load(base, project_file(source))
        data = battery_data(load(base, project_file(battery_project)), tdm_project, lift, target)
        config = hunt_config(template, tdm, name, target, data)
        research.check_config_types(config, template, target)
        files[f'{MODES_DIR}artefact_hunt/{name}.options'] = bcw.dump(config)
        hunt_project_edits(edits, source, target, tdm_project, data, target)
        cfg = f'artefact_hunt/{name}.options'
        if old:
            edits['replace_string'].append({'files': 'gameplay/db_static_dictionaries', 'from': old[1], 'to': cfg})
            moved.append((old[0], cfg, target))
        else:
            rows.append((AH_FIRST_ID + len(rows), cfg, target))
        said.append(f'{(old or rows[-1])[0]} {name}: project {target} of {source}, from {battery_project}'
                    + (f' (was {old[1]})' if old else '') + ': '
                    f"{len(data['spawners'])} artefact spawn points; bases "
                    + '; '.join(f"team {b['team']} {ah.spot(b['position'])} ({len(b['shapes'])} shape, props "
                                f"{', '.join(e['visual_name'].rsplit('/', 1)[-1] for e in b['props'])}, "
                                f"{len(b['solids'])} collision)" for b in data['bases']))
    dictionary_rows(edits, anchor, rows)
    edits['_about'] = (
        f'Artifact Hunt (mode 13, docs/notes/artefact_hunt.md, "Maps") on the {len(rows)} team maps artefact-hunt-1 '
        'has not (0.46c2 had the mode on the nine stock levels alone), from each map\'s own battery mode: '
        'projects/<level>/<weather>_ah/, a copy of the map\'s Team Deathmatch project with its two battery '
        'containers\' props (their models, collision and walker collision, as its battery project has them); '
        f'maps_dict {AH_FIRST_ID}-{AH_FIRST_ID + len(rows) - 1} for the configs of the pack {PACK_ID}'
        + (f'; and the same for the maps whose Team Deathmatch project is no longer the one artefact-hunt-1 copied '
           f'(London, v0.32\'s now): their ids ' + ', '.join(str(m[0]) for m in moved) + ' play the pack\'s configs '
           'of the new projects' if moved else '')
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
    args = parser.parse_args()
    dictionary, team = team_maps(args.base)
    taken = {entry['id'] for entry in dictionary}
    last = dictionary[-1]['id']
    ft, ft_rows = freeze_edits(team, last)
    ah_edits, files, ah_rows, ah_moved, said = hunt(args.base, team, ft_rows[-1][0] if ft_rows else last)
    new = [row[0] for row in ft_rows + ah_rows]
    if taken & set(new) or len(set(new)) != len(new):
        sys.exit(f'maps_dict ids taken or twice: {sorted(taken & set(new))}, {len(new) - len(set(new))} twice')
    print(f'{len(team)} team maps; Freeze Tag {len(ft_rows)} new:')
    print('\n'.join(f'  {map_id} {cfg} ({project})' for map_id, cfg, project in ft_rows))
    print(f'Artifact Hunt {len(ah_rows)} new, {len(ah_moved)} moved to a new project:')
    print('\n'.join('  ' + line for line in said))
    if args.write:
        dump_edits(ft, args.write / FT_EDITS)
        dump_edits(ah_edits, args.write / AH_EDITS)
    if args.pack:
        if args.pack.exists():
            sys.exit(f'{args.pack}: already there')
        pack = {
            'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
            'description': f'Artifact Hunt (mode 13) on the {len(ah_rows)} team maps 0.46c2 did not have it on '
                           f'(maps {AH_FIRST_ID}-{AH_FIRST_ID + len(ah_rows) - 1})'
                           + (f' and on the new projects of maps ' + ', '.join(str(m[0]) for m in ah_moved)
                              + ' (their Team Deathmatch project changed)' if ah_moved else '')
                           + ', from each map\'s battery mode (the artefact\'s spawn points its battery spawners, a '
                           f'team\'s base its container): their configs (maps_dict rows and projects: {AH_EDITS})',
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
