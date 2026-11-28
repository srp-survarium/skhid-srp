#!/usr/bin/env python3
"""Author the lamp edit list from manifest-base projects and stock model bounds.

Input projects must include the manifest's base patches and Research pack,
never server-game-data.db or a previous release's generated files. Output is
an edit-list source, not a resources.db patch. Its explicit bindings can be
reviewed/retuned per map without changing client code.
"""
import argparse
import importlib.util
import itertools
import json
import math
from pathlib import Path

EDIT = Path(__file__).resolve().parents[1] / 'content-edit/content_edit.py'
spec = importlib.util.spec_from_file_location('content_edit', EDIT)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
spec = importlib.util.spec_from_file_location('probe_light', Path(__file__).with_name('probe_light.py'))
probe_light = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe_light)

# The prefixes a fixture model has: stock, and the ports' own copies of the models (a port imports a
# model under its namespace where its materials or settings differ from players'; the London ports'
# lamps are such copies, the same meshes, and the 0.69d ports' (v069d_<map>/)).
PORT_PREFIXES = ('', 'ported/v011e/', 'ported/v021d/', 'ported/v032a/', 'ported/v044a/', 'v069d_rudnya/', 'v069d_vostok/')
# The London ports' fixtures, beside the stock ones the older ports never had.
LONDON_PREFIXES = ('ported/v032a/', 'ported/v044a/')
BASE_REPLACEMENTS = {
    'props/light/lamp_on': 'props/light/srp_lamp_broken',
    'props/light/plafon_lamp_on': 'props/light/srp_plafon_lamp_broken',
    'props/light/alarm_red_light': 'props/light/alarm_broken',
    'props/light/alarm_red': 'props/light/alarm_broken',
    'props/light/white_lamp': 'props/light/white_broken',
    'props/light/green_lamp': 'props/light/green_lamp_empty',
    'props/light/green_lamp_with_wire': 'props/light/green_lamp_with_wire_not_lamp',
    'props/light/fluor_lamp': 'props/light/fluor_lamp_base',
    'props/light/plafon_lamp': 'props/light/srp_plafon_lamp_broken',
    'props/light/lamp': 'props/light/srp_lamp_broken',
}
REPLACEMENTS = {
    prefix + intact: broken if '/srp_' in broken else prefix + broken
    for prefix in PORT_PREFIXES
    for intact, broken in BASE_REPLACEMENTS.items()
} | {
    # Fluorescent fixtures with their tubes in the model, stock only (v0.11's
    # lamp_midle models are other meshes): the housing stays, the tubes go.
    'props/light/lamp_midle_01': 'props/light/srp_lamp_midle_01_broken',
    'props/light/lamp_midle_02': 'props/light/srp_lamp_midle_02_broken',
    'props/light/lamp_midle_03': 'props/light/srp_lamp_midle_03_broken',
    'props/light/lamp_big_01': 'props/light/srp_lamp_big_01_broken',
}
# Lit tubes that are models of their own (Vector, level_09: fx/lamp2 and its
# two flicker variants, three tubes in a tubeless lamp_midle_04 housing).
# A broken fixture hides them; nothing replaces them.
TUBE_GLASS = {prefix + 'fx/' + tube for prefix in ('',) + LONDON_PREFIXES for tube in ('lamp2', 'lamp2_bad', 'lamp2_good')}
# Kerosene lamps (London, Tarakanovsky Fort, level_04, and the 0.21 ports'
# copies of the models: Fort and Chemical Plant v0.21): a frame that stays,
# standing, hung on a rope or nailed to a wall; its lit glass, a model of its
# own; and its flame, a particle object in the glass (FLAME), which lights
# the place too: there is no light object. A break hides the glass and puts
# the flame out. A frame without the lit glass is an unlit lamp: scenery.
# The chain lamp (ceiling_group_chain_lamp) has its glass in its own mesh,
# the oil lamp (stand_group_oil_lamp) is never lit, a torch has no glass:
# scenery all three.
LANTERN_PREFIXES = PORT_PREFIXES
LANTERN_GLASS = {prefix + 'props/light/' + model + '_glass_emit' for prefix in LANTERN_PREFIXES
                 for model in ('stand_group_batlamp', 'wall_group_nailed_batlamp')}
FLAME, FLAME_REACH = 'fx/lights/oil_lamp', 0.4
GLASS = TUBE_GLASS | LANTERN_GLASS
# Housings that stay when their glass breaks. A housing belongs to a fixture
# (its bounds, so a shot into the housing breaks the glass) only together
# with glass; on its own it is scenery.
HOUSINGS = {prefix + 'props/light/lamp_midle_04' for prefix in ('',) + LONDON_PREFIXES} | {glass.removesuffix('_glass_emit') for glass in LANTERN_GLASS}
PARTS = REPLACEMENTS.keys() | GLASS | HOUSINGS
# Glass that is no lamp: School's vodka bottles, each a fixture of its own
# with its one intact visual (make_bottles.py), outside the lamp rule above,
# which would merge bottles lying side by side into one fixture.
BOTTLES = {prefix + 'props/kitchen/bottle_vodka' for prefix in PORT_PREFIXES}
# A broken bottle's shards (scripts/bottle-shards, pack bottle-shards@1): the
# loose pieces of its row, kind 'shard', placed as the bottle but at each
# one's centre. School's bottles (stock and the v0.21 port) have them.
SHARDS = 8
# The most loose pieces a project's table may have: the client's MAX_PIECES
# and its registry's MAX_LOOSE (lamps/model.rs, vostok/project.rs), past
# which it refuses the map at its load; and the most objects it binds
# (MAX_BINDINGS: each visual, loose piece, light, ambient light, particle and
# probe of every row).
MAX_PIECES = 384
MAX_BINDINGS = 1024
BOTTLE_SHARDS = {prefix + 'props/kitchen/bottle_vodka':
                 [prefix + f'props/kitchen/srp_bottle_vodka_shard{k}' for k in range(SHARDS)]
                 for prefix in ('', 'ported/v021d/')}
# Figures with a loose piece (scripts/skeleton-cap): the sitting skeleton and
# its cap. A figure's row is its piece's (its bounds, what a shot or a blast
# must reach); its broken model is the figure without the piece, placed as
# it; the piece (the row's `loose`) is a model of its own, placed as the
# figure but at the piece's centre, inside the row's bounds. Not in PARTS:
# the lamp rule never makes a fixture of them. A table of figures says
# leftover 'body' (the client marks the figure's collision once broken),
# one of lamps leaves it out (their glass's stays: never marked).
FIGURE_FOLDERS = ('character/human/static/remains/', 'ported/v021d/character/human/static/remains/',
                  'v069d_rudnya/character/human/static/remains/', 'ported/v011e/character/human/remains/',
                  'ported/v032a/character/human/static/remains/', 'ported/v044a/character/human/static/remains/')
FIGURES = {folder + 'skeleton_sitting1': (folder + 'srp_skeleton_sitting1_bare', folder + 'srp_skeleton_sitting1_cap')
           for folder in FIGURE_FOLDERS}


def _pack(name, path):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).resolve().parents[1] / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Later lists' fixtures (breakable-lamps/fixtures.py): the kitchen things a
# shot throws whole (kitchen-ware-1: the loose copy, kind 'ware') or breaks
# (its shards, kind 'shard'), Vector's capsules (lab-capsule-glass-1: the
# frame without its glass while broken, the glass's shards, kind
# 'pane_shard'), Vector's jars (lab-jars-1: each jar of a rack drawn alone,
# its stump while broken, its shards, kind 'pane_shard'), Vector's windows (vector-windows-1: each
# bay of its building's glass drawn alone, what its frame keeps while broken,
# no pieces) and School's bells (school-bells-1: rows of reaction 'ring',
# nothing drawn).
_ware = _pack('kitchen_ware_pack', 'kitchen-ware/make_pack.py')
_capsule = _pack('lab_capsule_pack', 'lab-capsule/make_pack.py')
_jars = _pack('lab_jars_pack', 'lab-jars/make_pack.py')
_windows = _pack('vector_windows_pack', 'vector-windows/make_pack.py')
WARE_LOOSE = {name: _ware.loose_name(name) for name in _ware.WARE}
CERAMIC_SHARDS = {name: _ware.shard_names(name) for name in _ware.CERAMIC}
# a figure's scale its pieces are cut at (they are placed unscaled)
PIECE_SCALE = dict(_ware.SCALE)
CAPSULES = {g.model: (g.broken, g.shard_names) for g in _capsule.GLASSES}
JARS = dict(zip(_jars.jar_names(), _jars.broken_names()))
JAR_SHARDS = {jar: _jars.shard_names(k) for k, jar in enumerate(_jars.jar_names())}
# the bays a building's glass has (Vector's 23), and more
PANES = {_windows.pane_name(k): _windows.broken_name(k) for k in range(64)}
BELLS = ('props/electric/bell_school', 'ported/v021d/props/electric/bell_school')
# A bell's collision object: a port that merged the bell's visual into a cluster keeps it at the bell
# (School (v0.69): three of its four; scripts/school-bells/make_edits.py bells_of).
BELL_SOLIDS = tuple(f'resources/models/{name}.model/collision' for name in BELLS)
# Fluorescent glass: a model's tubes are its glass, not its housing or frame.
# The local height of the tubes' axis in the models that hold them (the
# triangles make_pack.py drops: lamp_midle_0N y -0.099..-0.055, lamp_big_01
# -0.844..-0.800, under 0.88 m of rods); fx/lamp2* are tubes themselves.
TUBE_Y = {'lamp_midle_01': -0.077, 'lamp_midle_02': -0.077, 'lamp_midle_03': -0.077, 'lamp_big_01': -0.822}
TUBES = TUBE_Y.keys() | {'lamp2', 'lamp2_bad', 'lamp2_good'}
# Light shafts are ordinary render visuals, independently of object_light.
# Stock, v0.21 and v0.11 use the same converted mesh (CRC d3f2a927),
# v0.11 uses a different model name and full opacity (stock uses 0.05).
# Its bright cone must start at the emitter. The pointed tip fades out:
# aligning that tip left a visible gap in 2026-10-28-21-47-05.png. Align the
# first ring instead (mesh-local y below), lifting these scaled cones by
# 0.143247 m from the apex alignment. Stock/v0.21 remain untouched.
# A lit alarm lamp has no light object: its red glow is an ambient light
# beside it (Vector's 0.6 m from the lamp, level_05's up to 2.4 m, two at one
# lamp), among the rooms' grey and coloured fills. A red one that near is the
# lamp's, and goes out with it.
LIT_ALARMS = {prefix + 'props/light/alarm_red_light' for prefix in PORT_PREFIXES}
ALARM_GLOW_REACH = 2.5
LEGACY_CONE = 'ported/v011e/editor/cone'
CONE_APEX = (0.0, -0.009467163123190403, 0.0)
CONE_LIGHT_START = (0.0, -0.1327432245016098, 0.0)


def tree(buf, off=0):
    kind = c.rec(buf, off)[3]
    if kind == c.T_NAMED:
        return {c.name_at(buf, c.rec(buf, k)[1]).decode(): tree(buf, k) for k in c.children(buf, off)}
    if kind == c.T_INDEXED:
        return [tree(buf, k) for k in c.children(buf, off)]
    return c.value_of(buf, off)


def world_bounds(visual, low, high):
    # mapdata::Rotation::from_angles: engine row-vector Rx * Ry * Rz.
    x, y, z = visual['rotation']
    xs, xc, ys, yc, zs, zc = math.sin(x), math.cos(x), math.sin(y), math.cos(y), math.sin(z), math.cos(z)
    axes = ((yc * zc, -yc * zs, ys),
            (ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            (-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc))
    points = [[sum(p[j] * visual['scale'][j] * axes[j][k] for j in range(3)) + visual['position'][k]
               for k in range(3)] for p in itertools.product(*zip(low, high))]
    return ([min(p[k] for p in points) for k in range(3)],
            [max(p[k] for p in points) for k in range(3)])


def glass_line(group, bounds):
    """A fixture's glass in world space, where its break plays: the longest
    local axis of its longest glass part, through that part's centre and
    transformed with its placement (parallel parts of that length, a
    housing's tubes, averaged); a tube model's at its tubes' height."""
    parts = [v for _, v in group if v['visual_name'] not in HOUSINGS]
    tubes = [v for v in parts if v['visual_name'].rsplit('/', 1)[-1] in TUBES]
    lines = []
    for v in tubes or parts:
        low, high = bounds[v['visual_name']]
        axis = max(range(3), key=lambda j: (high[j] - low[j]) * abs(v['scale'][j]))
        ends = []
        for end in (low[axis], high[axis]):
            point = [(a + b) / 2 for a, b in zip(low, high)]
            point[1] = TUBE_Y.get(v['visual_name'].rsplit('/', 1)[-1], point[1])
            point[axis] = end
            ends.append(world_bounds(v, point, point)[0])
        lines.append(ends)
    a, b = max(lines, key=lambda line: math.dist(*line))
    length, d = math.dist(a, b), [q - p for p, q in zip(a, b)]
    same = []
    for p, q in lines:
        dot = sum(x * (y1 - y0) for x, y0, y1 in zip(d, p, q))
        if abs(math.dist(p, q) - length) < 0.01 and abs(dot) > 0.95 * length * math.dist(p, q):
            same.append((p, q) if dot > 0 else (q, p))
    return tuple([sum(line[i][k] for line in same) / len(same) for k in range(3)] for i in (0, 1))


def named(v):
    """A render visual as the rules read it: by `visual_name`. A 0.69d port's entries name their
    model by `lib_name` alone (the game draws it); they are read by it and selected by it."""
    return v if 'visual_name' in v else dict(v, visual_name=v.get('lib_name'), by_lib_name=True)


def name_key(v):
    return 'lib_name' if v.get('by_lib_name') else 'visual_name'


def as_model(v, model):
    """`v` naming `model` the way its entry names one."""
    return dict(v, visual_name=model, lib_name=model) if v.get('by_lib_name') else dict(v, visual_name=model)


def selector(v):
    return {key: v[key] for key in (name_key(v), 'position', 'rotation', 'scale')}


def is_red(colour):
    """An ambient light's colour (an i32: red in its low byte, then green, blue)."""
    red, green, blue = (colour >> shift & 0xff for shift in (0, 8, 16))
    return red >= 2 * max(green, blue)


def nearest_fixture(path, position, centers, limit, among=None):
    among = range(len(centers)) if among is None else among
    distances = sorted((math.dist(position, centers[i]), i) for i in among)
    if not distances or distances[0][0] > limit:
        return None
    if len(distances) > 1 and distances[1][0] - distances[0][0] < 0.15:
        raise ValueError(f'{path}: ambiguous light/cone at {position}')
    return distances[0][1]


def rebound_lights(path, lamps, objects):
    """Bind a replacement lighting set to existing fixtures without moving them."""
    names = sorted(lamps)
    centers = [[(lamps[name]['min_' + axis] + lamps[name]['max_' + axis]) / 2
                for axis in 'xyz'] for name in names]
    bindings = {name: {} for name in names}
    for light in objects:
        if light.get('game_object_type') != 'light':
            continue
        near = nearest_fixture(path, light['position'], centers, 0.85)
        if near is not None:
            row = bindings[names[near]]
            row[f'light{len(row)}'] = dict(zip(('x', 'y', 'z'), light['position']), name=light['full_name'])
    return bindings


def close(a, b):
    """Two lamp tables alike: the same keys and values, numbers within the f32 rounding of a file."""
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(close(a[k], b[k]) for k in a)
    if isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool):
        return abs(a - b) <= 1e-4 * max(1, abs(a))
    return a == b


def author(path, data, bounds, break_material, effect_material, cubemap=None, days=None):
    """A project's lamp edits. A project a pack made from a lamp-carrying day (London's and the
    0.21 maps' rain and evening) has the day's broken models and table already: its fixtures are
    authored over them, and the table is replaced only where this rule now binds otherwise.
    `cubemap(probe)` is a probe's irradiance cube map (probe_light.py); `days` gathers each
    level's day split for its other weathers (main authors a level's day projects first)."""
    root = tree(data)
    carried = root.get('srp_lamps')
    visuals = [named(v) for v in root.get('render_visuals', [])]
    components = [(i, v) for i, v in enumerate(visuals) if v.get('visual_name') in PARTS]
    # Overlapping bulb/cover meshes form one fixture (level_06). A shot
    # through its cover must extinguish the bulb inside it as well.
    lamps, boxes = [], []
    for component in components:
        box = world_bounds(component[1], *bounds[component[1]['visual_name']])
        touching = [i for i, other in enumerate(boxes)
                    if all(box[0][k] <= other[1][k] and other[0][k] <= box[1][k] for k in range(3))]
        group = [component]
        for i in reversed(touching):
            group += lamps.pop(i)
            other = boxes.pop(i)
            box = ([min(a, b) for a, b in zip(box[0], other[0])],
                   [max(a, b) for a, b in zip(box[1], other[1])])
        lamps.append(group); boxes.append(box)
    fixtures = [(group, box) for group, box in zip(lamps, boxes)
                if any(v['visual_name'] not in HOUSINGS for _, v in group)]
    if not fixtures:
        return [], [], None
    lamps, boxes = map(list, zip(*fixtures))
    centers = [[(a + b) / 2 for a, b in zip(lo, hi)] for lo, hi in boxes]
    lights, cones = [[] for _ in lamps], [[] for _ in lamps]

    def nearest(position, limit, among=None):
        return nearest_fixture(path, position, centers, limit, among)

    alarms = [i for i, group in enumerate(lamps) if any(v['visual_name'] in LIT_ALARMS for _, v in group)]
    lanterns = [i for i, group in enumerate(lamps) if any(v['visual_name'] in LANTERN_GLASS for _, v in group)]
    glows, flames = [[] for _ in lamps], [[] for _ in lamps]
    for light in root.get('client_objects', []):
        if light.get('game_object_type') == 'light':
            near = nearest(light['position'], 0.85)
            if near is not None:
                lights[near].append(light)
        elif light.get('game_object_type') == 'ambient_light' and is_red(light['color']):
            near = nearest(light['position'], ALARM_GLOW_REACH, alarms)
            if near is not None:
                glows[near].append(light)
        elif light.get('game_object_type') == 'particle' and light.get('lib_name') == FLAME:
            near = nearest(light['position'], FLAME_REACH, lanterns)
            if near is not None:
                flames[near].append(light)
    # The room probes a lamp's light is baked into, and what each keeps
    # once the lamp is broken (probe_light.py); a weather's split falls back
    # on its level's day.
    days = {} if days is None else days
    level, weather = Path(path).parts[-3], Path(path).parts[-2].rsplit('_', 1)[0]
    probes, lamp_light = probe_light.lamp_probes(
        [o for o in root.get('client_objects', []) if o.get('game_object_type') == 'environment_probe'],
        [(i, light) for i, own in enumerate(lights) for light in own], cubemap,
        None if weather == 'day' else days.get(level))
    if weather == 'day':
        days.setdefault(level, lamp_light)
    # The fill lights (ambient lights) inside a lamp's room probe are its
    # room's baked light too: they go out with it (the nearest lamp's).
    for light in root.get('client_objects', []):
        if light.get('game_object_type') != 'ambient_light' or any(light in g for g in glows):
            continue
        rooms = [i for i, own in probes.items() if any(probe_light.holds(p, light['position']) for p, _ in own)]
        if rooms:
            glows[min(rooms, key=lambda i: math.dist(centers[i], light['position']))].append(light)
    placements, result = [], bytearray(data)
    for i, v in enumerate(visuals):
        legacy = v.get('visual_name') == LEGACY_CONE
        if legacy or v.get('visual_name', '').endswith('props/light/cone_light_volume'):
            near = nearest(v['position'], 0.65)
            if near is not None:
                cones[near].append(i)
            if not legacy:
                continue
            if near is not None:
                # Hanging fixtures emit at the bulb's bottom, not the
                # wire/socket pivot or the whole model's centre.
                emitters = [component for _, component in lamps[near]
                            if component['visual_name'].rsplit('/', 1)[-1]
                            in ('green_lamp', 'green_lamp_with_wire', 'lamp_on', 'plafon_lamp_on',
                                'lamp', 'plafon_lamp')]
                lit = [v for v in emitters if v['visual_name'].endswith('_on')]
                emitters = lit or emitters
                if len(emitters) != 1:
                    raise ValueError(f'{path}: cone needs one hanging lamp emitter')
                fixture = emitters[0]
                local_tip = [0, bounds[fixture['visual_name']][0][1], 0]
                tip = world_bounds(fixture, local_tip, local_tip)[0]
            else:
                # The two outdoor shafts belong to streetlights, whose
                # fixtures are not breakable yet. Align to their authored
                # light emitters without binding them to a distant lamp.
                emitters = [light for light in root.get('client_objects', [])
                            if light.get('game_object_type') == 'light'
                            and math.dist(light['position'], v['position']) < 0.65]
                if not emitters:
                    continue
                if len(emitters) != 1:
                    raise ValueError(f'{path}: ambiguous standalone cone emitter')
                tip = emitters[0]['position']
            start = world_bounds(v, CONE_LIGHT_START, CONE_LIGHT_START)[0]
            position = [p + t - a for p, t, a in zip(v['position'], tip, start)]
            if carried is not None and close(dict(enumerate(position)), dict(enumerate(v['position']))):
                continue
            edit = {'file': path, 'list': 'render_visuals', 'where': selector(v),
                    'from': {'position': v['position']}, 'to': {'position': position}}
            c.set_entry(result, edit['list'], edit['where'], edit['from'], edit['to'])
            placements.append(edit)
            visuals[i] = dict(v, position=position)

    additions, added = [], set()
    for _, v in components:
        if v['visual_name'] not in REPLACEMENTS:
            continue
        match = selector(v)
        replacement = REPLACEMENTS[v['visual_name']]
        broken = selector(as_model(v, replacement))
        broken_key = json.dumps(broken, sort_keys=True)
        if broken_key in added:
            continue
        added.add(broken_key)
        if carried is not None and any(all(k in f and c.same(f[k], val) for k, val in
                                           broken.items()) for f in visuals):
            continue
        edit = {'files': path, 'list': 'render_visuals', 'after': match, 'like': match,
                'values': {'lib_name': replacement} if v.get('by_lib_name') else
                          {'visual_name': replacement, 'lib_name': replacement}}
        assert c.add_entry(result, edit['list'], match, match, edit['values']) is not None
        additions.append(edit)
    final_visuals = tree(result)['render_visuals']

    def index(v):
        matches = [i for i, f in enumerate(final_visuals)
                   if all(k in f and c.same(f[k], val) for k, val in selector(v).items())]
        if len(matches) != 1:
            raise ValueError(f'{path}: duplicate visual {selector(v)}')
        return matches[0]

    rows = {}
    for i, group in enumerate(lamps):
        vs, broken_indices = {}, set()
        for j, (_, v) in enumerate(group):
            if v['visual_name'] in HOUSINGS:
                continue
            vs[f'intact{j}'] = {'index': index(v), 'state': 0}
            if v['visual_name'] in GLASS:
                continue
            broken_index = index(as_model(v, REPLACEMENTS[v['visual_name']]))
            if broken_index not in broken_indices:
                vs[f'broken{j}'] = {'index': broken_index, 'state': 1}
                broken_indices.add(broken_index)
        for j, k in enumerate(cones[i]):
            vs[f'cone{j}'] = {'index': index(visuals[k]), 'state': 0}
        ls = {f'light{j}': dict(zip(('x', 'y', 'z'), light['position']), name=light['full_name'])
              for j, light in enumerate(lights[i])}
        lo, hi = boxes[i]
        row = dict(zip(('min_x', 'min_y', 'min_z', 'max_x', 'max_y', 'max_z'), lo + hi))
        start, end = glass_line(group, bounds)
        row |= dict(zip(('glass_from_x', 'glass_from_y', 'glass_from_z', 'glass_to_x', 'glass_to_y', 'glass_to_z'),
                        start + end))
        rows[f'lamp{i:03d}'] = dict(row, visuals=vs, lights=ls)
        # The few fixtures with one; the others' rows have no table.
        for table, key, objects in (('ambient_lights', 'light', glows[i]), ('particles', 'particle', flames[i])):
            if objects:
                rows[f'lamp{i:03d}'][table] = {
                    f'{key}{j}': dict(zip(('x', 'y', 'z'), o['position']), name=o['full_name'])
                    for j, o in enumerate(objects)}
        if probes.get(i):
            rows[f'lamp{i:03d}']['probes'] = {
                f'probe{j}': dict(zip(('x', 'y', 'z'), probe['position']), name=probe['full_name'], keeps=keeps)
                for j, (probe, keeps) in enumerate(probes[i])}
    value = {'version': 3, 'break_material': break_material, 'effect_material': effect_material, 'lamps': rows}
    if carried is not None and close(carried, value):
        return [], [], None
    return placements, additions, {'files': [path], 'table': '', 'key': 'srp_lamps', 'value': value,
                                   'replaces': carried is not None}


def model_name(v):
    """A render visual's model: its visual_name, or (the v0.69 ports' maps
    have none) its lib_name."""
    return v.get('visual_name', v.get('lib_name', ''))


def check_built(path, data):
    """Problems of a built client_project's srp_lamps against the file itself:
    each visual index names its role's model (intact: a breakable lamp inside
    its row's bounds, or a figure; broken: that lamp's broken model, or the
    figure's bare one, placed as it; cone: a light shaft; loose: the figure's
    piece or a bottle's shard, of its kind, turned as it, unscaled and inside
    the row's bounds; at most MAX_PIECES in all), the table's leftover
    'body' exactly when its fixtures are figures, each light position is exactly one light of the map, each
    ambient light position exactly one ambient light and each particle
    position exactly one particle object, each probe position exactly one
    environment probe (what the client binds; a later edit list or a
    map-editor save can move them)."""
    root = tree(data)
    if 'srp_lamps' not in root:
        return []
    visuals = [named(v) for v in root.get('render_visuals', [])]
    placed = {kind: [o['position'] for o in root.get('client_objects', []) if o.get('game_object_type') == kind]
              for kind in ('light', 'ambient_light', 'particle', 'environment_probe')}
    problems = []
    broken_of = REPLACEMENTS | {figure: models[0] for figure, models in FIGURES.items()} \
        | {capsule: models[0] for capsule, models in CAPSULES.items()} | JARS | PANES
    table_leftover = root['srp_lamps'].get('leftover', 'glass')
    bindings = 0
    for name, row in root['srp_lamps']['lamps'].items():
        bindings += sum(len(row.get(t, {})) for t in ('visuals', 'loose', 'lights', 'ambient_lights', 'particles',
                                                       'probes'))
        if row.get('reaction', 'break') == 'ring':
            problems += ring_problems(path, name, row, visuals, root.get('collision_objects', []))
            continue

        def visual(key, table='visuals'):
            index = row[table][key]['index']
            return visuals[index] if 0 <= index < len(visuals) else None

        def inside(v):
            return all(row['min_' + a] - 1e-3 <= p <= row['max_' + a] + 1e-3 for a, p in zip('xyz', v['position']))
        for key in row['visuals']:
            v, role = visual(key), key.rstrip('0123456789')
            model = model_name(v) if v else None
            if v is None:
                ok = False
            elif role == 'intact':
                ok = model in FIGURES or model in CAPSULES or (
                    (model in REPLACEMENTS or model in GLASS or model in BOTTLES or model in WARE_LOOSE
                     or model in CERAMIC_SHARDS or model in JARS or model in PANES) and inside(v))
            elif role == 'broken':
                intact = visual('intact' + key[len(role):])
                ok = intact is not None and model_name(intact) in broken_of and \
                    model == broken_of[model_name(intact)] and \
                    all(v[k] == intact[k] for k in ('position', 'rotation', 'scale'))
            else:
                ok = role == 'cone' and (model == LEGACY_CONE or model.endswith('props/light/cone_light_volume'))
            if not ok:
                problems.append(f'{path}: {name} {key}: render_visuals[{row["visuals"][key]["index"]}] is {model!r}')
        pieces_of = {figure: ([models[1]], 'headwear') for figure, models in FIGURES.items()} \
            | {bottle: (shards, 'shard') for bottle, shards in BOTTLE_SHARDS.items()} \
            | {ware: ([loose], 'ware') for ware, loose in WARE_LOOSE.items()} \
            | {jug: (shards, 'shard') for jug, shards in CERAMIC_SHARDS.items()} \
            | {capsule: (models[1], 'pane_shard') for capsule, models in CAPSULES.items()} \
            | {jar: (shards, 'pane_shard') for jar, shards in JAR_SHARDS.items()}
        figures = [visual(key) for key in row['visuals'] if key.startswith('intact')]
        figures = [v for v in figures if v is not None and model_name(v) in pieces_of]
        for key, piece in row.get('loose', {}).items():
            v = visual(key, 'loose')
            ok = v is not None and len(figures) == 1 and model_name(v) in pieces_of[model_name(figures[0])][0] \
                and piece.get('kind', 'headwear') == pieces_of[model_name(figures[0])][1] \
                and v['rotation'] == figures[0]['rotation'] and inside(v) \
                and all(abs(s - PIECE_SCALE.get(model_name(figures[0]), 1.0)) < 1e-3 for s in figures[0]['scale']) \
                and all(abs(s - 1) < 0.01 for s in v['scale']) \
                and (piece.get('kind') != 'ware' or ware_shape_ok(piece)) \
                and (piece.get('kind') != 'pane_shard' or pane_frame_ok(piece))     # the client's seat is rigid (2 %)
            if not ok:
                model = model_name(v) if v else None
                problems.append(f'{path}: {name} loose {key}: render_visuals[{row["loose"][key]["index"]}] is {model!r}')
        for end in ('from', 'to'):
            if not all(row['min_' + a] - 1e-3 <= row[f'glass_{end}_{a}'] <= row['max_' + a] + 1e-3 for a in 'xyz'):
                problems.append(f'{path}: {name}: its glass line leaves its bounds')
        for table, kind, what in (('lights', 'light', 'lights'), ('ambient_lights', 'ambient_light', 'ambient lights'),
                                  ('particles', 'particle', 'particles'),
                                  ('probes', 'environment_probe', 'environment probes')):
            for key, light in row.get(table, {}).items():
                found = sum(p == [light['x'], light['y'], light['z']] for p in placed[kind])
                if found != 1:
                    problems.append(f'{path}: {name} {key}: {found} {what} of the map at its position')
        # a figure's row marks its body once broken, every other its glass never
        figure = any(model_name(visuals[v['index']]) in FIGURES for key, v in row['visuals'].items()
                     if key.startswith('intact') and 0 <= v['index'] < len(visuals))
        leftover = row.get('leftover', table_leftover)
        if leftover != ('body' if figure else 'glass'):
            problems.append(f'{path}: {name}: leftover {leftover!r} for a {"figure" if figure else "glass"} fixture')
    pieces = sum(len(row.get('loose', {})) for row in root['srp_lamps']['lamps'].values())
    if pieces > MAX_PIECES:
        problems.append(f'{path}: {pieces} loose pieces, more than the client binds ({MAX_PIECES})')
    if bindings > MAX_BINDINGS:
        problems.append(f'{path}: {bindings} bound objects, more than the client binds ({MAX_BINDINGS})')
    if all(row.get('reaction', 'break') == 'ring' for row in root['srp_lamps']['lamps'].values()):
        problems.append(f'{path}: a lamp table of bells alone (the client keeps a history of breaking rows)')
    return problems


def ware_shape_ok(piece):
    """A ware's capsule as the client parses it (WareShape::new, WareAxis)."""
    half, radius = piece.get('half'), piece.get('radius')
    return isinstance(half, (int, float)) and isinstance(radius, (int, float)) and 0 <= half <= 0.5 \
        and 0 < radius <= 0.3 and piece.get('axis') in ('x', 'y', 'z')


def pane_frame_ok(piece):
    """A pane shard's frame as the client parses it (lamps::model::pane_frame):
    its long side and its level outward normal, unit and across each other."""
    out = [piece.get(f'out_{a}') for a in 'xyz']
    along = [piece.get(f'along_{a}') for a in 'xyz']
    if not all(isinstance(v, (int, float)) for v in out + along):
        return False
    dot = sum(a * b for a, b in zip(out, along))
    return abs(math.hypot(*out) - 1) < 0.01 and abs(math.hypot(*along) - 1) < 0.01 and abs(dot) < 0.95 \
        and abs(out[1]) < 1e-6


def ring_problems(path, name, row, visuals, solids=()):
    """A bell's row: no visual, no piece, no light of its own; its interval
    the client's (over 0, to 600 s); a bell of the map inside its bounds (its
    render visual, or its collision object where a port merged the visual into
    a cluster); its glass line (where the ring plays) inside them too."""
    problems = []
    inside = lambda p: all(row['min_' + a] - 1e-3 <= c <= row['max_' + a] + 1e-3 for a, c in zip('xyz', p))
    every = row.get('ring_every')
    if not isinstance(every, (int, float)) or not 0 < every <= 600:
        problems.append(f'{path}: {name}: ring_every {every!r}')
    if any(row.get(t) for t in ('visuals', 'loose', 'lights', 'ambient_lights', 'particles', 'probes')):
        problems.append(f'{path}: {name}: a bell binds nothing')
    if not any(model_name(v) in BELLS and inside(v['position']) for v in visuals) \
            and not any(o.get('lib_name') in BELL_SOLIDS and inside(o['position']) for o in solids):
        problems.append(f'{path}: {name}: no bell of the map in its bounds')
    for end in ('from', 'to'):
        if not inside([row[f'glass_{end}_{a}'] for a in 'xyz']):
            problems.append(f'{path}: {name}: its glass line leaves its bounds')
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--projects', type=Path, required=True)
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    projects = []
    for p in sorted((args.projects / 'projects').glob('*/*/client_project')):
        if p.parts[-3] != 'lobby_scene' and p.parts[-2].endswith(('_2', '_3', '_4', '_5')):
            projects.append(p)
    used = {named(v).get('visual_name') for p in projects
            for v in tree(p.read_bytes()).get('render_visuals', [])} & PARTS
    bounds = {}
    for name in sorted(used):
        b = (args.stock / f'models/{name}.model/render/export_properties').read_bytes()
        bounds[name] = [c.value_of(b, c.find(b, 'bounding_box/' + key)) for key in ('min', 'max')]
    materials = tree((args.stock / 'game_materials/game.materials').read_bytes())['materials']
    # The glass every fixture's collision is, and the material whose pair
    # with it plays a break's shards and sound (bullet x glass: the game's
    # own fx/material_hits/glass), whatever broke the lamp: a bullet or a blast.
    live = {name: [m['id'] for m in materials if m['name'] == name and not m.get('deleted')]
            for name in ('glass', 'bullet')}
    if any(len(ids) != 1 for ids in live.values()):
        raise ValueError('expected one live glass and one live bullet material in the authoring game data')
    def cubemap(probe):
        for tree in (args.stock, args.projects):
            path = tree / 'textures' / f"{probe['texture']}_diffuse.dds"
            if path.is_file():
                return path
        raise FileNotFoundError(f"{probe['full_name']}: no {probe['texture']}_diffuse.dds in --stock or --projects")
    edits = {'_about': 'Breakable lamps: one bullet breaks the fixture, removes its linked lights and cones '
             "(a lit alarm lamp's red ambient glow and a kerosene lamp's flame too; the room probe a tube lamp lights "
             'keeps only its share without the lamp, probe_light.py), '
             'and displays the broken model. Source authored by scripts/breakable-lamps/make_edits.py '
             'from manifest-base projects and Research. Explicit indices refer to the final render_visuals '
             'after these additions. Bulb bounds use stock geometry and placed rotation/scale. '
             'Legacy cones start at the emitter: their full-opacity material exposes offsets '
             'hidden by the 0.26 material. Stock and v0.21 placements stay unchanged.',
             'set_entry': [], 'add_entry': [], 'add_value': []}
    days = {}
    # A level's day projects first: its other weathers' probes use the day's split.
    for p in sorted(projects, key=lambda p: (p.parts[-3], not p.parts[-2].startswith('day_'), p.parts[-2])):
        placements, adds, spec = author(str(p.relative_to(args.projects)), p.read_bytes(), bounds,
                                        live['glass'][0], live['bullet'][0], cubemap, days)
        if spec:
            if spec.pop('replaces'):
                edits.setdefault('remove_key', []).append({'files': spec['files'][0], 'keys': ['srp_lamps']})
            edits['set_entry'].extend(placements)
            edits['add_entry'].extend(adds); edits['add_value'].append(spec)
            rows = spec['value']['lamps']
            print(f'{p.parts[-3]}/{p.parts[-2]}: {len(rows)} lamps, '
                  f'{sum(len(r["lights"]) for r in rows.values())} lights, '
                  f'{sum(len(r.get("ambient_lights", {})) for r in rows.values())} ambient lights, '
                  f'{sum(len(r.get("particles", {})) for r in rows.values())} flames, '
                  f'{sum(len(r.get("probes", {})) for r in rows.values())} probes')
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
