#!/usr/bin/env python3
"""The guns' iron sights at 0.69d0's eye: ../content-edit/iron-sights-1.json.

usage: python3 -I iron_sights.py <players' files>
       python3 -I iron_sights.py --check

<players' files>  the players' gameplay/items/weapons/* (what `set` starts from)

With no optic the game's eye is the gun's whole body's own `aim` locator, moved along its line by the
gun's `parameters/aim_offset` (calculate_aim_alignment 0x244D40, the matrix the cook makes at
0x24A8E7). 0.69d0's iron sights keep the gun's eye relief instead: every one of its `ironsight`
modules sets the eye with `plane_slot: body_aim`, at the gun's eye plane. iron_sights.tsv holds, for
every gun of the modules table content.tsv ships, how far that body's eye goes along its line to the
plane (weapon_modules::iron_sights; the crate's test `the_shipped_table` writes and checks the file).
Each gun's aim_offset becomes it, so that with nothing fitted, and with only a muzzle device or a
grip, the eye is 0.69d0's; a fitted optic's modification sets aim_offset to 0 as before
(scope-fits/modules_edit.py), its eye the table's. Left as they are: the guns with an optic of their
own (addons/rifle_scope_dict_id), whose aim_offset is that optic's eye relief and whose iron sights
are never seen.
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
EDITS = HERE.parent / 'content-edit'
OUT = EDITS / 'iron-sights-1.json'
PATH = 'parameters/aim_offset'
ABOUT = ("Each gun's parameters/aim_offset is its iron sights' 0.69d0 eye: the whole body's eye along its line to "
         "the gun's eye plane (body_aim), from scripts/weapon-modules/iron_sights.tsv (made by iron_sights.py); a gun "
         "with an optic of its own keeps its own (_left_out).")


def offsets():
    """{gun dict id: aim_offset} of iron_sights.tsv."""
    rows = [line.split('\t') for line in (HERE / 'iron_sights.tsv').read_text().splitlines() if not line.startswith('#')]
    return {int(gun): float(value) for gun, value in rows}


def guns():
    """{config's game path: its gun's dict id}, as weapon-modules-1.json names them."""
    lists = json.loads((EDITS / 'weapon-modules-1.json').read_text())['add_value']
    return {path: edit['value'] for edit in lists if edit['key'] == 'srp_gun' for path in edit['files']}


def make(live):
    sys.path.insert(0, str(EDITS))
    import content_edit as ce
    want = offsets()
    edits = {'_about': ABOUT, 'set': [], 'add_value': [], '_left_out': []}
    for path, gun in sorted(guns().items()):
        buf = bytearray((live / path).read_bytes())
        values = {p: (t, v) for p, t, v in ce.walk(buf)}
        if '/addons/rifle_scope_dict_id' in values:
            optic = ce.as_number(*values['/addons/rifle_scope_dict_id'])
            edits['_left_out'].append({'file': path, 'why': f'its own optic, addons/rifle_scope_dict_id {optic}'})
            continue
        to = want[gun]
        if '/' + PATH in values:
            t, raw = values['/' + PATH]
            edit = {'file': path, 'path': PATH, 'from': round(ce.as_number(t, raw), 6), 'to': to}
            edits['set'].append(edit | ({'as_float': True} if t == ce.T_INT else {}))
        else:
            edits['add_value'].append({'files': [path], 'table': 'parameters', 'key': 'aim_offset', 'value': to})
    OUT.write_text(json.dumps(edits, indent=1, ensure_ascii=False) + '\n')
    print(f'{OUT.name}: {len(edits["set"])} set, {len(edits["add_value"])} added, {len(edits["_left_out"])} left out')


def check():
    """Every gun of weapon-modules-1.json once: its aim_offset iron_sights.tsv's, or left out for an
    optic of its own."""
    want, edits = offsets(), json.loads(OUT.read_text())
    taken = [(e['file'], e['to']) for e in edits['set'] if e['path'] == PATH]
    taken += [(e['files'][0], e['value']) for e in edits['add_value']
              if (e['table'], e['key'], len(e['files'])) == ('parameters', 'aim_offset', 1)]
    left = [e['file'] for e in edits['_left_out'] if re.fullmatch(r'its own optic, addons/rifle_scope_dict_id \d+', e['why'])]
    named = guns()
    files = [path for path, _ in taken] + left
    problems = [f'{path}: aim_offset {value}, the table says {want[named[path]]}'
                for path, value in taken if path in named and value != want[named[path]]]
    if sorted(files) != sorted(named) or len(edits['set']) + len(edits['add_value']) != len(taken):
        problems.append(f'{OUT.name} is not one edit or one left-out row for each of the {len(named)} guns')
    if set(want) != set(named.values()):
        problems.append('iron_sights.tsv is not of the guns weapon-modules-1.json names')
    if problems:
        sys.exit('\n'.join(problems + ['(python3 -I iron_sights.py <the players files>)']))
    print(f'{OUT.name}: the {len(taken)} guns without an optic of their own at iron_sights.tsv, {len(left)} left out')


def main():
    if sys.argv[1:] == ['--check']:
        return check()
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    make(Path(sys.argv[1]))


if __name__ == '__main__':
    main()
