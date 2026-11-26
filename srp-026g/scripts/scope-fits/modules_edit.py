#!/usr/bin/env python3
"""The scope items' edit list: ../content-edit/scope-modules-1.json.

usage: python3 -I modules_edit.py <players' files> <modules-069d.json> [--out <list>]
       python3 -I modules_edit.py --check [<modules-069d.json>]

<players' files>     the players' gameplay/db_static_dictionaries and gameplay/items/scopes/shop_scope_*
<modules-069d.json>  the record of the weapon-modules pack's build (resource-porter's port-modules):
                     its `optics`, each optic's dict id and its one pair of models

A scope on the modules table (docs/notes/attachments-scopes.md) is its shop item alone: no variant.
For each optic of the record, in its shop item's config:
- data/idle_model and data/aimed_model become the optic's own two models of the pack, their origin
  the frame of the slot the optic goes in (the table places them on each gun);
- its modification, which changed nothing, is 994000 + its dict id (the fitted items' ids) and adds
  data/srp_scope = the item's own dict id to the weapon that wears its record: what the game's cook
  reads to fit it (the table's row, the scope object), and data/srp_scope_mount = its record's
  value's low half (which of the scope's ways onto the gun it takes, 0 the first: the weapon kit's,
  docs/notes/scope-mounts.md); and it sets the weapon's parameters/aim_offset to 0 (the eye is the
  table's alone).
Everything else of the item stays (its zoom, its price, its texts).
And in the dictionary: an items_compatibility row (`compatibility_<scope><weapon>`) for each pair of
the record: the lobby's movie greys a scope by these rows. The 169 pairs whose variant a base patch
made have theirs in that patch's dictionary (released-pairs.tsv less retired-variants.tsv: the rows
of the three retired packs' pairs were their edit lists', which are gone); every other pair's is here.

--out writes the list elsewhere (a quick pack's, for a local test: only the optics of its record).
--check says whether the list in the tree is the record's (the record of the weapon-modules version
release/content.tsv lists, beside its pack, when none is named): each optic's two models, and exactly
the record's pairs that have no row in a base patch. A new pack version changes the table's pairs
(2026-11-21: the event MP7 and the VSSK's camouflaged copy became guns of the table and 24 pairs had
no row), so the list is made again with it.
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

NAME = 'scope-modules-1.json'
# A fitted item's modification is 994000 + its dict id (docs/notes/attachments.md, the contract).
FIRST_ID = 994000


def scope_ids():
    """{0.69d0 module: (its shop item's dict id, the item's name)} of ../weapon-modules/ids.tsv's scopes."""
    ids = {}
    for line in (HERE.parent / 'weapon-modules/ids.tsv').read_text().splitlines():
        if line and not line.startswith('#'):
            module, dict_id, kind, item = line.split('\t')
            if kind == 'scope':
                ids[module] = (int(dict_id), item)
    return ids


def new_pairs(record, ids):
    """[(weapon dict id, scope dict id)] of the record's scope pairs without a row in a base patch's
    dictionary, in the rows' order. A base patch has the rows of the pairs its own variants showed."""
    retired = {int(line.split('\t')[0]) for line in (HERE / 'retired-variants.tsv').read_text().splitlines()
               if line and not line.startswith('#')}
    released = {(w, int(s)) for w, _, _, s, variant, _ in (
        line.split('\t') for line in (HERE / 'released-pairs.tsv').read_text().splitlines()
        if line and not line.startswith('#')) if int(variant) not in retired}
    pairs = set()
    for gun_item, (_, donor) in record['items'].items():
        pairs |= {(gun_item, ids[f['module']][0]) for f in record['guns'][donor]['plan']['fits'] if f['kind'] == 'scope'}
    pairs |= {(row['gun'], ids[row['module']][0]) for row in record['legacy']}
    by_name = {item: dict_id for item, (dict_id, _) in record['items'].items()}
    return sorted((by_name[weapon], scope) for weapon, scope in pairs - released)


def compatibility(pairs):
    return [{'files': ['gameplay/db_static_dictionaries'], 'table': 'items_compatibility',
             'key': f'compatibility_{scope}{weapon:05d}', 'value': {'first_item': weapon, 'second_item': scope}}
            for weapon, scope in pairs]


def listed_record():
    """The record beside the weapon-modules pack release/content.tsv lists (the main checkout's private/content-packs)."""
    root = HERE.parents[2]
    (version,) = re.findall(r'^pack\tweapon-modules@([^\t]+)\t', (root / 'release/content.tsv').read_text(), re.M)
    common = subprocess.run(['git', '-C', str(root), 'rev-parse', '--path-format=absolute', '--git-common-dir'],
                            check=True, capture_output=True, text=True).stdout.strip()
    return Path(common).parent / 'private/content-packs' / f'weapon-modules-{version}.modules-069d.json'


def check(record_path):
    """Whether ../content-edit/scope-modules-1.json is what `record_path` gives; needs no players' files."""
    record, ids = json.loads(record_path.read_text()), scope_ids()
    listed = json.loads((HERE.parent / 'content-edit' / NAME).read_text())
    problems = []
    to = {}
    for edit in listed['replace_string']:
        to.setdefault(edit['files'], set()).add(edit['to'])
    models = {frozenset((optic['idle_model'], optic['aimed_model'])) for optic in record['optics'].values()}
    if {frozenset(v) for v in to.values()} != models:
        problems.append("the scope items' models are not the record's optics' (every optic's idle and aimed model)")
    want, have = compatibility(new_pairs(record, ids)), listed['add_value']
    for row in want:
        if row not in have:
            problems.append(f"no row for weapon {row['value']['first_item']} / scope {row['value']['second_item']}, "
                            'a pair of the table without a row in a base patch')
    for row in have:
        if row not in want:
            problems.append(f"{row['key']}: no pair of the table, or a base patch has its row")
    for problem in problems:
        print(f'scope-modules: {problem}')
    if problems:
        raise SystemExit(f'{NAME} is not what {record_path} gives ({len(problems)}): make it again '
                         "(modules_edit.py <players' files> <that record>)")
    print(f"{NAME}: the record's ({len(models)} optics, {len(have)} pairs' rows; {record_path.name})")


def main():
    if '--check' in sys.argv:
        rest = [a for a in sys.argv[1:] if a != '--check']
        return check(Path(rest[0]) if rest else listed_record())
    sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
    import model as cfgs
    live, record = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text())
    out = Path(sys.argv[sys.argv.index('--out') + 1]) if '--out' in sys.argv else HERE.parent / 'content-edit' / NAME
    items = {r['dict_id']: r for r in cfgs.config(live / 'gameplay/db_static_dictionaries')['items_dict']}
    ids = scope_ids()
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
        # without the key has none: the modifier finds nothing and is skipped. The released modifier
        # stays first, as it is (a set of a key no weapon has: skipped), so no key changes its type.
        fitted = [old, {'path': ['data'], 'value': ['srp_scope', dict_id], 'type': 3},
                  {'path': ['data'], 'value': ['srp_scope_mount', '<low_param>'], 'type': 3},
                  {'path': ['parameters', 'aim_offset'], 'value': 0.0, 'type': 0}]
        modifications.append({'file': path, 'list': 'modifications', 'where': {'id': row['id']}, 'from': frm,
                              'to': {'id': FIRST_ID + dict_id, 'modifiers': fitted}})
        numbers.append({'file': path, 'path': 'parameters/modification', 'from': row['id'], 'to': FIRST_ID + dict_id})
    # The lobby's rows of the pairs: those a base patch's dictionary lacks.
    for item, (dict_id, _) in record['items'].items():
        assert items[dict_id]['name'] == item, (item, dict_id, items[dict_id]['name'])
    compatible = compatibility(new_pairs(record, ids))
    # The players' dictionary of release 68 has every released pair's row: the base patches' (kept) and the
    # retired lists' (made here now). A row made here is either one of the retired lists' or new.
    rows = cfgs.config(live / 'gameplay/db_static_dictionaries')['items_compatibility']
    retired_rows = {c['key'] for c in compatible} & set(rows)
    assert len(rows) - len(retired_rows) + len(compatible) == len(set(rows) | {c['key'] for c in compatible})
    about = ("Scopes on the modules table (pack weapon-modules, docs/notes/attachments-scopes.md): each shop "
             "scope item's data/idle_model and data/aimed_model are the optic's own two models of the pack, and "
             "its modification adds data/srp_scope = its own dict id to the weapon wearing it (it changed nothing "
             "before: the variants did) and data/srp_scope_mount = its record's value (its way onto the gun) and sets "
             "its parameters/aim_offset to 0; an items_compatibility row for each "
             "pair of the table but the 169 a base patch's dictionary has. Made by scope-fits/modules_edit.py "
             "from the pack's record.")
    out.write_text(json.dumps({'_about': about, 'replace_string': models, 'set': numbers, 'set_entry': modifications,
                               'add_value': compatible}, indent=1) + '\n')
    print(f'{len(modifications)} scope items, {len(compatible)} pairs\' rows ({len(retired_rows)} the retired lists\' before)')


if __name__ == '__main__':
    main()
