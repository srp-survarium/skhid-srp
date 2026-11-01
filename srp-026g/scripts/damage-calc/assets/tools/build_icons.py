#!/usr/bin/env python3
"""Crop Survarium 0.26g item icons from the lobby_menu.swf atlases.

How the game does it (lobby_menu.swf ActionScript, ffdec export):
  LobbyMenu.createIcons -> new IconContainer(name, 171, 83, atlas, vertical=true, count=0,
                                             stepH=0, stepV=8, atStart=false)
  for "weapons", "equipment", "consumables" and "vouchers".
  IconContainer: columns = floor(W / (171+0)), rows = floor(H / (83+8));
                 cell (c, r) is the 171x83 rect at (171*c, 91*r)   (atStart=false: gap after the cell)
  ItemFactory.createBaseItem: iconId = (icon_column-1) * rows + (icon_row-1)  (column-major)
  and the config's ui_desc.icon = [icon_row, icon_column], 1-based.
  Atlas choice (ItemFactory.getCategory): equipment = armour, weapons = firearms,
  consumables = everything else usable (ammo, grenades, drugs, artefacts, boosters, devices),
  vouchers = vouchers and money/gold.

usage: build_icons.py <config_json_dir> <lobby_images_dir> <out_dir> [--webp]
"""
import glob, json, os, sys
from PIL import Image

cfg_dir, img_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
fmt = 'webp' if '--webp' in sys.argv else 'png'
CELL_W, CELL_H, STEP_V = 171, 83, 8
ATLASES = {
    'equipment':   '3273_LobbyMenu_EquipmentIcons.png',
    'weapons':     '3277_LobbyMenu_WeaponIcons.png',
    'consumables': '3276_LobbyMenu_AmmoIcons.png',
    'vouchers':    '3282_LobbyMenu_VoucherIcons.png',
}
OUT_W = 171  # native cell width (no scaling); set lower to shrink the set


def group_of(folder, name):
    if folder == 'armour':
        return 'armour', 'equipment'
    if folder == 'weapons':
        if name.startswith('wpn_'):
            return 'weapon', 'weapons'
        if name.startswith('ammo_'):
            return 'ammo', 'consumables'
        return 'grenade', 'consumables'
    if folder == 'medicines':  # drugs made whole from one table (docs/notes/medicines.md)
        return 'drug', 'consumables'
    if folder in ('artefacts', 'drugs', 'boosters', 'devices'):
        return folder.rstrip('s'), 'consumables'
    if folder == 'money':  # money/gold use the voucher atlas (Category.GOLD)
        return 'money', 'vouchers'
    if folder == 'vouchers':
        return 'voucher', 'vouchers'
    return None, None


loc = json.load(open(os.path.join(cfg_dir, 'localization__english__localization.json')))['strings']
# Items are named as gen_calc_data.py names them: the config's file name, or
# the item dictionary's name where file names clash (Skhid's exo pieces).
dict_rows = json.load(open(os.path.join(cfg_dir, 'gameplay__db_static_dictionaries.json')))['items_dict']
by_cfg = {i['cfg_name']: i for i in dict_rows}
stems = {}
for c in by_cfg:
    stems.setdefault(c.rsplit('/', 1)[-1].removesuffix('.options'), []).append(c)
atl = {k: Image.open(os.path.join(img_dir, v)).convert('RGBA') for k, v in ATLASES.items()}
geom = {k: {'file': f'lobby_menu.swf / {ATLASES[k]} (swf character id {ATLASES[k].split("_")[0]})',
            'size': list(im.size), 'cell': [CELL_W, CELL_H], 'pitch': [CELL_W, CELL_H + STEP_V],
            'columns': im.size[0] // CELL_W, 'rows': im.size[1] // (CELL_H + STEP_V)}
        for k, im in atl.items()}
os.makedirs(os.path.join(out, 'icons'), exist_ok=True)
items = {}
for f in sorted(glob.glob(os.path.join(cfg_dir, 'gameplay__items__*.options.json'))):
    parts = os.path.basename(f)[:-len('.options.json')].split('__')
    folder, name = parts[2], parts[-1]
    row = by_cfg.get('/'.join(parts) + '.options')
    if row and len(stems.get(name, [])) > 1:
        name = row['name']
    group, atlas = group_of(folder, name)
    if not group:
        continue
    ui = json.load(open(f)).get('ui_desc') or {}
    icon = ui.get('icon')
    if not (isinstance(icon, list) and len(icon) == 2 and icon[0] >= 1 and icon[1] >= 1):
        print('no icon:', name, icon); continue
    row, col = icon
    g = geom[atlas]
    if row > g['rows'] or col > g['columns']:
        print('icon outside atlas:', name, icon, atlas); continue
    x, y = CELL_W * (col - 1), (CELL_H + STEP_V) * (row - 1)
    crop = atl[atlas].crop((x, y, x + CELL_W, y + CELL_H))
    if crop.getbbox() is None:
        print('empty cell:', name, icon, atlas)
    crop = crop.resize((OUT_W, round(CELL_H * OUT_W / CELL_W)), Image.LANCZOS)
    fn = f'icons/{name}.{fmt}'
    if fmt == 'webp':
        crop.save(os.path.join(out, fn), 'WEBP', quality=88, method=6, alpha_quality=90)
    else:
        q = crop.quantize(colors=255, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE)
        q.save(os.path.join(out, fn), optimize=True)
    tname = (ui.get('text_descriptions') or {}).get('name') or f'st_{name}_name'
    items[name] = {'file': fn, 'group': group, 'subfolder': '/'.join(parts[2:-1]),
                   'atlas': atlas, 'icon': icon, 'rect': [x, y, CELL_W, CELL_H],
                   'name_key': tname, 'name_en': (loc.get(tname) or '').strip() or None}
json.dump({'source': 'Survarium 0.26g resources.db: flash_movies/lobby_menu.swf (icon atlases) + '
                     'gameplay/items/** ui_desc.icon',
           'icon_rule': 'ui_desc.icon = [row, column], 1-based; rect = (171*(column-1), 91*(row-1), 171, 83)',
           'output_size': [OUT_W, round(CELL_H * OUT_W / CELL_W)],
           'atlases': geom, 'items': items},
          open(os.path.join(out, 'icons.json'), 'w'), indent=1, ensure_ascii=False)
print(len(items), 'icons')
