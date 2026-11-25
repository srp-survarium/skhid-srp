"""Check the first-person FOV edit against every shipped weapon item.
Pass a retail game.options dump (vostok-port dump-config --leaves) to also
compare each factor to its 0.69d0 source.
"""
import argparse
import json
import math
from pathlib import Path
import re

SCRIPTS = Path(__file__).resolve().parents[1]
CONTENT = SCRIPTS / 'content-edit'


def profiles():
    rows = (SCRIPTS / 'remodel-069/items.tsv').read_text().splitlines()
    donors = {cfg: donor for _, cfg, _, donor, _ in (row.split('\t') for row in rows if not row.startswith('#'))}
    guns = json.loads((CONTENT / 'weapon-modules-1.json').read_text())['add_value']
    ids = {cfg: edit['value'] for edit in guns if edit['key'] == 'srp_gun' for cfg in edit['files']}
    assert donors.keys() == ids.keys(), 'weapon module and remodel item sets differ'
    return {Path(cfg).stem: (ids[cfg], donor) for cfg, donor in donors.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--retail-leaves', type=Path)
    args = parser.parse_args()
    guns = profiles()
    [edit] = json.loads((CONTENT / 'weapon-view-069.json').read_text())['add_value']
    assert (edit['files'], edit['table'], edit['key']) == (['gameplay/db_static_dictionaries'], '', 'srp_weapon_views')
    rows = edit['value']
    assert rows.keys() == guns.keys(), 'view profiles must cover every gun'
    assert len({row['gun'] for row in rows.values()}) == len(rows), 'duplicate gun id'
    if args.retail_leaves:
        values = {(gun, key): float(number) for gun, key, number in re.findall(
            r'^weapons\.(\w+)\.parameters\.((?:aimed_)?weapon_fov_factor)\t(?:f32|i32):([^\n]+)',
            args.retail_leaves.read_text(), re.M)}
    for name, (gun_id, donor) in guns.items():
        row = rows[name]
        assert row['gun'] == gun_id, name
        hip, aimed = row['fov']
        assert all(math.isfinite(v) and 0 < v <= 1 for v in (hip, aimed)), name
        if args.retail_leaves:
            assert [values[donor, k] for k in ('weapon_fov_factor', 'aimed_weapon_fov_factor')] == row['fov'], donor
    print(f'{len(rows)} weapon view profiles checked' + (' against retail' if args.retail_leaves else ''))


if __name__ == '__main__':
    main()
