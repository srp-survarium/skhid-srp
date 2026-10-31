#!/usr/bin/env python3
"""make_pack.py: the content pack of the corrected Flag of Serbia decal, as 0.69d has it (2026-10-29;
../../docs/notes/serbia-flag.md).

usage: python3 -I make_pack.py --donor <folder> --out <packs folder>/serbia-flag-<version>

--donor  0.69d's files at their game paths (`resources-db extract <0.69d resources.NN.db> <folder>`
         of tools/vostok-resources-db; the textures are in resources.04.db, the icon in
         resources.43.db): every path of DONOR.
--out    the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and
         the manifest's `pack` row (release/content.tsv).

The stock decal `char_decal_4_34` ("Flag Of Serbia", texture character/human/decal/4_34, lobby icon
[4, 34] of LobbyMenu_DecalsIcons) is blue over white over red: the flag of Yugoslavia (and of Serbia
and Montenegro), not Serbia's. 0.69d fixed it: its 4_34 is Serbia's state flag, red over blue over
white with the coat of arms, on the same worn frame.

- Textures: 0.69d's diffuse, normal and specular maps as they are, over 0.26g's (the same formats:
  DXT5, DXT5, DXT1, 512x512, 8 mips). The fmap is the same file in both; 0.26g's .options stay
  (0.69d's hold the same values in its newer format).
- Icon: 0.69d's (128x128) scaled to the atlas's 82x82 cells as decals-069d's are (Pillow's
  Lanczos, alpha premultiplied while scaling), drawn over the stock icon in its own cell
  (`replaces_sha256`: the porter checks that the cell holds the stock icon), so the item's config
  stays as it is.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import sys

PACK_ID = 'serbia-flag'
VERSION = '1'
DECAL = 'textures/character/human/decal/4_34'
ICON_069 = 'flash_movies/textures/ui_icons/items/decals/decal_4_34.png'
# 0.69d's files: sha256
DONOR = {
    DECAL + '.dds': '47f86746b0eaf356664c2b984895b6a1d6ab1262f38fa3e41c5940d430acc695',
    DECAL + '_nmap.dds': '524398c3bfc1507d208fce2bd6b713d0eade8f27e0bbeecd1757affe844ace85',
    DECAL + '_smap.dds': '3d494e0860001e2b66fe088f922c2f7213177b152253786a244bfdef3ad84a7d',
    ICON_069: '40aceef1814d983012cdc4f227616f8ed64f6408da55c53cec6b64c014391f13',
}
# the stock 0.26g textures they replace: sha256
STOCK = {
    DECAL + '.dds': 'd28572bc9504b3489dc316891d03ffcd1ffcc6a5b0edf440ac7d27ed770ca90a',
    DECAL + '_nmap.dds': '9de09ce227b019b51d3f5c558be87191cdf0155330e3b4907bd078fc395752f3',
    DECAL + '_smap.dds': '983b9f4ae95eb98c83723b4ee6d765a8ab9ba0633fcbf9ac7542d2ea5285610f',
}
MOVIE = 'flash_movies/lobby_menu.swf'
SYMBOL = 'LobbyMenu_DecalsIcons'
CELL = 82                       # the decal atlas's square cells
ICON = (4, 34)                  # ui_desc/icon of char_decal_4_34: [row, col], 1-based
ICON_SHA256 = 'ada4fcf32ca6df220036c78d17b4423f794dafbd26661ec05b92c10fc219f95d'   # the stock cell's ARGB
ICON_SOURCE = 'icons/LobbyMenu_DecalsIcons/r04c34.png'


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def icon_png(data):
    """0.69d's icon at CELL x CELL, as decals-069d's make_pack.py scales its icons."""
    from PIL import Image
    image = Image.open(io.BytesIO(data)).convert('RGBA')
    assert image.width == image.height, f'0.69d icon is {image.width}x{image.height}, not square'
    out = io.BytesIO()
    image.resize((CELL, CELL), Image.Resampling.LANCZOS).save(out, format='PNG', optimize=False)
    return out.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: exists; the pack goes in a new folder')

    donor = {}
    for path, sha in DONOR.items():
        data = (args.donor / path).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'--donor: {path} is not 0.69d\'s (sha256 {sha})')
        donor[path] = data
    files = {'files/' + path: donor[path] for path in STOCK}
    files[ICON_SOURCE] = icon_png(donor[ICON_069])
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Flag of Serbia decal (char_decal_4_34) as 0.69d fixed it: Serbia\'s state flag, red '
                       'over blue over white with the coat of arms (stock: blue over white over red, the flag '
                       'of Yugoslavia); 0.69d\'s textures and lobby icon',
        'operations': [{'op': 'replace_file', 'path': path, 'base_sha256': sha} for path, sha in STOCK.items()] + [
            {'op': 'atlas_icons_add', 'swf': MOVIE, 'symbol': SYMBOL,
             'icons': [{'row': ICON[0], 'col': ICON[1], 'source': ICON_SOURCE,
                        'replaces_sha256': ICON_SHA256}]},
        ],
        'notes': ['made by srp-026g/scripts/serbia-flag/make_pack.py from 0.69d\'s files'],
    }
    for path, data in files.items():
        target = args.out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(STOCK)} textures and the icon, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
