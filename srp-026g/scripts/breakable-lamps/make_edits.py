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
    for prefix in ('', 'ported/v011e/', 'ported/v021d/')
    for intact, broken in BASE_REPLACEMENTS.items()
}
# Light shafts are ordinary render visuals, independently of object_light.
# Stock, v0.21 and v0.11 use the same converted mesh (CRC d3f2a927),
# v0.11 uses a different model name and full opacity (stock uses 0.05).
# Its bright cone must start at the emitter. The pointed tip fades out:
# aligning that tip left a visible gap in 2026-10-28-21-47-05.png. Align the
# first ring instead (mesh-local y below), lifting these scaled cones by
# 0.143247 m from the apex alignment. Stock/v0.21 remain untouched.
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


def selector(v):
    return {key: v[key] for key in ('visual_name', 'position', 'rotation', 'scale')}


def author(path, data, bounds, break_material):
    root = tree(data)
    visuals = root.get('render_visuals', [])
    components = [(i, v) for i, v in enumerate(visuals) if v.get('visual_name') in REPLACEMENTS]
    if not components:
        return [], [], None
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
    centers = [[(a + b) / 2 for a, b in zip(lo, hi)] for lo, hi in boxes]
    lights, cones = [[] for _ in lamps], [[] for _ in lamps]

    def nearest(position, limit):
        distances = sorted((math.dist(position, p), i) for i, p in enumerate(centers))
        if distances[0][0] > limit:
            return None
        # Ambiguous fixtures need an explicit map edit, never two owners.
        if len(distances) > 1 and distances[1][0] - distances[0][0] < 0.15:
            raise ValueError(f'{path}: ambiguous light/cone at {position}')
        return distances[0][1]

    for light in root.get('client_objects', []):
        if light.get('game_object_type') == 'light':
            near = nearest(light['position'], 0.85)
            if near is not None:
                lights[near].append(light)
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
            edit = {'file': path, 'list': 'render_visuals', 'where': selector(v),
                    'from': {'position': v['position']}, 'to': {'position': position}}
            c.set_entry(result, edit['list'], edit['where'], edit['from'], edit['to'])
            placements.append(edit)
            visuals[i] = dict(v, position=position)

    additions, added = [], set()
    for _, v in components:
        match = selector(v)
        replacement = REPLACEMENTS[v['visual_name']]
        broken_key = json.dumps(dict(match, visual_name=replacement), sort_keys=True)
        if broken_key in added:
            continue
        added.add(broken_key)
        edit = {'files': path, 'list': 'render_visuals', 'after': match, 'like': match,
                'values': {'visual_name': replacement, 'lib_name': replacement}}
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
            broken = dict(v, visual_name=REPLACEMENTS[v['visual_name']])
            vs[f'intact{j}'] = {'index': index(v), 'state': 0}
            broken_index = index(broken)
            if broken_index not in broken_indices:
                vs[f'broken{j}'] = {'index': broken_index, 'state': 1}
                broken_indices.add(broken_index)
        for j, k in enumerate(cones[i]):
            vs[f'cone{j}'] = {'index': index(visuals[k]), 'state': 0}
        ls = {f'light{j}': dict(zip(('x', 'y', 'z'), light['position']), name=light['full_name'])
              for j, light in enumerate(lights[i])}
        lo, hi = boxes[i]
        row = dict(zip(('min_x', 'min_y', 'min_z', 'max_x', 'max_y', 'max_z'), lo + hi))
        rows[f'lamp{i:03d}'] = dict(row, visuals=vs, lights=ls)
    return placements, additions, {'files': [path], 'table': '', 'key': 'srp_lamps',
                       'value': {'version': 2, 'break_material': break_material, 'lamps': rows}}


def check_built(path, data):
    """Problems of a built client_project's srp_lamps against the file itself:
    each visual index names its role's model (intact: a breakable lamp inside
    its row's bounds; broken: that lamp's broken model, placed as it; cone: a
    light shaft), each light position is exactly one light of the map (what
    the client binds; a later edit list or a map-editor save can move them)."""
    root = tree(data)
    if 'srp_lamps' not in root:
        return []
    visuals = root.get('render_visuals', [])
    lights = [o['position'] for o in root.get('client_objects', []) if o.get('game_object_type') == 'light']
    problems = []
    for name, row in root['srp_lamps']['lamps'].items():
        def visual(key):
            index = row['visuals'][key]['index']
            return visuals[index] if 0 <= index < len(visuals) else None
        for key in row['visuals']:
            v, role = visual(key), key.rstrip('0123456789')
            model = v.get('visual_name', '') if v else None
            if v is None:
                ok = False
            elif role == 'intact':
                ok = model in REPLACEMENTS and all(
                    row['min_' + a] - 1e-3 <= p <= row['max_' + a] + 1e-3 for a, p in zip('xyz', v['position']))
            elif role == 'broken':
                intact = visual('intact' + key[len(role):])
                ok = intact is not None and intact.get('visual_name') in REPLACEMENTS and \
                    model == REPLACEMENTS[intact['visual_name']] and \
                    all(v[k] == intact[k] for k in ('position', 'rotation', 'scale'))
            else:
                ok = role == 'cone' and (model == LEGACY_CONE or model.endswith('props/light/cone_light_volume'))
            if not ok:
                problems.append(f'{path}: {name} {key}: render_visuals[{row["visuals"][key]["index"]}] is {model!r}')
        for key, light in row['lights'].items():
            found = sum(p == [light['x'], light['y'], light['z']] for p in lights)
            if found != 1:
                problems.append(f'{path}: {name} {key}: {found} lights of the map at its position')
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
    used = {v.get('visual_name') for p in projects
            for v in tree(p.read_bytes()).get('render_visuals', [])} & REPLACEMENTS.keys()
    bounds = {}
    for name in sorted(used):
        b = (args.stock / f'models/{name}.model/render/export_properties').read_bytes()
        bounds[name] = [c.value_of(b, c.find(b, 'bounding_box/' + key)) for key in ('min', 'max')]
    materials = tree((args.stock / 'game_materials/game.materials').read_bytes())['materials']
    glass = [m['id'] for m in materials if m['name'] == 'glass' and not m.get('deleted')]
    if len(glass) != 1:
        raise ValueError('expected one live glass material in the authoring game data')
    edits = {'_about': 'Breakable lamps: one bullet breaks the fixture, removes its linked lights and cones, '
             'and displays the broken model. Source authored by scripts/breakable-lamps/make_edits.py '
             'from manifest-base projects and Research. Explicit indices refer to the final render_visuals '
             'after these additions. Bulb bounds use stock geometry and placed rotation/scale. '
             'Legacy cones start at the emitter: their full-opacity material exposes offsets '
             'hidden by the 0.26 material. Stock and v0.21 placements stay unchanged.',
             'set_entry': [], 'add_entry': [], 'add_value': []}
    for p in projects:
        placements, adds, spec = author(str(p.relative_to(args.projects)), p.read_bytes(), bounds, glass[0])
        if spec:
            edits['set_entry'].extend(placements)
            edits['add_entry'].extend(adds); edits['add_value'].append(spec)
            rows = spec['value']['lamps']
            print(f'{p.parts[-3]}/{p.parts[-2]}: {len(rows)} lamps, '
                  f'{sum(len(r["lights"]) for r in rows.values())} lights')
    args.out.write_text(json.dumps(edits, indent=2, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
