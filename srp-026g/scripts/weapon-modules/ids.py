#!/usr/bin/env python3
"""The fitted modules' dict ids: ids.tsv (docs/notes/attachments.md, the table's `fits`).

usage: python3 -I ids.py <plan> <players' files> [--write]

<plan>            the output of resource-porter's `plan-modules-069d <game.options> <sidecar>`: each
                  gun's fits as 0.69d0's data gives them
<players' files>  the players' gameplay/db_static_dictionaries (the dict ids in use)

A module a player fits needs an item, and an item a dict id. ids.tsv is append-only: a row is never
changed, a number never given to another module.
- A scope's id is its shop item's (../scope-fits/optics.tsv says which 0.69d0 module each sold scope
  is): ownership does not change.
- A muzzle device or grip 0.69d0 sells (it rolls stats on it) that fits a gun of ours takes the next
  number from 2601 that the dictionary and this table do not use, in the order of 0.69d0's ids,
  muzzle devices first. (2601-2635 are the parked muzzle port's, kept for the same modules.)
Without --write it only says what it would add.

usage: python3 -I ids.py --movie [--check]

The lobby movie knows a muzzle device's and a grip's slot by a table made of ids.tsv,
../../flash/lobby_menu/tables/survarium/data/ModuleKinds.tsv (the class survarium.data.ModuleKinds),
and what each device and grip changes on its weapon by AttachmentStats.tsv beside it, made of
../muzzle-devices/devices.tsv and ../weapon-grips/grips.tsv: written at every --write and by
--movie; --movie --check only says whether they are current (exit status 1 if not).
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

FIRST = 2601
HEAD = ('# The fitted modules\' dict ids (ids.py; append-only): 0.69d0 module, our dict id, its kind, our item.\n'
        '# No item is made for an id no gun takes in the built table; the id stays taken: 2652-2655 (the VSSK\'s\n'
        '# barrels, its suppressor in other shapes), 2667, 2669, 2670 (the OTs-14\'s and the SR-3M\'s own foregrips).\n'
        '# module\tdict id\tkind\titem\n')


TABLE = HERE / 'ids.tsv'
MOVIE = HERE.parent.parent / 'flash/lobby_menu/tables/survarium/data/ModuleKinds.tsv'
STATS = MOVIE.with_name('AttachmentStats.tsv')
DEVICES = HERE.parent / 'muzzle-devices/devices.tsv'
GRIPS = HERE.parent / 'weapon-grips/grips.tsv'
# A stat a device or a grip changes (a share of the weapon's own value: modifier type 2), as the
# card shows the game's own weapon roll of the same key: that roll's line (its ui_desc prop_id) and
# icon (gameplay/items/modifications: 2070.. aim time, 2080.. the six kick bounds, 2020.. barrel
# rifling). The weapon's card and Alt then count it as such a roll (UpgradeWindow.withFitted).
STAT_UI = {
    'parameters/aim_time': {'prop': 102, 'icon': 39},
    'recoil': {'prop': 115, 'icon': 40},
    'dispersion/base_dispersion': {'prop': 2, 'icon': 34},
}


def read_rows():
    return [line.split('\t') for line in TABLE.read_text().splitlines()
            if line and not line.startswith('#')] if TABLE.exists() else []


def movie_table(rows):
    """The lobby's ModuleKinds of ids.tsv's rows: a list a kind the window has a slot for (scopes are
    the shop's own range), a dict id a row."""
    head = ('# The Attachments window\'s slot of a muzzle device or a grip, by its dict id. Written by\n'
            '# srp-026g/scripts/weapon-modules/ids.py from ids.tsv: do not edit. The lobby build makes it the\n'
            '# class survarium.data.ModuleKinds (tools/actionscript/README.md, "Tables").\n'
            'list\tdict:int\n')
    return head + ''.join(f'{kind}\t{row[1]}\n' for kind in ('muzzle', 'grip') for row in rows if row[2] == kind)


def table_rows(path):
    return [line.split('\t') for line in path.read_text().splitlines() if line and not line.startswith('#')]


def stats_table():
    """The lobby's AttachmentStats: a row a stat of a device (devices.tsv's stats, `key share; ...`)
    or a grip (grips.tsv's recoil and aim_time), in dict id order."""
    found = []
    for row in table_rows(DEVICES):
        for stat in filter(None, (part.strip() for part in row[6].split(';'))):
            if stat == '-':
                continue
            key, share = stat.rsplit(' ', 1)
            found.append((int(row[0]), key, float(share)))
    for row in table_rows(GRIPS):
        for key, share in (('recoil', row[5]), ('parameters/aim_time', row[6])):
            if float(share):
                found.append((int(row[0]), key, float(share)))
    head = ('# What a muzzle device or a grip changes on its weapon: a row a stat, its share of the weapon\'s\n'
            '# own value (modifier type 2), the card line and icon of the game\'s weapon roll of that key.\n'
            '# Written by srp-026g/scripts/weapon-modules/ids.py from devices.tsv and grips.tsv: do not edit.\n'
            '# The lobby build makes it the class survarium.data.AttachmentStats.\n'
            'list\tdict:int\tshare:String\tprop:int\ticon:int\n')
    out = []
    for dict_id, key, share in sorted(found, key=lambda f: f[0]):
        if key not in STAT_UI:
            sys.exit(f'{dict_id}: no card line for the stat {key} (ids.py STAT_UI)')
        ui = STAT_UI[key]
        out.append(f"stats\t{dict_id}\t{share:g}\t{ui['prop']}\t{ui['icon']}\n")
    return head + ''.join(out)


def movie(check):
    for path, text in ((MOVIE, movie_table(read_rows())), (STATS, stats_table())):
        if not check:
            path.write_text(text)
        elif not path.is_file() or path.read_text() != text:
            sys.exit(f'{path} is not what its sources give: python3 -I {Path(__file__).name} --movie')


def main():
    if sys.argv[1:2] == ['--movie']:
        return movie('--check' in sys.argv)
    plan, live = Path(sys.argv[1]), Path(sys.argv[2])
    table = TABLE
    rows = read_rows()
    have = {r[0] for r in rows}
    sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
    import model as cfgs  # the players' dictionary (NumPy)
    items = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']
    by_name = {r['name']: r['dict_id'] for r in items}
    used = set(by_name.values()) | {int(r[1]) for r in rows}
    fits = {'muzzle': set(), 'grip': set(), 'scope': set()}
    for line in plan.read_text().splitlines():
        words = line.split()
        if words[:1] == ['fit']:
            fits[words[1].lower()].add(words[2])
    new = []
    for line in (HERE.parent / 'scope-fits/optics.tsv').read_text().splitlines():
        if not line or line.startswith('#'):
            continue
        item, module = line.split('\t')[:2]
        if module not in have:
            if module not in fits['scope']:
                print(f'{module} ({item}): fits no gun of the plan', file=sys.stderr)
            new.append([module, str(by_name[item]), 'scope', item])
    next_id = FIRST
    for kind in ('muzzle', 'grip'):
        for module in sorted(fits[kind] - have):
            while next_id in used:
                next_id += 1
            used.add(next_id)
            new.append([module, str(next_id), kind, f'srp_{kind}_{module}'])
    for row in new:
        print('\t'.join(row))
    print(f'{len(rows)} rows, {len(new)} new', file=sys.stderr)
    if '--write' in sys.argv and new:
        table.write_text(HEAD + ''.join('\t'.join(r) + '\n' for r in rows + new))
        movie(check=False)


if __name__ == '__main__':
    main()
