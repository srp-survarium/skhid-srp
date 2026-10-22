#!/usr/bin/env python3
"""make_pack.py: the content pack of Research, mode 5 (srp-026g/docs/notes/research_mode.md).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --london <folder> --out <packs>/research-mode-<version>

--stock   0.26g's files the maps are made from, at their game paths or flattened as dbgrab writes
          them (projects__level_01__day_4__client_project): every projects/level_0N/<w>_4/client_project
          of the stock resources.db (w day, evening, rain; level_08 prerain for rain), level_09's
          day_4 of resources.db.patch_14 (London), and the stock db's listing of every file
          (`dbgrab <resources.db> list > stock.list`, then each patch's appended) as stock.list.
--donor   0.27d0 build 2500's files (the release that brought Research): projects/level_0N/<w>_5
          and <w>_4 client_project, gameplay/game_modes/level_0N_<w>_5.options, the six
          localization/<language>/localization, and the files of the models the units' props use
          that 0.26g lacks (MODELS).
--london  0.31a2's London: projects/level_09/day_5 and day_4 client_project (its zones and props).
--out     the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
          manifest's `pack` row (release/content.tsv).

Every input is checked against inputs.sha256. What the pack holds, per map and weather of 0.27d
(24) and London's day (25 maps):
- gameplay/game_modes/level_0N_<w>_5.options: 0.27d's config (its parameters, as the owner
  approved them unchanged) with rules [93, 91, RULE_CLASS, 94] (exe patch 53's class), the map's
  project, and the units: mode.parameters.capture_zones[] = {cp_id, position, shapes[] {type,
  position, axis_x, axis_y, axis_z, half}} from the capturable_zone objects and the
  collision_geometry they name, in world space; capture_award 100 (0.27d's capture_the_point,
  shared); capture_reach and capture_half_height (a player's capsule, research_rule_026g.h).
- projects/level_0N/<w>_5/client_project: 0.26g's <w>_4 project with what 0.27d's <w>_5 changed
  against 0.27d's own <w>_4 (the units' props: render visuals, collision objects, sounds,
  particles; the cover it took away), its respawn points, and without its zones (the config
  has them) and the objects 0.26g cannot load (LEFT_OUT).
- the models those props use that 0.26g lacks (byte for byte 0.27d's: the model format is the
  same, MODELS);
- maps_dict rows (MAP_IDS), the texts (0.27d's keys in the six languages; srp's own in texts.tsv)
  and the hud's strings (localization/flash_movies_localizations.options hud_strings).
"""
import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import re
import struct
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bcw  # noqa: E402

PACK_ID = 'research-mode'
VERSION = '2'
RULE_CLASS = 0x1E0           # client-patches/research_native_026g.c RN_CLASS (patch 53)
RULES = [93, 91, RULE_CLASS, 94]
CAPTURE_AWARD = 100          # 0.27d gameplay/awards/events.options capture_the_point: shared, factor 100
CAPTURE_REACH = 0.4          # m: a player's capsule radius, added round each shape
CAPTURE_HALF_HEIGHT = 0.9    # m: half the capsule's height over its centre (player_bots_behaviour.md)
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
WEATHERS = {'day': 'day', 'evn': 'evening', 'rain': 'rain', 'prerain': 'prerain'}

# The maps: (level, 0.27d weather, maps_dict id). 145-149 are 0.26g's own rows (level_0N_day_5,
# never with a file); 0.27d's other ids (152-170) are taken by srp's ported maps, so the others
# have ids of their own from 184 (after London's 181-183), London's day last.
MAP_IDS = [
    ('01', 'day', 145), ('02', 'day', 184), ('03', 'day', 185), ('04', 'day', 146), ('05', 'day', 186),
    ('06', 'day', 147), ('07', 'day', 148), ('08', 'day', 149),
    ('01', 'evn', 187), ('02', 'evn', 188), ('03', 'evn', 189), ('04', 'evn', 190), ('05', 'evn', 191),
    ('06', 'evn', 192), ('07', 'evn', 193), ('08', 'evn', 194),
    ('01', 'rain', 195), ('02', 'rain', 196), ('03', 'rain', 197), ('04', 'rain', 198), ('05', 'rain', 199),
    ('06', 'rain', 200), ('07', 'rain', 201), ('08', 'prerain', 202),
    ('09', 'day', 203),
]
# 0.27d's keys of the mode's texts, in the six languages the donor has them.
DONOR_TEXTS = ('st_mode_05_name', 'st_mode_05_description', 'st_match_settings_checkbox_capture_points',
               'st_match_settings_checkbox_capture_points_hint', 'st_capture_info',
               'st_start_match_welcome_message_5', 'st_finish_match_mode_05_winner_message',
               'st_finish_match_mode_05_loser_message', 'st_event_capture_the_point_description',
               'st_event_kill_invader_description')
# The hud movie's names of texts (survarium.data.LocalizationData) and their keys.
HUD_STRINGS = [['research_capturing', 'st_capture_info']]
# Models of the props 0.26g has not; their files are 0.27d's as they are.
# (0.27d moved level_01's tractor beside unit 1 for cover; 0.26g's level_01 has none.)
MODELS = ('item/quest/quest_stellage/transmitter/transmitter_cable_corner', 'props/box/low_cover_corner',
          'props/box/props_boxes_base_a_set', 'vehicle/construct&utility/tractor')
# What 0.26g cannot load of the mode's objects, and why (research_mode.md, "Left out").
LEFT_OUT = {
    'capturable_zone': 'the units: in the config (capture_zones), read by exe patch 53',
    'effect_zone': "the spawn's invulnerability zone: 0.26g's projects have none (the respawn points give 6 s)",
    'generic_anomaly_core': "level_03's extra anomaly: 0.27d's anomaly format, not the mode's",
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def game_path(name):
    return name if '/' in name else name.replace('__', '/')


class Folder:
    """Files at their game paths or flattened, checked against inputs.sha256."""

    def __init__(self, root, sums, what):
        self.root, self.sums, self.what, self.files = Path(root), sums, what, {}
        for folder, _, names in os.walk(self.root):
            for name in names:
                rel = os.path.relpath(os.path.join(folder, name), self.root).replace(os.sep, '/')
                self.files[game_path(rel)] = Path(folder) / name

    def has(self, path):
        return path in self.files

    def read(self, path, checked=True):
        if path not in self.files:
            sys.exit(f'{self.what}: no {path} in {self.root}')
        data = self.files[path].read_bytes()
        if not checked:
            return data
        want = self.sums.get(f'{self.what}/{path}')
        if want is None:
            sys.exit(f'inputs.sha256 has no {self.what}/{path}')
        if sha256(data) != want:
            sys.exit(f'{self.what}: {path} is not the file this pack is made of (sha256 {want})')
        return data


# ---- binary configs ----------------------------------------------------------------------

def load_localization(data):
    """A localization file: its texts are UTF-8 (a few of other strings are not: kept as bytes)."""
    def walk(node):
        kind, payload = node[0], node[1]
        if kind == bcw.STRING:
            try:
                return payload.decode('utf-8')
            except UnicodeDecodeError:
                return payload
        if kind == bcw.NAMED:
            return {key.decode('utf-8'): walk(child) for key, child in payload}
        if kind == bcw.INDEXED:
            return [walk(child) for child in payload]
        return bcw.to_py(node)
    return walk(bcw.read(data))


def porter_text(text):
    """The game keeps texts as UTF-8 bytes; the porter reads and writes a config's strings as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098), so a pack names the UTF-8 bytes so read
    (decals-069d's make_pack.py)."""
    raw = text.encode('utf-8')
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in raw)


# ---- geometry ----------------------------------------------------------------------------

def rotation(angles):
    """The game's rotation of a config's `rotation` (0x725E0, run under Unicorn on (0.3, 0.7,
    -0.4) for ROTATION_CHECK): rows are the local axes in world space (v * M)."""
    def rx(a):
        c, s = math.cos(a), math.sin(a)
        return [[1, 0, 0], [0, c, s], [0, -s, c]]

    def ry(a):
        c, s = math.cos(a), math.sin(a)
        return [[c, 0, -s], [0, 1, 0], [s, 0, c]]

    def rz(a):
        c, s = math.cos(a), math.sin(a)
        return [[c, s, 0], [-s, c, 0], [0, 0, 1]]

    def mul(a, b):
        return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
    return mul(mul(rx(-angles[0]), ry(-angles[1])), rz(-angles[2]))


ROTATION_CHECK = ((0.3, 0.7, -0.4), [[0.7045, 0.2978, 0.6442], [-0.1967, 0.9541, -0.2260],
                                     [-0.6819, 0.0325, 0.7307]])


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def zone_shapes(geometry):
    """A collision_geometry's meshes as capture_zones shapes, in world space. The geometry
    objects of every unit are unrotated and unscaled (checked): a mesh's centre is its position
    over the object's, its axes its own rotation's rows; `scale` is its half extents (0x7A1F0:
    sphere radius x, box half extents, cylinder radius x and half height y)."""
    if any(abs(a) > 1e-6 for a in geometry['rotation']) or any(abs(s - 1) > 1e-6 for s in geometry['scale']):
        sys.exit(f"{geometry['full_name']}: a rotated or scaled zone geometry (not handled)")
    origin = geometry['position']
    shapes = []
    for mesh in geometry['meshes']:
        if mesh['type'] not in (0, 1, 2):
            sys.exit(f"{geometry['full_name']}: mesh type {mesh['type']}")
        axes = rotation(mesh['rotation'])
        shapes.append({
            'type': mesh['type'],
            'position': tuple(f32(origin[i] + mesh['position'][i]) for i in range(3)),
            'axis_x': tuple(f32(v) for v in axes[0]),
            'axis_y': tuple(f32(v) for v in axes[1]),
            'axis_z': tuple(f32(v) for v in axes[2]),
            'half': tuple(f32(v) for v in mesh['scale']),
        })
    return shapes


def units_of(project, where):
    """capture_zones of a donor project: its capturable_zone objects by cp_id."""
    objects = {o.get('full_name'): o for o in project['client_objects']}
    units = []
    for zone in project['client_objects']:
        if zone.get('game_object_type') != 'capturable_zone':
            continue
        shapes = []
        for link in zone['collision_geometries']:
            geometry = objects.get(link['name'])
            if geometry is None or geometry.get('game_object_type') != 'collision_geometry':
                sys.exit(f"{where}: {zone['full_name']} names no geometry {link['name']}")
            shapes += zone_shapes(geometry)
        units.append({'cp_id': zone['cp_id'], 'position': tuple(zone['position']), 'shapes': shapes})
    units.sort(key=lambda unit: unit['cp_id'])
    if [unit['cp_id'] for unit in units] != [0, 1, 2]:
        sys.exit(f'{where}: the units are not cp_id 0, 1, 2')
    if any(len(unit['shapes']) > 8 for unit in units):
        sys.exit(f'{where}: a unit of more than 8 shapes (research_rule_026g.h RR_MAX_SHAPES)')
    return units


# ---- projects ----------------------------------------------------------------------------

def rounded(values, step=0.01):
    return tuple(round(v / step) for v in values)


def entry_key(entry):
    """What makes a render visual or a collision object the same placement in two projects: its
    model, place, turn (as a matrix: 0.27d writes some turns of 0.26g's half a turn round, -3.142
    for 3.142) and size."""
    turn = tuple(v for row in rotation(entry['rotation']) for v in row)
    return (entry.get('lib_name'), entry.get('visual_name'), entry.get('cgroup'), entry.get('cmask'),
            rounded(entry['position']), rounded(turn, 0.02), rounded(entry['scale']))


def object_key(obj):
    return (obj.get('game_object_type'), obj.get('lib_name'), obj.get('sound_name'), rounded(obj['position']))


# ---- value types --------------------------------------------------------------------------

def kind_of(value):
    """A value's type in the game's binary config (bcw.from_py's mapping)."""
    if isinstance(value, bool):
        return 'bool'
    if isinstance(value, int):
        return 'int'
    if isinstance(value, float):
        return 'float'
    if isinstance(value, str):
        return 'string'
    if isinstance(value, tuple):
        return f'float{len(value)}'
    if isinstance(value, dict):
        return 'table'
    if isinstance(value, list):
        return 'list'
    raise TypeError(type(value))


def slot(key):
    """A key as a path names it: a colour ramp's `key0`, `key1`, ... are one (the game reads them
    as `key%d` from 0 while there is one, 0x350080: a ramp may have more keys than 0.26g's have)."""
    return 'key#' if re.fullmatch(r'key\d+', key) else key


def project_paths(project, schema=None):
    """{path: {types}} of a project: a client object's keys under ('client_objects', its type), a
    list's items under '[]'."""
    schema = {} if schema is None else schema

    def walk(value, path):
        schema.setdefault(path, set()).add(kind_of(value))
        if isinstance(value, dict):
            for key, child in value.items():
                walk(child, path + (slot(key),))
        elif isinstance(value, list):
            for child in value:
                walk(child, path + ('[]',))
    for key, value in project.items():
        if key == 'client_objects':
            schema.setdefault(('client_objects',), set()).add('list')
            for obj in value:
                walk(obj, ('client_objects', obj.get('game_object_type')))
        else:
            walk(value, (key,))
    return schema


def conform(value, path, schema, where, log):
    """`value` in 0.26g's form at `path`: the keys 0.26g has there (others dropped and logged, at
    any depth), each of a type 0.26g has there (an int where 0.26g has floats only made a float);
    a type 0.26g never has there stops the build."""
    have = schema.get(path)
    if have is None:
        sys.exit(f'{where}: {"/".join(map(str, path))}: not a path of 0.26g\'s projects')
    kind = kind_of(value)
    if kind == 'int' and kind not in have and have == {'float'}:
        log.add('made a float: ' + '/'.join(map(str, path)))
        return float(value)
    if kind not in have:
        sys.exit(f'{where}: {"/".join(map(str, path))} is {kind}; 0.26g has {sorted(have)} there')
    if isinstance(value, dict):
        out = {}
        for key, child in value.items():
            if path + (slot(key),) not in schema:
                log.add('dropped: ' + '/'.join(map(str, path + (key,))))
                continue
            out[key] = conform(child, path + (slot(key),), schema, where, log)
        return out
    if isinstance(value, list):
        return [conform(child, path + ('[]',), schema, where, log) for child in value]
    return value


def check_types(project, schema, where):
    """Every value of the made project at a path 0.26g's projects have, of a type they have there."""
    for path, kinds in project_paths(project).items():
        if path not in schema:
            sys.exit(f'{where}: {"/".join(map(str, path))}: a path 0.26g\'s projects have not')
        if kinds - schema[path]:
            sys.exit(f'{where}: {"/".join(map(str, path))} is {sorted(kinds)}; 0.26g has {sorted(schema[path])}')


def check_config_types(config, stock, where):
    """The config's values at the paths a stock 0.26g mode config has: of the stock's types."""
    have = {}

    def walk(value, path, into):
        into.setdefault(path, set()).add(kind_of(value))
        if isinstance(value, dict):
            for key, child in value.items():
                walk(child, path + (key,), into)
        elif isinstance(value, list):
            for child in value:
                walk(child, path + ('[]',), into)
    walk(stock, (), have)
    made = {}
    walk(config, (), made)
    for path, kinds in made.items():
        if path in have and kinds - have[path]:
            sys.exit(f'{where}: {"/".join(map(str, path))} is {sorted(kinds)}; 0.26g\'s configs have '
                     f'{sorted(have[path])}')


def converted(obj, templates, where):
    """A 0.27d object in 0.26g's form: the keys 0.26g's objects of its type have (`is_enabled`
    is 0.26g's `enabled`, `layer` default), none it has not; it must have every key each of
    them has."""
    kind = obj.get('game_object_type')
    if kind not in templates:
        sys.exit(f'{where}: no 0.26g object of type {kind} to take the form of')
    allowed, always = templates[kind]
    out = {key: value for key, value in obj.items() if key in allowed}
    if 'enabled' in allowed and 'enabled' not in out:
        out['enabled'] = obj.get('is_enabled', True)
    if 'layer' in allowed:
        out['layer'] = 'default'
    missing = always - set(out)
    if missing:
        sys.exit(f"{where}: {obj.get('full_name')} lacks {sorted(missing)} of 0.26g's {kind}")
    return out


def templates_of(projects):
    """By object type (respawn_point for the spawns): (every key a 0.26g object of the type has,
    the keys all of them have)."""
    seen = {}
    for project in projects:
        for obj in project['client_objects']:
            seen.setdefault(obj.get('game_object_type'), []).append(set(obj))
        for point in project['server_objects']['respawn_points']:
            seen.setdefault('respawn_point', []).append(set(point))
    return {kind: (set().union(*keys), set.intersection(*keys)) for kind, keys in seen.items()}


def transplant(target, donor5, donor4, templates, schema, where, log):
    """`target` (0.26g's <w>_4) with what 0.27d's <w>_5 (donor5) changed against its own <w>_4
    (donor4): a placement donor5 has and neither donor4 nor the target has is the mode's, added;
    one the target and donor4 have and donor5 has not is one the mode took away, removed. What
    0.27d changed on the map in both its projects (and the target lacks) is not the mode's and
    is left as 0.26g has it. What is added is in 0.26g's form (conform: its keys at every depth,
    its types). Copies are deep copies: a vector stays a tuple (a float2/3/4), never a list (an
    indexed table the game would read as floats)."""
    dropped = set()
    out = copy.deepcopy(target)
    for section in ('render_visuals', 'collision_objects'):
        before = {entry_key(e) for e in donor4[section]}
        after = {entry_key(e) for e in donor5[section]}
        have = {entry_key(e) for e in out[section]}
        removed = (have & before) - after
        out[section] = [e for e in out[section] if entry_key(e) not in removed]
        added = [e for e in donor5[section] if entry_key(e) not in before and entry_key(e) not in have]
        out[section] += [conform(copy.deepcopy(e), (section, '[]'), schema, where, dropped) for e in added]
        log.append(f'{where}: {section} +{len(added)} -{len(removed)}')
    zones = set()
    for obj in donor5['client_objects']:
        if obj.get('game_object_type') in ('capturable_zone', 'effect_zone'):
            zones.update(link['name'] for link in obj.get('collision_geometries', []))
    before = {object_key(o) for o in donor4['client_objects']}
    after = {object_key(o) for o in donor5['client_objects']}
    have = {object_key(o) for o in out['client_objects']}
    removed = (have & before) - after
    # An object another object names (a geometry of an anomaly's artefact,
    # whose anomaly 0.27d's level_03 reworked and the pack leaves as 0.26g
    # has it) stays.
    staying = json.dumps([o for o in out['client_objects'] if object_key(o) not in removed])
    removed = {key for key in removed
               if not any(object_key(o) == key and o.get('full_name') and json.dumps(o['full_name']) in staying
                          for o in out['client_objects'])}
    kept = [o for o in out['client_objects'] if object_key(o) not in removed]
    left_out, added = {}, []
    for obj in donor5['client_objects']:
        if object_key(obj) in before or object_key(obj) in have:
            continue
        kind = obj.get('game_object_type')
        if kind in LEFT_OUT or obj.get('full_name') in zones:
            left_out[kind] = left_out.get(kind, 0) + 1
            continue
        added.append(conform(converted(obj, templates, where), ('client_objects', kind), schema, where, dropped))
    out['client_objects'] = kept + added
    log.append(f'{where}: client_objects +{len(added)} -{len(removed)}, left out {dict(sorted(left_out.items()))}')
    out['server_objects']['respawn_points'] = [
        conform(converted({**point, 'game_object_type': 'respawn_point'}, templates, where),
                ('server_objects', 'respawn_points', '[]'), schema, where, dropped)
        for point in donor5['server_objects']['respawn_points']]
    if dropped:
        log.append(f'{where}: to 0.26g\'s form: {"; ".join(sorted(dropped))}')
    names = {o.get('full_name') for o in out['client_objects']}
    for obj in out['client_objects']:
        for link in obj.get('collision_geometries') or []:
            if isinstance(link, dict) and link.get('name') not in names:
                sys.exit(f"{where}: {obj.get('full_name')} names {link.get('name')}, which the project has not")
    return out


def model_refs(project):
    refs = set()
    for entry in project['render_visuals']:
        refs.add(entry.get('visual_name') or entry['lib_name'])
    for entry in project['collision_objects']:
        name = entry['lib_name']
        if name.startswith('resources/models/') and '.model/' in name:
            refs.add(name[len('resources/models/'):name.index('.model/')])
    return refs


# ---- configs -----------------------------------------------------------------------------

def mode_config(donor, level, weather, map_id, project_path, units, name):
    config = copy.deepcopy(donor)
    mode = config['mode']
    mode['name'] = name
    mode['rules'] = list(RULES)
    mode['mode'] = 5
    mode['match_making_params']['map_name'] = project_path
    mode['match_making_params']['map_id'] = int(level)
    parameters = mode['parameters']
    parameters['capture_zones'] = units
    parameters['capture_award'] = CAPTURE_AWARD
    parameters['capture_reach'] = CAPTURE_REACH
    parameters['capture_half_height'] = CAPTURE_HALF_HEIGHT
    return config


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--london', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    check = rotation(ROTATION_CHECK[0])
    if any(abs(check[i][j] - ROTATION_CHECK[1][i][j]) > 1e-4 for i in range(3) for j in range(3)):
        sys.exit('rotation(): not the game\'s')
    sums = {}
    for line in (HERE / 'inputs.sha256').read_text().splitlines():
        if line and not line.startswith('#'):
            sha, path = line.split(None, 1)
            sums[path.strip()] = sha
    stock = Folder(args.stock, sums, 'stock')
    donor = Folder(args.donor, sums, 'donor')
    london = Folder(args.london, sums, 'london')
    listing = {line.split('\t')[0] for line in stock.read('stock.list', checked=False).decode().splitlines()
               if '\t' in line}

    files, log, maps, configs = {}, [], [], []
    targets = []
    for level, weather, _ in MAP_IDS:
        ours = WEATHERS[weather]
        if level == '09':
            targets.append(bcw.loads(stock.read(f'projects/level_09/{ours}_4/client_project')))
        else:
            targets.append(bcw.loads(stock.read(f'projects/level_{level}/{ours}_4/client_project')))
    templates = templates_of(targets)
    schema = {}
    for target in targets:
        project_paths(target, schema)
    stock_config = bcw.loads(stock.read('gameplay/game_modes/level_09_day_4.options'))
    referenced = set()
    for (level, weather, map_id), target in zip(MAP_IDS, targets):
        ours = WEATHERS[weather]
        where = f'level_{level}/{ours}_5'
        source, folder = (london, 'level_09') if level == '09' else (donor, f'level_{level}')
        donor5 = bcw.loads(source.read(f'projects/{folder}/{weather}_5/client_project'))
        donor4 = bcw.loads(source.read(f'projects/{folder}/{weather}_4/client_project'))
        project = transplant(target, donor5, donor4, templates, schema, where, log)
        check_types(project, schema, where)
        referenced |= model_refs(project) - model_refs(target)
        project_path = f'level_{level}/{ours}_5'
        files[f'projects/{project_path}/client_project'] = bcw.dump(project)
        units = units_of(donor5, where)
        config_name = f'level_{level}_{weather}_5'
        template = donor.read(f'gameplay/game_modes/level_01_day_5.options' if level == '09'
                              else f'gameplay/game_modes/{config_name}.options')
        config = mode_config(bcw.loads(template), level, weather, map_id, project_path, units, config_name)
        if level == '09':
            # London has no 0.27d config: level_01's with London's names and loading screen
            # (patch 14's level_09_day_4: loading_screen_id, map_name).
            day4 = bcw.loads(stock.read('gameplay/game_modes/level_09_day_4.options'))['mode']
            config['mode']['ui'] = {**config['mode']['ui'], 'loading_screen_id': day4['ui']['loading_screen_id'],
                                    'map_name': day4['ui']['map_name']}
            config['mode']['match_making_params']['map_weather_id'] = day4['match_making_params']['map_weather_id']
            config['mode']['match_making_params']['map_id'] = day4['match_making_params']['map_id']
        check_config_types(config, stock_config, where)
        files[f'gameplay/game_modes/{config_name}.options'] = bcw.dump(config)
        configs.append({'cfg_mode_name': f'{config_name}.options', 'id': map_id})
        maps.append({'id': map_id})
        log.append(f'{where}: units ' + ', '.join(f"{u['cp_id']}: {len(u['shapes'])} shapes" for u in units))
    for model in sorted(referenced):
        prefix = f'models/{model}.model/'
        if any(name.startswith(prefix) for name in listing):
            continue
        if model not in MODELS:
            sys.exit(f'a prop uses {model}, which 0.26g has not and MODELS does not port')
        for path in sorted(p for p in sums if p.startswith('donor/' + prefix)):
            files[path[len('donor/'):]] = donor.read(path[len('donor/'):])
        settings = bcw.loads(files[prefix + 'settings'])
        for surface in settings['material_settings'].values():
            if f"material_instances/{surface['material_name']}.material" not in listing:
                sys.exit(f"{model}: its material {surface['material_name']} is not in 0.26g")
        log.append(f'model {model}: ported from 0.27d (its materials are 0.26g\'s)')

    texts = {key: {} for key in DONOR_TEXTS}
    for language in LANGUAGES:
        strings = load_localization(donor.read(f'localization/{language}/localization'))['strings']
        for key in DONOR_TEXTS:
            if not isinstance(strings.get(key), str):
                sys.exit(f'donor {language}: no {key} in UTF-8')
            texts[key][language] = strings[key]
    header, *table = rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    for row in table:
        texts[row[0]] = dict(zip(LANGUAGES, row[1:]))

    operations = [{'op': 'add_file', 'path': path} for path in files]
    operations.append({'op': 'dictionary_add', 'table': 'maps_dict', 'entries': configs})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {key: porter_text(by[language]) for key, by in texts.items()}})
    operations.append({'op': 'config_append', 'path': 'localization/flash_movies_localizations.options',
                       'array_path': 'hud_strings', 'values': HUD_STRINGS})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'Research (mode 5) as 0.27d shipped it: 25 maps (8 levels x 3 weathers, London day), '
                       'their configs for exe patch 53, the units\' props, texts (research_mode.md)',
        'operations': operations,
        'server': {'maps': maps},
        'notes': ['made by srp-026g/scripts/research-mode/make_pack.py from inputs.sha256 and texts.tsv',
                  'donor: Survarium 0.27d0 build 2500 (levels 1-8), 0.31a2 (London)'] + log,
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print('\n'.join(log))
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
