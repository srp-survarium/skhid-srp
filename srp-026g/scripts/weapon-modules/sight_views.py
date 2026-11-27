#!/usr/bin/env python3
"""The second sights' world zoom: ../content-edit/sight-views-1.json (docs/notes/second-sights.md,
"The view (0.26g)").

usage: python3 -I sight_views.py [--live <folder>] [--check]

--live <folder>  the scope items' configs at their game paths (`srp-game-data get --db <stock> --patch
                 <each listed patch>... --out <folder> gameplay/items/scopes/<item>.options`): take each
                 second sight's fov_factor and near_plane_factor from its optic item's config into
                 sight_factors.tsv
--check          only say whether the files are what their sources give (exit status 1 if not)

Without --live, the factors are sight_factors.tsv's. A weapon aimed through a second sight zooms the world
as that optic does when it is the scope (update_camera 0x1F3480: the camera's zoom 1 + (fov_factor - 1)
x aim, its near plane (1 + (near_plane_factor - 1) x aim) x 0.05): client patch 161 reads them from the
items dictionary's srp_sight_views, one row an optic, {sight: its dict id, fov: [fov_factor,
near_plane_factor]}. The optics are every one the lobby's SecondSights table offers (that table is the
listed weapon-modules pack's record's: second_sights.py --check), their items ids.tsv's.
"""
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
SIGHTS = SCRIPTS.parent / 'flash/lobby_menu/tables/survarium/data/SecondSights.tsv'
FACTORS = HERE / 'sight_factors.tsv'
EDIT = SCRIPTS / 'content-edit/sight-views-1.json'
ABOUT = ("The second sights' world zoom (client patch 161, docs/notes/second-sights.md): under each optic the lobby's "
         "SecondSights offers as a second sight, its item's data/fov_factor and data/near_plane_factor "
         "(sight_factors.tsv, from the scope items' configs; made by scripts/weapon-modules/sight_views.py).")
HEAD = ('# The optics a weapon takes as a second sight and their items\' zoom factors, as their configs have them\n'
        '# (gameplay/items/scopes/<item>.options, data/fov_factor and data/near_plane_factor). Written by\n'
        '# srp-026g/scripts/weapon-modules/sight_views.py --live <the scope items\' configs>: do not edit.\n'
        '# optic\titem\tfov_factor\tnear_plane_factor\n')


def optics():
    """The optic dict ids SecondSights offers."""
    rows = [line.split('\t') for line in SIGHTS.read_text().splitlines() if line.startswith('fits\t')]
    found = sorted({int(optic) for _, _, optic, _ in rows})
    if not found:
        sys.exit(f'{SIGHTS} offers no second sight')
    return found


def items():
    """{dict id: item} of ids.tsv's scope modules."""
    out = {}
    for line in (HERE / 'ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            cells = line.split('\t')
            if len(cells) >= 4 and cells[2] == 'scope':
                out[int(cells[1])] = cells[3]
    return out


def shortest(value):
    """The shortest decimal of the f32 `value`."""
    bits = struct.pack('<f', value)
    for digits in range(1, 10):
        text = f'{value:.{digits}g}'
        if struct.pack('<f', float(text)) == bits:
            return float(text)
    return value


def from_configs(live):
    sys.path.insert(0, str(SCRIPTS / 'content-edit'))
    import content_edit as ce
    names, rows = items(), []
    for optic in optics():
        path = Path(live) / f'gameplay/items/scopes/{names[optic]}.options'
        values = {p: ce.as_number(t, v) for p, t, v in ce.walk(bytearray(path.read_bytes()))
                  if t in (ce.T_INT, ce.T_FLOAT)}
        try:
            fov, near = values['/data/fov_factor'], values['/data/near_plane_factor']
        except KeyError as missing:
            sys.exit(f'{path}: no {missing}')
        rows.append(f'{optic}\t{names[optic]}\t{shortest(fov)}\t{shortest(near)}\n')
    return HEAD + ''.join(rows)


def factors():
    """{optic: (item, fov_factor, near_plane_factor)} of sight_factors.tsv."""
    out = {}
    for line in FACTORS.read_text().splitlines():
        if line and not line.startswith('#'):
            optic, item, fov, near = line.split('\t')
            out[int(optic)] = (item, float(fov), float(near))
    return out


def edit_list():
    table = factors()
    missing = [optic for optic in optics() if optic not in table]
    if missing:
        sys.exit(f'{FACTORS.name} has no factors of {missing}: run sight_views.py --live')
    value = {}
    for optic in optics():
        item, fov, near = table[optic]
        if not (0 < fov <= 1 and 0 < near <= 1):
            sys.exit(f'{item}: a factor outside (0, 1]: {fov}, {near}')
        value[item] = {'sight': optic, 'fov': [fov, near]}
    edits = {'_about': ABOUT, 'add_value': [
        {'files': ['gameplay/db_static_dictionaries'], 'table': '', 'key': 'srp_sight_views', 'value': value}]}
    return json.dumps(edits, indent=1) + '\n'


def main():
    args = sys.argv[1:]
    check = '--check' in args
    live = args[args.index('--live') + 1] if '--live' in args else None
    wrong = []
    if live is not None:
        text = from_configs(live)
        if check:
            wrong += [FACTORS] if FACTORS.read_text() != text else []
        else:
            FACTORS.write_text(text)
    text = edit_list()
    if check:
        wrong += [EDIT] if EDIT.read_text() != text else []
        for path in wrong:
            print(f'{path} is not what its sources give: run sight_views.py', file=sys.stderr)
        if wrong:
            sys.exit(1)
        print(f'{EDIT.name}: as {FACTORS.name} and {SIGHTS.name} give' + (f', the factors as {live} has them' if live else ''))
        return
    EDIT.write_text(text)
    print(f'{EDIT}: {len(json.loads(text)["add_value"][0]["value"])} optics')


if __name__ == '__main__':
    main()
