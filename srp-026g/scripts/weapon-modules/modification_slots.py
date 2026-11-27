#!/usr/bin/env python3
"""The lobby movie's modification markers (0.69d0's workshop, docs/notes/modification-069.md): the
slots of the prototype's guns a marker stands at, what each takes, the carriers that bring slots, and
the slot each way of an optic stands in.

usage: python3 -I modification_slots.py [<modules-069d.json>] [--check]

<modules-069d.json>  the record of the weapon-modules pack's build (resource-porter's port-modules);
                     default: the one beside the pack release/content.tsv lists (the main checkout's
                     private/content-packs/weapon-modules-<version>.modules-069d.json)

Writes three tables of ../../flash/lobby_menu/tables/survarium/data/ (the classes survarium.data.*):

ModificationSlots     a row a slot of a gun of GUNS where a player puts something: the gun's slot a
                      muzzle device, grip or optic fit of its plan goes in (the optics' every way), or
                      a carrier its plan names goes in; its list what of the weapon kit it takes
                      (muzzle, grip, scope; plain: only carriers), its name's text key (NAMES), the row
                      of markers it is in (above or below the gun), whether the gun has its own part
                      there (the first choice is that one, else nothing), its place along the gun
                      (ORDER, the muzzle first).
ModificationCarriers  a row a carrier of the plan: the slot it goes in, its 0.69d0 module and text key
                      (CARRIERS), the slots it brings.
ModificationWays      a row a way of an optic onto the gun: its dict id (ids.tsv), the way's number
                      (weapon_modules::mounts: the fit's slot first, then its `others`), the slot it
                      stands in.

A slot is its byte of weapon_modules::SlotId (crates/weapon-modules/src/slot_ids.rs), as the exe's
points name it. A slot, carrier or chain of more than one carrier the tables do not name stops it.
--check only says whether the tables are what the record gives (exit status 1 if not).
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DATA = HERE.parent.parent / 'flash/lobby_menu/tables/survarium/data'
SLOT_IDS = HERE.parent.parent / 'crates/weapon-modules/src/slot_ids.rs'
# The prototype's guns, by their item.
GUNS = ('ak_74m',)
# A slot's name and its row of markers.
NAMES = {
    'm24x15_muzzle': ('st_srp_muzzle_slot', 'below'),
    'picatinny_scope_handguard': ('st_srp_modification_handguard_rail', 'above'),
    'picatinny_grip': ('st_srp_grip_slot', 'below'),
    'forend': ('st_srp_mount_handguard', 'below'),
    'picatinny_scope_medium': ('st_srp_modification_receiver_rail', 'above'),
    'scope_dovetail': ('st_srp_modification_dovetail', 'above'),
}
# The slots from the muzzle back: each row's markers stand in this order along the gun.
ORDER = ('m24x15_muzzle', 'picatinny_scope_handguard', 'picatinny_grip', 'forend', 'picatinny_scope_medium',
         'scope_dovetail')
CARRIERS = {
    'ak_midwest_extended_tactical_forend': 'st_srp_modification_tactical_forend',
    'custom_dovetail_01_picatinnyrail': 'st_srp_modification_rail_adapter',
}
KITS = {'muzzle': 'muzzle', 'grip': 'grip', 'scope': 'scope'}


def head(about, columns):
    return (''.join(f'# {line}\n' for line in about) +
            '# Written by srp-026g/scripts/weapon-modules/modification_slots.py from the listed weapon-modules\n'
            '# pack\'s record: do not edit.\n' + '\t'.join(['list'] + columns) + '\n')


HEADS = {
    'ModificationSlots': head(
        ['The modification markers\' slots (survarium.upgrade.ModuleTree): a row a slot of a gun where a player',
         'puts something; the list what of the weapon kit it takes (muzzle, grip, scope; plain: only carriers),',
         'its name\'s text key, its row of markers, own 1 when the gun has its own part there, order its place',
         'along the gun from the muzzle. A slot is its weapon_modules::SlotId byte.'],
        ['weapon:int', 'slot:int', 'name:String', 'row:String', 'own:int', 'order:int']),
    'ModificationCarriers': head(
        ['The carriers of the modification markers\' guns: the slot each goes in, its 0.69d0 module, its',
         'name\'s text key, the slots it brings (their bytes).'],
        ['weapon:int', 'slot:int', 'module:String', 'name:String', 'brings:String']),
    'ModificationWays': head(
        ['The ways of an optic onto a modification markers\' gun: the way\'s number (the kit record\'s,',
         'weapon_modules::mounts) and the slot it stands in.'],
        ['weapon:int', 'optic:int', 'way:int', 'slot:int']),
}


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


def slot_bytes():
    return {name: int(number) for number, name in re.findall(r'= (\d+), "([a-z0-9_]+)";', SLOT_IDS.read_text())}


def tables(record):
    ids, numbers, items = module_ids(), slot_bytes(), record['items']
    out = {name: [] for name in HEADS}
    for item in GUNS:
        weapon, donor = items[item]
        gun = record['guns'][donor]
        plan = gun['plan']
        takes, own = {}, {part['slot'] for part in plan['parts'] if part['stock']}
        ways = []
        for fit in plan['fits']:
            kind = KITS.get(fit['kind'])
            if kind is None:
                continue
            slots = [fit['slot']] + [other['slot'] for other in fit.get('others', [])] if kind == 'scope' else [fit['slot']]
            for way, slot in enumerate(slots):
                takes.setdefault(slot, set()).add(kind)
                if kind == 'scope':
                    ways.append((ids[fit['module']], way, slot))
        carriers = {}
        for brought, chain in plan['carriers'].items():
            if len(chain) != 1:
                sys.exit(f'{item}: {brought} comes with a chain of {len(chain)} carriers: the markers take one')
            (link,) = chain
            carriers.setdefault((link['slot'], link['module']), []).append(brought)
            takes.setdefault(link['slot'], set())
        for slot, kinds in takes.items():
            if slot not in NAMES:
                sys.exit(f'{item}: a slot {slot} the markers do not name: give it NAMES and ORDER')
            if len(kinds) > 1:
                sys.exit(f'{item}: {slot} takes {sorted(kinds)}: a slot takes one kind of the kit')
        for slot in sorted(takes, key=ORDER.index):
            name, row = NAMES[slot]
            (kind,) = takes[slot] or {'plain'}
            out['ModificationSlots'].append(
                f'{kind}\t{weapon}\t{numbers[slot]}\t{name}\t{row}\t{int(slot in own)}\t{ORDER.index(slot)}\n')
        for (slot, module), brings in sorted(carriers.items(), key=lambda entry: ORDER.index(entry[0][0])):
            if module not in CARRIERS:
                sys.exit(f'{item}: a carrier {module} the markers do not name: give it CARRIERS')
            out['ModificationCarriers'].append(
                f'carriers\t{weapon}\t{numbers[slot]}\t{module}\t{CARRIERS[module]}\t'
                f'{",".join(str(numbers[s]) for s in sorted(brings, key=ORDER.index))}\n')
        out['ModificationWays'] += [f'ways\t{weapon}\t{optic}\t{way}\t{numbers[slot]}\n' for optic, way, slot in sorted(ways)]
    return {name: HEADS[name] + ''.join(rows) for name, rows in out.items()}


def main():
    args = [a for a in sys.argv[1:] if a != '--check']
    path = Path(args[0]) if args else listed_record()
    made = tables(json.loads(path.read_text()))
    if '--check' in sys.argv[1:]:
        stale = [name for name, text in made.items()
                 if not (DATA / f'{name}.tsv').is_file() or (DATA / f'{name}.tsv').read_text() != text]
        if stale:
            print(f'{", ".join(stale)}: not what {path} gives: run modification_slots.py', file=sys.stderr)
            sys.exit(1)
        print(f'{", ".join(made)}: as {path.name} gives')
        return
    for name, text in made.items():
        (DATA / f'{name}.tsv').write_text(text)
        print(f'{DATA / name}.tsv: {text.count(chr(10)) - HEADS[name].count(chr(10))} rows')


if __name__ == '__main__':
    main()
