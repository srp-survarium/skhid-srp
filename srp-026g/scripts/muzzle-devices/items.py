#!/usr/bin/env python3
"""items.py [--check]: the muzzle devices' rows of ../content-edit/items.tsv.

The items table has a row for every entry of the dictionary (release/build-content stops on one
without): a device's is its dict id, name and price of devices.tsv, no premium. Written in dict id
order (a row of ours already there is replaced). --check: writes nothing and fails when the table
is not what this gives.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import devices as dev  # noqa: E402

TABLE = HERE.parent / 'content-edit/items.tsv'


def main():
    lines = TABLE.read_text(encoding='utf-8').split('\n')
    head = next(i for i, line in enumerate(lines) if line.startswith('items\t')) + 1
    if lines[-1] != '':
        sys.exit(f'{TABLE}: no final newline')
    rows = {int(line.split('\t')[0]): line for line in lines[head:-1]}
    if list(rows) != sorted(rows):
        sys.exit(f'{TABLE}: its rows are not in dict id order')
    for device in dev.read_table():
        row = '\t'.join([str(device['dict_id']), device['item'], str(device['price']), 'false', '', ''])
        other = rows.get(device['dict_id'])
        if other is not None and other.split('\t')[1] != device['item']:
            sys.exit(f'{TABLE}: {device["dict_id"]} is {other.split(chr(9))[1]}')
        rows[device['dict_id']] = row
    text = '\n'.join(lines[:head] + [rows[key] for key in sorted(rows)]) + '\n'
    if text == TABLE.read_text(encoding='utf-8'):
        print('items.tsv has the devices\' rows')
    elif '--check' in sys.argv:
        sys.exit('items.tsv has not the muzzle devices\' rows (items.py)')
    else:
        TABLE.write_text(text, encoding='utf-8')
        print('items.tsv: written')


if __name__ == '__main__':
    main()
