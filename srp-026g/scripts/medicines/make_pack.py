#!/usr/bin/env python3
"""make_pack.py: the content pack of the medicines, every one made whole from one table (the owner,
2026-10-31; srp-026g/docs/notes/medicines.md).

usage: python3 -I make_pack.py --stock <folder> --donor <folder> --out <packs folder>/medicines-<version>
       (needs Pillow for the icons: nix-shell -p 'python3.withPackages(p: [p.pillow])')

--stock  0.26g's gameplay/items/drugs/medkit.options from the stock resources.db (no Skhid patch
         carries it), at its game path or flattened as dbgrab writes it (gameplay__items__...).
--donor  the icons, the same way: 0.27d0's lobby atlas flash_movies/textures/ui_items/ammo.png and
         0.46c2's flash_movies/textures/ui_icons/items/ammo/ammo_3_28.png (donor.sha256).
--out    the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and
         the manifest's `pack` row (release/content.tsv).

Every input is checked against its sha256 (STOCK below, donor.sha256). What the pack holds, for
each row of medicines.tsv:
- its config, gameplay/items/medicines/<name>.options: 0.26g's medkit.options with the row's keys,
  icon cell, faction, levels, times, weight, heals, injuries and card, by content_edit.py's edits;
  read back, the row is what it says and nothing else of the template differs. A new path, so no
  value of a live config changes type (release/content_type_check.py); the bandages' and the
  Medikit's dictionary rows are repointed to theirs by content-edit medicines-1.
- the coloured medkits' dictionary rows (the stock medkit's row: a stack of category 10) and their
  icons in LobbyMenu_AmmoIcons: 0.27d's atlas cell as it is, or 0.46c2's icon (126x78) placed in a
  cell where 0.27d placed the same box (the blue one, 0.46c2's ammo_3_16, at 19, 2).
The texts are in client-patches/localization/*.tsv.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

PACK_ID = 'medicines'
VERSION = '1'
FOLDER = 'gameplay/items/medicines/'

TEMPLATE = 'gameplay/items/drugs/medkit.options'
STOCK = {TEMPLATE: '52ba7be9a7434e7c1cfde583f0fd9e262f1e4103af6884abc4108fbbd2830814'}
# The template as it is (PROVEN by `drug`, read back before any edit).
LIMBS = ('left_hand', 'right_hand', 'left_leg', 'right_leg')
INJURIES = (('left_hand', 3), ('right_hand', 3), ('left_leg', 4), ('right_leg', 4))
TEMPLATE_DRUG = {
    'key': 'st_base_medkit', 'icon': (15, 1), 'faction': None, 'level': 3, 'mm': 3,
    'use': 2, 'active': 3, 'kg': 0.2,
    'heal': {'head': 1, 'body': 1, 'pain': 0, **dict.fromkeys(LIMBS, 1)},
    'injuries': INJURIES,
    'card': {12: '100', 19: '2.0', 18: '3.0', 15: '0.2'},
}
# A drug heals `amount` of a pool a second over its active time, after its use time (bandages.md:
# medkit::tick 0x33D790, increase_health clamps at the pool's max). The pools' max.
POOLS = {'head': 0.4, 'body': 1.0, 'pain': 1.0, **dict.fromkeys(LIMBS, 0.5), 'infection': 1.0}
# Affects (the exe's hit_affects_type_enum, its names' table in .data: death 0, bleeding 1, concussion
# 2, hand_damage 3, leg_damage 4, critical_poisoning 5, poisoning 6, radiation_sickness 7, blindness
# 8; the medkit's 3 on the hands and 4 on the legs agree). The `infection` pool takes toxic and
# radiation damage (human_hit_params) and gives poisoning; radiation sickness lasts 10 s.
TOXINS = (('infection', 6), ('infection', 7))
# 0.26g's misspelt stamina key, and the exe's spelling (weapon-balance-1 renamed it in every item).
STAMINA = ('stamina_regenation_speed_modifier', 'stamina_regeneration_speed_modifier')
# Not set: parameters/heal_amount (neither the exe nor a movie names it; the template's 1 stays).
# The card's rows: 19 use, 18 active, 15 weight; medical-cards-1's 135 arms and legs, 136 body and
# head, 137 pain (after 18); 13 intoxication (the antitoxin's). 12 ("Heal amount") is not shown.
COLUMNS = ('name', 'dict_id', 'key', 'colour', 'faction', 'level', 'mm', 'cost', 'use', 'active', 'kg',
           'limbs', 'body_head', 'pain', 'toxins', 'injuries', 'row', 'col', 'source', 'english')

ATLAS = 'flash_movies/lobby_menu.swf'
SYMBOL = 'LobbyMenu_AmmoIcons'
CELL = (171, 83)
ROW_PITCH = 91
ATLAS_027D = 'flash_movies/textures/ui_items/ammo.png'
ICONS_046C2 = 'flash_movies/textures/ui_icons/items/ammo/'
# Where 0.27d's atlas has 0.46c2's 126x78 icon of the same box in its cell (ammo_3_16 in [16, 3]).
ICON_046C2_AT = (19, 2)
# The stock medkit's dictionary row but its id, name, config and cost.
ROW = {'is_premium': False, 'is_stack': True, 'item_category': 10}


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


def read_table():
    rows = []
    for line in (HERE / 'medicines.tsv').read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('#'):
            continue
        cells = line.split('\t')
        if len(cells) != len(COLUMNS):
            sys.exit(f'medicines.tsv: {cells[0]} has {len(cells)} columns, not {len(COLUMNS)}')
        row = dict(zip(COLUMNS, cells))
        for k in ('dict_id', 'faction', 'level', 'mm', 'cost', 'use', 'active', 'limbs', 'body_head',
                  'pain', 'toxins', 'row', 'col'):
            row[k] = int(row[k])
        row['kg'] = float(row['kg'])
        if row['injuries'] not in ('yes', 'no'):
            sys.exit(f'medicines.tsv: {row["name"]} injuries is yes or no')
        row['config'] = FOLDER + row['name'] + '.options'
        rows.append(row)
    return rows


def amount(pct, pool, active):
    """The heal a second of `pct` % of `pool` over `active` s: 100 is 1 a second (the stock medkit's)."""
    return 1 if pct == 100 else round(pct / 100 * POOLS[pool] / active, 4)


def drug_of(row):
    """What the config of a row says, as `drug` reads it."""
    share = {'head': row['body_head'], 'body': row['body_head'], 'pain': row['pain'],
             **dict.fromkeys(LIMBS, row['limbs'])}
    heal = {pool: amount(pct, pool, row['active']) for pool, pct in share.items()}
    if row['toxins']:
        heal['infection'] = amount(row['toxins'], 'infection', row['active'])
    card = {19: f'{row["use"]}.0', 18: f'{row["active"]}.0', 135: str(row['limbs']),
            136: str(row['body_head']), 137: str(row['pain'])}
    if row['toxins']:
        card[13] = str(row['toxins'])
    card[15] = f'{row["kg"]:g}'
    return {
        'key': row['key'], 'icon': (row['row'], row['col']), 'faction': row['faction'] or None,
        'level': row['level'], 'mm': row['mm'],
        'use': row['use'], 'active': row['active'], 'kg': row['kg'], 'heal': heal,
        'injuries': (INJURIES if row['injuries'] == 'yes' else ()) + (TOXINS if row['toxins'] else ()),
        'card': card,
    }


def drug(buf):
    """What a config says of everything a row sets."""
    def value(path):
        return ce.value_of(buf, ce.find(buf, path))

    def entries(path):
        return range(len(ce.children(buf, ce.find(buf, path))))

    def number(path):
        v = value(path)
        return round(v, 4) if isinstance(v, float) else v
    use, active = value('data/activation_delay_sec'), value('data/activity_time_sec')
    if (value('parameters/activation_time'), value('parameters/activity_time_sec')) != (use, active):
        sys.exit('the card\'s and the drug\'s times differ')
    name, description = value('ui_desc/text_descriptions/name'), value('ui_desc/text_descriptions/description')
    if (name.removesuffix('_name'), description.removesuffix('_description')) != (name[:-5], name[:-5]):
        sys.exit(f'{name} and {description} are not one key\'s name and description')
    try:
        faction = value('parameters/faction')
    except KeyError:
        faction = None
    return {
        'key': name[:-5], 'icon': (value('ui_desc/icon[0]'), value('ui_desc/icon[1]')), 'faction': faction,
        'level': value('parameters/item_level'), 'mm': value('parameters/match_making_level'),
        'use': use, 'active': active, 'kg': number('parameters/weight'),
        'heal': {value(f'data/influences[{i}]/body_part'): number(f'data/influences[{i}]/amount')
                 for i in entries('data/influences')},
        'injuries': tuple((value(f'data/remove_affects[{i}]/body_part'), value(f'data/remove_affects[{i}]/affect'))
                          for i in entries('data/remove_affects')),
        'card': {value(f'ui_desc/props_list[{i}]/prop_id'): value(f'ui_desc/props_list[{i}]/prop_value')
                 for i in entries('ui_desc/props_list')},
    }


# What a row may change of the template: these values, and everything under these tables.
SET = {'/parameters', '/data/activation_delay_sec', '/data/activity_time_sec', '/parameters/activation_time',
       '/parameters/activity_time_sec', '/parameters/weight', '/parameters/item_level',
       '/parameters/match_making_level', '/parameters/faction',
       '/parameters/stamina_regenation_speed_modifier', '/parameters/stamina_regeneration_speed_modifier'}
TABLES = ('/data/influences', '/data/remove_affects', '/ui_desc/props_list', '/ui_desc/icon',
          '/ui_desc/text_descriptions')


def made(template, row):
    """The row's config; read back, it is the row and nothing else of the template differs."""
    base, want = TEMPLATE_DRUG, drug_of(row)
    buf = bytearray(template)
    if drug(buf) != base:
        sys.exit(f'{TEMPLATE} is not the medkit TEMPLATE_DRUG says')
    for old, new in ((base['key'] + '_name', want['key'] + '_name'),
                     (base['key'] + '_description', want['key'] + '_description')):
        if old != new and ce.replace_string(buf, old, new) != 1:
            sys.exit(f'{TEMPLATE} does not name {old} once')
    for i in (0, 1):
        ce.set_number(buf, f'ui_desc/icon[{i}]', base['icon'][i], want['icon'][i])
    if want['faction']:
        ce.add_value(buf, 'parameters', 'faction', want['faction'])
    ce.set_number(buf, 'parameters/item_level', base['level'], want['level'])
    ce.set_number(buf, 'parameters/match_making_level', base['mm'], want['mm'])
    # The stamina key spelt as the exe reads it, as every live item has it (weapon-balance-1).
    if not ce.rename_key(buf, 'parameters', STAMINA[0], STAMINA[1]):
        sys.exit(f'{TEMPLATE}: no parameters/{STAMINA[0]}')
    for path in ('data/activation_delay_sec', 'parameters/activation_time'):
        ce.set_number(buf, path, base['use'], want['use'])
    for path in ('data/activity_time_sec', 'parameters/activity_time_sec'):
        ce.set_number(buf, path, base['active'], want['active'])
    ce.set_number(buf, 'parameters/weight', base['kg'], want['kg'])
    for i, pool in enumerate(base['heal']):
        ce.set_number(buf, f'data/influences[{i}]/amount', base['heal'][pool], want['heal'][pool], as_float=True)
    last = list(base['heal'])[-1]
    for pool in [p for p in want['heal'] if p not in base['heal']]:
        if ce.add_entry(buf, 'data/influences', {'body_part': last}, {'body_part': last},
                        {'body_part': pool, 'amount': want['heal'][pool]}) is None:
            sys.exit(f'{TEMPLATE}: no influence {last}')
        last = pool
    for part, affect in base['injuries']:
        if (part, affect) not in want['injuries']:
            ce.remove_entry(buf, 'data/remove_affects', {'body_part': part, 'affect': affect})
    last = base['injuries'][-1]
    for part, affect in [i for i in want['injuries'] if i not in base['injuries']]:
        if ce.add_entry(buf, 'data/remove_affects', {'body_part': last[0], 'affect': last[1]},
                        {'body_part': last[0], 'affect': last[1]}, {'body_part': part, 'affect': affect}) is None:
            sys.exit(f'{TEMPLATE}: no remove_affects entry {last}')
        last = (part, affect)
    for prop, text in base['card'].items():
        if prop not in want['card']:
            ce.remove_entry(buf, 'ui_desc/props_list', {'prop_id': prop})
        elif want['card'][prop] != text:
            ce.set_prop(buf, prop, text, want['card'][prop])
    last = 18
    for prop in [p for p in want['card'] if p not in base['card']]:
        if ce.add_entry(buf, 'ui_desc/props_list', {'prop_id': last}, {'prop_id': 18},
                        {'prop_id': prop, 'prop_value': want['card'][prop]}) is None:
            sys.exit(f'{TEMPLATE}: no card prop {last}')
        last = prop
    if drug(buf) != want:
        sys.exit(f'{row["name"]}: reads back as {drug(buf)}, not {want}')
    old = {p: (t, v) for p, t, v in ce.walk(bytearray(template))}
    new = {p: (t, v) for p, t, v in ce.walk(buf)}

    def settable(path):
        return path in SET or any(path == t or path.startswith((t + '/', t + '[')) for t in TABLES)
    for path in set(old) | set(new):
        if old.get(path) != new.get(path) and not settable(path):
            sys.exit(f'{row["name"]}: {path} is {new.get(path)}, the template\'s {old.get(path)}')
    return bytes(buf)


def icon_png(donor, row, out):
    """A coloured medkit's icon as a cell-sized RGBA PNG."""
    from PIL import Image
    if row['source'].startswith('0.27d '):
        r, c = (int(n) for n in row['source'].removeprefix('0.27d [').rstrip(']').split(', '))
        atlas = Image.open(io.BytesIO(donor[ATLAS_027D])).convert('RGBA')
        x, y = (c - 1) * CELL[0], (r - 1) * ROW_PITCH
        image = atlas.crop((x, y, x + CELL[0], y + CELL[1]))
    elif row['source'].startswith('0.46c2 '):
        icon = Image.open(io.BytesIO(donor[ICONS_046C2 + row['source'].removeprefix('0.46c2 ') + '.png']))
        icon = icon.convert('RGBA')
        if icon.width + ICON_046C2_AT[0] > CELL[0] or icon.height + ICON_046C2_AT[1] > CELL[1]:
            sys.exit(f'{row["name"]}: 0.46c2 icon is {icon.width}x{icon.height}, larger than its place')
        image = Image.new('RGBA', CELL, (0, 0, 0, 0))
        image.paste(icon, ICON_046C2_AT)
    else:
        sys.exit(f'{row["name"]}: unknown icon source {row["source"]!r}')
    if image.getchannel('A').getbbox() is None:
        sys.exit(f'{row["name"]}: its icon is empty')
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
    rows = read_table()
    donor_sums = {}
    for line in (HERE / 'donor.sha256').read_text().splitlines():
        sha, path = line.split(None, 1)
        donor_sums[path.strip()] = sha
    stock = read_inputs(args.stock, STOCK, '--stock')
    donor = read_inputs(args.donor, donor_sums, '--donor')

    files, icons, new = {}, [], [row for row in rows if row['source'] != 'stock']
    for row in rows:
        files[row['config']] = made(stock[TEMPLATE], row)
    for row in new:
        source = f'icons/{SYMBOL}/r{row["row"]:02}c{row["col"]:02}.png'
        icon_png(donor, row, args.out / source)
        icons.append({'row': row['row'], 'col': row['col'], 'source': source})

    operations = [{'op': 'add_file', 'path': path} for path in files]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        dict(ROW, base_cost=row['cost'], cfg_name=row['config'], dict_id=row['dict_id'], name=row['name'])
        for row in new]})
    operations.append({'op': 'atlas_icons_add', 'swf': ATLAS, 'symbol': SYMBOL, 'icons': icons})
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the medicines, each standing on its own (the owner, 2026-10-31): the bandages, the '
                       'Medikit and the Military (blue), Black Market (green) and Science (yellow) medkits, '
                       'made whole from one table (srp-026g/docs/notes/medicines.md)',
        'operations': operations,
        'server': {'items': [{'dict_id': row['dict_id']} for row in rows]},
        'notes': [
            'made by srp-026g/scripts/medicines/make_pack.py from medicines.tsv and donor.sha256',
            f'configs: 0.26g medkit.options made into each row, at {FOLDER}<name>.options',
            'the bandages\' and the Medikit\'s dictionary rows repointed by content-edit medicines-1',
            'texts: srp-026g/client-patches/localization/*.tsv',
        ] + [f'{row["dict_id"]} {row["name"]}: {row["english"]} ({row["colour"]}), icon {row["source"]}'
             for row in rows],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, {len(icons)} icons, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
