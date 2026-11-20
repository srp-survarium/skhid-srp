#!/usr/bin/env python3
"""Audit every sold optic against 0.69's actual attachment-slot graph.

Module-to-item links alone include piggyback optics. A standalone fit needs a
path from the weapon through non-optical mounts, respecting weapon exceptions.
Existing fits are always retained, even when the donor no longer offers them.
"""
import argparse
import csv
from dataclasses import dataclass
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model


def rows(path):
    return [line.split('\t') for line in path.read_text().splitlines()
            if line and not line.startswith('#')]


def sequence(value):
    # Empty indexed tables are emitted as empty named tables by the donor.
    if value == {}:
        return []
    if not isinstance(value, list):
        raise ValueError(f'expected indexed table, got {value!r}')
    return value


@dataclass(frozen=True)
class Step:
    module: str
    slot: str
    locator: str


@dataclass(frozen=True)
class Mount:
    root: str
    steps: tuple[Step, ...] = ()


def effective_module(game, name, weapon):
    module = game['weapon_modules'][name]
    return {**module, **module.get('exceptions', {}).get(weapon, {})}


def optical(module):
    return module.get('group', '').startswith('scope_') and module.get('group') != 'scope_rail'


def mounts(game, weapon):
    """Reachable slots and their least invasive non-optical attachment chain."""
    gun = game['weapons'][weapon]
    defaults = {row['module'] for row in sequence(gun['default_modules'])}
    usable = defaults | {row['module'] for row in game['module_to_item_links'] if row['item'] == weapon}

    def cost(path):
        replacement = sum(step.module not in defaults and
                          'picatinnyrail' not in step.module for step in path.steps)
        return replacement, len(path.steps), tuple(step.module for step in path.steps)

    slots = {row['id']: Mount(row['locator']) for row in sequence(gun['slots'])
             if row.get('accept_module', True)}
    changed = True
    while changed:
        changed = False
        for name in sorted(usable):
            module = effective_module(game, name, weapon)
            if optical(module):
                continue
            for attachment in sequence(module.get('slots_to_attach_to', [])):
                slot = attachment['slot']
                if slot not in slots or name in {s.module for s in slots[slot].steps}:
                    continue
                params = module[attachment['attach_params']]
                for added in sequence(params.get('slots_to_add', [])):
                    if not added.get('accept_module', True):
                        continue
                    path = Mount(slots[slot].root,
                                 slots[slot].steps + (Step(name, slot, added['locator']),))
                    if added['id'] not in slots or cost(path) < cost(slots[added['id']]):
                        slots[added['id']] = path
                        changed = True
    return slots, cost


def audit(live, donor):
    dictionary = model.config(live / 'gameplay/db_static_dictionaries')
    game = model.config(donor / 'gameplay/game.options')
    aliases = {'a_545': 'a545', **{r[0]: r[1] for r in rows(HERE.parent / 'scope-fits/guns.tsv')}}
    optics = {r[0]: r[1] for r in rows(HERE.parent / 'scope-fits/optics.tsv')}
    optics['shop_scope_pu_3_5x'] = 'pu_35x22_scope'
    fitted = set()
    for path in (live / 'gameplay/items/scopes').glob('*.options'):
        config = model.config(path)
        if fit := config.get('srp_scope_fit'):
            fitted.update((gun, fit['scope']) for gun in fit['weapons'])
    output = []
    for item in dictionary['items_dict']:
        if item['item_category'] not in (8, 13, 14, 15, 16, 17):
            continue
        name = item['name']
        plain = item.get('srp_like') or re.sub(r'_premium(_\d+)?$', '', name)
        key = aliases.get(name, aliases.get(plain, plain))
        if key not in game['weapons']:
            raise ValueError(f'{name}: no reviewed donor weapon mapping ({key})')
        slots, cost = mounts(game, key)
        linked = {r['module'] for r in game['module_to_item_links'] if r['item'] == key}
        for scope, module_name in optics.items():
            module = effective_module(game, module_name, key)
            choices = [(a['slot'], slots[a['slot']]) for a in sequence(module['slots_to_attach_to'])
                       if a['slot'] in slots and module_name in linked]
            choice = min(choices, key=lambda pair: (cost(pair[1]), pair[0])) if choices else None
            present = (name, scope) in fitted
            status = 'preserved' if present else 'missing' if choice else 'not standalone' if module_name in linked else 'not offered'
            slot, mount = choice if choice else ('', None)
            output.append(dict(weapon=name, donor_weapon=key, scope=scope, module=module_name,
                               status=status, slot=slot, root=mount.root if mount else '',
                               chain=json.dumps([vars(s) for s in mount.steps] if mount else [], separators=(',', ':'))))
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('live', 'donor', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.live, args.donor)
    with args.out.open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(result[0]), delimiter='\t')
        writer.writeheader()
        writer.writerows(result)
    from collections import Counter
    print(dict(Counter(row['status'] for row in result)))


if __name__ == '__main__':
    main()
