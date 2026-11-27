#!/usr/bin/env python3
"""make_pack.py: the content pack of the 1,000 number decals, "Number 1" to "Number 1000", each
given to the account of that id (docs/notes/number-decals.md).

usage: python3 -I make_pack.py --sources <folder> --stock <folder> --out <packs folder>/number-decals-<version>
       [--preview <folder>]
       (needs numpy, scipy and Pillow, and ImageMagick:
        nix-shell -p 'python3.withPackages(p: [p.numpy p.scipy p.pillow])' imagemagick)

--sources  the owner's 1,000 pictures, "Без имени-1_Набор данных <N>.png" (sources.tsv: their
           sha256; kept in private/number-decals, not in the repository: 25 MB).
--stock    0.26g's flag char_decal_4_41: its config, textures and their .options, as dbgrab
           writes them from the stock resources.db (snufkin-decals' STOCK).
--out      the pack's folder (a new one): pack.json, files/ and icons/.
--preview  also writes a contact sheet of a sample of the diffuse textures there.

What the pack holds, for each number N (names.py: the ids, names and rows):
- the item config char_decal_num_<NNNN> from 0.26g's flag char_decal_4_41 (prm_decl, level 4, no
  drop), its modification id 210000 + N, texture, name key and icon cell changed;
- the diffuse texture, DXT5 256x256 with 7 mips: the white stencil digits, every number cut out by
  the same window: the height of all of them, the width of the widest ("1000") and a margin, so
  every number has the same size and baseline and sits in the middle; the colour white everywhere
  (no fringe under the transparent part);
- the normal map and specular 16x16 (3 mips): 4_41's cloth, one value each (its weave's median,
  its specular's median where its flag is); 4_41's 16x16 fresnel map and the four .options as
  they are (one .options holds no size);
- the dictionary row 5000 + N (no price, premium: content-edit/items.tsv; shop.tsv says no shop
  sells it), the name in the pack's six languages from client-patches/localization, and one icon
  for all of them, a "№" (icon.png) at [1, 25] of the decal atlas.
"""
import argparse
import concurrent.futures
import importlib.util
import io
import json
import os
from pathlib import Path
import struct
import sys

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import names as num  # noqa: E402


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cloth = load('cloth_decals', HERE.parent / 'snufkin-decals' / 'make_pack.py')
kosovo, decals_069d = cloth.kosovo, cloth.decals_069d

PACK_ID = 'number-decals'
VERSION = '1'
T = cloth.T
STOCK = cloth.STOCK
SOURCE_NAME = 'Без имени-1_Набор данных {}.png'
SIZE = 256              # the diffuse texture
MIPS = 7                # 256 down to 4
SMALL = 16              # the normal map and specular, as 0.26g's 16x16 fresnel maps
SMALL_MIPS = 3
MARGIN = 8              # pixels of the texture beside the widest number
ICON_CELL = (1, 25)
ICON = HERE / 'icon.png'
ICON_SHA256 = '045f5442a98b6061a432c30862ef4db2dc744cff3ddd2f32c9876a52e2ba9e13'


def read_sources(folder):
    """{N: PNG bytes}, each checked against sources.tsv."""
    want = {}
    for line in (HERE / 'sources.tsv').read_text(encoding='utf-8').splitlines():
        if line and not line.startswith('#'):
            number, sha = line.split('\t')
            want[int(number)] = sha
    if sorted(want) != list(num.NUMBERS):
        sys.exit('sources.tsv: not the numbers 1 to 1000')
    out = {}
    for number, sha in want.items():
        path = Path(folder) / SOURCE_NAME.format(number)
        if not path.is_file():
            sys.exit(f'--sources: no {path.name} in {folder}')
        data = path.read_bytes()
        if kosovo.sha256(data) != sha:
            sys.exit(f'--sources: {path.name} is not the picture this pack is made of (sha256 {sha})')
        out[number] = data
    return out


def coverage(data):
    """The picture's alpha (0..255), its metadata (Photoshop's XMP) dropped: only the pixels are
    read."""
    with Image.open(io.BytesIO(data)) as image:
        rgba = np.asarray(image.convert('RGBA'))
    if rgba[..., :3][rgba[..., 3] > 0].min() < 250:
        raise ValueError('a picture with colour: the digits are white')
    return rgba[..., 3]


def extent(data):
    """(top, bottom, left, right) of the digits in the picture."""
    alpha = coverage(data)
    rows, cols = np.nonzero(alpha.max(axis=1))[0], np.nonzero(alpha.max(axis=0))[0]
    return int(rows[0]), int(rows[-1]) + 1, int(cols[0]), int(cols[-1]) + 1


def window(extents):
    """The cut every number shares, in the pictures' pixels: (the rows' top and bottom of all of
    them, the scale that fits the widest number in the texture)."""
    top = min(e[0] for e in extents)
    bottom = max(e[1] for e in extents)
    widest = max(e[3] - e[2] for e in extents)
    scale = (SIZE - 2 * MARGIN) / widest
    if (bottom - top) * scale > SIZE - 2 * MARGIN:
        raise ValueError('the numbers are taller than they are wide')
    return top, bottom, scale


def diffuse(alpha, top, bottom, scale):
    """The number in the texture: its own middle in the middle across, the shared rows' middle
    in the middle down (one baseline), every number at the same scale."""
    cols = np.nonzero(alpha.max(axis=0))[0]
    middle_x = (cols[0] + cols[-1] + 1) / 2
    middle_y = (top + bottom) / 2
    side = SIZE / scale
    pad = int(np.ceil(side))
    padded = np.pad(alpha, pad)
    box = (middle_x - side / 2 + pad, middle_y - side / 2 + pad,
           middle_x + side / 2 + pad, middle_y + side / 2 + pad)
    image = Image.fromarray(padded, 'L')
    cut = image.resize((SIZE, SIZE), Image.Resampling.LANCZOS, box=box)
    out = np.full((SIZE, SIZE, 4), 255.0)
    out[..., 3] = np.asarray(cut, dtype=np.float64)
    return out


def dds_like(like, size, mips):
    """A Dds of `like`'s format and header at another size: the header's size, mip count and
    linear size (the data's bytes + 1, as the game's are), and its sRGB byte."""
    block = {b'DXT1': 8, b'DXT5': 16}[like.fourcc]
    body = sum((-(-max(1, size >> m) // 4)) ** 2 * block for m in range(mips))
    header = bytearray(like.data[:128])
    struct.pack_into('<III', header, 12, size, size, body + 1)
    struct.pack_into('<I', header, 28, mips)
    return kosovo.Dds(bytes(header) + bytes(body) + bytes([like.srgb]))


def uniform(value, like):
    channels = len(value)
    return kosovo.encoded(np.broadcast_to(np.asarray(value, dtype=np.float64), (SMALL, SMALL, channels)).copy(), like)


def encode_one(args):
    number, data, top, bottom, scale, like = args
    pixels = diffuse(coverage(data), top, bottom, scale)
    return number, pixels.astype(np.uint8), kosovo.encoded(pixels, kosovo.Dds(like))


def contact_sheet(made, path):
    """A sample of the diffuse textures on an olive cloth, the texture's edge drawn, with their
    numbers' order: 1-12, 42, 99-101, 111, 404, 777, 888, 999, 1000."""
    sample = list(range(1, 13)) + [42, 99, 100, 101, 111, 404, 777, 888, 999, 1000]
    cols, cell = 6, SIZE + 8
    rows = -(-len(sample) // cols)
    sheet = Image.new('RGB', (cols * cell, rows * cell), (24, 24, 24))
    for k, number in enumerate(sample):
        tile = Image.new('RGBA', (SIZE, SIZE), (78, 86, 60, 255))
        tile.alpha_composite(Image.fromarray(made[number], 'RGBA'))
        sheet.paste(tile.convert('RGB'), ((k % cols) * cell + 4, (k // cols) * cell + 4))
    sheet.save(path, format='PNG')


def build(out, preview, sources, stock, icon, names):
    if out.exists():
        sys.exit(f'{out}: already there; the pack goes in a new folder')
    like = {role: kosovo.Dds(stock[T + '4_41' + role + '.dds']) for role in ('', '_nmap', '_smap', '_fmap')}
    big = dds_like(like[''], SIZE, MIPS)
    small = {role: dds_like(like[role], SMALL, SMALL_MIPS) for role in ('_nmap', '_smap')}
    weave = cloth.cloth(like['_nmap'].pixels('RGBA'))
    nmap = uniform(np.median(weave.reshape(-1, 4), axis=0), small['_nmap'])
    smap = uniform(cloth.specular(like[''].pixels('RGBA'), like['_smap'].pixels('RGB'))[0, 0], small['_smap'])

    with concurrent.futures.ProcessPoolExecutor(max_workers=min(8, os.cpu_count() or 1)) as pool:
        top, bottom, scale = window(list(pool.map(extent, (sources[n] for n in num.NUMBERS), chunksize=8)))
        jobs = [(n, sources[n], top, bottom, scale, big.data) for n in num.NUMBERS]
        encoded = {n: (pixels, dds) for n, pixels, dds in pool.map(encode_one, jobs, chunksize=8)}

    files, items = {}, []
    for n in num.NUMBERS:
        rc = num.rc(n)
        item = {'kind': 'flag', 'rc': rc, 'dict_id': num.dict_id(n), 'name': num.item_name(n),
                'config': decals_069d.DECAL_CONFIGS + f'char_decal_{rc}.options',
                'modification': num.FIRST_MODIFICATION + n, 'texture': 'character/human/decal/' + rc,
                'key': num.name_key(n), 'icon': ICON_CELL,
                'names': {language: names[language][num.name_key(n)] for language in cloth.LANGUAGES}}
        items.append(item)
        files[item['config']] = decals_069d.item_config(stock[decals_069d.DECAL_CONFIGS + 'char_decal_4_41.options'], item)
        made = {'': encoded[n][1], '_nmap': nmap, '_smap': smap, '_fmap': stock[T + '4_41_fmap.dds']}
        for role in cloth.ROLES:
            files[T + rc + role + '.dds'] = made[role]
            files[T + rc + role + '.options'] = stock[T + '4_41' + role + '.options']
    source = 'icons/LobbyMenu_DecalsIcons/r{:02}c{:02}.png'.format(*ICON_CELL)
    files[source] = icon

    operations = [{'op': 'add_file', 'path': path} for path in files if not path.startswith('icons/')]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        {'base_cost': 0, 'cfg_name': item['config'], 'dict_id': item['dict_id'], 'is_premium': True,
         'is_stack': False, 'item_category': 26, 'name': item['name']} for item in items]})
    for language in cloth.LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {item['key']: decals_069d.porter_text(item['names'][language]) for item in items}})
    operations.append({'op': 'atlas_icons_add', 'swf': cloth.ATLAS, 'symbol': 'LobbyMenu_DecalsIcons',
                       'icons': [{'row': ICON_CELL[0], 'col': ICON_CELL[1], 'source': source}]})
    first, last = items[0], items[-1]
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': (f'Number 1 to Number 1000 ({first["dict_id"]}-{last["dict_id"]}), give-only stencil '
                        'decals, each given to the account of that id (srp-026g/docs/notes/number-decals.md)'),
        'operations': operations,
        'server': {'items': [{'dict_id': item['dict_id']} for item in items]},
        'notes': [
            'made by srp-026g/scripts/number-decals/make_pack.py from the owner\'s 1,000 pictures '
            '(sources.tsv) and 0.26g\'s flag 4_41 (its config, cloth, specular, fresnel map and .options)',
            f'dictionary ids {first["dict_id"]}-{last["dict_id"]}, modification ids '
            f'{first["modification"]}-{last["modification"]}, one icon at [{ICON_CELL[0]}, {ICON_CELL[1]}]',
        ],
    }
    for path, data in files.items():
        target = out / (path if path.startswith('icons/') else 'files/' + path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    if preview:
        preview.mkdir(parents=True, exist_ok=True)
        contact_sheet({n: pixels for n, (pixels, _) in encoded.items()}, preview / 'contact-sheet.png')
    digest = kosovo.sha256((out / 'pack.json').read_bytes())
    total = sum(len(data) for data in files.values())
    print(f'{out}: {len(files) - 1} files, 1 icon, {total} bytes, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--sources', required=True, type=Path)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--preview', type=Path)
    args = parser.parse_args()
    sources = read_sources(args.sources)
    stock = kosovo.read_inputs(args.stock, STOCK, '--stock')
    icon = ICON.read_bytes()
    if kosovo.sha256(icon) != ICON_SHA256:
        sys.exit(f'{ICON}: not the icon this pack is made with (make_icon.py)')
    names = {}
    for language in cloth.LANGUAGES:
        table = num.LOCALIZATION / (('russian' if language == 'original' else language) + '.tsv')
        names[language] = dict(line.split('\t', 1) for line in table.read_text(encoding='utf-8').splitlines()
                               if line.startswith('st_character_decals_num_'))
    build(args.out, args.preview, sources, stock, icon, names)


if __name__ == '__main__':
    main()
