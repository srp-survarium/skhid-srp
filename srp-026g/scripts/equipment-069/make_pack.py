#!/usr/bin/env python3
"""Build equipment-069 from the pinned inputs (prepare_inputs.py) and the reviewed items.tsv.

The models, materials and textures are resource-porter's (`vostok-port build-armour-069d`,
pinned as the `bundle` group); this script writes the items around them: a config from a
native back piece of the same kind (`template_id`: parameters, defect and modification masks,
repair, card layout, sounds), the dictionary rows, the catalog's texts and the 0.69d icons in
free equipment-atlas cells. The output is a resource-porter content pack, not a hand-assembled
patch; release/build-content composes it.
"""
import argparse
import copy
import json
from pathlib import Path

from common import (DICTIONARY, DONOR_ICONS, MODIFICATIONS, MOVIE, PACK, Inputs, bcw, equipment046, sha, specs,
                    text_keys)

VERSION = '1'
DATA_TYPE = 4  # the native equipment data type


def model_surface(inputs, model):
    """The one LOD0 surface of a bundle model (a back piece's part_name)."""
    settings = inputs.config('bundle', f'models/{model}.skinned_model/settings')
    surfaces = settings['lod_hierrarchy']['LOD0']['surfaces']
    if len(surfaces) != 1:
        raise ValueError(f'{model}: {len(surfaces)} LOD0 surfaces; a back piece draws one part')
    return surfaces[0]


def config_path(spec, template_row):
    folder = template_row['cfg_name'].rsplit('/', 1)[0]
    return f"{folder}/eq069_{spec['id_069']}.options"


def convert(template, spec, part):
    """The template's config with this piece's model, level, faction, weight, drop weight,
    built-in bonuses, card, icon and texts."""
    result = copy.deepcopy(template)
    params = result['parameters']
    if params['type'] != 'arm_back':
        raise ValueError(f"{spec['id']}: template {spec['template_id']} is no back piece")
    params.update(faction=int(spec['faction']), item_level=int(spec['level']),
                  match_making_level=int(spec['level']), base_rarity_level=0,
                  weight=float(spec['weight']), drop_weight=int(spec['drop_weight']))
    result['default_modifications'] = [int(v) for v in spec['default_modifications'].split(',') if v]
    result['data'] = {'part_name': part, 'base_model': spec['model'], 'type': DATA_TYPE}
    # A back piece's card shows its weight.
    result['ui_desc']['props_list'] = [{'prop_id': 15, 'prop_value': f"{params['weight']:.1f}"}]
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
    occupied = set()
    for spec in sorted(rows, key=lambda r: int(r['id'])):
        ident = int(spec['id'])
        template_row = base_by_id[int(spec['template_id'])]
        template = inputs.config('base', template_row['cfg_name'])
        config = convert(template, spec, model_surface(inputs, spec['model']))
        path = config_path(spec, template_row)
        if any(r['cfg_name'] == path for r in base):
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
                       'template_id': int(spec['template_id']), 'donor_item': spec['id_069']})
    if {r['name'] for r in base} & {e['name'] for e in entries}:
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
                'notes': ['0.69d backpacks (models by resource-porter build-armour-069d), adapted to 0.26g by '
                          'scripts/equipment-069/items.tsv.']}
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
