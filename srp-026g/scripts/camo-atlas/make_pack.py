#!/usr/bin/env python3
"""Build the camo-atlas resource-porter pack from its artwork and the retail 0.26g archive.

python3 make_pack.py --db /path/to/resources.db --reader /path/to/srp-game-data --out /path/to/camo-atlas-3
Requires Pillow, NumPy and ImageMagick. Catalog order owns the permanent item IDs and icon cells.
"""
import argparse
import csv
import hashlib
import importlib.util
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit
import icons
import pbr_026

# Reuse the config writer that checks every changed leaf against the New Year 2015 item.
spec = importlib.util.spec_from_file_location('decal_pack', HERE.parent / 'decals-069d/make_pack.py')
decals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(decals)

FIRST_ITEM = 2800
FIRST_MOD = 130100
PRICE = 1
CLOTH = 'textures/character/human/camo/renesance/khaki_fabric_01'
DIFFUSE = decals.CAMO_TEXTURES + 'cyfra_01_camo.dds'
STOCK = {
    decals.TEMPLATES['camo']['config']: decals.STOCK[decals.TEMPLATES['camo']['config']],
    **{p: h for p, h in decals.STOCK.items() if p.startswith(decals.CAMO_TEXTURES)},
    DIFFUSE: '10bcdeef3de29610079f6178e2f09954fcc8145f91166c5d4654e57cc346de19',
    CLOTH + '_nmap.dds': '91b107d875725c8311c171d7eecb78ea8f35c1f2a2d09af27a49d7f5510655ab',
    CLOTH + '_smap.dds': '0c4676b3dcef0a11dad4ddb3b5115081c3b08c4c957c4e61f23fd225595fdffe',
}


def catalog():
    seen = set()
    for folder in (HERE / 'initial', HERE / 'art'):
        with (folder / 'catalog.tsv').open(encoding='utf-8', newline='') as stream:
            for row in csv.DictReader(stream, delimiter='\t'):
                slug = row['slug']
                if slug in seen or not re.fullmatch(r'[a-z][a-z0-9]*(?:-[a-z0-9]+)*', slug):
                    raise ValueError(f'invalid or duplicate slug {slug!r}')
                seen.add(slug)
                yield row, folder / (slug + '.webp')


def build(stock, target):
    ops, entries, names, images = [], [], {}, []
    like = pbr_026.Dds(stock[DIFFUSE])

    def add(path, data):
        dest = target / 'files' / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        ops.append({'op': 'add_file', 'path': path})

    for index, (row, path) in enumerate(catalog()):
        name = 'camo_atlas_' + row['slug'].replace('-', '_')
        key = 'st_' + name
        texture = 'character/human/camo/atlas/' + name
        config = decals.CAMO_CONFIGS + name + '.options'
        item = dict(name=name, kind='camo', key=key, texture=texture,
                    modification=FIRST_MOD + index, icon=icons.icon_cell(index))
        options = bytearray(decals.item_config(stock[decals.TEMPLATES['camo']['config']], item))
        # Stock drop camouflages each have weight 160 within the camouflage category.
        content_edit.set_string(options, 'parameters/type', 'prm_camo', 'drp_camo')
        content_edit.set_number(options, 'parameters/drop_weight', 0, 160)
        add(config, options)
        with Image.open(path) as image:
            if image.width != image.height or image.width < like.width:
                raise ValueError(f'{path}: expected square artwork at least {like.width}px')
            pixels = np.asarray(image.convert('RGB').resize((like.width, like.height), Image.Resampling.LANCZOS))
        for role in decals.CAMO_ROLES:
            base = 'textures/' + texture + role
            add(base + '.options', stock[decals.CAMO_TEXTURES + 'new_year' + role + '.options'])
            add(base + '.dds', stock[CLOTH + role + '.dds'] if role else pbr_026.encoded(pixels, like))
        entries.append(dict(dict_id=FIRST_ITEM + index, name=name, cfg_name=config, item_category=25,
                            is_premium=True, is_stack=False, base_cost=PRICE))
        names[key] = decals.porter_text(row['name'])
        images.append(path)
    if not entries:
        raise ValueError('empty catalog')
    ops.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    ops.append({'op': 'localization_add', 'languages': list(decals.LANGUAGES), 'strings': names})
    ops.extend(icons.icon_operations(target, images))
    manifest = dict(format_version=1, id='camo-atlas', version='3', target_build='0.26g',
                    description='Camouflage Atlas appearance items for armour and weapons',
                    operations=ops, server={'items': [{'dict_id': e['dict_id']} for e in entries]})
    manifest_bytes = (json.dumps(manifest, indent=2, ensure_ascii=False) + '\n').encode()
    (target / 'pack.json').write_bytes(manifest_bytes)
    print(f'{len(entries)} camouflages, pack.json sha256 {hashlib.sha256(manifest_bytes).hexdigest()}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--db', required=True, type=Path)
    parser.add_argument('--reader', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        parser.error(f'{args.out}: output already exists')
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.out.parent) as scratch:
        root = Path(scratch)
        subprocess.run([str(args.reader.resolve()), 'get', '--db', str(args.db.resolve()),
                        '--out', str(root / 'stock'), *STOCK], check=True)
        stock = decals.read_inputs(root / 'stock', STOCK, 'retail archive')
        result = root / 'pack'
        build(stock, result)
        result.rename(args.out)


if __name__ == '__main__':
    main()
