#!/usr/bin/env python3
"""Build equipment-069 from the pinned inputs (prepare_inputs.py) and the reviewed items.tsv.

The models, materials and textures are resource-porter's (`vostok-port build-armour-069d`,
pinned as the `bundle` group); this script writes the items around them: a config (`template`:
a native back piece of the same kind, its parameters, defect and modification masks, repair,
card layout and sounds; or an equipment-046 piece copied whole, its models the bundle's), the
dictionary rows, the catalog's texts and the 0.69d icons in free equipment-atlas cells. The
output is a resource-porter content pack, not a hand-assembled patch; release/build-content
composes it.
"""
import argparse
import copy
import json
import math
from pathlib import Path

from common import (DICTIONARY, DONOR_ICONS, E046_MANIFEST, MODIFICATIONS, MOVIE, PACK, Inputs, bcw, e046_entries,
                    equipment046, model_parts, sha, specs, template, text_keys)

VERSION = '4'
DATA_TYPE = 4  # the native equipment data type


def model_surfaces(inputs, model):
    """A bundle model's LOD0 surfaces."""
    return inputs.config('bundle', f'models/{model}.skinned_model/settings')['lod_hierrarchy']['LOD0']['surfaces']


def model_surface(inputs, model):
    """The one LOD0 surface of a bundle model (a back piece's or a mask's part_name)."""
    surfaces = model_surfaces(inputs, model)
    if len(surfaces) != 1:
        raise ValueError(f'{model}: {len(surfaces)} LOD0 surfaces; a back piece or a mask draws one part')
    return surfaces[0]


def config_path(spec, cfg_name):
    """Beside the config the piece starts from."""
    return f"{cfg_name.rsplit('/', 1)[0]}/eq069_{spec['id_069']}.options"


def built_ins(spec):
    return [int(v) for v in spec['default_modifications'].split(',') if v]


def protection(spec):
    """items.tsv's armour and isolation (percent) as the config's fractions."""
    return float(spec['armour']) / 100, float(spec['isolation']) / 100


def convert(template, spec, part):
    """The template's config with this piece's model, level, faction, weight, drop weight,
    built-in bonuses, protection (a mask's on every hit type of the template's regions; a back
    piece has none), card, icon and texts."""
    e046 = equipment046()
    result = copy.deepcopy(template)
    params = result['parameters']
    armour, isolation = protection(spec)
    if params['type'] == 'arm_back':
        if (armour, isolation) != (0, 0) or 'hit_params' in result:
            raise ValueError(f"{spec['id']}: a back piece has no protection")
    elif params['type'] == 'arm_mask':
        params.update(armor=armour, ano_armor=isolation)
        result['hit_params'] = {
            region: {'hit_types': {kind: {'armor': armour if kind in e046.PHYSICAL else isolation}
                                   for kind in e046.HIT_TYPES}}
            for region in template['hit_params']}
    else:
        raise ValueError(f"{spec['id']}: template {spec['template']} is neither a back piece nor a mask")
    params.update(faction=int(spec['faction']), item_level=int(spec['level']),
                  match_making_level=int(spec['level']), base_rarity_level=0,
                  weight=float(spec['weight']), drop_weight=int(spec['drop_weight']))
    result['default_modifications'] = built_ins(spec)
    result['data'] = {'part_name': part, 'base_model': spec['model'], 'type': DATA_TYPE}
    result['ui_desc']['props_list'] = e046.card(params, params['type'])
    result['ui_desc']['icon'] = [int(spec['icon_row']), int(spec['icon_col'])]
    result['ui_desc']['text_descriptions'] = text_keys(spec)
    return result


def bundle_fits(inputs):
    """The porter's fit record, by model."""
    return {piece['model']: piece for piece in json.loads(inputs.read('bundle', '.records/fit-report.json'))}


def copy_piece(source, spec, price, fits, inputs):
    """An equipment-046 piece's config whole, drawn with the bundle's recolours of its models (the
    porter's `same_as`), with the row's built-in bonuses (the Snow-20's winter isolation in
    place of the Sniper's own). The row states the copied piece's other numbers; any other is
    refused."""
    result = copy.deepcopy(source)
    params = result['parameters']
    armour, isolation = protection(spec)
    stated = {'faction': int(spec['faction']), 'item_level': int(spec['level']),
              'drop_weight': int(spec['drop_weight']), 'weight': float(spec['weight']),
              'armor': armour, 'ano_armor': isolation, 'price': int(spec['price'])}
    # Floats are stored as f32.
    copied = {key: stated[key] if isinstance(stated[key], float) and math.isclose(value, stated[key], abs_tol=1e-5)
              else value
              for key, value in (('faction', params['faction']), ('item_level', params['item_level']),
                                 ('drop_weight', params['drop_weight']), ('weight', params['weight']),
                                 ('armor', params['armor']), ('ano_armor', params['ano_armor']), ('price', price))}
    if stated != copied:
        raise ValueError(f"{spec['id']}: items.tsv {stated} is not the copied piece's {copied}")
    recolours = {fit['fit']['model']: model for model, fit in fits.items() if fit['fit']['kind'] == 'same_as'}
    parts = model_parts(result)
    for part in parts:
        if part['base_model'] not in recolours:
            raise ValueError(f"{spec['id']}: the bundle does not recolour {part['base_model']}")
        part['base_model'] = recolours[part['base_model']]
        if part['part_name'] not in model_surfaces(inputs, part['base_model']):
            raise ValueError(f"{spec['id']}: {part['base_model']} has no surface {part['part_name']}")
    if spec['model'] not in {part['base_model'] for part in parts}:
        raise ValueError(f"{spec['id']}: the config does not draw {spec['model']}")
    result['default_modifications'] = built_ins(spec)
    result['ui_desc']['icon'] = [int(spec['icon_row']), int(spec['icon_col'])]
    result['ui_desc']['text_descriptions'] = text_keys(spec)
    return result


def write_pack(inputs, out):
    e046 = equipment046()
    e029 = e046.e029
    base = inputs.config('base', DICTIONARY)['items_dict']
    base_by_id = {r['dict_id']: r for r in base}
    rows = specs()
    inputs.read('base', MODIFICATIONS)  # the balance audit's effect values
    ids = [int(r['id']) for r in rows]
    if len(ids) != len(set(ids)) or set(ids) & base_by_id.keys():
        raise ValueError('new equipment ids are duplicated or already occupied')
    base_atlas = e029.atlas(inputs.read('base', MOVIE))
    base_text = e029.strings(inputs.read('base', 'localization/english/localization'))
    texts = {lang: e046.catalog(source) for lang, source in e046.LANGUAGES.items()}
    files, operations, entries, icons, report = {}, [], [], [], []
    for path in inputs.group('bundle'):
        if path.startswith('.records/'):
            continue
        files['files/' + path] = inputs.read('bundle', path)
        operations.append({'op': 'add_file', 'path': path})
    e046_rows = e046_entries(json.loads(inputs.read('equipment046', E046_MANIFEST)))
    fits = bundle_fits(inputs)
    occupied = set()
    for spec in sorted(rows, key=lambda r: int(r['id'])):
        ident = int(spec['id'])
        start = template(spec)
        if start.copy:
            template_row = e046_rows[start.ident]
            source = inputs.config('equipment046', 'files/' + template_row['cfg_name'])
            config = copy_piece(source, spec, template_row['base_cost'], fits, inputs)
        else:
            template_row = base_by_id[start.ident]
            source = inputs.config('base', template_row['cfg_name'])
            config = convert(source, spec, model_surface(inputs, spec['model']))
        path = config_path(spec, template_row['cfg_name'])
        if any(r['cfg_name'] == path for r in base + list(e046_rows.values())):
            raise ValueError(f'{path}: config path already taken')
        entries.append({'cfg_name': path, 'is_stack': False, 'name': f"eq069_{spec['id_069']}",
                        'item_category': template_row['item_category'], 'dict_id': ident,
                        'base_cost': int(spec['price']), 'is_premium': False})
        row, col = config['ui_desc']['icon']
        if (row, col) in occupied or (col <= base_atlas[0] // 171 and any(e029.cell(base_atlas, row, col)[3::4])):
            raise ValueError(f'occupied target icon cell {row},{col}')
        occupied.add((row, col))
        donor_row, donor_col = spec['icon_069'].split(',')
        icon_file = f'icons/{ident}.png'
        files[icon_file] = e046.icon(inputs.read('donor069', DONOR_ICONS.format(row=donor_row, col=donor_col)))
        icons.append({'row': row, 'col': col, 'source': icon_file})
        for key in config['ui_desc']['text_descriptions'].values():
            if key in base_text:
                raise ValueError(f'{key}: text key already in the released dictionaries')
        raw = bcw.dump(config)
        if bcw.dump(bcw.loads(raw)) != raw:
            raise ValueError(f'{ident}: config roundtrip failed')
        files['files/' + path] = raw
        operations.append({'op': 'add_file', 'path': path})
        report.append({'id': ident, 'name': spec['name'], 'level': int(spec['level']),
                       'faction': int(spec['faction']), 'weight': float(spec['weight']),
                       'price': int(spec['price']), 'drop_weight': int(spec['drop_weight']),
                       'default_modifications': config['default_modifications'],
                       'template': spec['template'], 'donor_item': spec['id_069']})
    if {r['name'] for r in base + list(e046_rows.values())} & {e['name'] for e in entries}:
        raise ValueError('dictionary name collision')
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    keys = sorted(k for spec in rows for k in text_keys(spec).values())
    for lang, table in texts.items():
        missing = [k for k in keys if k not in table]
        if missing:
            raise ValueError(f'{e046.LANGUAGES[lang]}.tsv lacks {missing[:3]}')
        operations.append({'op': 'localization_add', 'languages': [lang],
                           'strings': {k: e029.porter_text(table[k]) for k in keys}})
    operations.append({'op': 'atlas_icons_add', 'icons': icons})
    manifest = {'format_version': 1, 'id': PACK, 'version': VERSION, 'target_build': '0.26g',
                'operations': operations,
                'notes': ['0.69d armour (models by resource-porter build-armour-069d): two backpacks, the Snow-20 '
                          'set and the Delta mask, adapted to 0.26g by scripts/equipment-069/items.tsv.']}
    files['pack.json'] = (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode()
    files['balance.json'] = (json.dumps(report, ensure_ascii=False, indent=2) + '\n').encode()
    out.mkdir(parents=True, exist_ok=False)
    for name, raw in sorted(files.items()):
        dest = out / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(raw)
    print(f'{PACK}@{VERSION}: {len(entries)} items, {len(operations)} operations; pack.json {sha(files["pack.json"])}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    write_pack(Inputs(args.inputs), args.out)


if __name__ == '__main__':
    main()
