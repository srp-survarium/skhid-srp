#!/usr/bin/env python3
"""Check the built pack against native equipment and its compiled-UI source."""
import argparse
import json
import math
from pathlib import Path
import re

from make_pack import bcw, material_switches, missing_maps, movie_metadata, shipped_switches, shipped_textures, specs, HERE


def check_shaders(pack):
    """Every material draws with a shader the released game has compiled, and
    every map it switches on has its texture in the game or the pack."""
    shipped = shipped_switches()
    files = pack / 'files'
    textures = shipped_textures(str(path.relative_to(files)) for path in files.glob('textures/**/*.dds'))
    materials = sorted(files.glob('material_instances/**/*.material'))
    assert materials, 'no materials in the pack'
    for path in materials:
        material = bcw.loads(path.read_bytes())
        for combination in material_switches(material):
            assert combination in shipped, (str(path.relative_to(pack)), 'no 0.26g shader', combination)
        missing = list(missing_maps(material, textures))
        assert not missing, (str(path.relative_to(pack)), 'map switched on without its texture', missing)
    return len(materials)


def check(pack, base):
    manifest = json.loads((pack / 'pack.json').read_text())
    operations = manifest['operations']
    entries = next(op['entries'] for op in operations if op['op'] == 'dictionary_add')
    assert len(entries) == 84
    assert sum(row['is_premium'] for row in entries) == 22
    assert not {948, 1816} & {row['dict_id'] for row in entries}, 'existing Protective mask must be retained'
    native = bcw.loads((base / 'gameplay/db_static_dictionaries').read_bytes())['items_dict']
    by_id = {row['dict_id']: row for row in native + entries}
    assert len(by_id) == len(native) + len(entries), 'dictionary id collision'

    def config(ident):
        path = by_id[ident]['cfg_name']
        source = pack / 'files' / path
        return bcw.loads((source if source.exists() else base / path).read_bytes())

    occupied = set()
    expected_ui = []
    native_parameters = {'base_rarity_level'}.union(*(config(int(r['template_id']))['parameters'] for r in specs()))
    for row in entries:
        ident = row['dict_id']
        item = config(ident)
        params = item['parameters']
        assert set(params) <= native_parameters, (ident, 'non-native parameter')
        assert 1 <= params['item_level'] <= 15
        assert params['match_making_level'] == params['item_level']
        assert item['data']['type'] == (1 if params['type'] == 'arm_oxy' else 4)
        icon = tuple(item['ui_desc']['icon'])
        assert icon not in occupied, (ident, 'duplicate icon')
        occupied.add(icon)
        assert row['base_cost'] > 0
        props = {p['prop_id']: p['prop_value'] for p in item['ui_desc']['props_list']}
        assert 134 not in props, '0.29 bleeding protection has no native card property'
        for prop, key, scale in [(6, 'armor', 100), (8, 'ano_armor', 100), (15, 'weight', 1), (11, 'oxygen_amount', 1)]:
            if prop in props:
                assert math.isclose(float(props[prop]), params[key] * scale, abs_tol=0.05), (ident, key)
        for slot in item.get('hit_params', {}).values():
            assert set(slot['hit_types']) <= {'ranged', 'slash', 'explosive', 'radiation', 'shock', 'fire', 'toxic'}
            assert all(0 <= hit['armor'] <= 1 for hit in slot['hit_types'].values())
        if row['is_premium']:
            meta = item['srp_premium']
            plain = config(meta['plain_item'])
            expected = dict(plain['parameters'], drop_weight=0)
            expected.setdefault('base_rarity_level', 0)
            assert params == expected, (ident, 'premium combat parameters differ from plain')
            assert item.get('hit_params') == plain.get('hit_params'), (ident, 'premium protection differs')
            assert item['default_modifications'] == plain.get('default_modifications', [])
            assert not set(meta['bonuses']) & set(item['default_modifications'])
            assert meta['bonuses'] == [1240, 1240 + 10 * params['faction']]
            expected_ui.append([ident, meta['plain_item'], meta['bonuses']])
        else:
            assert 'srp_premium' not in item

    ui = pack / 'ui/Equipment029.as'
    actual_ui = [json.loads(match) for match in re.findall(r'\[\d+,\d+,\[\d+,\d+\]\]', ui.read_text())]
    assert sorted(actual_ui) == sorted(expected_ui), 'movie premiums differ from server metadata'
    committed = HERE.parent.parent / 'flash/lobby_menu/src/survarium/constants/Equipment029.as'
    assert ui.read_text() == committed.read_text() == movie_metadata(specs()), 'stale generated UI source'
    localizations = [op for op in operations if op['op'] == 'localization_add']
    assert len(localizations) == 6
    keys = set(localizations[0]['strings'])
    assert all(set(op['strings']) == keys for op in localizations)
    for row in entries:
        texts = config(row['dict_id'])['ui_desc']['text_descriptions']
        assert set(texts.values()) <= keys
        assert all(op['strings'][texts['name']] for op in localizations), row['dict_id']
    shaded = check_shaders(pack)
    print(f'84 equipment configs, 22 exact premium counterparts, icons, six localizations and {shaded} materials\' shaders checked')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('base', type=Path)
    args = parser.parse_args()
    check(args.pack, args.base)
