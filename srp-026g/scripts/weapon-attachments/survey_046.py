#!/usr/bin/env python3
"""survey_046.py: which 0.26g weapons each muzzle device of devices.tsv fits, from 0.46c2.

usage: python3 -I survey_046.py <0.46c2 game.leaves> <0.26g dictionary leaves> > fit.tsv

<0.46c2 game.leaves>: `vostok-port dump-db-config --leaves` of 0.46c2's game config (the weapon
survey's work/builds/v046c2/d/game.leaves: weapons.<name>.attaches.<kind>.idle_model);
<0.26g dictionary leaves>: the same of the live gameplay/db_static_dictionaries (items_dict).

A device fits a 0.26g weapon when 0.46c2 gave that weapon the device's model in one of its muzzle
kinds (flash_hider, compensator_h, compensator_v, compensator_u). A 0.26g weapon is looked up by
its dictionary name, then name_046, then without a _premium[_N] ending (a premium copy fits as
its plain weapon), then the names 0.46c2 spells otherwise (ALIASES). Output (fit.tsv, which
make_pack.py reads): device name TAB its weapons, `<dictionary name>:<dict id>` one space apart,
sorted by name.
"""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
KINDS = ('flash_hider', 'compensator_h', 'compensator_v', 'compensator_u')
ALIASES = {'a_545': 'a545'}
WEAPON_CATEGORIES = {8, 13, 14, 15, 16, 17}


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    pattern = re.compile(r'^weapons\.([^.]+)\.attaches\.(' + '|'.join(KINDS) + r')\.idle_model\tstring:"(.*)"$')
    models = {}
    for line in open(sys.argv[1], encoding='utf-8', errors='replace'):
        m = pattern.match(line.rstrip('\n'))
        if m:
            models.setdefault(m.group(1), set()).add('models/' + m.group(3) + '.model')
    items = {}
    for line in open(sys.argv[2], encoding='utf-8'):
        m = re.match(r'^items_dict\[(\d+)\]\.(\w+)\t\w+:(.*)$', line.rstrip('\n'))
        if m:
            items.setdefault(int(m.group(1)), {})[m.group(2)] = m.group(3).strip('"')
    ids = {item['name']: int(item['dict_id']) for item in items.values()
           if int(item.get('item_category', 0)) in WEAPON_CATEGORIES
           and item.get('cfg_name', '').startswith('gameplay/items/weapons/wpn_')}
    weapons = sorted(ids)

    def donor(name):
        plain = re.sub(r'_premium(_\d+)?$', '', name)
        for candidate in (name, name + '_046', plain, plain + '_046', ALIASES.get(plain), ALIASES.get(plain, '') + '_046'):
            if candidate in models:
                return candidate
        return None

    for line in (HERE / 'devices.tsv').read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('#'):
            continue
        name, model = line.split('\t')[1], line.split('\t')[3]
        fits = [w for w in weapons if donor(w) and model in models[donor(w)]]
        print(f'{name}\t{" ".join(f"{w}:{ids[w]}" for w in fits)}')


if __name__ == '__main__':
    main()
