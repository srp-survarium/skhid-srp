#!/usr/bin/env python3
"""The scope items' edit list: ../content-edit/scope-modules-1.json.

usage: python3 -I modules_edit.py <players' files> <modules-069d.json> [--out <list>]

<players' files>     the players' gameplay/db_static_dictionaries and gameplay/items/scopes/shop_scope_*
<modules-069d.json>  the record of the weapon-modules pack's build (resource-porter's port-modules):
                     its `optics`, each optic's dict id and its one pair of models

A scope on the modules table (docs/notes/attachments-scopes.md) is its shop item alone: no variant.
For each optic of the record, in its shop item's config:
- data/idle_model and data/aimed_model become the optic's own two models of the pack, their origin
  the frame of the slot the optic goes in (the table places them on each gun);
- its modification, which changed nothing, is 994000 + its dict id (the fitted items' ids) and adds
  data/srp_scope = the item's own dict id to the weapon that wears its record: what the game's cook
  reads to fit it (the table's row, the scope object); and it sets the weapon's
  parameters/aim_offset to 0 (the eye is the table's alone).
Everything else of the item stays (its zoom, its price, its texts).
And in the dictionary: an items_compatibility row (`compatibility_<scope><weapon>`, as the released
ones) for each pair of the record that no release had (released-pairs.tsv): the lobby's movie greys
a scope by these rows.

--out writes the list elsewhere (a quick pack's, for a local test: only the optics of its record).
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model as cfgs  # noqa: E402

NAME = 'scope-modules-1.json'
# A fitted item's modification is 994000 + its dict id (docs/notes/attachments.md, the contract).
FIRST_ID = 994000


def main():
    live, record = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text())
    out = Path(sys.argv[sys.argv.index('--out') + 1]) if '--out' in sys.argv else HERE.parent / 'content-edit' / NAME
    items = {r['dict_id']: r for r in cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']}
    ids = {}
    for line in (HERE.parent / 'weapon-modules/ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            module, dict_id, kind, item = line.split('\t')
            if kind == 'scope':
                ids[module] = (int(dict_id), item)
    models, modifications, numbers = [], [], []
    for module, optic in sorted(record['optics'].items(), key=lambda kv: kv[1]['dict']):
        dict_id, name = ids[module]
        assert optic['dict'] == dict_id and items[dict_id]['name'] == name, (module, optic['dict'], dict_id)
        path = items[dict_id]['cfg_name']
        cfg = cfgs.config(live / path)
        idle, aimed = cfg['data']['idle_model'], cfg['data']['aimed_model']
        assert idle != aimed and optic['idle_model'] != optic['aimed_model'], (module, idle, aimed)
        models.append({'files': path, 'from': aimed, 'to': optic['aimed_model']})
        models.append({'files': path, 'from': idle, 'to': optic['idle_model']})
        (row,) = cfg['modifications']
        assert row['id'] == cfg['parameters']['modification'], module
        (old,) = row['modifiers']
        frm = {'id': row['id'], 'modifiers[0]/path[0]': old['path'][0], 'modifiers[0]/value': old['value'],
               'modifiers[0]/type': old['type']}
        assert (old['path'], old['value'], old['type']) == (['srp_scope_noop'], 0, 0), (module, old)
        # The eye is the table's alone: the gun's aim_offset (the eye relief of its iron sights or
        # of its default optic, which the game adds to whatever the eye is) is taken out. A gun
        # without the key has none: the modifier finds nothing and is skipped.
        fitted = [{'path': ['data'], 'value': ['srp_scope', dict_id], 'type': 3},
                  {'path': ['parameters', 'aim_offset'], 'value': 0.0, 'type': 0}]
        modifications.append({'file': path, 'list': 'modifications', 'where': {'id': row['id']}, 'from': frm,
                              'to': {'id': FIRST_ID + dict_id, 'modifiers': fitted}})
        numbers.append({'file': path, 'path': 'parameters/modification', 'from': row['id'], 'to': FIRST_ID + dict_id})
    # The pairs no release had: the lobby's rows for them.
    released = {(w, int(s)) for w, _, _, s, _, _ in (
        line.split('\t') for line in (HERE / 'released-pairs.tsv').read_text().splitlines()
        if line and not line.startswith('#'))}
    by_name = {r['name']: r['dict_id'] for r in items.values()}
    pairs = set()
    for gun_item, (_, donor) in record['items'].items():
        pairs |= {(gun_item, ids[f['module']][0]) for f in record['guns'][donor]['plan']['fits'] if f['kind'] == 'scope'}
    pairs |= {(row['gun'], ids[row['module']][0]) for row in record['legacy']}
    rows = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_compatibility']
    compatible = [{'files': ['gameplay/db_static_dictionaries'], 'table': 'items_compatibility',
                   'key': f'compatibility_{scope}{by_name[weapon]:05d}',
                   'value': {'first_item': by_name[weapon], 'second_item': scope}}
                  for weapon, scope in sorted(pairs - released, key=lambda p: (by_name[p[0]], p[1]))]
    assert not {c['key'] for c in compatible} & set(rows), 'a compatibility row is there already'
    about = ("Scopes on the modules table (pack weapon-modules, docs/notes/attachments-scopes.md): each shop "
             "scope item's data/idle_model and data/aimed_model are the optic's own two models of the pack, and "
             "its modification adds data/srp_scope = its own dict id to the weapon wearing it (it changed nothing "
             "before: the variants did) and sets its parameters/aim_offset to 0; an items_compatibility row for each "
             "pair the table has and no release had. Made by scope-fits/modules_edit.py from the pack's record.")
    out.write_text(json.dumps({'_about': about, 'replace_string': models, 'set': numbers, 'set_entry': modifications,
                               'add_value': compatible}, indent=1) + '\n')
    print(f'{len(modifications)} scope items, {len(compatible)} new pairs')


if __name__ == '__main__':
    main()
