#!/usr/bin/env python3
"""make_pack.py: the content pack of two decals, the Flag of Kosovo and the Coat of arms of Kosovo, made
from the Government of Kosovo's official files in 0.69d's styles (../../docs/notes/kosovo-decals.md).

usage: python3 -I make_pack.py --official <folder> --donor <folder> --stock <folder> --out <packs folder>/kosovo-decals-<version>
       [--preview <folder>]
       (needs numpy, scipy and Pillow, pdftocairo and ImageMagick:
        nix-shell -p 'python3.withPackages(p: [p.numpy p.scipy p.pillow])' poppler-utils imagemagick)

--official  the two PDFs of the Office of the Prime Minister (OFFICIAL below: the flag and the coat of
            arms, CorelDRAW vectors of 2008, CMYK).
--donor     0.69d's files at their game paths or flattened (DONOR below; `vostok-port extract` from the
            volumes 069d-vs-026-items/work/latest.tsv names): Albania's, Chile's and Vietnam's flags,
            Bosnia's coat of arms.
--stock     0.26g's templates (STOCK: the flag char_decal_4_41's config and its textures' .options), as
            decals-069d takes them.
--out       the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and the
            manifest's `pack` row (release/content.tsv).
--preview   also writes the four maps of both decals there as PNGs.

What the pack holds (textures.py makes the textures):
- char_decal_4_76 "Flag of Kosovo": an embroidered patch as 0.69d's flags (Albania's patch, the
  threads of Chile's and Vietnam's); char_decal_5_43 "Coat of arms of Kosovo": a worn painted shield
  as 0.69d's coats of arms (Bosnia's wear). The next free names after 0.69d's 4_75 and 5_42.
- their item configs from 0.26g's flag char_decal_4_41 (prm_decl, level 4, no drop), as
  decals-069d's flags: the modification id, texture, name key and icon cell changed;
- the textures: DXT5 diffuse and normal map, DXT1 specular, each 512x512 with 8 mips and the header
  of its donor's; the fresnel map is the donor's; the .options are 0.26g's 4_41's;
- the dictionary rows, the names in the six languages, and the icons in the decal atlas: the flag at
  [4, 42], beside 0.26g's flags, the coat of arms at [3, 26] (row 5 of the coats of arms is full).
"""
import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402
import textures  # noqa: E402

_spec = importlib.util.spec_from_file_location('decals_069d', HERE.parent / 'decals-069d' / 'make_pack.py')
decals_069d = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(decals_069d)

PACK_ID = 'kosovo-decals'
VERSION = '1'
LANGUAGES = decals_069d.LANGUAGES
DECAL_CONFIGS = decals_069d.DECAL_CONFIGS
DECAL_TEXTURES = decals_069d.DECAL_TEXTURES
ATLAS = decals_069d.ATLAS
CELL = decals_069d.DECAL_ICON

# https://kryeministri.rks-gov.net/wp-content/uploads/2022/03/<name> (linked from the Ministry of
# Culture's page of the state symbols, mkrs-ks.org)
OFFICIAL = {
    'flamuri_republikes_se_kosoves.pdf': '453650d7d8165ca952f97c08c747641a25bd4f13d2e69a75a9e01057236102a1',
    'stema_republikes_se_kosoves.pdf': '719364e1a21c783c6b695dc9cd2f3b7fefe9b0845dee583db008e2fb9b51ec15',
}
RENDER_DPI = 300        # the official pages at 300 dpi: the flag 2955 x 1997 pixels, 6x the patch's field

T = DECAL_TEXTURES
DONOR = {
    T + '4_69.dds': '16067becdfaf444614c333639592411038f830a853c73be718c2f0653ecaeee1',
    T + '4_69_nmap.dds': '6f88f9e5abbd92467e33ec47d165a3af9bf86edd49662239ab2a3272a472c7ed',
    T + '4_69_smap.dds': '90bbd9d7a6e3a84bc5816f6714d7ffb58443665ec62c943a009799bbaa4ee875',
    T + '4_69_fmap.dds': 'e94d28b1eda06166eea8642ac03a051fa2aef8aac2b3c0d27c77581b8d6a56fc',
    T + '4_62.dds': '9a42b5a642f37634013385c0afdefa54150a4bcdcaa78611e384747601ddfb45',
    T + '4_60.dds': '3391d65f6b2511700f67e8adad9c1bd2b93eda0989155ed9b93c5a3214d5e809',
    T + '4_60_nmap.dds': '454bb8553ea26509d7936aa108d54f6075be58d86b50a8b701df1c16d9e75005',
    T + '5_42.dds': '97f118066fc0cc151173e19adbb9bc865cf55392c5817dbb7201dfaad3369a36',
    T + '5_42_nmap.dds': 'f4cafd4cda75fd0898c28d3e5152f09bb0f8f63b55d0406a2615db187c7bef18',
    T + '5_42_smap.dds': '1e7e8943fe528e04b8925e70cd7d1005318e41e7401ffeb3dc864a5765e01f6a',
    T + '5_42_fmap.dds': '3074dfa33bca62b437c4c8803748e1c3df134b93c340f82654624168fa031671',
}
STOCK = {path: sha for path, sha in decals_069d.STOCK.items()
         if path == DECAL_CONFIGS + 'char_decal_4_41.options' or path.startswith(DECAL_TEXTURES + '4_41')}
ROLES = decals_069d.DECAL_ROLES          # '', '_fmap', '_nmap', '_smap'

ITEMS = [
    {'dict_id': 1894, 'rc': '4_76', 'kind': 'flag', 'icon': (4, 42), 'donor': '4_69',
     'names': {'english': 'Flag of Kosovo', 'german': 'Flagge des Kosovo', 'original': 'Флаг Косово',
               'polish': 'Flaga Kosowa', 'russian': 'Флаг Косово', 'ukrainian': 'Прапор Косова'}},
    {'dict_id': 1895, 'rc': '5_43', 'kind': 'flag', 'icon': (3, 26), 'donor': '5_42',
     'names': {'english': 'Coat of arms of Kosovo', 'german': 'Wappen des Kosovo', 'original': 'Герб Косово',
               'polish': 'Godło Kosowa', 'russian': 'Герб Косово', 'ukrainian': 'Герб Косова'}},
]
# As 0.26g's and 0.69d's flags: premium, never dropped; content-edit/items.tsv prices them in silver
# (30,000, their 300 gold times 100).
PRICE = 30000


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class Dds:
    """A DXT1 or DXT5 .dds of the game's: its header, size, mips and the byte after its mips (as
    rifle-scopes/pbr_026.py reads them)."""

    def __init__(self, data):
        if data[:4] != b'DDS ':
            raise ValueError('not a DDS')
        self.data = bytes(data)
        self.height, self.width = struct.unpack_from('<II', data, 12)
        self.mips = max(1, struct.unpack_from('<I', data, 28)[0])
        self.fourcc = bytes(data[84:88])
        block = {b'DXT1': 8, b'DXT5': 16}.get(self.fourcc)
        if block is None:
            raise ValueError(f'a {self.fourcc!r} DDS (DXT1 or DXT5 only)')
        end = 128 + sum(-(-max(1, self.width >> m) // 4) * -(-max(1, self.height >> m) // 4) * block
                        for m in range(self.mips))
        if len(data) != end + 1 or data[end] not in (0, 1):
            raise ValueError(f'{len(data)} bytes: not its mips ({end - 128} bytes) and an sRGB byte')
        self.srgb = data[end]

    def pixels(self, mode):
        return np.asarray(Image.open(io.BytesIO(self.data)).convert(mode), dtype=np.float64)


def encoded(pixels, like):
    """`pixels` (RGB or RGBA, 0..255) in the format, size and mips of `like` (a Dds), with its header
    and sRGB byte."""
    p = np.clip(np.round(pixels), 0, 255).astype(np.uint8)
    if p.shape[:2] != (like.height, like.width):
        raise ValueError('pixels of another size')
    mode = 'RGBA' if p.shape[2] == 4 else 'RGB'
    with tempfile.TemporaryDirectory() as t:
        src, dst = Path(t) / 'd.png', Path(t) / 'd.dds'
        Image.fromarray(p, mode).save(src)
        # names relative to the folder: ImageMagick 7.1 refuses some absolute paths
        subprocess.run(['magick', src.name, '-define', f'dds:compression={like.fourcc.decode().lower()}',
                        '-define', f'dds:mipmaps={like.mips - 1}', dst.name], check=True, capture_output=True, cwd=t)
        made = dst.read_bytes()
    if made[84:88] != like.fourcc or struct.unpack_from('<II', made, 12) != (like.height, like.width) \
            or max(1, struct.unpack_from('<I', made, 28)[0]) != like.mips:
        raise ValueError('the encoder made another format, size or mip count')
    out = bytes(like.data[:128]) + made[128:] + bytes([like.srgb])
    Dds(out)
    return out


def read_inputs(folder, wanted, what):
    found = {}
    for root, _, names in os.walk(folder):
        for name in names:
            rel = os.path.relpath(os.path.join(root, name), folder).replace(os.sep, '/')
            found[ce.game_path(rel)] = os.path.join(root, name)
            found.setdefault(name, os.path.join(root, name))
    out = {}
    for path, sha in wanted.items():
        if path not in found:
            sys.exit(f'{what}: no {path} in {folder}')
        data = Path(found[path]).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{what}: {path} is not the file this pack is made of (sha256 {sha})')
        out[path] = data
    return out


def render(pdf):
    """An official page as RGB pixels, without antialiasing (each pixel one of the flat fills)."""
    with tempfile.TemporaryDirectory() as t:
        Path(t, 'page.pdf').write_bytes(pdf)
        subprocess.run(['pdftocairo', '-png', '-r', str(RENDER_DPI), '-antialias', 'none', '-singlefile',
                        'page.pdf', 'page'], check=True, capture_output=True, cwd=t)
        return np.asarray(Image.open(Path(t, 'page.png')).convert('RGB'), dtype=np.float64)


def make_textures(official, donor):
    """{rc: {role: pixels}} of both decals (role '', '_nmap', '_smap')."""
    d = {path.removeprefix(T).removesuffix('.dds'): Dds(data) for path, data in donor.items()}
    albania = d['4_69'].pixels('RGBA')
    flag_design = textures.design_flag(render(official['flamuri_republikes_se_kosoves.pdf']),
                                       textures.flag_field(albania))
    flag = textures.flag(albania, d['4_69_nmap'].pixels('RGBA'), d['4_69_smap'].pixels('RGB'),
                         d['4_62'].pixels('RGBA'), d['4_60'].pixels('RGBA'), d['4_60_nmap'].pixels('RGBA'),
                         flag_design)
    bosnia = d['5_42'].pixels('RGBA')
    coat_design = textures.design_shield(render(official['stema_republikes_se_kosoves.pdf']), bosnia[..., 3])
    coat = textures.coat_of_arms(bosnia, d['5_42_nmap'].pixels('RGBA'), d['5_42_smap'].pixels('RGB'), coat_design)
    return {'4_76': dict(zip(('', '_nmap', '_smap'), flag)), '5_43': dict(zip(('', '_nmap', '_smap'), coat))}


def icon(diffuse, kind):
    """The lobby icon (CELL x CELL RGBA): the flag as 0.69d draws its flags' icons (the texture at a
    quarter, 128 x 128, then scaled to the cell as decals-069d's), the coat of arms as 0.69d's 5_42
    (its shape's box, as tall as the cell, centred)."""
    image = Image.fromarray(np.clip(np.round(diffuse), 0, 255).astype(np.uint8), 'RGBA')
    if kind == 'patch':
        image = image.resize((128, 128), Image.Resampling.BOX)
        return image.resize((CELL, CELL), Image.Resampling.LANCZOS)
    image = image.crop(image.getchannel('A').point(lambda a: 255 if a > 128 else 0).getbbox())
    w = max(1, round(image.width * CELL / image.height))
    image = image.resize((w, CELL), Image.Resampling.LANCZOS)
    square = Image.new('RGBA', (CELL, CELL), (0, 0, 0, 0))
    square.paste(image, ((CELL - w) // 2, 0))
    return square


def png(image):
    out = io.BytesIO()
    image.save(out, format='PNG', optimize=False)
    return out.getvalue()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--official', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--preview', type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there; the pack goes in a new folder')
    official = read_inputs(args.official, OFFICIAL, '--official')
    donor = read_inputs(args.donor, DONOR, '--donor')
    stock = read_inputs(args.stock, STOCK, '--stock')

    made = make_textures(official, donor)
    files, icons = {}, []
    for item in ITEMS:
        rc, base = item['rc'], DECAL_TEXTURES + item['rc']
        item.update(name='decals_' + rc, config=DECAL_CONFIGS + f'char_decal_{rc}.options',
                    modification=200000 + 100 * int(rc.split('_')[0]) + int(rc.split('_')[1]),
                    texture='character/human/decal/' + rc, key=f'st_character_decals_{rc}_name')
        files[item['config']] = decals_069d.item_config(stock[DECAL_CONFIGS + 'char_decal_4_41.options'], item)
        for role in ROLES:
            like = donor[DECAL_TEXTURES + item['donor'] + role + '.dds']
            files[base + role + '.dds'] = like if role == '_fmap' else encoded(made[rc][role], Dds(like))
            files[base + role + '.options'] = stock[DECAL_TEXTURES + '4_41' + role + '.options']
        row, col = item['icon']
        source = f'icons/LobbyMenu_DecalsIcons/r{row:02}c{col:02}.png'
        files[source] = png(icon(made[rc][''], 'patch' if rc.startswith('4_') else 'shield'))
        icons.append({'row': row, 'col': col, 'source': source})

    operations = [{'op': 'add_file', 'path': path} for path in files if not path.startswith('icons/')]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        {'base_cost': PRICE, 'cfg_name': item['config'], 'dict_id': item['dict_id'], 'is_premium': True,
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
        'description': 'the Flag of Kosovo and the Coat of arms of Kosovo (1894, 1895), from the Government of '
                       'Kosovo\'s official files, in 0.69d\'s styles: an embroidered patch, a worn painted shield '
                       '(srp-026g/docs/notes/kosovo-decals.md)',
        'operations': operations,
        'server': {'items': [{'dict_id': item['dict_id']} for item in ITEMS]},
        'notes': [
            'made by srp-026g/scripts/kosovo-decals/make_pack.py (textures.py) from the official PDFs and 0.69d0\'s '
            'decals 4_69 (Albania), 4_62 (Chile), 4_60 (Vietnam), 5_42 (Bosnia and Herzegovina)',
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
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files) - len(icons)} files, {len(icons)} icons, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
