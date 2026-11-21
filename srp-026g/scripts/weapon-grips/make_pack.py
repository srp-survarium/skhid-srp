#!/usr/bin/env python3
"""make_pack.py: the content pack weapon-grips, 0.69d0's foregrips as items a player buys and fits
to a weapon's kit slot 5 (docs/notes/attachments-grips.md, attachments.md).

usage: python3 -I make_pack.py --donor <folder> --modules <pack folder> --out <packs folder>/weapon-grips-<version>
                               [--check]
       (needs numpy and Pillow: nix-shell -p 'python3.withPackages(p: [p.numpy p.pillow])')

--donor    donor.sh's folder (0.69d0's game.options rows, its names and its pictures of the grips).
--modules  the pack weapon-modules as built (its files/: every grip's model must be there).
--out      the pack's folder (a new one): pack.json, files/ and icons/, for the manifest's `pack`
           row (release/content.tsv), which this prints.

What the pack holds, a grip of ../weapon-modules/ids.tsv each (grips.py reads them):
- its item config, gameplay/items/attachments/grip/<item>.options: a shop scope item's shape (the
  client's own kind of a weapon's fitted item, category 22). Its modification puts the item's dict
  id on the weapon as data/srp_grip (a type 3 add to the weapon's `data`, which every gun has with
  its srp_gun), which is all the game reads to draw the grip, its carrier and the left hand
  (client patch 161 and the modules table); and its stats, type 2 on the six kick bounds and on
  parameters/aim_time, as the stock recoil and aim-time rolls (weapon-rolls.md);
- its dictionary row (the price) and its icon, 0.69d0's own picture in the stock icons' frame.
The texts are the localization tables' (texts.py). Also writes grips.tsv here: what each grip is
in 0.69d0 and here. With --check it writes the pack and nothing else, and fails unless grips.tsv,
the pack's row of release/content.tsv and the grips' rows of ../content-edit/items.tsv are what it
makes (tools/check weapon-grips).
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import sys

from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import content_edit as ce  # noqa: E402
import grips as lib  # noqa: E402
import render_icon  # noqa: E402

PACK_ID = 'weapon-grips'
VERSION = '1'
CATEGORY = 22
KICK_BOUNDS = [f'{bound}_{side}_recoil' for side in ('top', 'left', 'right') for bound in ('min', 'max')]
ADD_KEY, FACTOR = 3, 2  # a modifier's type: a new key of a table; base x max(0, 1 + sum)


def config(grip):
    """A grip item's config as a value: a shop scope item's keys (weapon_scopes.md), the grip's own
    model as its two models, no card lines (the Attachments window shows a fitted item's stats)."""
    modifiers = [{'path': ['data'], 'value': ['srp_grip', grip.dict_id], 'type': ADD_KEY}]
    modifiers += [{'path': ['recoil', key], 'value': grip.recoil, 'type': FACTOR} for key in KICK_BOUNDS]
    if grip.aim_time:
        modifiers.append({'path': ['parameters', 'aim_time'], 'value': grip.aim_time, 'type': FACTOR})
    return {
        'parameters': {'weight': 0.0, 'modification': grip.modification},
        'modifications': [{'id': grip.modification, 'modifiers': modifiers}],
        'ui_desc': {'text_descriptions': {'name': grip.name_key, 'description': grip.description_key},
                    'icon': list(grip.icon), 'props_list': {}},
        'data': {'idle_model': grip.model, 'aimed_model': grip.model, 'type': 5, 'fov_factor': 1.0,
                 'near_plane_factor': 0.5, 'change_scope_factor': 1.0, 'hide_weapon_on_aim': False},
    }


def encoded(value):
    """The value as a binary config, read back."""
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, ce.append(buf, b'\0'), '', ce.as_table(value))
    leaves = {path: (kind, raw) for path, kind, raw in ce.walk(buf)}

    def want(node, path=''):
        if isinstance(node, dict):
            for key, child in node.items():
                want(child, f'{path}/{key}')
        elif isinstance(node, list):
            for i, child in enumerate(node):
                want(child, f'{path}[{i}]')
        else:
            kind, raw = leaves.pop(path)
            got = raw.decode() if kind == ce.T_STR else bool(raw) if kind == ce.T_BOOL else ce.as_number(kind, raw)
            same = got == node if not isinstance(node, float) else abs(got - node) < 1e-6
            if not same or (kind == ce.T_FLOAT) != isinstance(node, float):
                raise SystemExit(f'{path}: wrote {node!r}, reads {got!r}')
    want(value)
    if any(kind not in (ce.T_NAMED, ce.T_INDEXED) for kind, _ in leaves.values()):
        raise SystemExit(f'the config has values nobody wrote: {sorted(leaves)}')
    return bytes(buf)


def table(grips):
    head = ('# made by make_pack.py: each grip of ../weapon-modules/ids.tsv, what 0.69d0 rolls on it at its bought\n'
            f'# grade ({lib.GRADE}: an effect and a penalty, each a key of the weapon\'s parameters), and what it is here:\n'
            '# recoil a factor on the six kick bounds, aim_time on parameters/aim_time (grips.py has the rule),\n'
            '# price in silver, icon its cell [row, column] of the lobby\'s LobbyMenu_AmmoIcons.\n'
            '# dict id\tmodule\t0.69d0 dict id\teffect\tpenalty\trecoil\taim_time\tprice\ticon\n')
    return head + ''.join(
        f'{g.dict_id}\t{g.module}\t{g.donor_dict_id}\t{g.effect.key} {g.effect.value:g}\t{g.penalty.key} {g.penalty.value:g}'
        f'\t{g.recoil:g}\t{g.aim_time:g}\t{lib.PRICE}\t{g.icon[0]} {g.icon[1]}\n' for g in grips)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', type=Path, required=True)
    parser.add_argument('--modules', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there; the pack goes in a new folder')
    grips = lib.grips(args.donor)
    files, icons = {}, []
    for grip in grips:
        if not (args.modules / 'files/models' / (grip.model + '.model')).is_dir():
            sys.exit(f'{grip.module}: the pack {args.modules} has no model {grip.model}')
        files[grip.config] = encoded(config(grip))
        row, column = grip.icon
        source = f'icons/{lib.ICON_ATLAS}/r{row:02}c{column:02}.png'
        picture = io.BytesIO()
        render_icon.framed(Image.open(args.donor / 'icons' / f'{grip.module}.png')).save(picture, 'PNG')
        files[source] = picture.getvalue()
        icons.append({'row': row, 'col': column, 'source': source})
    operations = [{'op': 'add_file', 'path': path} for path in files if not path.startswith('icons/')]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': [
        {'base_cost': lib.PRICE, 'cfg_name': grip.config, 'dict_id': grip.dict_id, 'is_premium': False,
         'is_stack': False, 'item_category': CATEGORY, 'name': grip.item} for grip in grips]})
    operations.append({'op': 'atlas_icons_add', 'swf': 'flash_movies/lobby_menu.swf', 'symbol': lib.ICON_ATLAS,
                       'icons': icons})
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': f"0.69d0's {len(grips)} foregrips as items of a weapon's kit slot 5 "
                       f'({grips[0].dict_id}-{grips[-1].dict_id}): their configs, dictionary rows and icons '
                       '(srp-026g/docs/notes/attachments-grips.md)',
        'operations': operations,
        'server': {'items': [{'dict_id': grip.dict_id} for grip in grips]},
        'notes': ['made by srp-026g/scripts/weapon-grips/make_pack.py from 0.69d0\'s game.options and pictures '
                  '(donor.sh, donor.sha256) and scripts/weapon-modules/ids.tsv; the models are the pack '
                  'weapon-modules\''] + [f'{g.dict_id} {g.item}: recoil {lib.percent(g.recoil)} %'
                                         + (f', aim time {lib.percent(g.aim_time)} %' if g.aim_time else '')
                                         for g in grips],
    }
    for path, data in files.items():
        target = args.out / (path if path.startswith('icons/') else 'files/' + path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = hashlib.sha256((args.out / 'pack.json').read_bytes()).hexdigest()
    print(f'{args.out}: {len(grips)} items, pack.json sha256 {digest}')
    row = f'pack\t{PACK_ID}@{VERSION}\t{digest}\t'
    items = [f'{grip.dict_id}\t{grip.item}\t{lib.PRICE}\tfalse\t\t' for grip in grips]
    if args.check:
        repo = HERE.parents[2]
        stale = [what for what, ok in (
            ('grips.tsv', (HERE / 'grips.tsv').read_text(encoding='utf-8') == table(grips)),
            (f'release/content.tsv ({row.strip()})', any(
                line.startswith(row) for line in (repo / 'release/content.tsv').read_text(encoding='utf-8').splitlines())),
            ('content-edit/items.tsv', set(items) <= set(
                (HERE.parent / 'content-edit/items.tsv').read_text(encoding='utf-8').splitlines()))) if not ok]
        if stale:
            sys.exit('not what make_pack.py makes: ' + '; '.join(stale))
        return
    (HERE / 'grips.tsv').write_text(table(grips), encoding='utf-8')
    print(f'manifest row: {row}{pack["description"]}')
    print('items.tsv rows:')
    print('\n'.join(items))


if __name__ == '__main__':
    main()
