#!/usr/bin/env python3
"""The guns' edit lists: ../content-edit/remodel-069-1.json and weapon-modules-1.json.

usage: python3 -I edits.py <players' files>

<players' files>  the players' gameplay/db_static_dictionaries and gameplay/items/weapons/*
For each row of ../remodel-069/items.tsv (a live gun item, its config, its live model, its remodel):
- remodel-069-1.json: the config's object/model becomes its remodel (the whole body);
- weapon-modules-1.json: data/srp_gun, the item's own dict id: the gun's record in the modules table
  (docs/notes/attachments.md, "What the core needs of the content", 5). A config without a data
  table gets one.
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as cfgs  # noqa: E402


def main():
    live = Path(sys.argv[1])
    ids = {r['name']: r['dict_id'] for r in cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']}
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
    out = HERE.parent / 'content-edit'
    (out / 'remodel-069-1.json').write_text(json.dumps({
        '_about': "Every live gun on 0.69d0's model (pack weapon-modules, docs/notes/remodel-069.md): each weapon "
                  "config's object/model set to its remodel's whole body, weapons/remodel_069/<0.69d0 item>/<0.69d0 "
                  "item> (remodel-069/items.tsv; made by weapon-modules/edits.py). The configs keep everything else.",
        'replace_string': models}, indent=1) + '\n')
    (out / 'weapon-modules-1.json').write_text(json.dumps({
        '_about': "Each gun names its record of the modules table (gameplay/weapon_modules.srpm, pack "
                  "weapon-modules): data/srp_gun, the weapon item's own dict id (docs/notes/attachments.md; made by "
                  "weapon-modules/edits.py; after the weapon-camo lists, which make data).",
        'add_value': guns}, indent=1) + '\n')
    print(f'{len(models)} guns')


if __name__ == '__main__':
    main()
