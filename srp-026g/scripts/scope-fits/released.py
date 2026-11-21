#!/usr/bin/env python3
"""The released scope pairs: released-pairs.tsv, made once from release 68's game data.

usage: python3 -I released.py <players' files>

<players' files>  release 68's gameplay/db_static_dictionaries, gameplay/items/scopes/* and
                  gameplay/items/weapons/* (the stock db and every patch of release/patches.tsv)

Up to release 68 a scope went on a weapon by a variant, a category-22 item made for the pair whose
config names it (`srp_scope_fit { scope, weapons }`, docs/notes/weapon_scopes.md). Every such pair is
one a player may have: released-pairs.tsv lists each with the variant that showed it (of two, the
lower dict id, as the server kept). It is what "no released pair is lost" is checked against from
now on (srp-check-scope-fits) and what the modules table's legacy rows are made of (resource-porter's
port-modules --released): a pair 0.69d0 does not give stands where its variant put it.
The file is a record of release 68: it is never made again from later data.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as cfgs  # noqa: E402

HEAD = ('# The (weapon, scope) pairs released up to revision 68 (released.py; a record, never made again).\n'
        '# weapon\tweapon dict id\tscope\tscope dict id\tvariant dict id\tvariant config\n')


def main():
    live = Path(sys.argv[1])
    items = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']
    by_name = {r['name']: r for r in items}
    by_id = {r['dict_id']: r for r in items}

    def named(value):
        return by_name[value] if isinstance(value, str) else by_id[value]
    pairs = {}
    for row in sorted(items, key=lambda r: r['dict_id']):
        if row['item_category'] != 22:
            continue
        fit = cfgs.config(live / row['cfg_name']).get('srp_scope_fit')
        if not fit:
            continue
        weapons = fit['weapons'] if isinstance(fit['weapons'], list) else [fit['weapons']]
        for weapon in weapons:
            pairs.setdefault((named(weapon)['dict_id'], named(fit['scope'])['dict_id']), row)
    lines = [f"{by_id[w]['name']}\t{w}\t{by_id[s]['name']}\t{s}\t{v['dict_id']}\t{v['cfg_name']}\n"
             for (w, s), v in sorted(pairs.items())]
    (HERE / 'released-pairs.tsv').write_text(HEAD + ''.join(lines))
    print(f'{len(lines)} pairs of {len({s for _, s in pairs})} scopes on {len({w for w, _ in pairs})} weapons')


if __name__ == '__main__':
    main()
