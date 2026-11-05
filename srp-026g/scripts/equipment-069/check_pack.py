#!/usr/bin/env python3
"""Check the built pack against native equipment, equipment-046, the catalog and the porter's
fit record."""
import argparse
import copy
import csv
import json
import math
from pathlib import Path
import struct

from common import (DICTIONARY, E046_MANIFEST, MODIFICATIONS, MOVIE, SCRIPTS, _load, bcw, e046_entries,
                    equipment046, model_parts, sound_file, specs, template, text_keys)


def equipment046_check():
    folder = SCRIPTS / 'equipment-046'
    return _load('equipment046_check_pack', folder / 'check_pack.py', folder)


def equipment046_rows():
    with (SCRIPTS / 'equipment-046' / 'items.tsv').open() as source:
        return list(csv.DictReader(source, delimiter='\t', quoting=csv.QUOTE_NONE))


def mesh(raw):
    """A 0.26g skinned surface's vertex positions and indices (chunks 3 and 4)."""
    chunks, at = {}, 0
    while at < len(raw):
        ident, size = struct.unpack_from('<II', raw, at)
        chunks[ident] = raw[at + 8:at + 8 + size]
        at += 8 + size
    vertices, indices = chunks[3], chunks[4]
    count = struct.unpack_from('<I', vertices, 0)[0]
    stride = (len(vertices) - 4) // count
    positions = [struct.unpack_from('<3f', vertices, 4 + i * stride) for i in range(count)]
    return positions, indices[4:4 + 2 * struct.unpack_from('<I', indices, 0)[0]]


def built_ins(spec):
    return [int(v) for v in spec['default_modifications'].split(',') if v]


def check_native(spec, item, template_item, row, by_id, fits):
    """A back piece (no protection, its weight on the card) or a mask (the row's armour and
    isolation on every hit type of the template's regions; armour, isolation, weight on the
    card), the template's parameters with the row's level, faction, weight and drop weight."""
    params, ident = item['parameters'], row['dict_id']
    assert set(params) <= set(template_item['parameters']) | {'base_rarity_level'}, (ident, 'non-native parameter')
    assert params['type'] == template_item['parameters']['type'], ident
    assert params['item_level'] == params['match_making_level'] == int(spec['level'])
    assert params['faction'] == int(spec['faction']) and row['base_cost'] == int(spec['price'])
    assert params['drop_weight'] == int(spec['drop_weight']) > 0
    assert math.isclose(params['weight'], float(spec['weight']), abs_tol=1e-5)
    assert item['default_modifications'] == built_ins(spec), ident
    assert item['data']['type'] == 4 and 'srp_premium' not in item
    assert row['item_category'] == by_id[template(spec).ident]['item_category']
    props = [(p['prop_id'], p['prop_value']) for p in item['ui_desc']['props_list']]
    armour, isolation = float(spec['armour']) / 100, float(spec['isolation']) / 100
    if params['type'] == 'arm_back':
        assert (armour, isolation) == (0, 0) and 'hit_params' not in item, ident
        assert props == [(15, f"{params['weight']:.1f}")], (ident, props)
        return
    assert params['type'] == 'arm_mask', ident
    assert math.isclose(params['armor'], armour, abs_tol=1e-5) and math.isclose(params['ano_armor'], isolation, abs_tol=1e-5)
    assert sorted(item['hit_params']) == sorted(template_item['hit_params']), ident
    for region in item['hit_params'].values():
        for kind, hit in region['hit_types'].items():
            want = armour if kind in ('ranged', 'slash', 'explosive') else isolation
            assert math.isclose(hit['armor'], want, abs_tol=1e-5), (ident, kind)
    assert props == [(6, f'{armour * 100:g}'), (8, f'{isolation * 100:g}'), (15, f"{params['weight']:.1f}")], (ident, props)
    # Worn over the native head in the mask slot alone (the owner's rule for ported headwear,
    # T28 for the Protective mask): fitted over the bare head, a helmet goes on over it.
    assert set(item['data']) == {'part_name', 'base_model', 'type'}, (ident, 'head, slots or extra models')
    assert fits[spec['model']]['fit']['kind'] == 'mask', ident


def check_copy(spec, item, source, row, e046_row, fits, pack, inputs):
    """The equipment-046 piece whole but for its models (the bundle's recolours), its built-in
    bonuses (the row's), its icon and texts; each surface it draws the copied piece's mesh."""
    ident = row['dict_id']
    assert row['base_cost'] == e046_row['base_cost'] == int(spec['price']), ident
    assert row['item_category'] == e046_row['item_category'], ident
    expected = copy.deepcopy(source)
    expected['ui_desc']['icon'] = item['ui_desc']['icon']
    expected['ui_desc']['text_descriptions'] = text_keys(spec)
    expected['default_modifications'] = built_ins(spec)
    for mine, theirs in zip(model_parts(item), model_parts(expected)):
        recoloured = fits[mine['base_model']]['fit']
        assert recoloured == {'kind': 'same_as', 'model': theirs['base_model']}, ident
        assert mine['part_name'] == theirs['part_name'], ident
        # The released mesh: the copied piece's surface, vertex for vertex.
        released = inputs / 'equipment046/files' / (f"models/{theirs['base_model']}.skinned_model/render/"
                                                     f"{theirs['part_name']}/converted_model")
        built = pack / 'files' / f"models/{mine['base_model']}.skinned_model/render/{mine['part_name']}/converted_model"
        assert mesh(released.read_bytes()) == mesh(built.read_bytes()), (ident, mine)
        theirs['base_model'] = mine['base_model']
    assert item == expected, (ident, 'differs from the copied piece beyond its models, built-ins, icon and texts')
    params = item['parameters']
    assert (params['faction'], params['item_level'], params['drop_weight']) == (
        int(spec['faction']), int(spec['level']), int(spec['drop_weight'])), ident
    for name, value in (('weight', float(spec['weight'])), ('armor', float(spec['armour']) / 100),
                        ('ano_armor', float(spec['isolation']) / 100)):
        assert math.isclose(params[name], value, abs_tol=1e-5), (ident, name)


def check(pack, inputs):
    base = inputs / 'base'
    rows = specs()
    e046, e046check = equipment046(), equipment046_check()
    e029 = e046.e029
    manifest = json.loads((pack / 'pack.json').read_text())
    operations = manifest['operations']
    entries = next(op['entries'] for op in operations if op['op'] == 'dictionary_add')
    assert len(entries) == len(rows) == 8 and not any(row['is_premium'] for row in entries)
    native = bcw.loads((base / DICTIONARY).read_bytes())['items_dict']
    e046_manifest = json.loads((inputs / 'equipment046' / E046_MANIFEST).read_text())
    assert (e046_manifest['id'], e046_manifest['version']) == ('equipment-046', '4'), 'not equipment-046@4'
    e046_rows = e046_entries(e046_manifest)
    by_id = {row['dict_id']: row for row in native + entries}
    assert len(by_id) == len(native) + len(entries), 'dictionary id collision'
    # equipment-046 (the pack below this one in release/content.tsv, not in base-patches.tsv).
    pending = equipment046_rows()
    assert not {int(r['id']) for r in pending} & {e['dict_id'] for e in entries}, 'id taken by equipment-046'
    for field in ('cfg_name', 'name'):
        new = [r[field] for r in entries]
        taken = {r[field] for r in native} | {r[field] for r in e046_rows.values()}
        assert len(set(new)) == len(new) and not set(new) & taken, f'{field} collision'
    # Surfaces as 0.26g draws them, no 16-byte config, every material a released character's
    # shader combination with every map shipped (equipment-046's asset checks).
    surfaces, materials = e046check.check_assets(pack, base)
    kinds, ranked = e046check.promoted_floor(bcw.loads((base / MODIFICATIONS).read_bytes()))
    base_atlas = e029.atlas((base / MOVIE).read_bytes())
    taken = {(int(r['icon_row']), int(r['icon_col'])) for r in pending}
    fits = {piece['model']: piece for piece in json.loads((inputs / 'bundle/.records/fit-report.json').read_text())}

    def config(ident):
        path = by_id[ident]['cfg_name']
        source = pack / 'files' / path
        return bcw.loads((source if source.exists() else base / path).read_bytes())

    spec_of = {int(r['id']): r for r in rows}
    occupied = set()
    for row in entries:
        ident = row['dict_id']
        spec = spec_of[ident]
        start = template(spec)
        item = config(ident)
        if start.copy:
            source = bcw.loads((inputs / 'equipment046/files' / e046_rows[start.ident]['cfg_name']).read_bytes())
            check_copy(spec, item, source, row, e046_rows[start.ident], fits, pack, inputs)
        else:
            check_native(spec, item, config(start.ident), row, by_id, fits)
        for part in model_parts(item):
            assert part['part_name'] in surfaces[part['base_model']], (ident, part)
        icon = tuple(item['ui_desc']['icon'])
        assert icon not in occupied and icon not in taken, (ident, 'duplicate icon')
        assert not (icon[1] <= base_atlas[0] // 171 and any(e029.cell(base_atlas, *icon)[3::4])), (ident, 'icon cell taken')
        assert (pack / f'icons/{ident}.png').exists()
        occupied.add(icon)
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
    # The porter's record: every back piece sinks into no stock torso deeper than a native back
    # piece; every recolour has its 0.69d model's vertices and triangles; every mask was fitted.
    drawn = {part['base_model'] for row in entries for part in model_parts(config(row['dict_id']))}
    assert sorted(fits) == sorted(drawn), 'the bundle and the configs draw other models'
    for piece in fits.values():
        fit = piece['fit']
        if fit['kind'] == 'back':
            assert fit['torsos'] and all(t['share'] <= t['native_max'] for t in fit['torsos']), piece['item']
        else:
            assert fit['kind'] in ('same_as', 'mask'), piece['item']
    print(f'{len(entries)} equipment configs, icons, six localizations (the catalog\'s), {len(surfaces)} models and '
          f'{materials} materials\' shaders, {len(fits)} fit records checked')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('inputs', type=Path, help="prepare_inputs.py's folder")
    args = parser.parse_args()
    check(args.pack, args.inputs)
