#!/usr/bin/env python3
"""The lobby movie's second sights: which optics each weapon takes on its offset rail, and on which
sides (docs/notes/second-sights.md).

usage: python3 -I second_sights.py [<modules-069d.json>] [--check]

<modules-069d.json>  the record of the weapon-modules pack's build (resource-porter's port-modules);
                     default: the one beside the pack release/content.tsv lists (the main checkout's
                     private/content-packs/weapon-modules-<version>.modules-069d.json)

Writes ../../flash/lobby_menu/tables/survarium/data/SecondSights.tsv (the class
survarium.data.SecondSights): a row a (weapon, optic) of every gun item of the record, each its
plan's second-sight fits, or its base gun's when it takes those (`fits_like`), the sides a mask (1
left: angled_collim_short_l, 2 right: _r; survarium.upgrade.SightPlace). The optics' dict ids are
ids.tsv's. --check only says whether the table is what the record gives (exit status 1 if not).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
TABLE = HERE.parent.parent / 'flash/lobby_menu/tables/survarium/data/SecondSights.tsv'
SIDES = {'angled_collim_short_l': 1, 'angled_collim_short_r': 2}
HEAD = ('# The Attachments window\'s second sight: the optics each weapon takes on its offset rail, one row a\n'
        '# (weapon dict id, optic dict id); sides the rail\'s sides it goes on, a mask (1 left, 2 right:\n'
        '# survarium.upgrade.SightPlace). A weapon with no row has no second-sight slot. The lobby build\n'
        '# makes it the class survarium.data.SecondSights (tools/actionscript/README.md, "Tables"); its\n'
        '# list fits must have a row (the movie reads SecondSights.fits). Written by\n'
        '# srp-026g/scripts/weapon-modules/second_sights.py from the listed weapon-modules pack\'s record:\n'
        '# do not edit.\n'
        'list\tweapon:int\toptic:int\tsides:int\n')


def listed_record():
    root = HERE.parents[2]
    (version,) = re.findall(r'^pack\tweapon-modules@([^\t]+)\t', (root / 'release/content.tsv').read_text(), re.M)
    common = subprocess.run(['git', '-C', str(root), 'rev-parse', '--path-format=absolute', '--git-common-dir'],
                            check=True, capture_output=True, text=True).stdout.strip()
    return Path(common).parent / 'private/content-packs' / f'weapon-modules-{version}.modules-069d.json'


def module_ids():
    ids = {}
    for line in (HERE / 'ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            module, dict_id = line.split('\t')[:2]
            ids[module] = int(dict_id)
    return ids


def rows(record):
    ids, guns, items = module_ids(), record['guns'], record['items']
    out = {}
    for name, (weapon, donor) in items.items():
        gun = guns.get(donor) or guns.get(name)
        if gun is None:
            continue
        base = guns.get(gun.get('fits_like') or donor, gun)
        plan = base if base.get('plan', {}).get('fits') else gun
        for fit in plan.get('plan', {}).get('fits', []):
            if fit.get('kind') != 'second_sight':
                continue
            optic = ids[fit['module']]
            out[(weapon, optic)] = out.get((weapon, optic), 0) | SIDES[fit['slot']]
    if not out:
        sys.exit('the record has no second sight: the movie needs a row')
    return ''.join(f'fits\t{w}\t{o}\t{s}\n' for (w, o), s in sorted(out.items()))


def main():
    args = [a for a in sys.argv[1:] if a != '--check']
    path = Path(args[0]) if args else listed_record()
    text = HEAD + rows(json.loads(path.read_text()))
    if '--check' in sys.argv[1:]:
        if TABLE.read_text() != text:
            print(f'{TABLE} is not what {path} gives: run second_sights.py', file=sys.stderr)
            sys.exit(1)
        print(f'{TABLE.name}: as {path.name} gives')
        return
    TABLE.write_text(text)
    print(f'{TABLE}: {text.count(chr(10)) - HEAD.count(chr(10))} rows')


if __name__ == '__main__':
    main()
