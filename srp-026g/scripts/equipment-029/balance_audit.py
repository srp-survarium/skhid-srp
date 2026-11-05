#!/usr/bin/env python3
"""Compare the pack with published equipment, including weight and built-in bonuses.

This checks stat trade-offs, not damage breakpoints or playtest outcomes. Pieces
with different body coverage or occupied slots are not interchangeable peers.
"""
import argparse
from collections import defaultdict
import json
from pathlib import Path

from make_pack import bcw, DICTIONARY, specs

LOWER_IS_BETTER = {
    'aim_time_modifier', 'dispersion_modifier', 'recoil_modifier',
    'stamina_spending_speed_modifier', 'artefact_search_time_modifier',
    'respawn_time_modifier', 'breath_vibration_modifier',
}


def audit(pack, base):
    manifest = json.loads((pack / 'pack.json').read_text())
    added = next(op['entries'] for op in manifest['operations'] if op['op'] == 'dictionary_add')
    native = bcw.loads((base / DICTIONARY).read_bytes())['items_dict']
    mods = bcw.loads((base / 'gameplay/items/modifications/modifications.options').read_bytes())
    mods = {row['id']: row for row in mods['modifications']}
    entries = {row['dict_id']: row for row in native + added}
    configs = {}
    for row in native + added:
        if '/armour/' not in row['cfg_name']:
            continue
        path = pack / 'files' / row['cfg_name']
        if not path.exists():
            path = base / row['cfg_name']
        if path.exists():
            configs[row['dict_id']] = bcw.loads(path.read_bytes())

    def bonuses(ident):
        effects = defaultdict(float)
        for mid in configs[ident].get('default_modifications', []):
            for name, value in mods[mid]['lobby_info'].items():
                effects[name] += value
        return {key: round(value, 6) for key, value in effects.items()}

    def comparable(ident):
        item = configs[ident]
        return (item['parameters']['type'], tuple(sorted(item.get('hit_params', {}))),
                tuple(sorted(item['data'].get('additional_model_slots', []))))

    def features(ident):
        item = configs[ident]
        p = item['parameters']
        result = {'weight': -p['weight'], 'oxygen': p.get('oxygen_amount', 0)}
        # Compare actual body-region protection, not only the card's main number.
        for region, values in item.get('hit_params', {}).items():
            for kind, hit in values['hit_types'].items():
                result[f'{region}/{kind}'] = hit['armor']
        result.update({key: -value if key in LOWER_IS_BETTER else value
                       for key, value in bonuses(ident).items()})
        return result

    def dominates(a, b):
        a, b = features(a), features(b)
        keys = a.keys() | b.keys()
        return (all(a.get(k, 0) >= b.get(k, 0) - 1e-6 for k in keys)
                and any(a.get(k, 0) > b.get(k, 0) + 1e-6 for k in keys))

    def summary(ident):
        p = configs[ident]['parameters']
        return {'id': ident, 'name': entries[ident]['name'], 'level': p['item_level'],
                'price': entries[ident].get('base_cost', 0),
                'armour': round(p.get('armor', 0) * 100),
                'isolation': round(p.get('ano_armor', 0) * 100),
                'weight': round(p['weight'], 3), 'bonuses': bonuses(ident),
                'covers': list(configs[ident].get('hit_params', {})),
                'also_occupies': configs[ident]['data'].get('additional_model_slots', [])}

    report = []
    for row in specs():
        if row['plain_id']:
            continue
        ident, peer = int(row['donor_id']), int(row['peer_id'])
        item = configs[ident]
        assert item['parameters']['type'] == configs[peer]['parameters']['type'], ident
        assert abs(item['parameters']['weight'] - float(row['weight'])) < 1e-5
        assert entries[ident]['base_cost'] == int(row['price'])
        assert item.get('default_modifications', []) == [int(x) for x in row['default_modifications'].split(',') if x]
        p, benchmark = item['parameters'], configs[peer]['parameters']
        if p['item_level'] == benchmark['item_level'] and 'armor' in p:
            assert abs(p['armor'] - benchmark['armor']) <= 0.15 + 1e-6, (ident, 'outside peer protection band')
        peers = [other['dict_id'] for other in native if not other['is_premium']
                 and other['dict_id'] in configs
                 and configs[other['dict_id']]['parameters']['item_level'] == item['parameters']['item_level']
                 and comparable(ident) == comparable(other['dict_id'])]
        report.append({'role': row['role'], 'item': summary(ident), 'benchmark': summary(peer),
                       'comparable_native_ids': peers,
                       'strictly_better_than': [summary(other) for other in peers if dominates(ident, other)],
                       'strictly_worse_than': [summary(other) for other in peers if dominates(other, ident)]})

    sets = {}
    for name, ids in {'Zubr': [316, 318, 319, 320, 317], 'Pilgrim': [310, 312, 315, 313, 314],
                      'Nomad': [956, 955, 953, 954, 957]}.items():
        effects = defaultdict(float)
        for ident in ids:
            for key, value in bonuses(ident).items():
                effects[key] += value
        sets[name] = {'pieces': [summary(ident) for ident in ids],
                      'weight': round(sum(configs[i]['parameters']['weight'] for i in ids), 3),
                      'bonuses': {k: round(v, 6) for k, v in effects.items()}}
    # Endgame roles: mobile Nomad below both heavy sets' protection, with a
    # useful movement advantage. The head comparison includes face coverage.
    for mobile, heavy, max_gap in zip([956, 955, 953, 954, 957], [316, 318, 319, 320, 317],
                                     [0.10, 0.10, 0.10, 0.10, 0.15]):
        a, b = configs[mobile]['parameters'], configs[heavy]['parameters']
        assert a['armor'] < b['armor'] and a['ano_armor'] < b['ano_armor']
        assert b['armor'] - a['armor'] <= max_gap + 1e-6, 'Nomad protection must remain competitive'
    assert 0.45 <= sets['Nomad']['weight'] / sets['Zubr']['weight'] <= 0.65
    movement = sets['Nomad']['bonuses']['movement_speed_modifier']
    assert sets['Zubr']['bonuses'].get('movement_speed_modifier', 0) < movement <= 0.10 + 1e-6
    return {'items': report, 'five_piece_sets': sets}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('base', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    report = audit(args.pack, args.base)
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    problems = []
    for row in report['items']:
        for kind in ('strictly_better_than', 'strictly_worse_than'):
            if row[kind]:
                problems.append(f"{row['item']['id']} {kind}: {[x['id'] for x in row[kind]]}")
    if problems:
        raise SystemExit('\n'.join(problems))
    print(f"{len(report['items'])} regular pieces checked: no strict stat dominance against comparable native pieces at the same level")
