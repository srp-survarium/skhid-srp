#!/usr/bin/env python3
"""Check the built pack against native equipment, the catalog and its compiled-UI source."""
import argparse
import json
import math
from pathlib import Path
import re
import struct

from make_pack import (bcw, cell, atlas, catalog, e029, movie_metadata, shipped_switches, shipped_textures, specs,
                       sound_file,
                       text_keys, FLASH_MAPPING, HIT_TYPES, LANGUAGES, LATER_STAGE_KEYS, PHYSICAL, SURFACE_CHUNKS)

MODIFICATIONS = 'gameplay/items/modifications/modifications.options'
WEAR = ('parameters/degrade_per_death', 'parameters/degrade_per_match', 'parameters/degrade_per_use')


def promoted_floor(mods):
    """For each rolling kind, its second-strongest row (built-in-bonuses-1 raises defaults to it)."""
    rows, kinds = {}, {}
    for row in mods['modifications']:
        kind = row['type_mask'].index('1')
        kinds[row['id']] = kind
        paths = ['/'.join(m['path']) for m in row['modifiers']]
        if row['drop_weight'] > 0 and not (paths and all(p in WEAR for p in paths)):
            rows.setdefault(kind, []).append((-row['drop_weight'], row['id']))
    ranked = {kind: [i for _, i in sorted(r)] for kind, r in rows.items()}
    return kinds, ranked


def check_assets(pack):
    """Surfaces as 0.26g draws them; no 16-byte config; every shader compiled, every map shipped."""
    files = pack / 'files'
    shipped = shipped_switches()
    textures = shipped_textures(str(p.relative_to(files)) for p in files.glob('textures/**/*.dds'))
    surfaces, materials = {}, 0
    for path in sorted(p for p in files.rglob('*') if p.is_file()):
        raw = path.read_bytes()
        rel = str(path.relative_to(files))
        if path.name == 'converted_model':
            at, kinds = 0, []
            while at < len(raw):
                kind, size = struct.unpack_from('<II', raw, at)
                kinds.append(kind)
                at += 8 + size
            assert at == len(raw) and kinds == list(SURFACE_CHUNKS), (rel, kinds)
            continue
        if path.suffix == '.dds' or path.name == 'bind_pose':
            continue
        assert not bcw.is_compact(raw), (rel, '16-byte config')
        value = bcw.loads(raw)
        if path.name == 'settings':
            lods = value['lod_hierrarchy']
            assert not lods['LOD1'] and not lods['LOD2'], (rel, 'LOD1/LOD2 surfaces')
            assert set(value['material_settings']) == set(lods['LOD0']['surfaces']), rel
            surfaces[rel.removeprefix('models/').removesuffix('.skinned_model/settings')] = lods['LOD0']['surfaces']
        if path.suffix == '.material':
            materials += 1
            for stage in value['material'].values():
                assert not set(LATER_STAGE_KEYS) & set(stage), (rel, 'a later build stage key')
            for combination in e029.material_switches(value):
                assert combination in shipped, (rel, 'no 0.26g shader', combination)
            missing = list(e029.missing_maps(value, textures))
            assert not missing, (rel, 'map switched on without its texture', missing)
    return surfaces, materials


def check(pack, base):
    rows = specs()
    manifest = json.loads((pack / 'pack.json').read_text())
    operations = manifest['operations']
    entries = next(op['entries'] for op in operations if op['op'] == 'dictionary_add')
    assert len(entries) == len(rows) == 35
    assert sum(row['is_premium'] for row in entries) == sum(1 for r in rows if r['plain_id']) == 1
    native = bcw.loads((base / 'gameplay/db_static_dictionaries').read_bytes())['items_dict']
    by_id = {row['dict_id']: row for row in native + entries}
    assert len(by_id) == len(native) + len(entries), 'dictionary id collision'
    for field in ('cfg_name', 'name'):
        new = [r[field] for r in entries]
        assert len(set(new)) == len(new) and not set(new) & {r[field] for r in native}, f'{field} collision'
    surfaces, materials = check_assets(pack)
    mods = bcw.loads((base / MODIFICATIONS).read_bytes())
    kinds, ranked = promoted_floor(mods)
    base_atlas = atlas((base / 'flash_movies/lobby_menu.swf').read_bytes())

    def config(ident):
        path = by_id[ident]['cfg_name']
        source = pack / 'files' / path
        return bcw.loads((source if source.exists() else base / path).read_bytes())

    spec_of = {int(r['id']): r for r in rows}
    native_parameters = {'base_rarity_level'}.union(*(config(int(r['template_id']))['parameters'] for r in rows))
    occupied, expected_ui = set(), []
    for row in entries:
        ident = row['dict_id']
        spec = spec_of[ident]
        item = config(ident)
        params = item['parameters']
        assert set(params) <= native_parameters, (ident, 'non-native parameter')
        assert params['item_level'] == params['match_making_level'] == int(spec['level'])
        assert params['faction'] == int(spec['faction']) and row['base_cost'] == int(spec['price'])
        assert item['data']['type'] == 4
        assert row['item_category'] == by_id[int(spec['template_id'])]['item_category']
        icon = tuple(item['ui_desc']['icon'])
        assert icon not in occupied, (ident, 'duplicate icon')
        assert not (icon[1] <= base_atlas[0] // 171 and any(cell(base_atlas, *icon)[3::4])), (ident, 'icon cell taken')
        assert (pack / f'icons/{ident}.png').exists()
        occupied.add(icon)
        props = [(p['prop_id'], p['prop_value']) for p in item['ui_desc']['props_list']]
        assert [p for p, _ in props] == ([15] if params['type'] == 'arm_back' else [6, 8, 15]), (ident, props)
        for prop, value in props:
            key, scale = {6: ('armor', 100), 8: ('ano_armor', 100), 15: ('weight', 1)}[prop]
            assert math.isclose(float(value), params[key] * scale, abs_tol=0.05), (ident, key)
        for region in item.get('hit_params', {}).values():
            assert tuple(region['hit_types']) == HIT_TYPES or set(region['hit_types']) == set(HIT_TYPES), ident
            for kind, hit in region['hit_types'].items():
                assert math.isclose(hit['armor'], params['armor' if kind in PHYSICAL else 'ano_armor']), (ident, kind)
        for key in ('data',) + tuple(k for k in item if k.startswith('data_')):
            for part in [item[key]] + item[key].get('additional_models', []):
                assert part['part_name'] in surfaces[part['base_model']], (ident, part)
            assert 'damage_protection' not in item[key] and 'remove_head_model' not in item[key], ident
        assert item['ui_desc']['text_descriptions'] == text_keys(spec)
        for sound in item.get('sounds', []):
            assert (base / sound_file(*sound)).exists(), (ident, 'sound not in the released game', sound)
        for mid in item.get('default_modifications', []):
            floor = ranked.get(kinds[mid])
            # built-in-bonuses-1 would raise a weaker default: the pack's are already at its strength.
            assert not floor or (mid in floor and floor.index(mid) >= len(floor) - 2), (ident, mid, 'below promotion')
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
            assert math.isclose(params['weight'], float(spec['weight']), abs_tol=1e-5)
    ui = pack / 'ui/Equipment046.as'
    actual_ui = [json.loads(m) for m in re.findall(r'\[\d+,\d+,\[\d+,\d+\]\]', ui.read_text())]
    assert sorted(actual_ui) == sorted(expected_ui), 'movie premiums differ from server metadata'
    assert ui.read_text() == FLASH_MAPPING.read_text() == movie_metadata(rows), 'stale generated UI source'
    localizations = [op for op in operations if op['op'] == 'localization_add']
    assert sorted(op['languages'][0] for op in localizations) == sorted(LANGUAGES)
    keys = {k for spec in rows for k in text_keys(spec).values()}
    for op in localizations:
        table = catalog(LANGUAGES[op['languages'][0]])
        assert set(op['strings']) == keys
        assert all(op['strings'][k] == e029.porter_text(table[k]) and table[k] for k in keys), op['languages']
    print(f'{len(entries)} equipment configs, {len(expected_ui)} exact premium counterpart, icons, '
          f'six localizations (the catalog\'s), {len(surfaces)} models and {materials} materials\' shaders checked')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('base', type=Path, help="prepare_inputs.py's base group")
    args = parser.parse_args()
    check(args.pack, args.base)
