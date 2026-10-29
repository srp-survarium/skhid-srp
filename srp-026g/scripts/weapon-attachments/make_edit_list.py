#!/usr/bin/env python3
"""make_edit_list.py: the edit list of the muzzle devices' weapons (content-edit's
weapon-muzzle-fit-<n>.json), from devices.tsv, fit.tsv and offsets.tsv.

usage: python3 -I make_edit_list.py > ../content-edit/weapon-muzzle-fit-1.json

- items_compatibility rows of gameplay/db_static_dictionaries, one for each weapon a device fits
  (fit.tsv), named compatibility_<device dict id><weapon dict id, 5 digits>: first_item the
  weapon, second_item the device. The server puts a device only on a weapon of such a row
  (survarium::v026g::modifications), the lobby movie lights its slot only for one
  (UpgradeWindow.addCompatibility, fits).
- data/srp_muzzle_offset of each weapon config of offsets.tsv whose offset is not 0 (client patch
  68 starts the device that far along the barrel from barrel_point).
"""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
DICTIONARY = 'gameplay/db_static_dictionaries'


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def main():
    ids = {row[1]: int(row[0]) for row in rows('devices.tsv')}
    pairs = []
    for name, weapons in rows('fit.tsv'):
        for weapon in weapons.split():
            _, weapon_id = weapon.split(':')
            pairs.append((ids[name], int(weapon_id)))
    compatibility = [{'files': [DICTIONARY], 'table': 'items_compatibility',
                      'key': f'compatibility_{device}{weapon:05d}',
                      'value': {'first_item': weapon, 'second_item': device}}
                     for device, weapon in sorted(pairs)]
    offsets = [{'files': [config], 'table': 'data', 'key': 'srp_muzzle_offset', 'value': float(offset)}
               for _, config, offset, _ in rows('offsets.tsv') if float(offset) != 0]
    about = ("The weapon kit's muzzle devices (pack weapon-attachments; srp-026g/docs/notes/weapon-kit.md): "
             f"{len(compatibility)} items_compatibility rows, each weapon a device fits as 0.46c2 gave that weapon "
             "the device's model (scripts/weapon-attachments/fit.tsv), and data/srp_muzzle_offset of "
             f"{len(offsets)} weapons whose stock muzzle device reaches past barrel_point "
             "(scripts/weapon-attachments/offsets.tsv). Made by scripts/weapon-attachments/make_edit_list.py.")
    lines = ['{', ' "_about": ' + json.dumps(about, ensure_ascii=False) + ',', ' "add_value": [']
    entries = compatibility + offsets
    lines += ['  ' + json.dumps(e, ensure_ascii=False) + (',' if i < len(entries) - 1 else '')
              for i, e in enumerate(entries)]
    lines += [' ]', '}']
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
