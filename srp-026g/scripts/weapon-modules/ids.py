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
../../flash/lobby_menu/tables/survarium/data/ModuleKinds.tsv (the class survarium.data.ModuleKinds):
written at every --write and by --movie; --movie --check only says whether it is current (exit
status 1 if not).
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


def movie(check):
    text = movie_table(read_rows())
    if not check:
        MOVIE.write_text(text)
    elif not MOVIE.is_file() or MOVIE.read_text() != text:
        sys.exit(f'{MOVIE} is not what ids.tsv gives: python3 -I {Path(__file__).name} --movie')


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
