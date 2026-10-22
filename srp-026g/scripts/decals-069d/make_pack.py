#!/usr/bin/env python3
"""make_pack.py: the content pack of the 0.69d decals and the camouflage the owner picked
(2026-10-22; srp-026g/docs/notes/decals-069d.md).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --out <packs folder>/decals-069d-<version>
       (needs Pillow for the icons: nix-shell -p 'python3.withPackages(p: [p.pillow])')

--stock  0.26g's templates, at their game paths or flattened as dbgrab writes them
         (gameplay__items__...), from the stock resources.db (no Skhid patch carries them):
         gameplay/items/upgrades/decals/char_decal_{4_41,3_4}.options,
         gameplay/items/upgrades/camouflages/premium/camo_new_year.options,
         textures/character/human/decal/4_41{,_fmap,_nmap,_smap}.options,
         textures/character/human/camo/premium/new_year{,_nmap,_smap}.options.
--donor  0.69d's files, the same way (`dbgrab <0.69d resources.NN.db> get <path> <folder>`; the
         volume of each: 069d-vs-026-items/work/latest.tsv): every path of donor.sha256.
--out    the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and
         the manifest's `pack` row (release/content.tsv).

Every input is checked against its sha256 (STOCK below, donor.sha256). What the pack holds:
- each decal's item config, made from a 0.26g decal of its kind by content_edit.py's edits: the
  modification id (200000 + 100 x row + column of its 0.69d name), the texture its modifier names,
  the name key and the icon's cell; read back, nothing else differs. A flag or a holiday decal
  from 0.26g's flag char_decal_4_41 (prm_decl, level 4, no drop); Jarilo from 0.26g's drop decal
  char_decal_3_4 (drp_decl, level 3, drop weight 160). The camouflage from 0.26g's New Year 2015
  (as camos-069 did). 0.69d's own configs are of a later format 0.26g cannot read.
- the textures as 0.69d has them (the formats of 0.26g's decals: DXT5 512^2 diffuse and normal
  map, DXT1 512^2 fmap and smap, 8 mips; the camouflage's DXT1 / DXT5 / DXT1), each with the
  .options of 0.26g's texture of the same role (0.69d's carry the same values, in its format).
- the dictionary rows, the names in 0.26g's six languages (0.69d's own texts, texts.tsv; the
  description is 0.26g's st_character_decals_generic_desc / st_camo_generic_desc), and the icons:
  0.69d's, scaled to the decal atlas's 82x82 cells; the camouflage's 72x72 and 140x140 as they are.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

PACK_ID = 'decals-069d'
VERSION = '1'
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')

DECAL_CONFIGS = 'gameplay/items/upgrades/decals/'
DECAL_TEXTURES = 'textures/character/human/decal/'
CAMO_CONFIGS = 'gameplay/items/upgrades/camouflages/premium/'
CAMO_TEXTURES = 'textures/character/human/camo/premium/'
DECAL_ROLES = ('', '_fmap', '_nmap', '_smap')
CAMO_ROLES = ('', '_nmap', '_smap')

# The templates: a kind's 0.26g item and what it holds of the values the port changes.
TEMPLATES = {
    'flag': {'config': DECAL_CONFIGS + 'char_decal_4_41.options', 'modification': 200441,
             'texture': 'character/human/decal/4_41', 'key': 'st_character_decals_4_41_name',
             'icon': (4, 41)},
    'drop': {'config': DECAL_CONFIGS + 'char_decal_3_4.options', 'modification': 200304,
             'texture': 'character/human/decal/3_4', 'key': 'st_character_decals_3_4_name',
             'icon': (3, 4)},
    'camo': {'config': CAMO_CONFIGS + 'camo_new_year.options', 'modification': 130015,
             'texture': 'character/human/camo/premium/new_year', 'key': 'st_camo_new_year',
             'icon': (1, 7)},
}
TEMPLATES['holiday'] = TEMPLATES['flag']

STOCK = {
    DECAL_CONFIGS + 'char_decal_4_41.options': '3d73d0d0ea24c2908a4b01ea81352645c0e5768f4cef9907c414ed07303f0156',
    DECAL_CONFIGS + 'char_decal_3_4.options': '63c36d452727f00d3c7731c8df23fea4eb87571300c4e4da6fc76ad34a2107e9',
    CAMO_CONFIGS + 'camo_new_year.options': '6f4c32c193669ffb97c3b1ddaaf0bf260df5dfbb0463033ba46bb238b935c86c',
    DECAL_TEXTURES + '4_41.options': 'df379225eac9eec632cf33389c4f62250ec2314f76b002cca18c4170297ceb90',
    DECAL_TEXTURES + '4_41_fmap.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
    DECAL_TEXTURES + '4_41_nmap.options': '7861fedb73812d0047394f32cf5aee9543390f5d2e9a3d7426c60ee13500ab71',
    DECAL_TEXTURES + '4_41_smap.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
    CAMO_TEXTURES + 'new_year.options': '0f1777736975b33474050d3969ba986994d638271049a0acfbdf35f095c26b8f',
    CAMO_TEXTURES + 'new_year_nmap.options': '7861fedb73812d0047394f32cf5aee9543390f5d2e9a3d7426c60ee13500ab71',
    CAMO_TEXTURES + 'new_year_smap.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
}

# Prices (the owner's call; docs/notes/decals-069d.md): `base_cost` and `is_premium` of the
# dictionary row. srp sells a premium item for its gold `base_cost` (shop_offers::
# premium_price_list); a decal that is not premium is never sold (prices.rs) but drops.
PRICES = {'flag': (300, True), 'holiday': (300, True), 'drop': (1000, False), 'camo': (300, True)}

DECAL_ICON = 82
ATLAS = 'flash_movies/lobby_menu.swf'
# The big camouflage atlas is a JPEG whose empty cells are solid red: the icon names the cell it
# replaces: [2, 7], red with the JPEG fringe of its neighbour (R 248-255, G and B up to 6), as the
# porter decodes revision 27's movie (`vostok-port pack check` reports the hash of a cell).
CAMO_BIG_EMPTY_SHA256 = '9c955a05ff2477edaa80f2c5dde3d79799c6e3df3e9a1e0c0ce488479353fd16'


def porter_text(text):
    """The game keeps texts as UTF-8 bytes; the porter reads and writes a config's strings as
    Windows-1251 (WHATWG's: byte 0x98 is U+0098), so a pack names the UTF-8 bytes so read."""
    raw = text.encode('utf-8')
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in raw)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def read_inputs(folder, wanted, what):
    found = {}
    for root, _, names in os.walk(folder):
        for name in names:
            rel = os.path.relpath(os.path.join(root, name), folder).replace(os.sep, '/')
            found[ce.game_path(rel)] = os.path.join(root, name)
    out = {}
    for path, sha in wanted.items():
        if path not in found:
            sys.exit(f'{what}: no {path} in {folder}')
        data = Path(found[path]).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{what}: {path} is not the file this pack is made of (sha256 {sha})')
        out[path] = data
    return out


def rows(name):
    """The tab-separated rows of a table beside this script, without its comment lines."""
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def read_items():
    items = []
    for dict_id, donor_id, donor, kind, icon_row, icon_col, english in rows('items.tsv'):
        item = {'dict_id': int(dict_id), 'donor_id': int(donor_id), 'donor': donor, 'kind': kind,
                'icon': (int(icon_row), int(icon_col)), 'english': english}
        if kind == 'camo':
            # As camos-069 had it: 0.26g's camouflages use modifications 130000 to 130015.
            item.update(name=donor, config=CAMO_CONFIGS + donor + '.options', modification=130016,
                        texture='character/human/camo/premium/' + donor, key='st_camo_steam')
        else:
            rc = donor.removeprefix('char_decal_')
            group, number = (int(n) for n in rc.split('_'))
            item.update(name='decals_' + rc, config=DECAL_CONFIGS + donor + '.options',
                        modification=200000 + 100 * group + number, texture='character/human/decal/' + rc,
                        key=f'st_character_decals_{rc}_name', rc=rc)
        items.append(item)
    return items


def read_texts():
    texts = {}
    header, *table = rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    for row in table:
        if len(row) != len(header):
            sys.exit(f'texts.tsv: {row[0]} has {len(row) - 1} texts, not {len(LANGUAGES)}')
        texts[row[0]] = dict(zip(LANGUAGES, row[1:]))
    return texts


def item_config(template, item):
    """The template with the item's six values; read back, every other value as it was."""
    base = TEMPLATES[item['kind']]
    buf = bytearray(template)
    ce.set_number(buf, 'modifications[0]/id', base['modification'], item['modification'])
    ce.set_number(buf, 'parameters/modification', base['modification'], item['modification'])
    ce.set_number(buf, 'ui_desc/icon[0]', base['icon'][0], item['icon'][0])
    ce.set_number(buf, 'ui_desc/icon[1]', base['icon'][1], item['icon'][1])
    if ce.replace_string(buf, base['texture'], item['texture']) != 1 or \
            ce.replace_string(buf, base['key'], item['key']) != 1:
        sys.exit(f'{base["config"]} does not name its texture and its name once each')
    old = {p: (t, v) for p, t, v in ce.walk(bytearray(template))}
    new = {p: (t, v) for p, t, v in ce.walk(buf)}
    texture_path = ('/modifications[0]/modifiers[0]/value[1]' if item['kind'] == 'camo'
                    else '/modifications[0]/modifiers[0]/value[1][0]')
    want = {
        '/modifications[0]/id': (ce.T_INT, item['modification']),
        '/parameters/modification': (ce.T_INT, item['modification']),
        '/ui_desc/icon[0]': (ce.T_INT, item['icon'][0]),
        '/ui_desc/icon[1]': (ce.T_INT, item['icon'][1]),
        texture_path: (ce.T_STR, item['texture'].encode()),
        '/ui_desc/text_descriptions/name': (ce.T_STR, item['key'].encode()),
    }
    if set(old) != set(new) or not set(want) <= set(new):
        sys.exit(f'{item["name"]}: the item config lost or gained a value')
    for path in old:
        expected = want.get(path, old[path])
        if new[path] != expected:
            sys.exit(f'{item["name"]}: {path} is {new[path]}, not {expected}')
    return bytes(buf)


def icon_png(data, size, out):
    """0.69d's icon as a size x size RGBA PNG: scaled whole (its own margins kept), the alpha
    premultiplied while scaling (Pillow's RGBA resize)."""
    import io
    from PIL import Image
    image = Image.open(io.BytesIO(data)).convert('RGBA')
    if image.width != image.height:
        sys.exit(f'{out}: 0.69d icon is {image.width}x{image.height}, not square')
    if image.size != (size, size):
        image = image.resize((size, size), Image.Resampling.LANCZOS)
    out.parent.mkdir(parents=True, exist_ok=True)
    image.save(out, format='PNG', optimize=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    items = read_items()
    texts = read_texts()
    donor_sums = {}
    for line in (HERE / 'donor.sha256').read_text().splitlines():
        sha, path = line.split(None, 1)
        donor_sums[path.strip()] = sha
    stock = read_inputs(args.stock, STOCK, '--stock')
    donor = read_inputs(args.donor, donor_sums, '--donor')

    files, icons, camo_icons = {}, [], []
    for item in items:
        files[item['config']] = item_config(stock[TEMPLATES[item['kind']]['config']], item)
        if item['kind'] == 'camo':
            for role in CAMO_ROLES:
                base = CAMO_TEXTURES + item['donor'] + role
                files[base + '.dds'] = donor[base + '.dds']
                files[base + '.options'] = stock[CAMO_TEXTURES + 'new_year' + role + '.options']
            row, col = item['icon']
            for symbol, folder, size in (('LobbyMenu_CamoIcons', 'camo_s/camo_s', 72),
                                         ('LobbyMenu_CamoIconsBig', 'camo/camo', 140)):
                source = f'icons/{symbol}/r{row:02}c{col:02}.png'
                icon_png(donor[f'flash_movies/textures/ui_icons/items/{folder}_{col}_{row}.png'], size,
                         args.out / source)
                icon = {'row': row, 'col': col, 'source': source}
                if symbol == 'LobbyMenu_CamoIconsBig':
                    icon['replaces_sha256'] = CAMO_BIG_EMPTY_SHA256
                camo_icons.append((symbol, icon))
            continue
        for role in DECAL_ROLES:
            base = DECAL_TEXTURES + item['rc'] + role
            files[base + '.dds'] = donor[base + '.dds']
            files[base + '.options'] = stock[DECAL_TEXTURES + '4_41' + role + '.options']
        row, col = item['icon']
        source = f'icons/LobbyMenu_DecalsIcons/r{row:02}c{col:02}.png'
        icon_png(donor[f'flash_movies/textures/ui_icons/items/decals/decal_{item["rc"]}.png'], DECAL_ICON,
                 args.out / source)
        icons.append({'row': row, 'col': col, 'source': source})

    operations = [{'op': 'add_file', 'path': path} for path in files]
    entries = []
    for item in items:
        cost, premium = PRICES[item['kind']]
        entries.append({'base_cost': cost, 'cfg_name': item['config'], 'dict_id': item['dict_id'],
                        'is_premium': premium, 'is_stack': False,
                        'item_category': 25 if item['kind'] == 'camo' else 26, 'name': item['name']})
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {item['key']: porter_text(texts[item['key']][language])
                                       for item in items}})
    operations.append({'op': 'atlas_icons_add', 'swf': ATLAS, 'symbol': 'LobbyMenu_DecalsIcons',
                       'icons': icons})
    for symbol in ('LobbyMenu_CamoIcons', 'LobbyMenu_CamoIconsBig'):
        operations.append({'op': 'atlas_icons_add', 'swf': ATLAS, 'symbol': symbol,
                           'icons': [icon for s, icon in camo_icons if s == symbol]})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': f'the owner\'s pick of 0.69d (2026-10-22): {len(items) - 1} decals and the Steam '
                       'camouflage (srp-026g/docs/notes/decals-069d.md)',
        'operations': operations,
        'server': {'items': [{'dict_id': item['dict_id']} for item in items]},
        'notes': [
            'made by srp-026g/scripts/decals-069d/make_pack.py from items.tsv, texts.tsv and donor.sha256',
            'donor: Survarium 0.69d0 (the volumes 069d-vs-026-items/work/latest.tsv names)',
            'item configs: 0.26g char_decal_4_41 (flags, holiday decals), char_decal_3_4 (Jarilo), '
            'camo_new_year (Steam camo) with the modification id, texture, icon cell and name key',
        ] + [f'{item["dict_id"]} {item["name"]}: 0.69d {item["donor_id"]} {item["donor"]} ({item["english"]})'
             for item in items],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, {len(icons) + len(camo_icons)} icons, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
