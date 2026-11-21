#!/usr/bin/env python3
"""The guns' edit lists: ../content-edit/remodel-069-1.json and weapon-modules-1.json.

usage: python3 -I edits.py <players' files> <modules-069d.json>

<players' files>     the players' gameplay/db_static_dictionaries and gameplay/items/weapons/*
<modules-069d.json>  the record of the full weapon-modules build (in the pack's folder): every gun's fits
For each row of ../remodel-069/items.tsv (a live gun item, its config, its live model, its remodel):
- remodel-069-1.json: the config's object/model becomes its remodel (the whole body);
- weapon-modules-1.json: data/srp_gun, the item's own dict id: the gun's record in the modules table
  (docs/notes/attachments.md, "What the core needs of the content", 5). A config without a data
  table gets one. And in the dictionary an items_compatibility row (weapon, item) for every muzzle
  device and grip the gun takes in the built table (the record's fits; a premium item's are its
  own rows): what the lobby's Attachments window lists by, as for scopes (weapon_scopes.md). A
  row's name is its own, compatibility_<item><weapon, five digits>, as the scope rows'. The scope
  rows are the scopes lane's (../scope-fits). srp reads compatibility only for a kit's ammo slots.
  An id of ids.tsv that fits no gun gets no row and no item (the VSSK's barrels 2652-2655, the own
  foregrips 2667, 2669, 2670).
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as cfgs  # noqa: E402


def compatible(record, items, ids, taken):
    """The items_compatibility rows: (weapon dict id, item dict id) of every muzzle and grip fit."""
    modules = {}
    for line in (HERE / 'ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            module, dict_id, kind, _ = line.split('\t')
            if kind in ('muzzle', 'grip'):
                modules[module] = int(dict_id)
    assert set(items) == set(record['items']), 'the record is not of every gun of items.tsv (a quick build?)'
    pairs = set()
    for item, (_, donor) in record['items'].items():
        gun = record['guns'][donor]
        for fit in gun['plan']['fits']:
            if fit['kind'] in ('muzzle', 'grip'):
                pairs.add((ids[item], modules[fit['module']]))
    rows = [{'files': ['gameplay/db_static_dictionaries'], 'table': 'items_compatibility',
             'key': f'compatibility_{module}{weapon:05d}', 'value': {'first_item': weapon, 'second_item': module}}
            for weapon, module in sorted(pairs, key=lambda p: (p[1], p[0]))]
    assert not {r['key'] for r in rows} & taken, 'a compatibility row is there already'
    unused = sorted(set(modules.values()) - {module for _, module in pairs})
    return rows, unused


def main():
    live, record = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text())
    dictionary = cfgs.config(live / 'gameplay/db_static_dictionaries')
    ids = {r['name']: r['dict_id'] for r in dictionary['items_dict']}
    rows = [line.split('\t') for line in (HERE.parent / 'remodel-069/items.tsv').read_text().splitlines()
            if line and not line.startswith('#')]
    models, guns = [], []
    for item, config, live_model, _, remodel in rows:
        cfg = cfgs.config(live / config)
        assert cfg['object']['model'] == live_model, (item, cfg['object']['model'])
        models.append({'files': config, 'from': live_model, 'to': remodel})
        if 'data' in cfg:
            assert 'srp_gun' not in cfg['data'], item
            guns.append({'files': [config], 'table': 'data', 'key': 'srp_gun', 'value': ids[item]})
        else:
            guns.append({'files': [config], 'table': '', 'key': 'data', 'value': {'srp_gun': ids[item]}})
    compat, unused = compatible(record, [r[0] for r in rows], ids, set(dictionary['items_compatibility']))
    out = HERE.parent / 'content-edit'
    (out / 'remodel-069-1.json').write_text(json.dumps({
        '_about': "Every live gun on 0.69d0's model (pack weapon-modules, docs/notes/remodel-069.md): each weapon "
                  "config's object/model set to its remodel's whole body, weapons/remodel_069/<0.69d0 item>/<0.69d0 "
                  "item> (remodel-069/items.tsv; made by weapon-modules/edits.py). The configs keep everything else.",
        'replace_string': models}, indent=1) + '\n')
    (out / 'weapon-modules-1.json').write_text(json.dumps({
        '_about': "Each gun names its record of the modules table (gameplay/weapon_modules.srpm, pack "
                  "weapon-modules): data/srp_gun, the weapon item's own dict id (docs/notes/attachments.md; made by "
                  "weapon-modules/edits.py; after the weapon-camo lists, which make data). And the "
                  f"{len(compat)} items_compatibility rows of the muzzle devices and grips with the guns that take "
                  "them in the table (the record of the pack's build), for the lobby's Attachments window.",
        'add_value': guns + compat}, indent=1) + '\n')
    print(f'{len(models)} guns, {len(compat)} compatibility rows; ids that fit no gun: {unused}')


if __name__ == '__main__':
    main()
