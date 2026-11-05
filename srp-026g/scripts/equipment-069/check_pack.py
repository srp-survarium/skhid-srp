#!/usr/bin/env python3
"""Check the built pack against native equipment, equipment-046, the catalog and the porter's
fit record."""
import argparse
import csv
import json
import math
from pathlib import Path

from common import (DICTIONARY, MODIFICATIONS, MOVIE, SCRIPTS, _load, bcw, equipment046, sound_file, specs,
                    text_keys)


def equipment046_check():
    folder = SCRIPTS / 'equipment-046'
    return _load('equipment046_check_pack', folder / 'check_pack.py', folder)


def equipment046_rows():
    with (SCRIPTS / 'equipment-046' / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t', quoting=csv.QUOTE_NONE))


def check(pack, inputs):
    base = inputs / 'base'
    rows = specs()
    e046, e046check = equipment046(), equipment046_check()
    e029 = e046.e029
    manifest = json.loads((pack / 'pack.json').read_text())
    operations = manifest['operations']
    entries = next(op['entries'] for op in operations if op['op'] == 'dictionary_add')
    assert len(entries) == len(rows) == 2 and not any(row['is_premium'] for row in entries)
    native = bcw.loads((base / DICTIONARY).read_bytes())['items_dict']
    by_id = {row['dict_id']: row for row in native + entries}
    assert len(by_id) == len(native) + len(entries), 'dictionary id collision'
    # equipment-046 (the pack below this one in release/content.tsv, not released yet).
    pending = equipment046_rows()
    assert not {int(r['id']) for r in pending} & {e['dict_id'] for e in entries}, 'id taken by equipment-046'
    for field in ('cfg_name', 'name'):
        new = [r[field] for r in entries]
        assert len(set(new)) == len(new) and not set(new) & {r[field] for r in native}, f'{field} collision'
    # Surfaces as 0.26g draws them, no 16-byte config, every material a released character's
    # shader combination with every map shipped (equipment-046's asset checks).
    surfaces, materials = e046check.check_assets(pack, base)
    kinds, ranked = e046check.promoted_floor(bcw.loads((base / MODIFICATIONS).read_bytes()))
    base_atlas = e029.atlas((base / MOVIE).read_bytes())
    taken = {(int(r['icon_row']), int(r['icon_col'])) for r in pending}

    def config(ident):
        path = by_id[ident]['cfg_name']
        source = pack / 'files' / path
        return bcw.loads((source if source.exists() else base / path).read_bytes())

    spec_of = {int(r['id']): r for r in rows}
    occupied = set()
    for row in entries:
        ident = row['dict_id']
        spec = spec_of[ident]
        item, template = config(ident), config(int(spec['template_id']))
        params = item['parameters']
        assert set(params) <= set(template['parameters']) | {'base_rarity_level'}, (ident, 'non-native parameter')
        assert params['type'] == template['parameters']['type'] == 'arm_back', ident
        assert params['item_level'] == params['match_making_level'] == int(spec['level'])
        assert params['faction'] == int(spec['faction']) and row['base_cost'] == int(spec['price'])
        assert params['drop_weight'] == int(spec['drop_weight']) > 0
        assert math.isclose(params['weight'], float(spec['weight']), abs_tol=1e-5)
        assert item['data']['type'] == 4 and 'hit_params' not in item and 'srp_premium' not in item
        assert row['item_category'] == by_id[int(spec['template_id'])]['item_category']
        assert item['data']['part_name'] in surfaces[item['data']['base_model']], (ident, item['data'])
        icon = tuple(item['ui_desc']['icon'])
        assert icon not in occupied and icon not in taken, (ident, 'duplicate icon')
        assert not (icon[1] <= base_atlas[0] // 171 and any(e029.cell(base_atlas, *icon)[3::4])), (ident, 'icon cell taken')
        assert (pack / f'icons/{ident}.png').exists()
        occupied.add(icon)
        props = [(p['prop_id'], p['prop_value']) for p in item['ui_desc']['props_list']]
        assert props == [(15, f"{params['weight']:.1f}")], (ident, props)
        assert item['ui_desc']['text_descriptions'] == text_keys(spec)
        for sound in item.get('sounds', []):
            assert (base / sound_file(*sound)).exists(), (ident, 'sound not in the released game', sound)
        for mid in item['default_modifications']:
            floor = ranked.get(kinds[mid])
            # built-in-bonuses-1 would raise a weaker default: the pack's are already at its strength.
            assert not floor or (mid in floor and floor.index(mid) >= len(floor) - 2), (ident, mid, 'below promotion')
    localizations = [op for op in operations if op['op'] == 'localization_add']
    assert sorted(op['languages'][0] for op in localizations) == sorted(e046.LANGUAGES)
    keys = {k for spec in rows for k in text_keys(spec).values()}
    for op in localizations:
        table = e046.catalog(e046.LANGUAGES[op['languages'][0]])
        assert set(op['strings']) == keys
        assert all(op['strings'][k] == e029.porter_text(table[k]) and table[k] for k in keys), op['languages']
    # The porter's record: every piece sinks into no stock torso deeper than a native back piece.
    fits = json.loads((inputs / 'bundle/.records/fit-report.json').read_text())
    assert sorted(f['model'] for f in fits) == sorted(r['model'] for r in rows)
    for piece in fits:
        assert piece['fit'] and all(t['share'] <= t['native_max'] for t in piece['fit']), piece['item']
    print(f'{len(entries)} equipment configs, icons, six localizations (the catalog\'s), {len(surfaces)} models and '
          f'{materials} materials\' shaders, the fit against {len(fits[0]["fit"])} torso surfaces checked')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('inputs', type=Path, help="prepare_inputs.py's folder")
    args = parser.parse_args()
    check(args.pack, args.inputs)
