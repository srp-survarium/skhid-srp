#!/usr/bin/env python3
"""Build the four Cirno and Neco-Arc decals using Snufkin's stock 4_41 cloth.

Pass --stock the extracted 0.26g flag files and --out a new pack folder.
The RGBA cutouts are the pack's art sources. See README.md for credits.
"""
import argparse
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
CATALOG = HERE.parents[1] / 'client-patches' / 'localization'
spec = importlib.util.spec_from_file_location('cloth_decals', HERE.parent / 'snufkin-decals' / 'make_pack.py')
cloth = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cloth)

# Dictionary id, decal resource, atlas cell, cutout.
ITEMS = (
    (2672, '4_79', (1, 21), 'bottled-fumo.png'),
    (2673, '4_80', (1, 22), 'snail-fumo.png'),
    (2674, '4_81', (1, 23), 'just-fumo.png'),
    (2675, '4_82', (1, 24), 'neco-arc.png'),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    stock = cloth.kosovo.read_inputs(args.stock, cloth.STOCK, '--stock')
    names = {}
    for language in cloth.LANGUAGES:
        table = CATALOG / (('russian' if language == 'original' else language) + '.tsv')
        names[language] = dict(line.split('\t', 1) for line in table.read_text(encoding='utf-8').splitlines()
                               if line.startswith('st_character_decals_'))
    items, cutouts = [], {}
    for dict_id, rc, icon, filename in ITEMS:
        key = f'st_character_decals_{rc}_name'
        items.append({'dict_id': dict_id, 'rc': rc, 'icon': icon, 'cutout': filename,
                      'names': {lang: table[key] for lang, table in names.items()}})
        cutouts[filename] = (HERE / 'cutouts' / filename).read_bytes()
    cloth.build(args.out, None, cutouts, stock, items, 'fumo-neco-decals', '2',
                'Bottled fumo, Snail fumo, Just fumo fumo and Neco-Arc (2672–2675), give-only cloth decals',
                ['Artwork credits and sources: srp-026g/scripts/fumo-neco-decals/README.md'])


if __name__ == '__main__':
    main()
