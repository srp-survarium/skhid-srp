#!/usr/bin/env python3
"""make_pack.py: the content of Artifact Hunt (srp-026g/docs/notes/artefact_hunt.md, "Data").

usage: python3 -I make_pack.py --inputs <folder> --out <packs>/artefact-hunt-<version>
           [--edits <content-edit>/artefact-hunt-1.json] [--mapdata <folder>] [--built <folder>]
       python3 -I make_pack.py --catalog <client-patches/localization> [--write]

--inputs   prepare_inputs.py's folder: 0.46c2's files (donor/), 0.26g's as players have them
           (live/) and live.list, each checked against inputs.sha256.
--out      the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
           manifest's `pack` row (release/content.tsv).
--edits    writes the edit list of the maps' projects there (it is made of the same inputs; its
           manifest row is `edit artefact-hunt-1.json`, after every edit of a Team Deathmatch map).
--built    a folder with the projects as the edit list made them, at their game paths
           (content_edit.py's output, or a patch of release/build-content unpacked): checks each
           against the map's own project and 0.46c2's points.
--mapdata  the `<project>.mapdata` files of 0.26g's 27 Team Deathmatch projects (`mapdata-dump
           --db <stock> --patch ... --project level_01/day_4 ...`): with them it prints the fit
           report, each of 0.46c2's points against 0.26g's floors and walls (floors.py).
--catalog  the mode's texts (texts.tsv, the 12 languages) against the game's dictionaries: says
           which rows they lack; --write puts them in, in key order.

Nine levels in three weathers, as 0.46c2 has them (27 maps). What is made:

The pack (the same inputs give the same pack):
- gameplay/game_modes/artefact_hunt/level_0N_<w>_ah.options: `mode.mode` MODE, `mode.rules` RULES,
  0.46c2's `mode.parameters` under their own names and types (whole seconds, ints), its
  `sound_notifications` block as it is, the lobby's and the loading screen's entries (`ui`: 0.26g's
  of the map's Team Deathmatch config, the mode's name and description), and the map's mode data
  in world space: `mode.artefact_hunt.spawners[] {position, rotation}` (0.46c2's
  victory_item_spawners' spawned_item), `.bases[] {team, position, shapes[] {type, position,
  axis_x, axis_y, axis_z, half}}` (its two victory_artefact_containers and their collision
  geometries, as Research's capture_zones), `.item` (item_values: the values of its
  gameplay/victory_items/artefact.options the rule and the patch read).
- the assets 0.26g lacks, in 0.26g's formats (asset()): the artefact's model, its three particles,
  the carrier's glow (0.26g's composite of a third-person effect and its first-person twin, the
  twin's particle made here), the base box, their materials and textures, the announcer's seven
  lines (0.46c2's collections in its seven voice folders and the en, ru and ua takes they name).

The edit list (applied by release/build-content after the rows above it, as Free for All's):
- projects/level_0N/<weather>_ah/client_project: a copy of the map's Team Deathmatch project as
  every row before leaves it (so the map is 0.26g's own, with every later fix of it), then
  0.46c2's Artifact Hunt respawn points in the place of Team Deathmatch's (each made like one of
  the project's own points: 0.26g's keys and types) and the two bases' box (its model and its two
  collision objects, made like entries of the project). Nothing else of 0.46c2's project.
- gameplay/db_static_dictionaries: a maps_dict row a config (FIRST_MAP_ID on), as Blackout's and
  Free for All's lists add theirs (MAPS_DICT_COUNT).

Texts are rows of the game's dictionaries (client-patches/localization/*.tsv), not of the pack:
texts.tsv holds them for the 12 languages.

moves.tsv: a point of 0.46c2 put elsewhere (a place the fit report proposes, taken by the
owner); the report says which rows are applied.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402
import dxt  # noqa: E402
import floors  # noqa: E402
from prepare_inputs import STAGES, maps, sha256  # noqa: E402

_spec = importlib.util.spec_from_file_location('research_make_pack', HERE.parent / 'research-mode' / 'make_pack.py')
research = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(research)

PACK_ID = 'artefact-hunt'
VERSION = '5'
EDITS = 'artefact-hunt-1.json'
# `mode.mode`, the lobby's number of the mode too (maps::GameMode::lobby_number is the rules'
# number): 2 to 5 and 8 are modes, 6 Blackout, 7 Scarcity's (#1083), 9 to 11 the lobby's numbers of
# Free for All's twists (twist::LobbyMode::number), 12 Freeze Tag's.
MODE = 13
# The rules of a match (artefact_hunt.md, "The rule's class and registration"): 0.26g's time rule
# 93 (waiting, countdown) and respawn rule 91, the mode's own (the exe patch's cook class, as
# Research's 0x1E0: the next class free in register_cook's table) and the kd rule 94.
RULE_CLASS = 0x1E1
RULES = [93, 91, RULE_CLASS, 94]
# The lobby's mode icon (ui.icon: SrvGameModeIcons frame icon + 1): 3, Artifact Hunt's own frame
# of the lobby movie (srp-026g/flash/lobby_menu; 0 to 2 are 0.26g's battery, mast and skull).
ICON = 3
# maps_dict ids: 300 + 3 x (level - 1) + (day 0, evening 1, rain 2). Above every id taken (281)
# and clear of the ports' block from 256; over a byte, so srp's block carries them (patch 154).
FIRST_MAP_ID = 300
# maps_dict's count when the edit list appends its rows (last in release/content.tsv: as the rows
# above leave it; blackout-mode-1 .. stalker-tdm-hold-1 append and remove theirs): append_entry
# refuses another, so a list placed elsewhere or a new row before it stops the build. The rows
# are the edit list's, not the pack's: a pack's rows come before every edit list and would move
# the counts those lists expect. 241 since London (v0.42)'s nine rows (290-298, pack london-v042) and
# one more landed before it (2026-11-22; the build said 241 where the list said 231); 243 with
# Vostok (v0.69)'s two (258, 259, pack vostok-069d); 247 with Cologne (v0.69)'s four (543-546, pack cologne-069d).
MAPS_DICT_COUNT = 247
FOLDER = 'artefact_hunt'
BOX = 'item/storage_device/box_atlant8c'
# Its bounding box (render/export_properties): half its width in x and z, its height over its foot.
BOX_HALF, BOX_HEIGHT = (0.606, 0.616), 1.152
# How far a box's foot may be off the floor under it (m): the owner saw 0.14 m (level_04's boxes,
# 2026-11-21: "its bottom edge does not touch the floor and its shadow is offset").
BOX_SNAP = 0.02
# The mode's texts a config names (texts.tsv has them in the 12 languages).
NAME_KEY, DESCRIPTION_KEY = f'st_mode_{MODE:02}_name', f'st_mode_{MODE:02}_description'
HINT_KEYS = [f'st_map_hint_{MODE}_{n}' for n in (1, 2, 3)]
# What else the mode names: the lobby's name, the hud movie's, the welcome, the round's restart, the hud's
# messages and the end's texts (the exe patch's st_artefact_hunt_*; two messages 0.46c2's keys).
OTHER_KEYS = (['st_srp_mode_artefact_hunt', 'st_srp_hud_artefact_hunt_carrying', 'st_srp_hud_artefact_hunt_sudden_death',
               f'st_start_match_welcome_message_{MODE}', 'st_round_restart_in', 'st_on_spawn_victory_artefact',
               'st_on_destroyed_victory_artefact']
              + [f'st_artefact_hunt_{text}' for text in ('you_picked', 'team_delivered', 'enemy_delivered',
                                                          'won_collected', 'lost_collected', 'won_time_out',
                                                          'lost_time_out', 'draw')])
LANGUAGES = ('english', 'ukrainian', 'russian', 'german', 'polish', 'serbian', 'czech', 'hungarian', 'finnish',
             'belarusian', 'croatian', 'portuguese')

# ---- assets ------------------------------------------------------------------------------
MESH_CHUNKS = (3, 4)        # vertices, indices: all a 0.26g mesh has (0.46c2 adds 110 and 113)
CONFIGS = ('.material', '.particle', '.options', '/settings', '/export_properties', '.sound_collection',
           '.raw_options', '/exported_primitives')
TEXTURE_OPTIONS = ('bump_mode', 'format', 'has_mip_levels', 'high_quality', 'srgb', 'texture_dimension', 'ver')
SOUND_ITEM = ('filename', 'start_offset', 'start_offset_max')
# A switch and the texture it draws: one whose texture no build has goes off.
MAPS = {
    'use_alphablended_normal': 'texture_alphablended_normal', 'use_detail_nmap': 'texture_detail_normal',
    'use_emissive_map': 'texture_emissive', 'use_nmap': 'texture_normal', 'use_tdiffuse': 'texture_diffuse',
    'use_tfresnel': 'texture_fresnel', 'use_troughness': 'texture_roughness',
    'use_tspecular_intensity': 'texture_specular_intensity', 'use_tspecular_power': 'texture_specular_power',
    'use_vertex_blended_normal': 'texture_vertex_blended_normal', 'use_ttransparency': 'texture_transparency',
    'use_temissive': 'texture_emissive', 'use_uv_distortion': 'uv_distortion_texture',
}
# Never shipped as a file: 0.26g has no reader of it; its values are the configs' `item`.
ITEM = 'gameplay/victory_items/artefact.options'
# The carrier's glow (body_effect): 0.46c2's particle effect type, 0.26g's (artefact-glow's
# effect_artefact_<kind>: a composite and its particle effects), a view and the twin's system.
DONOR_PARTICLE_EFFECT = 132
COMPOSITE_EFFECT, PARTICLE_EFFECT = 110, 114
FIRST_PERSON = 0
FIRST_PERSON_SUFFIX = '_fp'
ANIMATED_DOMAIN = 'particle_action_initial_animated_domain_type'
NEAR_EYES = ('Head', 'Face', 'Neck')


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def shortest(value):
    """The shortest decimal that is the same float in the game's files (an edit list's numbers)."""
    for digits in range(10):
        if f32(round(value, digits)) == f32(value):
            return round(value, digits) + 0.0
    return value


def vector(values):
    return [shortest(v) for v in values]


class Inputs:
    def __init__(self, root):
        self.root, self.sums = Path(root), {}
        for line in (HERE / 'inputs.sha256').read_text().splitlines():
            if line and not line.startswith('#'):
                want, path = line.split(None, 1)
                self.sums[path.strip()] = want
        self.listing = set((self.root / 'live.list').read_text().split('\n'))

    def read(self, path):
        if path not in self.sums:
            sys.exit(f'inputs.sha256 has no {path}')
        data = (self.root / path).read_bytes()
        if sha256(data) != self.sums[path]:
            sys.exit(f'{path} is not the file this pack is made of (sha256 {self.sums[path]})')
        return data

    def assets(self):
        """The donor files that are neither a map's config nor its project."""
        return sorted(p[len('donor/'):] for p in self.sums
                      if p.startswith('donor/') and not p.startswith(('donor/gameplay/game_modes/', 'donor/projects/')))


def forms():
    """stock_forms.tsv: {(kind, of): {what}}."""
    out = {}
    for line in (HERE / 'stock_forms.tsv').read_text().splitlines():
        if line and not line.startswith('#') and not line.startswith('kind\t'):
            kind, of, what = line.split('\t')
            out.setdefault((kind, of), set()).add(what)
    return out


def mesh(raw, path):
    out, at, kinds = bytearray(), 0, []
    while at < len(raw):
        kind, size = struct.unpack_from('<II', raw, at)
        if at + 8 + size > len(raw):
            sys.exit(f'{path}: a truncated chunk')
        if kind in MESH_CHUNKS:
            out += raw[at:at + 8 + size]
            kinds.append(kind)
        at += 8 + size
    if kinds != list(MESH_CHUNKS):
        sys.exit(f'{path}: chunks {kinds}, not the vertices and the indices')
    return bytes(out)


def material(value, path, stock, have, log, blind, colour_alpha):
    """A 0.46c2 material as 0.26g draws it: each stage's keys and its effect's keys those 0.26g's own
    materials of the stage have; a map whose texture nobody has off; then, of the combinations of
    switches 0.26g's own materials draw, the largest within the material's (equipment-029's rule:
    a combination no shipped material draws has no compiled shader). A material whose transparency
    texture no build has (0.46c2 names fx/artifacts/shain_smoke, which none of its 36 archives
    holds) goes into `blind`: without its alpha it draws each particle as a solid sheet (the owner's
    first test, 2026-11-21), so particle() leaves its emitters out."""
    off = []
    for stage, body in value['material'].items():
        effect = body.get('effect') if isinstance(body, dict) else None
        if not isinstance(effect, dict):
            continue
        if (stage, effect.get('effect_id')) not in STAGES:
            sys.exit(f'{path}: stage {stage} with effect {effect.get("effect_id")}: no form of 0.26g to give it')
        for key in sorted(set(body) - stock['stage_key', stage]):
            del body[key]
        if (effect.get('use_ttransparency_dxt1', {}).get('value') is True
                and effect.get('use_ttransparency', {}).get('value') is True):
            # 0.46c2's transparency from a DXT1 texture's colour: 0.26g reads a DXT5's alpha (dxt.py)
            colour_alpha.add(effect['texture_transparency']['value'])
            off.append(f'{stage}.use_ttransparency_dxt1 (its texture made DXT5, its alpha its colour)')
            del effect['use_ttransparency_dxt1']
        for key in sorted(set(effect) - stock['effect_key', stage]):
            if key.startswith('use_') and effect[key].get('value') is True:
                off.append(f'{stage}.{key} (0.26g has no such switch)')
            del effect[key]
        on = {key for key, flag in effect.items()
              if key.startswith('use_') and isinstance(flag, dict) and flag.get('value') is True}
        for switch in sorted(on & MAPS.keys()):
            texture = effect.get(MAPS[switch], {}).get('value')
            if texture and f'textures/{texture}.dds' not in have:
                effect[switch]['value'] = False
                on.discard(switch)
                off.append(f'{stage}.{switch} (no texture {texture} in either build)')
                if switch == 'use_ttransparency':
                    blind.add(path[len('material_instances/'):-len('.material')])
        drawn = [set(c.split()) for c in stock['combination', stage]]
        if on not in drawn:
            kept = max((c for c in drawn if c <= on), key=lambda c: (len(c), sorted(c)), default=None)
            if kept is None:
                sys.exit(f'{path}: no combination 0.26g draws within {sorted(on)}')
            for switch in sorted(on - kept):
                effect[switch]['value'] = False
                off.append(f'{stage}.{switch} (0.26g draws no such combination)')
    if off:
        log.append(f'{path}: off: {"; ".join(off)}')
    return value


def particle(value, path, stock, have, log, blind):
    """A 0.46c2 particle with the properties 0.26g's own nodes of each type have. A node type no
    stock particle uses is kept as it is and said: 0.26g's exe has the classes of the two the
    artefact's particles use (particle_action_external_screen_alignment, particle_action_simple_orbit)."""
    dropped, unknown, materials = set(), set(), set()

    def walk(node):
        if isinstance(node, dict):
            kind = node.get('particle_entity_type')
            if kind is not None:
                known = stock.get(('particle_property', kind))
                properties = node.get('properties') or {}
                if known is None:
                    unknown.add(kind)
                else:
                    for key in sorted(set(properties) - known):
                        del properties[key]
                        dropped.add(f'{kind}.{key}')
                if isinstance(properties.get('Material'), str):
                    materials.add(properties['Material'])
            for child in node.values():
                walk(child)
    walk(value)
    lod = value['LOD 0']
    out = sorted(guid for guid, node in lod['children'].items()
                 if (node.get('properties') or {}).get('Material') in blind)
    for guid in out:
        log.append(f"{path}: emitter {lod['children'][guid]['properties'].get('Name')} left out: its material "
                   f"{lod['children'][guid]['properties']['Material']} draws without its transparency texture")
        del lod['children'][guid]
    lod['children_order'] = [guid for guid in lod['children_order'] if guid not in out]
    for name in sorted(materials):
        if f'material_instances/{name}.material' not in have:
            sys.exit(f'{path}: its material {name} is in neither build')
    if dropped:
        log.append(f'{path}: properties 0.26g has not, left out: {", ".join(sorted(dropped))}')
    if unknown:
        log.append(f'{path}: node types no stock particle of 0.26g uses, kept: {", ".join(sorted(unknown))}')
    return value


def body_effect(value, path, have):
    """0.46c2's carrier glow (one particle effect, its type 132) as 0.26g's artefact glows are
    (artefact-glow's effect_artefact_<kind>): a composite (110) of the particle effect (114) with
    no `view` (everyone else's third person) and its first-person twin (`view` 0) playing
    <system>_fp, with the same time and end."""
    one = value['effect']
    if one.get('type') != DONOR_PARTICLE_EFFECT or 'view' in one or set(value) != {'effect'}:
        sys.exit(f'{path}: not 0.46c2\'s particle effect')
    system = one['particle_system']
    if f'particles/{system}.particle' not in have:
        sys.exit(f'{path}: its particle {system} is in neither build')
    third = dict(one, type=PARTICLE_EFFECT)
    first = dict(third, particle_system=system + FIRST_PERSON_SUFFIX, view=FIRST_PERSON)
    return {'effect': {'type': COMPOSITE_EFFECT, 'effects': [third, first]}}


def first_person(value):
    """A body glow's first-person copy (artefact-glow's `first_person_particle`): the same system
    with Head, Face and Neck off in every emitter's animated domain, so nothing sits on the
    camera. Returns it and how many domains changed."""
    value, changed = copy.deepcopy(value), [0]

    def walk(node):
        if isinstance(node, dict):
            if node.get('particle_entity_type') == ANIMATED_DOMAIN:
                properties = node['properties']
                if any(properties.get(part) is True for part in NEAR_EYES):
                    changed[0] += 1
                for part in NEAR_EYES:
                    if part in properties:
                        properties[part] = False
            for child in node.values():
                walk(child)
        elif isinstance(node, list):
            for child in node:
                walk(child)
    walk(value)
    return value, changed[0]


def item_values(item):
    """`mode.artefact_hunt.item` (artefact_hunt.md, "Data") of 0.46c2's artefact.options."""
    usable, lying = item['usable_shape'], item['collision_shape']
    if usable['type'] != 0 or len(set(usable['scale'])) != 1:
        sys.exit(f'{ITEM}: its usable shape is not a sphere')
    if lying['type'] != 0 or lying['position'][0] or lying['position'][2]:
        sys.exit(f'{ITEM}: its collision shape is not a sphere straight over its point')
    return {'model': item['object']['model'], 'particle_idle': item['object']['idle_particle'],
            'particle_spawn': item['object']['spawn_particle'],
            'particle_destroy': item['object']['destroy_particle'],
            'movement_speed_modifier': float(item['parameters']['movement_speed_modifier']),
            'health_regeneration_speed_modifier': float(item['parameters']['health_regeneration_speed_modifier']),
            'usable_radius': float(usable['scale'][0]), 'drop_lift': float(lying['position'][1])}


def asset(path, raw, stock, have, log, blind, colour_alpha):
    """A donor file as the pack ships it: 0.26g's formats and forms."""
    if path.endswith('/converted_model'):
        return mesh(raw, path)
    if path.startswith('textures/') and path[len('textures/'):-len('.dds')] in colour_alpha and path.endswith('.dds'):
        log.append(f'{path}: DXT1 -> DXT5, its alpha its colour (a transparency 0.46c2 reads from the colour)')
        return dxt.dxt1_to_dxt5(raw)
    if not path.endswith(CONFIGS):
        if bcw.is_compact(raw):
            sys.exit(f'{path}: a config of 16-byte records nobody converts')
        return raw
    value = bcw.loads(raw)
    if path.endswith('/settings'):
        value.pop('is_terrain', None)       # 0.46c2's; no 0.26g model has it
        for surface, settings in value['material_settings'].items():
            settings.setdefault('used_in_portals', False)       # every 0.26g surface has it
            if f"material_instances/{settings['material_name']}.material" not in have:
                sys.exit(f"{path}: {surface}'s material {settings['material_name']} is in neither build")
    elif path.endswith('.material'):
        value = material(value, path, stock, have, log, blind, colour_alpha)
    elif path.endswith('.particle'):
        value = particle(value, path, stock, have, log, blind)
    elif path.startswith('textures/'):
        value['options'] = {key: v for key, v in value['options'].items() if key in TEXTURE_OPTIONS}
        if path[len('textures/'):-len('.options')] in colour_alpha:
            if value['options'].get('format') != 'DXT1':
                sys.exit(f'{path}: a colour transparency not in DXT1')
            value['options']['format'] = 'DXT5'
    elif path.endswith('.sound_collection'):
        items = [item for item in value['collection']['sound_items'] if item.get('enabled', True)]
        for item in items:
            if f"sounds/single/{item['filename']}.high.ogg" not in have:
                sys.exit(f"{path}: its sound {item['filename']} is in neither build")
        value['collection']['sound_items'] = [{key: item[key] for key in SOUND_ITEM} for item in items]
    elif path.endswith('.raw_options'):
        if f"sounds/spl/{value['options']['spl_preset']}.spl" not in have:
            sys.exit(f"{path}: its loudness preset {value['options']['spl_preset']} is in neither build")
    elif path.startswith('gameplay/effects/'):
        value = body_effect(value, path, have)
    out = bcw.dump(value)
    if bcw.is_compact(out) or bcw.dump(bcw.loads(out)) != out:
        sys.exit(f'{path}: the config does not read back as written')
    return out


# ---- a map's mode data -------------------------------------------------------------------

def mode_data(project, where):
    """0.46c2's Artifact Hunt objects of a project: the artefact's spawn points, the two bases (each
    with its box's placement), the respawn points."""
    server = project['server_objects']
    by_guid = {o.get('guid_path'): o for o in project['client_objects']}
    spawners = []
    for spawner in server['victory_item_spawners']:
        item = spawner.get('spawned_item')
        if not item:
            sys.exit(f"{where}: {spawner['full_name']} has no spawned_item")
        spawners.append({'position': tuple(item['position']), 'rotation': tuple(item['rotation']),
                         'name': spawner['name']})
    boxes = [e for e in project['render_visuals'] if e['model_name'] == BOX]
    solids = [e for e in project['collision_objects'] if e['model_name'].startswith(f'resources/models/{BOX}.model/')]
    bases = []
    for container in sorted(server['victory_artefact_containers'], key=lambda c: c['team']):
        shapes = []
        for link in container['collision_geometries'].values():
            if link.get('__removed'):
                continue
            geometry = by_guid.get(link['path'][0])
            if geometry is None or geometry.get('game_object_type') != 'collision_geometry':
                sys.exit(f"{where}: {container['full_name']} names no geometry")
            shapes += research.zone_shapes(geometry)
        box = min(boxes, key=lambda e: math.dist(e['position'], container['position']))
        if math.dist(box['position'], container['position']) > 3:
            sys.exit(f"{where}: no box within 3 m of {container['full_name']}")
        bases.append({'team': container['team'], 'position': tuple(container['position']), 'shapes': shapes,
                      'box': {'position': tuple(box['position']), 'rotation': tuple(v + 0.0 for v in box['rotation'])},
                      'solids': sorted((e['model_name'], e['cgroup'], e['cmask']) for e in solids
                                       if math.dist(e['position'], box['position']) < 1e-3)})
    if [base['team'] for base in bases] != [0, 1] or len(boxes) != 2 or bases[0]['box'] == bases[1]['box']:
        sys.exit(f'{where}: not two bases, one a team, each with its box')
    for base in bases:
        if [solid[0].rsplit('/', 1)[1] for solid in base['solids']] != ['collision', 'walker_collision']:
            sys.exit(f'{where}: a box without its collision and its walker collision')
    spawns = []
    for point in server['respawn_points']:
        if point['team'] not in (0, 1) or not point.get('is_enabled', True):
            sys.exit(f"{where}: {point['full_name']} of team {point['team']}, or switched off")
        spawns.append({key: point[key] for key in ('team', 'priority', 'enemy_check_radius', 'ally_check_radius',
                                                   'use_for_first_respawn', 'use_for_ai', 'invulnerability_time')}
                      | {'position': tuple(point['position']), 'rotation': tuple(point['rotation']),
                         'name': point['name']})
    resupply = sorted((o['team'], tuple(o['position'])) for o in project['client_objects']
                      if o.get('game_object_type') == 'resupply_zone')
    return {'spawners': spawners, 'bases': bases, 'spawns': spawns, 'resupply': resupply}


def moved(data, level, moves, said):
    """`data` with the level's rows of moves.tsv applied."""
    data = copy.deepcopy(data)
    for kind, index, place, why in moves.get(level, []):
        if kind == 'base':
            base = data['bases'][index]
            delta = [place[i] - base['box']['position'][i] for i in range(3)]
            base['box']['position'] = place
            base['position'] = tuple(f32(base['position'][i] + delta[i]) for i in range(3))
            for shape in base['shapes']:
                shape['position'] = tuple(f32(shape['position'][i] + delta[i]) for i in range(3))
        else:
            data['spawners' if kind == 'spawner' else 'spawns'][index]['position'] = place
        said.add(f'level_{level}: {kind} {index} at ({place[0]:.2f}, {place[1]:.2f}, {place[2]:.2f}) by moves.tsv: {why}')
    return data


def take_proposals(rows):
    """Every place the fit report proposes into moves.tsv (a point's row replaced), as the owner takes
    them (2026-11-21: "Apply all proposals"); the next run makes the pack of them."""
    lines = (HERE / 'moves.tsv').read_text().rstrip('\n').split('\n')
    head = [line for line in lines if line.startswith('#') or line.startswith('level\t')]
    kept = {tuple(line.split('\t')[:3]): line for line in lines if line not in head}
    for (level, kind, index), (place, why) in sorted(rows.items()):
        kept[(level, kind, str(index))] = '\t'.join([level, kind, str(index)] + [f'{v:.3f}' for v in place]
                                                    + [f'the fit report: {why}'])
        print(f'moves.tsv: level_{level} {kind} {index} to {spot(place)}: {why}')
    order = sorted(kept, key=lambda k: (k[0], k[1], int(k[2])))
    (HERE / 'moves.tsv').write_text('\n'.join(head + [kept[k] for k in order]) + '\n')


def read_moves():
    moves = {}
    for line in (HERE / 'moves.tsv').read_text().splitlines():
        if line and not line.startswith('#') and not line.startswith('level\t'):
            level, kind, index, x, y, z, why = line.split('\t')
            if kind not in ('base', 'spawner', 'spawn'):
                sys.exit(f'moves.tsv: {kind}: base, spawner or spawn')
            moves.setdefault(level, []).append((kind, int(index), (f32(float(x)), f32(float(y)), f32(float(z))), why))
    return moves


def mode_config(donor, live, name, project, data):
    theirs, ours = donor['mode'], live['mode']
    ui = {key: ours['ui'][key] for key in ('loading_music', 'loading_screen_id', 'map_name')}
    ui.update({'icon': ICON, 'mode_name': NAME_KEY, 'description': DESCRIPTION_KEY})
    making = {key: ours['match_making_params'][key] for key in ('map_weather_id', 'map_id')}
    making.update({'map_name': project, 'max_players': theirs['match_making_params']['max_players']})
    return {'mode': {
        'name': name, 'mode': MODE, 'rules': list(RULES), 'ui': ui, 'match_making_params': making,
        'parameters': copy.deepcopy(theirs['parameters']),
        'sound_notifications': copy.deepcopy(theirs['sound_notifications']),
        'artefact_hunt': {
            'spawners': [{'position': s['position'], 'rotation': s['rotation']} for s in data['spawners']],
            'bases': [{'team': b['team'], 'position': b['position'], 'shapes': b['shapes']} for b in data['bases']],
            'item': copy.deepcopy(data['item']),
        },
    }}


# ---- the projects' edit list ---------------------------------------------------------------

def unique(entries, keys, where):
    """The first of `entries` that no other matches in `keys`: an edit's `like` and `after`."""
    def key(entry):
        return json.dumps([entry.get(k) for k in keys], sort_keys=True)
    seen = {}
    for entry in entries:
        seen[key(entry)] = seen.get(key(entry), 0) + 1 if all(k in entry for k in keys) else 2
    for entry in entries:
        if seen[key(entry)] == 1:
            return {k: (vector(entry[k]) if isinstance(entry[k], tuple) else entry[k]) for k in keys}
    sys.exit(f'{where}: no entry to make the new ones like')


def last(entries, keys, where):
    """The list's last entry no other matches in `keys` (an edit's `after` adding at the end), and
    how many entries follow it (exact copies of one another at the end of a list are not told apart:
    the new entries go before them). An entry's index is what the breakable lamps' bindings name:
    release/build-content's lamp check stops a build where a bound entry moved."""
    def key(entry):
        return json.dumps([vector(entry.get(k)) if isinstance(entry.get(k), tuple) else entry.get(k) for k in keys])
    counts = {}
    for entry in entries:
        counts[key(entry)] = counts.get(key(entry), 0) + 1
    for back, entry in enumerate(reversed(entries)):
        if all(k in entry for k in keys) and counts[key(entry)] == 1:
            return {k: (vector(entry[k]) if isinstance(entry[k], tuple) else entry[k]) for k in keys}, back
    sys.exit(f'{where}: no entry of its own {keys}')


def project_edits(edits, source, target, live, data, where, said):
    """The edits that make `target` of a copy of `source`: 0.46c2's respawn points for the
    project's own, and the bases' boxes."""
    file = f'projects/{target}/client_project'
    edits['copy'].append({'files': f'projects/{source}/client_project', 'from': f'projects/{source}/',
                          'to': f'projects/{target}/'})
    points = live['server_objects']['respawn_points']
    names = [point['full_name'] for point in points]
    if len(set(names)) != len(names) or len({frozenset(point) for point in points}) != 1:
        sys.exit(f'{where}: its respawn points share a name, or differ in their keys')
    # A point whose two radii are floats in the file (London's first team has whole 1s: an int
    # takes no 0.75), of the default layer.
    like = next((p for p in points if p.get('layer') == 'default' and p['team'] in (0, 1)
                 and isinstance(p['enemy_check_radius'], float) and isinstance(p['ally_check_radius'], float)), None)
    if like is None:
        sys.exit(f'{where}: no respawn point of the default layer with float radii to make the new ones like')
    after = {'full_name': names[-1]}
    for index, spawn in enumerate(data['spawns']):
        full_name = f"logic/modes/artefact_hunt/respawn/team{spawn['team'] + 1}/ah_respawn_point({index})"
        edits['add_entry'].append({
            'files': file, 'list': 'server_objects/respawn_points', 'after': after,
            'like': {'full_name': like['full_name']}, 'expected_files': 1,
            'values': {'name': f'ah_respawn_point({index})', 'full_name': full_name, 'point_id': index,
                       'team': spawn['team'], 'position': vector(spawn['position']),
                       'rotation': vector(spawn['rotation']), 'priority': spawn['priority'],
                       'enemy_check_radius': shortest(spawn['enemy_check_radius']),
                       'ally_check_radius': shortest(spawn['ally_check_radius']), 'layer': 'default'}})
        after = {'full_name': full_name}
    for name in names:
        edits['remove_entry'].append({'file': file, 'list': 'server_objects/respawn_points',
                                      'where': {'full_name': name}})
    # The boxes go after the list's last entry: an entry's index is what the breakable lamps'
    # bindings (srp_lamps, breakable-lamps make_edits.py) name, so none may move.
    visuals = [e for e in live['render_visuals'] if set(e) == {'lib_name', 'visual_name', 'position', 'rotation', 'scale'}]
    visual = unique(visuals, ('visual_name', 'position'), where)
    last_visual, after_visual = last(live['render_visuals'], ('visual_name', 'position', 'rotation', 'scale'), where)
    if after_visual:
        said.add(f'{where}: the boxes go before the {after_visual} render visuals at the end of the list that copy another')
    solid = unique([e for e in live['collision_objects'] if '#' not in e['lib_name']],
                   ('lib_name', 'position', 'cgroup', 'cmask'), where)
    last_solid, _ = last(live['collision_objects'], ('lib_name', 'position', 'rotation', 'cgroup', 'cmask'), where)
    for base in data['bases']:
        place = {'position': vector(base['box']['position']), 'rotation': vector(base['box']['rotation']),
                 'scale': [1.0, 1.0, 1.0]}
        edits['add_entry'].append({'files': file, 'list': 'render_visuals', 'after': last_visual, 'like': visual,
                                   'expected_files': 1, 'values': {'lib_name': BOX, 'visual_name': BOX, **place}})
        for lib_name, cgroup, cmask in base['solids']:
            edits['add_entry'].append({'files': file, 'list': 'collision_objects', 'after': last_solid, 'like': solid,
                                       'expected_files': 1,
                                       'values': {'lib_name': lib_name, 'cgroup': cgroup, 'cmask': cmask, **place}})


def check_project(built, live, data, where):
    """A project as the edit list makes it (release/build-content's, or content_edit.py's): the
    map's own but for its respawn points, which are 0.46c2's and nothing else, and the two boxes;
    every value at a path 0.26g's project has, of a type it has there."""
    schema = research.project_paths(live)
    research.check_types(built, schema, where)
    points = built['server_objects']['respawn_points']
    want = [(index, s['team'], s['position'], s['rotation'], s['priority']) for index, s in enumerate(data['spawns'])]
    have = [(p['point_id'], p['team'], tuple(p['position']), tuple(p['rotation']), p['priority']) for p in points]
    if have != want:
        sys.exit(f'{where}: its respawn points are not 0.46c2\'s {len(want)}, in their order')
    if len({frozenset(p) for p in points} | {frozenset(p) for p in live['server_objects']['respawn_points']}) != 1:
        sys.exit(f'{where}: a respawn point without the keys the map\'s own have')
    for base in data['bases']:
        place = (base['box']['position'], base['box']['rotation'])
        if [(tuple(e['position']), tuple(e['rotation'])) for e in built['render_visuals']
                if e.get('visual_name') == BOX] .count(place) != 1:
            sys.exit(f"{where}: team {base['team']}'s box is not there once")
        solids = sorted((e['lib_name'], e['cgroup'], e['cmask']) for e in built['collision_objects']
                        if e['lib_name'].startswith(f'resources/models/{BOX}.model/') and tuple(e['position']) == place[0])
        if solids != base['solids']:
            sys.exit(f"{where}: team {base['team']}'s box has not its two collision objects")
    for key in built:
        if key in ('render_visuals', 'collision_objects'):
            rest = [e for e in built[key] if BOX not in e['lib_name']]
            if rest != live[key]:
                sys.exit(f'{where}: {key} differ from the map\'s own by more than the boxes')
        elif key != 'server_objects' and built[key] != live[key]:
            sys.exit(f'{where}: {key} is not the map\'s own')
    if {k: v for k, v in built['server_objects'].items() if k != 'respawn_points'} != \
            {k: v for k, v in live['server_objects'].items() if k != 'respawn_points'}:
        sys.exit(f'{where}: server_objects differ from the map\'s own by more than the respawn points')
    return f"{where}: {len(points)} respawn points, protection {sorted({p['invulnerability_time'] for p in points})} s"


# ---- the fit report ------------------------------------------------------------------------

def spot(point):
    return f'({point[0]:.3f}, {point[1]:.3f}, {point[2]:.3f})'


def beside(world, point, reach, drop):
    """What else is there for a thing in the air at `point`: a don't-walk face (walker collision
    the body slides off, mapdata's `WalkerBehaviour::DontWalk`) in its column within `drop` under
    it, or ground within `reach` of its middle (it stands on an edge)."""
    x, y, z = point
    dont = [h for h, ground, walker in world.column(x, z) if walker and not ground and y - drop <= h <= y + floors.SUNK]
    if dont:
        return f'; a don\'t-walk face {y - max(dont):.2f} m under it'
    edge = []
    for n in range(8):
        dx, dz = reach * math.cos(n * math.pi / 4), reach * math.sin(n * math.pi / 4)
        edge += [h for h, ground, walker in world.column(x + dx, z + dz)
                 if ground and walker and y - drop <= h <= y + floors.SUNK]
    if edge:
        return f'; on an edge: ground {y - max(edge):.2f} m under it within {reach:.2f} m of its middle'
    return ''


def body_fit(world, point):
    """What is wrong with a body standing at `point`, or None."""
    at = world.stand(point)
    if at['below'] is None and at['above'] is None:
        return 'OFF THE MAP: no floor in its column'
    if at['below'] is None or (at['drop'] > 0.35 and at['above'] is not None and at['rise'] < min(at['drop'], 2.0)):
        return f"UNDER THE FLOOR: the floor is {at['rise']:.2f} m over it"
    if at['above'] is not None and at['rise'] < 0.35 and at['drop'] > 0.02:
        # a floor a step over the point's own: 0.46c2's ground is lower here
        return f"UNDER THE FLOOR: a floor is {at['rise']:.2f} m over it, another {at['drop']:.2f} m under it"
    if at['drop'] > 0.35 and at['above'] is not None and at['rise'] < floors.HEIGHT:
        return f"UNDER THE FLOOR: between two, {at['drop']:.2f} m over one and {at['rise']:.2f} m under the other"
    if at['drop'] > 0.35:
        return f"IN THE AIR: the floor is {at['drop']:.2f} m under it" + beside(world, point, floors.RADIUS, 0.35)
    if at['head'] is not None and at['head'] < floors.HEIGHT:
        return f"UNDER A CEILING: {at['head']:.2f} m of room over the floor"
    if at['wall'] < 0.25:
        return f"IN A WALL: a face {at['wall']:.2f} m from the body's middle"
    return None


def lying_fit(world, point):
    """What is wrong with the artefact lying at `point` (0.46c2 puts it 0.12 m over its spawner's
    point; its shape is a 0.13 m ball whose bottom is the point), or None."""
    at = world.stand(point, lying=True)
    if at['below'] is None and at['above'] is None:
        return 'OFF THE MAP: nothing in its column'
    if at['below'] is None or (at['drop'] > 0.5 and at['above'] is not None and at['rise'] < min(at['drop'], 1.0)):
        return f"UNDER THE FLOOR: a surface is {at['rise']:.2f} m over it"
    if at['drop'] > 0.5:
        return f"IN THE AIR: the surface is {at['drop']:.2f} m under it"
    near = world.place(point, reach=2.0)
    if near is None or abs(near[1][1] - point[1]) > 1.6:
        return 'OUT OF REACH: no place for a body to stand within 2 m of it'
    return None


def footprint(world, box, rotation):
    """How much of a box's foot has ground within 0.3 m under it (a 5 x 5 grid over its square)."""
    axes = research.rotation(rotation)
    held = 0
    for i in range(5):
        for j in range(5):
            u, w = BOX_HALF[0] * (i / 2 - 1), BOX_HALF[1] * (j / 2 - 1)
            x, z = box[0] + u * axes[0][0] + w * axes[2][0], box[2] + u * axes[0][2] + w * axes[2][2]
            held += any(ground and walker and box[1] - 0.3 <= h <= box[1] + floors.SUNK
                        for h, ground, walker in world.column(x, z))
    return f'; ground within 0.3 m under {held} of 25 points of its foot'


def foot_floor(world, box, rotation):
    """The highest walker ground under a box's foot (a 5 x 5 grid over its square, turned as it is),
    from 1 m under its foot to 0.5 m over it: where its foot rests (the model's origin is its foot:
    every surface's lowest vertex is at y 0), or None."""
    axes = research.rotation(rotation)
    top = None
    for i in range(5):
        for j in range(5):
            u, w = BOX_HALF[0] * (i / 2 - 1), BOX_HALF[1] * (j / 2 - 1)
            x, z = box[0] + u * axes[0][0] + w * axes[2][0], box[2] + u * axes[0][2] + w * axes[2][2]
            for h, ground, walker in world.column(x, z):
                if ground and walker and box[1] - 1.0 <= h <= box[1] + 0.5 and (top is None or h > top):
                    top = h
    return top


def box_fit(world, box, rotation):
    """What is wrong with a base's box standing at `box` (1.2 m square, 1.15 m tall), or None: a prop
    the eye judges, so a foot more than BOX_SNAP off the floor under it is named."""
    at = world.stand(box)
    if at['below'] is None and at['above'] is None:
        return 'OFF THE MAP: no floor in its column'
    top = foot_floor(world, box, rotation)
    if top is not None and abs(box[1] - top) > BOX_SNAP:
        side = 'over' if box[1] > top else 'under'
        return f'OFF THE FLOOR: its foot is {abs(box[1] - top):.3f} m {side} the highest floor under its foot'
    if at['below'] is None or (at['drop'] > 0.3 and at['above'] is not None and at['rise'] < min(at['drop'], 1.2)):
        return f"UNDER THE FLOOR: the floor is {at['rise']:.2f} m over its foot"
    if at['drop'] > 0.3:
        return f"IN THE AIR: the floor is {at['drop']:.2f} m under its foot{footprint(world, box, rotation)}"
    wall = world.nearest(box[0], box[2], at['below'] + 0.75, at['below'] + 1.0, 1.0)
    if wall < 0.35:
        return f'IN A WALL: a face {wall:.2f} m from its middle (its sides are 0.6 m from it)'
    near = world.place((box[0], at['below'], box[2]), reach=2.5)
    if near is None or abs(near[1][1] - at['below']) > 1.0:
        return 'OUT OF REACH: no place for a body to stand within 2.5 m of it'
    return None


def in_box(point, bases, reach, low, high):
    """The team whose base box `point` is in (or within `reach` of its sides, from `low` under its
    foot to `high` over it), or None: the boxes are the pack's, in no 0.26g map's floors."""
    for base in bases:
        position, axes = base['box']['position'], research.rotation(base['box']['rotation'])
        offset = [point[i] - position[i] for i in range(3)]
        local = [sum(offset[i] * axes[row][i] for i in range(3)) for row in range(3)]
        if (abs(local[0]) < BOX_HALF[0] + reach and abs(local[2]) < BOX_HALF[1] + reach
                and -low < local[1] < BOX_HEIGHT + high):
            return base['team']
    return None


def proposal(world, kind, point, rotation=None):
    """(text, place) of moves.tsv's place columns for a point that does not fit: a box's foot onto
    the floor under it, else the nearest fitting place; place None when there is none."""
    if kind == 'base':
        top = foot_floor(world, point, rotation)
        if top is not None:
            place = (point[0], top, point[2])
            return f'its foot on the floor at {spot(place)}, {top - point[1]:+.3f} m', place
        near = world.place(point, reach=10.0, clear=0.7, low=0.75, high=1.0, flat=0.6)
    else:
        near = world.place(point, reach=10.0)
    if near is None:
        return 'no fitting place within 10 m', None
    place = near[1] if kind != 'spawner' else (near[1][0], near[1][1] + 0.12, near[1][2])
    return f'nearest fitting place {spot(place)}, {near[0]:.2f} m away', place


def fit_report(folder, projects):
    """Prints each point of each map against the map's floors; returns the lines."""
    lines, cache, rows = [], {}, {}
    by_level = {}
    for level, source, data, live in projects:
        path = folder / (source.replace('/', '_') + '.mapdata')
        if not path.exists():
            sys.exit(f'--mapdata: no {path.name} (mapdata-dump --project {source})')
        mapdata = floors.load(path)
        points = ([b['box']['position'] for b in data['bases']] + [s['position'] for s in data['spawners']]
                  + [s['position'] for s in data['spawns']])
        # The three weathers of a level stand on the same collision but for what rain adds: one
        # world a set of placements.
        key = hashlib.sha256(repr((sorted(mapdata['placements']), sorted(points))).encode()).hexdigest()
        if key not in cache:
            world = floors.World(mapdata, points + [s[0] for s in mapdata['spawns']])
            found = []
            for base in data['bases']:
                found.append(('base', base['team'], f"team {base['team']}'s box", base['box']['position'],
                              box_fit(world, base['box']['position'], base['box']['rotation'])))
            for index, spawner in enumerate(data['spawners']):
                team = in_box(spawner['position'], data['bases'], 0.0, 0.0, -0.05)
                wrong = f"IN team {team}'s BOX" if team is not None else lying_fit(world, spawner['position'])
                found.append(('spawner', index, f"artefact spawn point {index} ({spawner['name']})", spawner['position'],
                              wrong))
            for index, spawn in enumerate(data['spawns']):
                team = in_box(spawn['position'], data['bases'], floors.RADIUS, floors.HEIGHT, 0.0)
                wrong = f"IN team {team}'s BOX" if team is not None else body_fit(world, spawn['position'])
                found.append(('spawn', index, f"respawn point {index} of team {spawn['team']} ({spawn['name']})",
                              spawn['position'], wrong))
            own = [body_fit(world, s[0]) for s in mapdata['spawns'] if s[2] in (0, 1)]
            text = []
            rotations = {base['team']: base['box']['rotation'] for base in data['bases']}
            for kind, index, what, point, wrong in found:
                if wrong:
                    said, place = proposal(world, kind, point, rotations.get(index) if kind == 'base' else None)
                    text.append(f'  {what} at {spot(point)}: {wrong}; {said}  [moves.tsv: {level[-2:]} {kind} {index}]')
                    if place is not None:
                        rows[(level[-2:], kind, index)] = (place, f'{wrong.split(";")[0]} (was {spot(point)})')
            counts = {kind: (sum(1 for f in found if f[0] == kind), sum(1 for f in found if f[0] == kind and f[4]))
                      for kind in ('base', 'spawner', 'spawn')}
            frame = [(live['levelmap_min'][i], live['levelmap_max'][i]) for i in (0, 2)] if 'levelmap_min' in live else []
            if frame and all(high - low > 20 for low, high in frame):
                outside = [what for _, _, what, point, _ in found
                           if not (frame[0][0] <= point[0] <= frame[0][1] and frame[1][0] <= point[2] <= frame[1][1])]
                text += [f'  {what}: outside the map\'s frame (levelmap_min, levelmap_max)' for what in outside]
            ours = sorted((o['team'], tuple(o['position'])) for o in live['client_objects']
                          if o.get('game_object_type') == 'resupply_zone')
            if [(t, tuple(round(v, 1) for v in p)) for t, p in ours] != \
                    [(t, tuple(round(v, 1) for v in p)) for t, p in data['resupply']]:
                text.append('  resupply zones: 0.26g\'s Team Deathmatch project keeps its own ('
                            + ', '.join(f'team {t} {spot(p)}' for t, p in ours) + '); 0.46c2\'s Artifact Hunt has '
                            + ', '.join(f'team {t} {spot(p)}' for t, p in data['resupply']))
            cache[key] = (tuple(sorted(counts.items())), tuple(text), (len(own), sum(1 for w in own if w)))
        by_level.setdefault(level, []).append((source, cache[key]))
    for level, sources in by_level.items():
        for found in dict.fromkeys(f for _, f in sources):
            counts, text, own = dict(found[0]), found[1], found[2]
            on = ', '.join(source for source, f in sources if f == found)
            lines.append(f'{level} on 0.26g\'s {on}: bases {counts["base"][0]} ({counts["base"][1]} named), artefact '
                         f'spawn points {counts["spawner"][0]} ({counts["spawner"][1]} named), respawn points '
                         f'{counts["spawn"][0]} ({counts["spawn"][1]} named); by the same test, of the project\'s '
                         f'own {own[0]} Team Deathmatch respawn points {own[1]} would be named')
            lines += text
    return lines, rows


# ---- texts ---------------------------------------------------------------------------------

def texts():
    rows = [line.split('\t') for line in (HERE / 'texts.tsv').read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#')]
    if tuple(rows[0]) != ('key',) + LANGUAGES or any(len(row) != len(rows[0]) or not all(row) for row in rows):
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}, none empty')
    table = {row[0]: dict(zip(LANGUAGES, row[1:])) for row in rows[1:]}
    if set(table) != {NAME_KEY, DESCRIPTION_KEY, *HINT_KEYS, *OTHER_KEYS}:
        sys.exit(f'texts.tsv: rows the mode does not name: {sorted(set(table) - {NAME_KEY, DESCRIPTION_KEY, *HINT_KEYS, *OTHER_KEYS})}')
    for key in [NAME_KEY, DESCRIPTION_KEY] + HINT_KEYS + OTHER_KEYS:
        if key not in table:
            sys.exit(f'texts.tsv has no {key} (MODE is {MODE})')
    return table


def catalog(folder, write):
    """The mode's texts against the dictionaries' TSVs (`key<TAB>text`, sorted by key)."""
    table, missing = texts(), 0
    for language in LANGUAGES:
        path = folder / f'{language}.tsv'
        lines = path.read_text(encoding='utf-8').split('\n')
        have = {line.split('\t', 1)[0]: line.split('\t', 1)[1] for line in lines[1:] if '\t' in line}
        for key, by in sorted(table.items()):
            if key in have:
                if have[key] != by[language]:
                    sys.exit(f'{path.name}: {key} is there with another text')
                continue
            missing += 1
            if write:
                at = next((n for n in range(1, len(lines)) if not lines[n] or lines[n].split('\t', 1)[0] > key),
                          len(lines))
                lines.insert(at, f'{key}\t{by[language]}')
            else:
                print(f'{path.name} lacks: {key}\t{by[language]}')
        if write:
            path.write_text('\n'.join(lines), encoding='utf-8')
    print(f'{len(table)} texts in {len(LANGUAGES)} languages: {missing} rows ' + ('written' if write else 'missing'))


# ---- main ----------------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--inputs', type=Path)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--edits', type=Path)
    parser.add_argument('--mapdata', type=Path)
    parser.add_argument('--built', type=Path)
    parser.add_argument('--catalog', type=Path)
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--take-proposals', action='store_true')
    args = parser.parse_args()
    if args.catalog:
        return catalog(args.catalog, args.write)
    if not args.inputs or not args.out:
        parser.error('--inputs and --out, or --catalog')
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    check = research.rotation(research.ROTATION_CHECK[0])
    if any(abs(check[i][j] - research.ROTATION_CHECK[1][i][j]) > 1e-4 for i in range(3) for j in range(3)):
        sys.exit('rotation(): not the game\'s')
    texts()
    inputs, stock, moves = Inputs(args.inputs), forms(), read_moves()
    files, log, said = {}, [], set()

    names, blind, colour_alpha = inputs.assets(), set(), set()
    have = inputs.listing | {name for name in names if name != ITEM}
    for name in names:
        if name != ITEM:
            files[name] = asset(name, inputs.read('donor/' + name), stock, have, log, blind, colour_alpha)
    for texture in sorted(colour_alpha):
        if f'textures/{texture}.dds' in have and f'textures/{texture}.dds' not in files:
            sys.exit(f'{texture}: a colour transparency the pack does not carry (0.26g\'s own would stay DXT1)')
    donor_item = bcw.loads(inputs.read('donor/' + ITEM))
    item = item_values(donor_item)
    effect_path = f"gameplay/effects/effect_{donor_item['data']['effect']}.options"
    for kind, name in (('model', f"models/{item['model']}.model/settings"), ('effect', effect_path),
                       *((key, f"particles/{item[key]}.particle")
                         for key in ('particle_idle', 'particle_spawn', 'particle_destroy')),
                       ('box', f'models/{BOX}.model/settings')):
        if name not in files:
            sys.exit(f'the artefact\'s {kind} {name} is not among the assets')
    system = bcw.loads(files[effect_path])['effect']['effects'][0]['particle_system']
    twin, changed = first_person(bcw.loads(files[f'particles/{system}.particle']))
    files[f'particles/{system}{FIRST_PERSON_SUFFIX}.particle'] = bcw.dump(twin)
    # the exe draws the carrier's glow itself, as these two particles (its effect is not read)
    item['carrier_particle'], item['carrier_particle_fp'] = system, system + FIRST_PERSON_SUFFIX
    log.append(f'particles/{system}{FIRST_PERSON_SUFFIX}.particle: the first-person twin of {system}, '
               f'Head, Face and Neck off in its {changed} animated domains that had them')

    edits = {'_about': '', 'copy': [], 'add_entry': [], 'remove_entry': [], 'append_entry': []}
    entries, projects, per_map, checked = [], [], [], []
    parameters = None
    for number, (level, theirs, config_weather, folder) in enumerate(maps()):
        where = f'level_{level}/{folder}_ah'
        donor_config = bcw.loads(inputs.read(f'donor/gameplay/game_modes/level_{level}_{theirs}_ah.options'))
        live_config = bcw.loads(inputs.read(f'live/gameplay/game_modes/level_{level}_{config_weather}_4.options'))
        donor_project = donor_config['mode']['match_making_params']['map_name']
        source = live_config['mode']['match_making_params']['map_name']
        if source != f'level_{level}/{folder}_4':
            sys.exit(f'{where}: 0.26g\'s Team Deathmatch project is {source}')
        if parameters not in (None, donor_config['mode']['parameters']):
            sys.exit(f'{where}: 0.46c2\'s parameters differ from the other maps\'')
        parameters = donor_config['mode']['parameters']
        data = mode_data(bcw.loads(inputs.read(f'donor/projects/{donor_project}/client_project')), where)
        data = moved(data, level, moves, said)
        data['item'] = item
        live = bcw.loads(inputs.read(f'live/projects/{source}/client_project'))
        name = f'level_{level}_{config_weather}_ah'
        config = mode_config(donor_config, live_config, name, where, data)
        research.check_config_types(config, live_config, where)
        files[f'gameplay/game_modes/{FOLDER}/{name}.options'] = bcw.dump(config)
        entries.append({'cfg_mode_name': f'{FOLDER}/{name}.options', 'id': FIRST_MAP_ID + number})
        edits['append_entry'].append({'file': 'gameplay/db_static_dictionaries', 'list': 'maps_dict',
                                      'expected_count': MAPS_DICT_COUNT + number, 'unique_key': 'id',
                                      'value': entries[-1]})
        project_edits(edits, source, where, live, data, where, said)
        projects.append((f'level_{level}', source, data, live))
        if args.built:
            built = bcw.loads((args.built / f'projects/{where}/client_project').read_bytes())
            checked.append(check_project(built, live, data, where))
        first = sum(1 for s in data['spawns'] if not s['use_for_first_respawn'])
        twice = len(data['spawns']) - len({(s['team'], s['position']) for s in data['spawns']})
        per_map.append(f"{FIRST_MAP_ID + number} {name}: project {where} of {source}; {len(data['spawners'])} artefact "
                       f"spawn points; bases " + ', '.join(f"team {b['team']} {spot(b['position'])}" for b in data['bases'])
                       + f"; {len(data['spawns'])} respawn points ("
                       + ', '.join(f"team {t} {sum(1 for s in data['spawns'] if s['team'] == t)}" for t in (0, 1))
                       + f"; {first} not for a first respawn in 0.46c2, a flag 0.26g's points have not"
                       + (f"; {twice} where another of the team stands, as 0.46c2 has them" if twice else '')
                       + ") for 0.26g's "
                       f"{len(live['server_objects']['respawn_points'])}")
    edits['_about'] = (
        f'Artifact Hunt (mode {MODE}, docs/notes/artefact_hunt.md; generated by scripts/artefact-hunt/make_pack.py, '
        'do not edit; after every edit of a Team Deathmatch map in release/content.tsv): each of the 27 maps\' own '
        'project, projects/level_0N/<weather>_ah/, a copy of the map\'s Team Deathmatch project as every row above '
        'leaves it, with 0.46c2\'s Artifact Hunt respawn points in the place of Team Deathmatch\'s (each made like a '
        'point of the project) and the two bases\' box (item/storage_device/box_atlant8c of the pack artefact-hunt: '
        'its model, its collision and its walker collision); a maps_dict row each config of the pack, ids '
        f'{FIRST_MAP_ID}-{FIRST_MAP_ID + len(entries) - 1}. The mode configs and the assets are the pack\'s.')

    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': f'Artifact Hunt (mode {MODE}) as 0.46c2 has it: the 27 configs of its nine levels (the '
                       f'artefact\'s spawn points, the bases, the item) for maps {FIRST_MAP_ID}-'
                       f'{FIRST_MAP_ID + len(entries) - 1} (their maps_dict rows: {EDITS}), the artefact\'s and the base box\'s models, particles, '
                       f'effect and the announcer\'s lines (artefact_hunt.md)',
        'operations': [{'op': 'add_file', 'path': path} for path in sorted(files)],
        'notes': ['made by srp-026g/scripts/artefact-hunt/make_pack.py from inputs.sha256, stock_forms.tsv and moves.tsv',
                  'donor: Survarium 0.46c2',
                  f'the maps\' projects are made by the edit list {EDITS} of the same maker'] + log + sorted(said),
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    if args.edits:
        args.edits.write_text(json.dumps(edits, ensure_ascii=False, separators=(',', ':')).replace('},{', '},\n{')
                              + '\n', encoding='utf-8')
    print('\n'.join(per_map + log + sorted(said)))
    kinds = {}
    for path in files:
        kind = ('configs' if path.startswith('gameplay/game_modes/') else path.split('/')[0])
        kinds[kind] = kinds.get(kind, 0) + 1
    print(f'{args.out}: {len(files)} files ({", ".join(f"{n} {k}" for k, n in sorted(kinds.items()))}), '
          f'{sum(map(len, files.values())) // 1024} KiB, pack.json sha256 {digest}')
    print(f'the edit list: {len(edits["copy"])} projects, {len(edits["add_entry"])} entries added, '
          f'{len(edits["remove_entry"])} taken out' + (f' ({args.edits})' if args.edits else ' (not written: --edits)'))
    print(f'manifest rows:\npack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}\nedit\t{EDITS}\t-\t'
          f'Artifact Hunt: the 27 maps\' own projects, Team Deathmatch\'s with 0.46c2\'s respawn points and bases, '
          f'and their maps_dict rows')
    if checked:
        print(f'\n{args.built}: the {len(checked)} projects are their maps\' own with 0.46c2\'s respawn points and '
              f'the boxes, every value of a type 0.26g has there:\n' + '\n'.join(checked))
    if args.mapdata:
        print('\nThe fit of 0.46c2\'s points on 0.26g\'s maps (floors.py; a body 1.8 m tall and 0.38 m round on the '
              'walker mesh):\n- a respawn point fits on a floor within 0.35 m under it, with a body\'s room over it '
              'and no face within 0.25 m of the body\'s middle;\n- an artefact spawn point fits up to 0.5 m over a '
              'surface, with a place for a body within 2 m;\n- a base\'s box fits with its foot within 0.3 m of a '
              'floor, no face within 0.35 m of its middle and a place for a body within 2.5 m.\nA point named below '
              'is in the pack as 0.46c2 has it, unless moves.tsv says where it goes.')
        lines, rows = fit_report(args.mapdata, projects)
        print('\n'.join(lines))
        if args.take_proposals:
            take_proposals(rows)
    else:
        print('\nno fit report: --mapdata was not given')


if __name__ == '__main__':
    main()
