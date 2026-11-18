#!/usr/bin/env python3
"""Compare the pack with published equipment (equipment-029 included, and the equipment-046
pieces the Snow-20 copies) and with itself.

As equipment-046's audit: armour and isolation where a piece has protection, weight and
built-in bonuses; peers are pieces of the same level, kind, protected regions and occupied
slots. It rejects strict stat dominance either way. This checks stat trade-offs, not playtests.
"""
import argparse
from collections import defaultdict
import json
from pathlib import Path

from common import DICTIONARY, E046_MANIFEST, MODIFICATIONS, bcw, e046_entries, specs

LOWER_IS_BETTER = {
    'aim_time_modifier', 'dispersion_modifier', 'recoil_modifier',
    'stamina_spending_speed_modifier', 'artefact_search_time_modifier',
    'respawn_time_modifier', 'breath_vibration_modifier',
}


def audit(pack, inputs):
    base = inputs / 'base'
    manifest = json.loads((pack / 'pack.json').read_text())
    added = next(op['entries'] for op in manifest['operations'] if op['op'] == 'dictionary_add')
    native = bcw.loads((base / DICTIONARY).read_bytes())['items_dict']
    mods = {row['id']: row for row in bcw.loads((base / MODIFICATIONS).read_bytes())['modifications']}
    e046 = inputs / 'equipment046'
    copied = [row for row in e046_entries(json.loads((e046 / E046_MANIFEST).read_text())).values()
              if (e046 / 'files' / row['cfg_name']).exists()]
    entries = {row['dict_id']: row for row in native + copied + added}
    configs = {}
    for row in native + copied + added:
        if '/armour/' not in row['cfg_name']:
            continue
        for path in (pack / 'files' / row['cfg_name'], e046 / 'files' / row['cfg_name'], base / row['cfg_name']):
            if path.exists():
                configs[row['dict_id']] = bcw.loads(path.read_bytes())
                break

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
        params = configs[ident]['parameters']
        result = {'weight': -params['weight']}
        if 'hit_params' in configs[ident]:
            result.update(armour=round(params.get('armor', 0), 6), isolation=round(params.get('ano_armor', 0), 6))
        result.update({k: -v if k in LOWER_IS_BETTER else v for k, v in bonuses(ident).items()})
        return result

    def dominates(a, b):
        a, b = features(a), features(b)
        keys = a.keys() | b.keys()
        return (all(a.get(k, 0) >= b.get(k, 0) - 1e-6 for k in keys)
                and any(a.get(k, 0) > b.get(k, 0) + 1e-6 for k in keys))

    def summary(ident):
        p = configs[ident]['parameters']
        return {'id': ident, 'name': entries[ident]['name'], 'level': p['item_level'], 'faction': p['faction'],
                'price': entries[ident].get('base_cost', 0), 'weight': round(p['weight'], 3),
                'armour': round(p.get('armor', 0), 3), 'isolation': round(p.get('ano_armor', 0), 3),
                'drop_weight': p.get('drop_weight'), 'bonuses': bonuses(ident)}

    regular = [entries[i] for i in configs if not entries[i]['is_premium']]
    report = []
    for row in specs():
        ident, peer = int(row['id']), int(row['peer_id'])
        assert configs[ident]['parameters']['type'] == configs[peer]['parameters']['type'], (ident, 'benchmark kind')
        peers = [other['dict_id'] for other in regular
                 if other['dict_id'] != ident and comparable(other['dict_id']) == comparable(ident)]
        report.append({'item': summary(ident), 'benchmark': summary(peer), 'comparable_ids': peers,
                       'peers': [summary(o) for o in peers],
                       'strictly_better_than': [summary(o) for o in peers if dominates(ident, o)],
                       'strictly_worse_than': [summary(o) for o in peers if dominates(o, ident)]})
    return {'items': report}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', type=Path)
    parser.add_argument('inputs', type=Path, help="prepare_inputs.py's folder")
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    report = audit(args.pack, args.inputs)
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    problems = [f"{row['item']['id']} {kind}: {[x['id'] for x in row[kind]]}"
                for row in report['items'] for kind in ('strictly_better_than', 'strictly_worse_than') if row[kind]]
    if problems:
        raise SystemExit('\n'.join(problems))
    pairs = sum(len(row['comparable_ids']) for row in report['items'])
    alone = [row['item']['id'] for row in report['items'] if not row['comparable_ids']]
    print(f"{len(report['items'])} pieces, {pairs} comparisons (native, 029 and the copied 046 pieces): "
          f"no strict stat dominance; no peer at the same level: {alone}")
