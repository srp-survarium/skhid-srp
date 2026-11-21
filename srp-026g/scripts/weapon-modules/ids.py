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
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as cfgs  # noqa: E402

FIRST = 2601
HEAD = ('# The fitted modules\' dict ids (ids.py; append-only): 0.69d0 module, our dict id, its kind, our item.\n'
        '# No item is made for an id no gun takes in the built table; the id stays taken: 2652-2655 (the VSSK\'s\n'
        '# barrels, its suppressor in other shapes), 2667, 2669, 2670 (the OTs-14\'s and the SR-3M\'s own foregrips).\n'
        '# module\tdict id\tkind\titem\n')


def main():
    plan, live = Path(sys.argv[1]), Path(sys.argv[2])
    table = HERE / 'ids.tsv'
    rows = [line.split('\t') for line in table.read_text().splitlines()
            if line and not line.startswith('#')] if table.exists() else []
    have = {r[0] for r in rows}
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


if __name__ == '__main__':
    main()
