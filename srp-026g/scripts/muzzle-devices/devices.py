#!/usr/bin/env python3
"""The muzzle devices and their numbers: devices.tsv (docs/notes/attachments-muzzles.md).

usage: python3 -I devices.py <0.69d0 gameplay/game.options> [--write]

Each muzzle row of ../weapon-modules/ids.tsv (a 0.69d0 module, our dict id and item) is a device,
or is left out by left_out.tsv with a reason checked here. A device's numbers are 0.69d0's own, read
from its game.options and turned by rules.tsv; nothing is typed a device:
- its two drop modifications at grade 7, the grade 0.69d0 sells (modules_drop_options, modifications);
- its family (families.tsv: by its main modification's key and its group) and so its price;
- its shot effect (attach_params' effects_to_override.particles.shoot; one a device);
- its model (weapons/modules/<x> is our weapons/modules_069/<x>, pack weapon-modules).
A shotgun device (one that narrows the spread) costs aim time, an eighth of the spread it takes off,
to 0.1 % (SPREAD_COST), whatever 0.69d0 gives it: they compete on the same guns, and 0.69d0's costs
are on keys 0.26g does not read, so the brakes and duckbills cost nothing and beat the chokes (the
owner, 2026-11-22: "check muzzles are similar"). Duckbill -15 % +1.9 %, brake -16 % +2 %, choke
-20 % +2.5 %.
Without --write: says what differs from devices.tsv and fails if anything does.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402

GRADE = 7
KICKS = tuple(f'recoil/{end}_{side}_recoil' for side in ('top', 'left', 'right') for end in ('min', 'max'))
MAIN_KEYS = {
    'parameters/unmasking_radius': 'flash_hider',
    'parameters/recoil_modifier': 'brake',
    'parameters/movement_speed_damage': 'brake',
    'parameters/dispersion_buck_shot_x_scale': 'brake',
}
GROUP_FAMILIES = {'choke': 'choke', 'duckbill': 'duckbill'}
SPREAD, AIM_TIME = 'dispersion/base_dispersion', 'parameters/aim_time'
# A shotgun device's aim time for each unit of spread it takes off, rounded to 0.001.
SPREAD_COST = 1 / 8
COLUMNS = ('dict id', 'item', 'module', 'family', 'price', 'shot effect', 'stats', "0.69d0's at grade 7", 'model',
           'slots')


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def muzzle_ids():
    ids = HERE.parent / 'weapon-modules/ids.tsv'
    return [(module, int(dict_id), item) for module, dict_id, kind, item in
            (line.split('\t') for line in ids.read_text().splitlines() if line and not line.startswith('#'))
            if kind == 'muzzle']


def number(value):
    """0.69d0 writes a group, a grade or a dict id as an int or as a text."""
    return int(value)


class Game:
    """0.69d0's game.options, the tables a device is read from."""

    def __init__(self, data):
        root = bcw.loads(data)
        self.modules = root['weapon_modules']
        self.weapons = root['weapons']
        self.links = {}
        for link in root['module_to_item_links']:
            self.links.setdefault(link['module'], []).append(link['item'])
        self.grades = {}
        for entry in root['modifications']:
            if 'group' in entry and 'grade' in entry:
                self.grades[(number(entry['group']), number(entry['grade']))] = entry['modifiers']
        self.drops = {number(info['dict_id']): info
                      for info in root['modules_drop_options']['modules_additional_infos']}

    def modification(self, group):
        """[(key, value, type)] of the modification `group` at GRADE."""
        return [('/'.join(m['path']), float(m['value']), number(m['type']))
                for m in self.grades[(group, GRADE)]]

    def main_group(self, key, modules):
        """The modification 0.69d0 rolls first on those of `modules` whose one key is `key`."""
        found = sorted({group for group in (self.drop_groups(module)[0] for module in modules)
                        if [k for k, _, _ in self.modification(group)] == [key]})
        if len(found) != 1:
            sys.exit(f'the devices have {len(found)} main modifications of {key} alone ({found})')
        return found[0]

    def drop_groups(self, module):
        info = self.drops.get(number(self.modules[module]['parameters']['dict_id']))
        if info is None:
            sys.exit(f'{module}: 0.69d0 rolls nothing on it (a gun\'s own part, not a device it sells)')
        return [number(info[key]) for key in ('first_drop_modifier', 'second_drop_modifier') if key in info]

    def attaches(self, module):
        """[(slot, its rules)] of a module."""
        entry = self.modules[module]
        return [(a['slot'], entry.get(a['attach_params'], {})) for a in entry['slots_to_attach_to']]

    def shot_effect(self, module):
        effects = {rules.get('effects_to_override', {}).get('particles', {}).get('shoot')
                   for _, rules in self.attaches(module)}
        if len(effects) != 1:
            sys.exit(f'{module}: its slots name different shot effects ({effects}); an item has one')
        for gun, exception in self.modules[module].get('exceptions', {}).items():
            if 'effects_to_override' in str(exception):
                sys.exit(f'{module}: another shot effect on {gun} (exceptions); an item has one')
        return effects.pop()

    def own_part(self, gun, slot):
        """The module a gun has in `slot` by default, or None."""
        for part in self.weapons[gun].get('default_modules', []):
            if part['slot'] == slot:
                return part['module']
        return None


def check_left_out(game, module, reason):
    entry = game.modules[module]
    if reason != 'suppressor_barrel':
        sys.exit(f'left_out.tsv: {module}: no such reason {reason}')
    if not entry['group'].endswith('_barrel'):
        sys.exit(f'left_out.tsv: {module}: group {entry["group"]} is no barrel')
    for gun in game.links.get(module, []):
        own = [game.own_part(gun, slot) for slot, _ in game.attaches(module)]
        if not any(part and game.modules[part]['group'] == 'silencer' for part in own):
            sys.exit(f'left_out.tsv: {module}: {gun}\'s own part in its slot is no silencer ({own})')


def stats_of(game, module, rules, modules):
    """(family key, [(our key, value)], 0.69d0's as a text, the keys taken as another) of a device:
    its two modifications through rules.tsv. `modules`: every device (what an `as` rule looks among)."""
    groups = game.drop_groups(module)
    theirs = [(group, game.modification(group)) for group in groups]
    main_key = theirs[0][1][0][0]
    if main_key not in MAIN_KEYS:
        sys.exit(f'{module}: main modification {main_key}, which no family is told by')
    ours, axes, look, instead = {}, {}, False, []
    for _, modifiers in theirs:
        for key, value, kind in modifiers:
            if key not in rules:
                sys.exit(f'{module}: rules.tsv has no {key}')
            how, to = rules[key]
            if how == 'look':
                look = True
            elif how == 'same':
                ours[to] = ours.get(to, 0.0) + value
            elif how == 'as':
                instead.append(key)
                for other, value, _ in game.modification(game.main_group(to, modules)):
                    if rules[other][0] != 'same':
                        sys.exit(f'rules.tsv: {key}: {other} is not taken as it is')
                    ours[rules[other][1]] = ours.get(rules[other][1], 0.0) + value
            elif how == 'axes':
                axes.setdefault(to, []).append(value)
            elif how != 'none':
                sys.exit(f'rules.tsv: {key}: no such way {how}')
    for to, values in axes.items():
        if len(values) > 2:
            sys.exit(f'{module}: more than two axes of {to}')
        ours[to] = sum(values) / 2
    if look:
        ours = {}
    if SPREAD in ours:
        ours[AIM_TIME] = round(-ours[SPREAD] * SPREAD_COST, 3)
    said = ' | '.join(f'{group}: ' + ', '.join(f'{key} {value:+.4g}' for key, value, _ in modifiers)
                      for group, modifiers in theirs)
    return main_key, [(key, round(value, 4)) for key, value in sorted(ours.items())], said, instead


def devices(game):
    rules = {key: (how, to) for key, how, to in rows('rules.tsv')}
    prices = {family: int(price) for family, price, _ in rows('families.tsv')}
    left_out = dict(rows('left_out.tsv'))
    out = []
    modules = [module for module, _, _ in muzzle_ids() if module not in left_out]
    for module, dict_id, item in muzzle_ids():
        if module in left_out:
            check_left_out(game, module, left_out.pop(module))
            continue
        entry = game.modules[module]
        main_key, stats, said, instead = stats_of(game, module, rules, modules)
        family = GROUP_FAMILIES.get(entry['group'], MAIN_KEYS[main_key])
        model = entry['model']
        if not model.startswith('weapons/modules/'):
            sys.exit(f'{module}: model {model} is not under weapons/modules/')
        out.append({'dict_id': dict_id, 'item': item, 'module': module, 'family': family, 'price': prices[family],
                    'shoot': game.shot_effect(module), 'stats': stats, 'theirs': said,
                    'model': 'weapons/modules_069/' + model[len('weapons/modules/'):],
                    'slots': [slot for slot, _ in game.attaches(module)], 'instead': instead})
    if left_out:
        sys.exit(f'left_out.tsv: no muzzle row of ids.tsv for {sorted(left_out)}')
    return out


def table(found):
    lines = ['# The muzzle devices (devices.py from 0.69d0\'s game.options, rules.tsv and families.tsv; never edited by hand).',
             '# stats: our key and the share of the gun\'s own value it adds (modifier type 2); recoil is the six kick bounds.',
             '# ' + '\t'.join(COLUMNS)]
    for d in found:
        stats = '; '.join(f'{key} {value:+g}' for key, value in d['stats']) or '-'
        lines.append('\t'.join([str(d['dict_id']), d['item'], d['module'], d['family'], str(d['price']),
                                d['shoot'] or '-', stats, d['theirs'], d['model'], ' '.join(d['slots'])]))
    return '\n'.join(lines) + '\n'


def read_table():
    """devices.tsv as devices() gives them."""
    out = []
    for dict_id, item, module, family, price, shoot, stats, theirs, model, slots in rows('devices.tsv'):
        pairs = [] if stats == '-' else [(key, float(value)) for key, value in (s.split(' ') for s in stats.split('; '))]
        out.append({'dict_id': int(dict_id), 'item': item, 'module': module, 'family': family, 'price': int(price),
                    'shoot': None if shoot == '-' else shoot, 'stats': pairs, 'theirs': theirs, 'model': model,
                    'slots': slots.split(' ')})
    return out


def modifiers(stats):
    """A device's stats as a modification's modifiers: [(path, value)], each of type 2."""
    out = []
    for key, value in stats:
        out += [(kick, value) for kick in KICKS] if key == 'recoil' else [(key, value)]
    return out


def main():
    game = Game(Path(sys.argv[1]).read_bytes())
    text = table(devices(game))
    path = HERE / 'devices.tsv'
    if '--write' in sys.argv:
        path.write_text(text, encoding='utf-8')
    elif not path.exists() or path.read_text(encoding='utf-8') != text:
        sys.stdout.write(text)
        sys.exit('devices.tsv is not what 0.69d0\'s data and the rules give (--write)')
    print(f'{len(text.splitlines()) - 3} devices')


if __name__ == '__main__':
    main()
