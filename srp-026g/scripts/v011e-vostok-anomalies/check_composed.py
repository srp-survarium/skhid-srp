#!/usr/bin/env python3
"""Verify the finished candidate against released maps and the current Radar repairs.

The reference/composed folders are srp-game-data get outputs from all released
archives, and those same archives plus the candidate, respectively. No patch
is built here. See README.md for the paths to extract.
"""
import argparse
import copy
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as pack


def replace_child(node, name, value):
    return node[0], [(key, value if key == name.encode() else child) for key, child in node[1]], node[2]


def change_path(node, path, change):
    if not path:
        return change(node)
    key, *rest = path
    return replace_child(node, key, change_path(pack.child(node, key), rest, change))



def without_ffa_spawns(root, project):
    """Verify the explicit solo additions, then compare the original map unchanged."""
    if not project.endswith('/day_4'):
        return root
    path = f'projects/{project}/client_project'
    recipe = json.loads((pack.HERE.parent / 'content-edit/ffa-ported-1.json').read_text())
    rows = [row for row in recipe['add_entry'] if row['files'] == path]
    assert len(rows) == 22, f'{project}: missing authored FFA source'
    server = pack.child(root, 'server_objects')
    table = pack.child(server, 'respawn_points')
    original = [node for node in table[1] if pack.bcw.to_py(node)['team'] != 2]
    solo = [pack.bcw.to_py(node) for node in table[1] if pack.bcw.to_py(node)['team'] == 2]
    assert len(solo) == len(rows), f'{project}: missing or extra solo spawns'
    template = pack.bcw.to_py(original[0])
    f32 = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
    for point, row in zip(solo, rows):
        expected = copy.deepcopy(template)
        expected.update(row['values'])
        for key in ('position', 'rotation'):
            expected[key] = tuple(f32(v) for v in expected[key])
        expected['enemy_check_radius'] = 20.0
        expected['ally_check_radius'] = 1.0
        for key in ('ally_zones', 'enemy_zones'):
            expected[key]['collision_geometries'] = type(expected[key]['collision_geometries'])()
        assert point == expected, f"{project}: solo spawn differs from its source: {point['name']}"
    retained = (table[0], original, len(original))
    return replace_child(root, 'server_objects', replace_child(server, 'respawn_points', retained))


def check(reference, composed, packs):
    for mode in pack.MODES:
        path = pack.project('v011e_level_03', mode)
        before = pack.bcw.read((reference / path).read_bytes())
        after = without_ffa_spawns(pack.bcw.read((composed / path).read_bytes()), f'v011e_level_03/{mode}')
        original_objects = pack.child(before, 'client_objects')
        result_objects = pack.child(after, 'client_objects')
        imported, retained = [], []
        prefixes = tuple(prefix for _, prefix, _ in pack.GROUPS)
        for node in result_objects[1]:
            (imported if pack.bcw.to_py(node)['full_name'].startswith(prefixes) else retained).append(node)
        assert len(imported) == 70
        assert retained == original_objects[1], f'{mode}: an existing object changed or disappeared'
        without_additions = (after[0], [(key, original_objects if key == b'client_objects' else value)
                                       for key, value in after[1]], after[2])
        if mode == 'day_2':
            donor = pack.bcw.read((reference / pack.project('level_03', mode)).read_bytes())
            server = pack.child(after, 'server_objects')
            spawners = pack.child(server, 'victory_item_spawners')
            excluded = {line.split('\t')[0] for line in
                        (pack.HERE / 'battery-exclusions.tsv').read_text().splitlines()[1:]}
            expected = [entry for entry in pack.bcw.to_py(donor)['server_objects']['victory_item_spawners']
                        if entry['full_name'] not in excluded]
            for i, entry in enumerate(expected):
                entry['point_id'] = i
            assert len(spawners[1]) == 45
            assert pack.bcw.to_py(spawners) == expected
            original_spawners = pack.child(pack.child(before, 'server_objects'), 'victory_item_spawners')
            restored = replace_child(server, 'victory_item_spawners', original_spawners)
            without_additions = replace_child(without_additions, 'server_objects', restored)
        assert without_additions == before, f'{mode}: existing geometry, lighting, spawns or settings changed'
        parsed = pack.bcw.to_py(after)
        assert len(parsed['server_objects']['respawn_points']) == 28
        assert all(o['invulnerability_time'] == 6 for o in parsed['server_objects']['respawn_points'])
        assert any(o['full_name'] == 'srp/artefact_keeper/pool' for o in parsed['client_objects'])
        for donor, prefix, _ in pack.GROUPS:
            source = pack.bcw.load(reference / pack.project(donor, mode))
            if donor == 'v021d_level_03':
                # Both ports restore the original transformer pulse. Corridor
                # tuning remains in the current 0.21d map, outside this group.
                raw = pack.bcw.load(packs / 'v011e-vostok-anomalies-1/files' / path)
                field = next(o for o in raw['client_objects']
                             if o['full_name'] == 'anomaly/electro/electric_hit_damage_zone')
                shape = copy.deepcopy(next(o for o in raw['client_objects']
                                           if o['full_name'] == 'anomaly/electro/electric_hit_shape'))
                xs = [o['position'][0] for o in source['render_visuals']
                      if o['lib_name'].endswith('/transformator_big')]
                assert len(xs) == 3
                mesh = shape['meshes'][0]
                left = shape['position'][0] + mesh['position'][0] - mesh['scale'][0]
                right = max(xs) + min(xs) - left
                f32 = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
                mesh['position'] = (f32(round((left + right) / 2 - shape['position'][0], 3)), *mesh['position'][1:])
                mesh['scale'] = (f32(round((right - left) / 2, 3)), *mesh['scale'][1:])
                replacements = {o['full_name']: o for o in (field, shape)}
                source['client_objects'] = [replacements.get(o['full_name'], o) for o in source['client_objects']]
                actual_donor = pack.bcw.to_py(without_ffa_spawns(
                    pack.bcw.read((composed / pack.project(donor, mode)).read_bytes()), f'{donor}/{mode}'))
                assert actual_donor == source, '0.21d changed beyond its transformer pulse and coverage fixes'
            expected = [copy.deepcopy(o) for o in source['client_objects'] if o['full_name'].startswith(prefix)]
            # maps.tsv gives v0.11 a six-second search; v0.21 uses five.
            for obj in expected:
                if obj['game_object_type'] == 'artefact_container':
                    obj['artefacts_search_time_sec'] = 6
                    assert obj['artefacts_cooldown_time_sec'] == 80
            actual = [pack.bcw.to_py(o) for o in imported if pack.bcw.to_py(o)['full_name'].startswith(prefix)]
            assert actual == expected, f'{mode}: group differs from the current donor and v0.11 rules: {prefix}'
            pack.validate_links(actual)
            if donor == 'v021d_level_03':
                center = [a + b for a, b in zip(shape['position'], mesh['position'])]
                inside = lambda p: all(abs(p[i] - center[i]) <= mesh['scale'][i] for i in range(3))
                particles = [o for o in actual if o['game_object_type'] == 'particle']
                assert len(particles) == 21 and all(inside(o['position']) for o in particles)
                assert all(left < x < right for x in xs), 'an electrified transformer is outside the hit field'
        print(f'{mode}: released map preserved except the requested battery layout; '
              '28 player spawns, protection, lamps and Keeper retained; 70 additions match current donors')
    check_research(reference, composed)
    repaired = 0
    for ident in ('radar-v011e-seams-1', 'radar-v011e-textures-1'):
        folder = packs / ident
        manifest = json.loads((folder / 'pack.json').read_text())
        for op in manifest['operations']:
            path = op['path']
            assert (composed / path).read_bytes() == (folder / 'files' / path).read_bytes(), path
            repaired += 1
    materials = list((composed / 'models/ported/v011e/level/03').glob('terrain_p_*.model/settings'))
    assert materials, 'no terrain materials extracted'
    changed_materials = 0
    for path in materials:
        original = pack.bcw.load(reference / path.relative_to(composed))
        expected = detailed_material(original)
        assert pack.bcw.load(path) == expected, path
        changed_materials += expected != original
    assert changed_materials > 0, 'terrain repair was not exercised'
    print(f'{repaired} seam/texture files match the current repair packs; '
          f'{len(materials)} terrain model settings retain the corrected materials')


def check_research(reference, composed):
    day4 = without_ffa_spawns(
        pack.bcw.read((composed / pack.project('v011e_level_03', 'day_4')).read_bytes()),
        'v011e_level_03/day_4')
    day5 = pack.bcw.read((composed / pack.project('v011e_level_03', 'day_5')).read_bytes())
    expected_map = day4
    crate = 'ported/v011e/props/box/half_cover_rect'
    position = (45.14099884033203, 0.4169999957084656, -64.29199981689453)
    removed_visual = None
    for section, expected_count in (('render_visuals', 1), ('collision_objects', 2)):
        old = pack.child(expected_map, section)
        removed = [i for i, node in enumerate(old[1])
                   if crate in pack.bcw.to_py(node)['lib_name']
                   and pack.bcw.to_py(node)['position'] == position]
        assert len(removed) == expected_count
        if section == 'render_visuals':
            removed_visual = removed[0]
        new = old[0], [node for i, node in enumerate(old[1]) if i not in removed], old[2] - len(removed)
        expected_map = replace_child(expected_map, section, new)
    assert removed_visual is not None
    lamps = pack.bcw.to_py(day4)['srp_lamps']['lamps']
    for name, lamp in lamps.items():
        for ref, value in lamp['visuals'].items():
            index = value['index']
            assert index != removed_visual
            if index > removed_visual:
                expected_map = change_path(expected_map, ['srp_lamps', 'lamps', name, 'visuals', ref, 'index'],
                                           lambda node: (node[0], node[1] - 1, node[2]))
    # The edit list carries pinned native station records. Check their exact
    # values and every existing typed node after normal content composition.
    edit = json.loads((pack.HERE.parent / 'content-edit/v011e-vostok-modes-1.json').read_text())
    restored = day5
    for section, count in (('render_visuals', 55), ('collision_objects', 88), ('client_objects', 12)):
        original = pack.child(expected_map, section)
        made = pack.child(day5, section)
        assert len(made[1]) == len(original[1]) + count
        assert made[1][:-count] == original[1], f'Research changed existing {section}'
        operation = 'append_entry' if section == 'client_objects' else 'add_entry'
        value_key = 'value' if section == 'client_objects' else 'values'
        expected = [e[value_key] for e in edit[operation] if e['list'] == section]
        actual = [pack.bcw.to_py(node) for node in made[1][-count:]]
        assert json.loads(json.dumps(actual)) == expected
        restored = replace_child(restored, section, original)
    assert restored == expected_map, 'Research changed more than the station C crate and its lamp index shifts'
    cfg = 'gameplay/game_modes/v011e_level_03_day_5.options'
    actual = pack.bcw.load(composed / cfg)
    expected = pack.bcw.load(reference / 'gameplay/game_modes/level_03_day_5.options')
    expected['mode']['name'] = 'v011e_level_03_day_5'
    expected['mode']['match_making_params'].update(map_name='v011e_level_03/day_5', map_id=13)
    expected['mode']['ui'].update(map_name='st_level_13_name', loading_screen_id=29)
    assert actual == expected, 'Research rules differ from the current donor'
    assert [zone['cp_id'] for zone in actual['mode']['parameters']['capture_zones']] == [0, 1, 2]
    dictionary = pack.bcw.load(composed / 'gameplay/db_static_dictionaries')['maps_dict']
    assert [row for row in dictionary if row['id'] == 249] == [
        {'cfg_mode_name': 'v011e_level_03_day_5.options', 'id': 249}]
    print('Research: map 249 registered; three native capture zones and rules, 55 visuals, '
          '88 collisions and 12 effects; station C crate removed, other map values and lamp bindings preserved')


def detailed_material(value):
    if isinstance(value, dict):
        return {key: detailed_material(child) for key, child in value.items()}
    if isinstance(value, list):
        return [detailed_material(child) for child in value]
    if value == 'ported/v011e/terrain/03_base_final_lod':
        return 'ported/v011e/terrain/03_base_final'
    return value


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--composed', type=Path, required=True)
    parser.add_argument('--packs', type=Path, required=True)
    args = parser.parse_args()
    check(args.reference, args.composed, args.packs)
