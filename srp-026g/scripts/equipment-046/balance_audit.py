#!/usr/bin/env python3
"""Compare the pack with published equipment (equipment-029 included) and with itself.

As equipment-029's audit: actual hit-region protection, weight and built-in
bonuses; peers are pieces of the same level, kind, covered regions and
occupied slots. It rejects strict stat dominance either way, also between two
new pieces. This checks stat trade-offs, not damage breakpoints or playtests.
"""
import argparse
from collections import defaultdict
import json
from pathlib import Path

from make_pack import bcw, specs, DICTIONARY, MODIFICATIONS

LOWER_IS_BETTER = {
    'aim_time_modifier', 'dispersion_modifier', 'recoil_modifier',
    'stamina_spending_speed_modifier', 'artefact_search_time_modifier',
    'respawn_time_modifier', 'breath_vibration_modifier',
}
# Level-15 sets for the Black Market heavy set: vest, pants, boots, gloves, helmet (and mask).
SETS = {'Zubr UM-4': [316, 318, 319, 320, 317], 'Pilgrim': [310, 312, 315, 313, 314],
        'Nomad': [956, 955, 953, 954, 957], 'Heavy armored (new)': [1011, 1012, 1007, 1008, 1009, 1010]}


def audit(pack, base):
    manifest = json.loads((pack / 'pack.json').read_text())
    added = next(op['entries'] for op in manifest['operations'] if op['op'] == 'dictionary_add')
    native = bcw.loads((base / DICTIONARY).read_bytes())['items_dict']
    mods = {row['id']: row for row in bcw.loads((base / MODIFICATIONS).read_bytes())['modifications']}
    entries = {row['dict_id']: row for row in native + added}
    new_ids = {row['dict_id'] for row in added}
    configs = {}
    for row in native + added:
        if '/armour/' not in row['cfg_name']:
            continue
        path = pack / 'files' / row['cfg_name']
        configs[row['dict_id']] = bcw.loads((path if path.exists() else base / row['cfg_name']).read_bytes())

    def bonuses(ident):
        effects = defaultdict(float)
        for mid in configs[ident].get('default_modifications', []):
            for name, value in mods[mid]['lobby_info'].items():
                effects[name] += value
        return {key: round(value, 6) for key, value in effects.items()}

    def comparable(ident):
        item = configs[ident]
        return (item['parameters']['type'], item['parameters']['item_level'], tuple(sorted(item.get('hit_params', {}))),
                tuple(sorted(item['data'].get('additional_model_slots', []))))

    def features(ident):
        p = configs[ident]['parameters']
        result = {'weight': -p['weight'], 'oxygen': p.get('oxygen_amount', 0)}
        for region, values in configs[ident].get('hit_params', {}).items():
            for kind, hit in values['hit_types'].items():
                result[f'{region}/{kind}'] = hit['armor']
        result.update({k: -v if k in LOWER_IS_BETTER else v for k, v in bonuses(ident).items()})
        return result

    def dominates(a, b):
        a, b = features(a), features(b)
        keys = a.keys() | b.keys()
        return (all(a.get(k, 0) >= b.get(k, 0) - 1e-6 for k in keys)
                and any(a.get(k, 0) > b.get(k, 0) + 1e-6 for k in keys))

    def summary(ident):
        p = configs[ident]['parameters']
        return {'id': ident, 'name': entries[ident]['name'], 'level': p['item_level'],
                'price': entries[ident].get('base_cost', 0), 'armour': round(p.get('armor', 0) * 100),
                'isolation': round(p.get('ano_armor', 0) * 100), 'weight': round(p['weight'], 3),
                'bonuses': bonuses(ident), 'covers': list(configs[ident].get('hit_params', {})),
                'also_occupies': configs[ident]['data'].get('additional_model_slots', [])}

    regular = [entries[i] for i in configs if not entries[i]['is_premium']]
    report = []
    for row in specs():
        if row['plain_id']:
            continue
        ident, peer = int(row['id']), int(row['peer_id'])
        item, benchmark = configs[ident]['parameters'], configs[peer]['parameters']
        assert item['type'] == benchmark['type'], (ident, 'benchmark of another kind')
        if item['item_level'] == benchmark['item_level'] and 'armor' in item:
            assert abs(item['armor'] - benchmark['armor']) <= 0.15 + 1e-6, (ident, 'outside peer protection band')
        peers = [other['dict_id'] for other in regular
                 if other['dict_id'] != ident and comparable(other['dict_id']) == comparable(ident)]
        report.append({'role': row['role'], 'item': summary(ident), 'benchmark': summary(peer),
                       'comparable_ids': peers, 'new_peers': [i for i in peers if i in new_ids],
                       'strictly_better_than': [summary(o) for o in peers if dominates(ident, o)],
                       'strictly_worse_than': [summary(o) for o in peers if dominates(o, ident)]})
    sets = {}
    for name, ids in SETS.items():
        effects = defaultdict(float)
        for ident in ids:
            for key, value in bonuses(ident).items():
                effects[key] += value
        sets[name] = {'pieces': [summary(i) for i in ids],
                      'weight': round(sum(configs[i]['parameters']['weight'] for i in ids), 3),
                      'bonuses': {k: round(v, 6) for k, v in effects.items()}}
    # The Black Market heavy set: Zubr keeps the strongest protection, the new set pays for
    # less armour and isolation with weight.
    zubr, heavy = sets['Zubr UM-4'], sets['Heavy armored (new)']
    assert all(p['armour'] < 90 and p['isolation'] < 50 for p in heavy['pieces'])
    assert heavy['weight'] < zubr['weight']
    return {'items': report, 'sets': sets}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('base', type=Path, help="prepare_inputs.py's base group")
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    report = audit(args.pack, args.base)
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    problems = [f"{row['item']['id']} {kind}: {[x['id'] for x in row[kind]]}"
                for row in report['items'] for kind in ('strictly_better_than', 'strictly_worse_than') if row[kind]]
    if problems:
        raise SystemExit('\n'.join(problems))
    pairs = sum(len(row['comparable_ids']) for row in report['items'])
    alone = [row['item']['id'] for row in report['items'] if not row['comparable_ids']]
    print(f"{len(report['items'])} regular pieces, {pairs} comparisons (native, 029 and new): no strict stat "
          f"dominance; no exact peer: {alone}")
