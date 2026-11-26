#!/usr/bin/env python3
"""The guns' first-person projection: ../content-edit/weapon-view-069.json.

usage: python3 -I view_profiles.py                      check the edit list against view_factors_069.tsv
       python3 -I view_profiles.py --write              make the edit list from it
       python3 -I view_profiles.py --retail <leaves>    make view_factors_069.tsv from 0.69d0's game.options

<leaves>  0.69d0's game.options as `vostok-port dump-config --leaves` prints it (`weapons.<item>.parameters.
          weapon_fov_factor<TAB>float:1.0`)

0.69d0's first-person camera (update_camera 0x2002C2D70) draws the held gun with 65 degrees times its
item's weapon_fov_factor at the hip, its aimed_weapon_fov_factor fully aimed, between them by the aim.
view_factors_069.tsv holds the two of every 0.69d0 item ../remodel-069/items.tsv makes a gun of, as
0.69d0 has them; the edit list puts them in the dictionary's srp_weapon_views under each gun's dict id
(weapon-modules-1.json's data/srp_gun), which patch 161 reads.
"""
import json
import re
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
FACTORS = HERE / 'view_factors_069.tsv'
EDIT = SCRIPTS / 'content-edit/weapon-view-069.json'
ABOUT = ("The 0.69d0 first-person weapon projection (patch 161): under each gun's dict id (weapon-modules-1.json), "
         "its 0.69d0 item's parameters/weapon_fov_factor and aimed_weapon_fov_factor (view_factors_069.tsv; "
         "made by scripts/weapon-modules/view_profiles.py).")
KEYS = ('weapon_fov_factor', 'aimed_weapon_fov_factor')


def items():
    """{config stem: (gun dict id, 0.69d0 item)} of every gun of remodel-069/items.tsv."""
    rows = [line.split('\t') for line in (SCRIPTS / 'remodel-069/items.tsv').read_text().splitlines()
            if line and not line.startswith('#')]
    donors = {config: donor for _, config, _, donor, _ in rows}
    lists = json.loads((SCRIPTS / 'content-edit/weapon-modules-1.json').read_text())['add_value']
    ids = {path: edit['value'] for edit in lists if edit['key'] == 'srp_gun' for path in edit['files']}
    if donors.keys() != ids.keys():
        sys.exit('remodel-069/items.tsv and weapon-modules-1.json are not of the same guns')
    return {Path(config).stem: (ids[config], donor) for config, donor in donors.items()}


def factors():
    """{0.69d0 item: [hip, aimed]} of view_factors_069.tsv."""
    rows = [line.split('\t') for line in FACTORS.read_text().splitlines() if not line.startswith('#')]
    return {donor: [float(hip), float(aimed)] for donor, hip, aimed in rows}


def edit():
    """The edit list, as view_factors_069.tsv makes it."""
    have = factors()
    value = {stem: {'gun': gun, 'fov': have[donor]} for stem, (gun, donor) in sorted(items().items())}
    return {'_about': ABOUT, 'add_value': [{'files': ['gameplay/db_static_dictionaries'], 'table': '',
                                            'key': 'srp_weapon_views', 'value': value}]}


def retail(leaves):
    """view_factors_069.tsv from 0.69d0's leaves: every donor's two factors, as its float32."""
    found = {(item, key): float(number) for item, key, number in re.findall(
        r'^weapons\.(\w+)\.parameters\.(\w+)\t(?:float|int):(\S+)$', leaves.read_text(), re.M) if key in KEYS}
    donors = sorted({donor for _, donor in items().values()})
    shortest = lambda v: float(f'{struct.unpack("<f", struct.pack("<f", v))[0]:.7g}')
    lines = [f'# 0.69d0 item\t{KEYS[0]}\t{KEYS[1]} (its parameters in 0.69d0 game.options; view_profiles.py --retail)']
    lines += [f'{donor}\t' + '\t'.join(repr(shortest(found[donor, key])) for key in KEYS) for donor in donors]
    FACTORS.write_text('\n'.join(lines) + '\n')
    print(f'{FACTORS.name}: {len(donors)} items')


def main():
    args = sys.argv[1:]
    if args[:1] == ['--retail'] and len(args) == 2:
        return retail(Path(args[1]))
    if args == ['--write']:
        EDIT.write_text(json.dumps(edit(), indent=1) + '\n')
        return print(f'{EDIT.name} written')
    if args:
        sys.exit(__doc__)
    want = edit()
    if json.loads(EDIT.read_text()) != want:
        sys.exit(f'{EDIT.name} is not view_factors_069.tsv\'s (python3 -I view_profiles.py --write)')
    rows = want['add_value'][0]['value']
    if len({row['gun'] for row in rows.values()}) != len(rows) or not all(0 < f <= 1 for row in rows.values() for f in row['fov']):
        sys.exit('a gun twice, or a factor outside (0, 1]')
    print(f'{EDIT.name}: the {len(rows)} guns at their 0.69d0 items\' factors')


if __name__ == '__main__':
    main()
