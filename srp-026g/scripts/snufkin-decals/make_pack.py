#!/usr/bin/env python3
"""make_pack.py: the content pack of two decals, Snufkin and Snufkin sitting, from frames of the 1990
Moomin anime (../../docs/notes/snufkin-decals.md).

usage: python3 -I make_pack.py --cutouts <folder> --stock <folder> --out <packs folder>/snufkin-decals-<version>
       [--preview <folder>]
       (needs numpy, scipy and Pillow, and ImageMagick:
        nix-shell -p 'python3.withPackages(p: [p.numpy p.scipy p.pillow])' imagemagick)

--cutouts  Snufkin cut out of the two frames (CUTOUTS below: RGBA PNGs, the frames' colours under BiRefNet's alpha; private/snufkin-decals).
--stock    0.26g's flag char_decal_4_41: its config, textures and their .options (STOCK), as dbgrab
           writes them from the stock resources.db.
--out      the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and the
           manifest's `pack` row (release/content.tsv).
--preview  also writes the diffuse, normal and specular maps of both decals there as PNGs.

What the pack holds:
- char_decal_4_77 "Snufkin" and char_decal_4_78 "Snufkin sitting": the frames printed on cloth as
  0.26g's flags are (4_41): the picture fitted in the texture, its edge colours carried out under the
  transparent part (no background colour bleeds in through filtering or mips), the cloth's weave in
  the normal map, 4_41's dull specular;
- their item configs from char_decal_4_41 (prm_decl, level 4, no drop): the modification id,
  texture, name key and icon cell changed;
- the textures in 4_41's formats (DXT5 diffuse and normal map, DXT1 specular, 512x512, 8 mips, its
  headers), 4_41's fresnel map and .options;
- the dictionary rows (no price: never bought, sold back for nothing; content-edit/shop.tsv says
  no shop sells them), the names in the six languages, the icons at [1, 19] and [1, 20].
"""
import argparse
import io
import importlib.util
import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


decals_069d = load('decals_069d', HERE.parent / 'decals-069d' / 'make_pack.py')
kosovo = load('kosovo_decals', HERE.parent / 'kosovo-decals' / 'make_pack.py')

PACK_ID = 'snufkin-decals'
VERSION = '1'
LANGUAGES = decals_069d.LANGUAGES
DECAL_CONFIGS = decals_069d.DECAL_CONFIGS
T = decals_069d.DECAL_TEXTURES
ATLAS = decals_069d.ATLAS
CELL = decals_069d.DECAL_ICON
ROLES = decals_069d.DECAL_ROLES          # '', '_fmap', '_nmap', '_smap'

CUTOUTS = {
    'snufkin_1.png': 'a8e2671594aee7acbdb1e36ecb6fe885b30bcb4b6759ed375b3f4c13055b7870',
    'snufkin_2.png': '4f0892f6c47a81aebc56a67929881f1c5659616a6a232126dcb4b9199f4b167f',
}
STOCK = {
    DECAL_CONFIGS + 'char_decal_4_41.options': '3d73d0d0ea24c2908a4b01ea81352645c0e5768f4cef9907c414ed07303f0156',
    T + '4_41.dds': 'aa43c630f5548f65b62190b2c293c229de5dae34e16cf8e2ccc63961596671fd',
    T + '4_41_nmap.dds': '1b11381f0898809fbb11643d1dafa387c34a7f2ec9fb56f90ca809d2b78a2e85',
    T + '4_41_smap.dds': '34833ce80ac106a1509a309ac890fc2dcff161478cd2718b990e5c127c519b5d',
    T + '4_41_fmap.dds': 'ed372a7668db0b97970d2c3e0cbe5340a77dedd36121709608a3c6af20f94a9c',
    T + '4_41.options': 'df379225eac9eec632cf33389c4f62250ec2314f76b002cca18c4170297ceb90',
    T + '4_41_fmap.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
    T + '4_41_nmap.options': '7861fedb73812d0047394f32cf5aee9543390f5d2e9a3d7426c60ee13500ab71',
    T + '4_41_smap.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
}

ITEMS = [
    {'dict_id': 1896, 'rc': '4_77', 'cutout': 'snufkin_1.png', 'icon': (1, 19),
     'names': {'english': 'Snufkin', 'german': 'Schnupferich', 'original': 'Снусмумрик',
               'polish': 'Włóczykij', 'russian': 'Снусмумрик', 'ukrainian': 'Нюхмумрик'}},
    {'dict_id': 1897, 'rc': '4_78', 'cutout': 'snufkin_2.png', 'icon': (1, 20),
     'names': {'english': 'Snufkin sitting', 'german': 'Schnupferich sitzend', 'original': 'Сидящий Снусмумрик',
               'polish': 'Siedzący Włóczykij', 'russian': 'Сидящий Снусмумрик', 'ukrainian': 'Нюхмумрик, що сидить'}},
]
SIZE = 512
MARGIN = 16             # pixels of cloth around the picture's box
CLOTH_ROWS = (2, 82)    # 4_41's normal map above its flag: the bare cloth's weave


def picture(cutout):
    """The cutout fitted in the texture (RGBA, 0..255 floats): scaled to the box inside MARGIN,
    centred; its alpha's soft fringe (the frame's background mixed in) narrowed, and every colour
    at the edge and outside it the nearest solid colour of the picture."""
    image = Image.open(io.BytesIO(cutout)).convert('RGBA')
    box = SIZE - 2 * MARGIN
    scale = min(box / image.width, box / image.height)
    w, h = round(image.width * scale), round(image.height * scale)
    image = image.resize((w, h), Image.Resampling.LANCZOS)
    canvas = np.zeros((SIZE, SIZE, 4))
    x, y = (SIZE - w) // 2, (SIZE - h) // 2
    canvas[y:y + h, x:x + w] = np.asarray(image, dtype=np.float64)
    alpha = np.clip((canvas[..., 3] - 64) * 255 / (230 - 64), 0, 255)
    solid = alpha >= 255
    _, (iy, ix) = ndimage.distance_transform_edt(~solid, return_indices=True)
    rgb = canvas[..., :3][iy, ix]
    return np.dstack([rgb, alpha])


def cloth(nmap):
    """A normal map of 4_41's bare weave over the whole texture: its strip above the flag, every
    other copy upside down (its y, green, turned) so no seam shows."""
    strip = nmap[CLOTH_ROWS[0]:CLOTH_ROWS[1]]
    flipped = strip[::-1].copy()
    flipped[..., 1] = 255 - flipped[..., 1]
    rows, k = [], 0
    while sum(len(r) for r in rows) < SIZE:
        rows.append(strip if k % 2 == 0 else flipped)
        k += 1
    return np.concatenate(rows)[:SIZE]


def specular(diffuse_4_41, smap_4_41):
    """4_41's specular of its cloth (the median where its flag is), everywhere."""
    on = diffuse_4_41[..., 3] > 200
    value = np.median(smap_4_41[on], axis=0)
    return np.broadcast_to(value, (SIZE, SIZE, 3)).copy()


def icon(diffuse):
    """The lobby icon: the picture's box, as tall as the cell, centred."""
    image = Image.fromarray(np.clip(np.round(diffuse), 0, 255).astype(np.uint8), 'RGBA')
    image = image.crop(image.getchannel('A').point(lambda a: 255 if a > 128 else 0).getbbox())
    w = max(1, round(image.width * CELL / image.height))
    image = image.resize((w, CELL), Image.Resampling.LANCZOS)
    square = Image.new('RGBA', (CELL, CELL), (0, 0, 0, 0))
    square.paste(image, ((CELL - w) // 2, 0))
    return square


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cutouts', required=True, type=Path)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--preview', type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there; the pack goes in a new folder')
    cutouts = kosovo.read_inputs(args.cutouts, CUTOUTS, '--cutouts')
    stock = kosovo.read_inputs(args.stock, STOCK, '--stock')
    like = {role: kosovo.Dds(stock[T + '4_41' + role + '.dds']) for role in ('', '_nmap', '_smap')}
    nmap = cloth(like['_nmap'].pixels('RGBA'))
    smap = specular(like[''].pixels('RGBA'), like['_smap'].pixels('RGB'))

    files, icons, made = {}, [], {}
    for item in ITEMS:
        rc = item['rc']
        item.update(kind='flag', name='decals_' + rc, config=DECAL_CONFIGS + f'char_decal_{rc}.options',
                    modification=200000 + 100 * int(rc.split('_')[0]) + int(rc.split('_')[1]),
                    texture='character/human/decal/' + rc, key=f'st_character_decals_{rc}_name')
        diffuse = picture(cutouts[item['cutout']])
        made[rc] = {'': diffuse, '_nmap': nmap, '_smap': smap}
        files[item['config']] = decals_069d.item_config(stock[DECAL_CONFIGS + 'char_decal_4_41.options'], item)
        for role in ROLES:
            stock_texture = stock[T + '4_41' + role + '.dds']
            files[T + rc + role + '.dds'] = stock_texture if role == '_fmap' else kosovo.encoded(made[rc][role], like[role])
            files[T + rc + role + '.options'] = stock[T + '4_41' + role + '.options']
        row, col = item['icon']
        source = f'icons/LobbyMenu_DecalsIcons/r{row:02}c{col:02}.png'
        files[source] = kosovo.png(icon(diffuse))
        icons.append({'row': row, 'col': col, 'source': source})

    operations = [{'op': 'add_file', 'path': path} for path in files if not path.startswith('icons/')]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        {'base_cost': 0, 'cfg_name': item['config'], 'dict_id': item['dict_id'], 'is_premium': True,
         'is_stack': False, 'item_category': 26, 'name': item['name']} for item in ITEMS]})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {item['key']: decals_069d.porter_text(item['names'][language]) for item in ITEMS}})
    operations.append({'op': 'atlas_icons_add', 'swf': ATLAS, 'symbol': 'LobbyMenu_DecalsIcons', 'icons': icons})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'Snufkin and Snufkin sitting (1896, 1897), from frames of the 1990 Moomin anime, printed '
                       'on cloth as 0.26g\'s flags; not sold (srp-026g/docs/notes/snufkin-decals.md)',
        'operations': operations,
        'server': {'items': [{'dict_id': item['dict_id']} for item in ITEMS]},
        'notes': [
            'made by srp-026g/scripts/snufkin-decals/make_pack.py from the cutouts of the two frames and 0.26g\'s '
            'flag 4_41 (its cloth, specular, fresnel map and .options)',
            'item configs: 0.26g char_decal_4_41 with the modification id, texture, icon cell and name key',
        ] + [f'{item["dict_id"]} {item["name"]}: {item["names"]["english"]}' for item in ITEMS],
    }
    for path, data in files.items():
        target = args.out / (path if path.startswith('icons/') else 'files/' + path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    if args.preview:
        args.preview.mkdir(parents=True, exist_ok=True)
        for rc, maps in made.items():
            for role, pixels in maps.items():
                view = pixels[..., 3] if role == '_nmap' else pixels   # the normal map's x
                mode = 'L' if view.ndim == 2 else ('RGBA' if view.shape[2] == 4 else 'RGB')
                Image.fromarray(np.clip(np.round(view), 0, 255).astype(np.uint8), mode).save(args.preview / f'{rc}{role}.png')
    digest = kosovo.sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files) - len(icons)} files, {len(icons)} icons, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
