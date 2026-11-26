#!/usr/bin/env python3
"""The lobby movie's scope ways: which weapons take an optic in more than one place, and what each
place is called (docs/notes/scope-mounts.md).

usage: python3 -I scope_mounts.py [<modules-069d.json>] [--check]

<modules-069d.json>  the record of the weapon-modules pack's build (resource-porter's port-modules);
                     default: the one beside the pack release/content.tsv lists (the main checkout's
                     private/content-packs/weapon-modules-<version>.modules-069d.json)

Writes ../../flash/lobby_menu/tables/survarium/data/ScopeMounts.tsv (the class
survarium.data.ScopeMounts): a row a (weapon, optic) of every gun item of the record whose scope fit
has other ways (its plan's, or its base gun's when it takes those: `fits_like`), its ways' names in
their order (weapon_modules::mounts: the fit's slot first, then its `others`), each the place on the
gun where the way's first part goes (WHERE): the movie's buttons. Every way of one row has another
name, and a place WHERE does not name stops it. The optics' dict ids are ids.tsv's. --check only says
whether the table is what the record gives (exit status 1 if not).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
TABLE = HERE.parent.parent / 'flash/lobby_menu/tables/survarium/data/ScopeMounts.tsv'
# The place a way is, by the gun's slot its first part goes in (the carrier's, or the optic's own
# when the gun has the slot), and by the carrier for a slot that is another place on other guns.
WHERE = {
    'scope_dovetail': 'receiver',
    'receivercover': 'receiver',
    'picatinny_scope_medium': 'receiver',
    'forend': 'handguard',
    'picatinny_scope_handguard': 'handguard',
    'picatinny_scope_handguard_svu': 'handguard',
}
CARRIED = {
    'barrel_bracket_01_picatinnyrail': 'barrel',
    'aks74u_zenit_b18_picatinnyrail': 'gas_tube',
}
HEAD = ('# The Attachments window\'s scope ways: the weapons that take an optic in more than one place, one row\n'
        '# a (weapon dict id, optic dict id); ways the places in the order of the way the kit record holds\n'
        '# (0 the first, weapon_modules::mounts), each the key of its button\'s text\n'
        '# (survarium.upgrade.ScopeMount). A pair with no row has one way and no buttons. The lobby build\n'
        '# makes it the class survarium.data.ScopeMounts (tools/actionscript/README.md, "Tables"); its\n'
        '# list fits must have a row (the movie reads ScopeMounts.fits). Written by\n'
        '# srp-026g/scripts/weapon-modules/scope_mounts.py from the listed weapon-modules pack\'s record:\n'
        '# do not edit.\n'
        'list\tweapon:int\toptic:int\tways:String\n')


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


def place(gun, way):
    """The name of a way's place on the gun ('receiver' ...)."""
    chain = way['chain']
    name = CARRIED.get(chain[0]['module']) if chain else None
    name = name or WHERE.get(chain[0]['slot'] if chain else way['slot'])
    if name is None:
        sys.exit(f'{gun}: a way in {way["slot"]} through {[link["module"] for link in chain]}: no place of WHERE')
    return name


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
            if fit.get('kind') != 'scope' or not fit.get('others'):
                continue
            ways = [place(name, way) for way in [fit] + fit['others']]
            if len(set(ways)) != len(ways):
                sys.exit(f'{name}, {fit["module"]}: two ways in one place {ways}: name them apart in WHERE')
            out[(weapon, ids[fit['module']])] = ','.join(f'st_srp_mount_{way}' for way in ways)
    if not out:
        sys.exit('the record has no scope of more than one way: the movie needs a row')
    return ''.join(f'fits\t{w}\t{o}\t{ways}\n' for (w, o), ways in sorted(out.items()))


def main():
    args = [a for a in sys.argv[1:] if a != '--check']
    path = Path(args[0]) if args else listed_record()
    text = HEAD + rows(json.loads(path.read_text()))
    if '--check' in sys.argv[1:]:
        if not TABLE.is_file() or TABLE.read_text() != text:
            print(f'{TABLE} is not what {path} gives: run scope_mounts.py', file=sys.stderr)
            sys.exit(1)
        print(f'{TABLE.name}: as {path.name} gives')
        return
    TABLE.write_text(text)
    print(f'{TABLE}: {text.count(chr(10)) - HEAD.count(chr(10))} rows')


if __name__ == '__main__':
    main()
