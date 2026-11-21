"""The devices' rules (devices.py, rules.tsv) on a made-up game.options, the checked-in table, and the
scale of the shot effects (make_pack.py) on made-up effects."""
import collections
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402
import devices as dev  # noqa: E402
sys.path.insert(0, str(HERE))      # this folder's make_pack, not research-mode's
import make_pack as mp  # noqa: E402

KEYS = {300: [('recoil_modifier', -0.04, 1)], 403: [('aim_time_modifier', 0.03, 1)],
        315: [('unmasking_radius', -0.4, 2)], 402: [('recoil_hip_modifier', 0.03, 1)],
        314: [('movement_speed_damage', 0.08, 1)],
        307: [('dispersion_buck_shot_x_scale', -0.2, 2), ('dispersion_buck_shot_y_scale', -0.2, 2)],
        308: [('dispersion_buck_shot_x_scale', 0.4, 2)], 405: [('dispersion_buck_shot_y_scale', -0.7, 2)],
        999: [('no_such_key', 1.0, 1)]}


def module(dict_id, group, slots, effect=None):
    entry = {'group': group, 'model': 'weapons/modules/x/' + group, 'parameters': {'dict_id': dict_id},
             'slots_to_attach_to': [{'slot': slot, 'attach_params': f'rules{i}'} for i, slot in enumerate(slots)]}
    for i, _ in enumerate(slots):
        one = effect[i] if isinstance(effect, list) else effect
        entry[f'rules{i}'] = {'effects_to_override': {'particles': {'shoot': one}}} if one else {}
    return entry


def game(modules, drops, weapons=None, links=()):
    return dev.Game(bcw.dump({
        'weapon_modules': modules,
        'weapons': weapons or {'gun': {'parameters': {'movement_speed_damage': 0.3}}},
        'module_to_item_links': [{'module': m, 'item': g} for m, g in links] or [{'module': '-', 'item': '-'}],
        'modifications': [{'group': group, 'grade': grade,
                           'modifiers': [{'path': ['parameters', key], 'value': value * (1 if grade == 7 else 9), 'type': kind}
                                         for key, value, kind in keys]}
                          for group, keys in KEYS.items() for grade in (6, 7)],
        'modules_drop_options': {'modules_additional_infos': [
            {'dict_id': dict_id, 'first_drop_modifier': first, 'second_drop_modifier': second}
            for dict_id, (first, second) in drops.items()]},
    }))


RULES = {key: (how, to) for key, how, to in dev.rows('rules.tsv')}


class Rules(unittest.TestCase):
    def stats(self, first, second, **more):
        g = game({'m': module(1, 'x', ['thread'])}, {1: (first, second)}, links=[('m', 'gun')], **more)
        return dev.stats_of(g, 'm', RULES, ['m'])[:2]

    def test_a_slowdown_that_is_dead_here_is_a_brakes_recoil(self):
        modules = {'m': module(1, 'x', ['thread']), 'brake': module(2, 'x', ['thread'])}
        g = game(modules, {1: (314, 403), 2: (300, 403)}, links=[('m', 'gun')])
        self.assertEqual(dev.stats_of(g, 'm', RULES, ['m', 'brake'])[1:], ([('parameters/aim_time', 0.03), ('recoil', -0.04)],
                         '314: parameters/movement_speed_damage +0.08 | 403: parameters/aim_time_modifier +0.03',
                         ['parameters/movement_speed_damage']))
        with self.assertRaises(SystemExit):      # no device rolls recoil first: nothing to take
            dev.stats_of(g, 'm', RULES, ['m'])

    def test_a_brake_keeps_its_two_numbers_at_the_bought_grade(self):
        self.assertEqual(self.stats(300, 403), ('parameters/recoil_modifier', [('parameters/aim_time', 0.03), ('recoil', -0.04)]))
        self.assertEqual(dev.modifiers([('recoil', -0.04)]), [(kick, -0.04) for kick in dev.KICKS])
        self.assertEqual(len(set(dev.KICKS)), 6)

    def test_a_flash_hider_takes_no_number_not_even_its_penalty(self):
        self.assertEqual(self.stats(315, 403), ('parameters/unmasking_radius', []))
        self.assertEqual(self.stats(315, 402), ('parameters/unmasking_radius', []))

    def test_the_pellet_axes_are_one_spread(self):
        self.assertEqual(self.stats(307, 403)[1], [('dispersion/base_dispersion', -0.2), ('parameters/aim_time', 0.03)])
        self.assertEqual(self.stats(308, 405)[1], [('dispersion/base_dispersion', -0.15)])

    def test_a_key_without_a_rule_stops_the_build(self):
        with self.assertRaises(SystemExit):
            self.stats(300, 999)
        with self.assertRaises(SystemExit):      # nor is it a family
            self.stats(999, 403)

    def test_a_device_has_one_shot_effect(self):
        one = game({'m': module(1, 'x', ['a', 'b'], 'fx/one')}, {1: (300, 403)})
        self.assertEqual(one.shot_effect('m'), 'fx/one')
        self.assertIsNone(game({'m': module(1, 'x', ['a'])}, {1: (300, 403)}).shot_effect('m'))
        with self.assertRaises(SystemExit):
            game({'m': module(1, 'x', ['a', 'b'], ['fx/one', 'fx/two'])}, {1: (300, 403)}).shot_effect('m')

    def test_a_barrel_is_left_out_only_where_the_guns_own_part_is_a_silencer(self):
        modules = {'barrel': module(1, 'heavy_barrel', ['thread']), 'can': module(2, 'silencer', ['thread']),
                   'brake': module(3, 'muzzle', ['thread'])}

        def weapons(own):
            return {'gun': {'parameters': {'movement_speed_damage': 0.3}, 'default_modules': [{'slot': 'thread', 'module': own}]}}
        dev.check_left_out(game(modules, {}, weapons('can'), [('barrel', 'gun')]), 'barrel', 'suppressor_barrel')
        with self.assertRaises(SystemExit):
            dev.check_left_out(game(modules, {}, weapons('brake'), [('barrel', 'gun')]), 'barrel', 'suppressor_barrel')
        with self.assertRaises(SystemExit):      # a brake is no barrel
            dev.check_left_out(game(modules, {}, weapons('can'), [('brake', 'gun')]), 'brake', 'suppressor_barrel')


class Table(unittest.TestCase):
    def test_the_table_reads_back_as_it_is_written(self):
        found = dev.read_table()
        self.assertEqual(dev.table(found), (HERE / 'devices.tsv').read_text(encoding='utf-8'))
        prices = {family: int(price) for family, price, _ in dev.rows('families.tsv')}
        ids = {module: (dict_id, item) for module, dict_id, item in dev.muzzle_ids()}
        left_out = {module for module, _ in dev.rows('left_out.tsv')}
        self.assertEqual({d['module'] for d in found} | left_out, set(ids))
        for d in found:
            self.assertEqual((d['dict_id'], d['item']), ids[d['module']])
            self.assertEqual(d['price'], prices[d['family']])
            # only a flash hider is without a number
            self.assertEqual(d['stats'] == [], d['family'] == 'flash_hider', d['module'])


def effect(light=None, materials=(), enabled=True):
    """A made-up shot effect: an emitter a material, and a light of that peak."""
    emitters = {f'e{i}': {'type_index': 2, 'properties': {'Material': name, 'Enabled': True}} for i, name in enumerate(materials)}
    if light is not None:
        key = {'position': (0.1, light), 'left_tangent': (0.0, light), 'right_tangent': (0.2, light), 'delta': 0, 'key_type': 1}
        zero = {'position': (1.0, 0.0), 'left_tangent': (0.9, 0.0), 'right_tangent': (1.1, 0.0), 'delta': 0, 'key_type': 1}
        emitters['light'] = {'type_index': 2, 'properties': {'Material': 'null', 'Enabled': enabled}, 'children': {
            'l': {'type_index': mp.LIGHT, 'properties': {'Enabled': True, 'Intensity': {'source': {'data': {'curve0': {'key0': key, 'key1': zero}}}}}}}}
    return {'LOD 0': {'children': emitters}}


def material(brightness, base=mp.FIRE):
    return {'material': {'instance': {'base_material': base}, 'forward': {mp.EMISSIVE: {'value': brightness}}}}


class Scale(unittest.TestCase):
    """0.46c2's effects on 0.26g's scale: an effect's lights by its family's stock light here over there,
    the fire materials by the one factor the families' own share."""
    STOCK = {'rifle': effect(0.8, ['front', 'idle']), 'pistol': effect(0.5, ['small'])}
    DONOR = {'rifle': effect(0.4, ['front', 'idle']), 'pistol': effect(0.1, ['small'])}
    HERE_ = {'front': material(4), 'small': material(4), 'idle': material(1), 'null': material(1, 'default_particle')}
    THERE = {'front': material(1), 'small': material(1), 'idle': material(1), 'null': material(1, 'default_particle')}

    def scale(self, effects, replaced, stock=None, here=None):
        return mp.scale_of(effects, replaced, (stock or self.STOCK).get, self.DONOR.get, (here or self.HERE_).get, self.THERE.get,
                           lambda name: name in ('idle', 'null'))

    def test_an_effect_takes_the_family_most_of_its_fits_replace(self):
        effects = {'brake': effect(0.2, ['jet', 'idle']), 'pocket': effect(0.1, ['jet']), 'smoke': effect(None, ['puff'])}
        replaced = {'brake': collections.Counter(rifle=5, pistol=2), 'pocket': collections.Counter(pistol=3),
                    'smoke': collections.Counter(rifle=1)}
        families, by_effect, fire = self.scale(effects, replaced)
        self.assertEqual(by_effect['brake'], ('rifle', 5, 7, 2.0))
        self.assertEqual(by_effect['pocket'], ('pistol', 3, 3, 5.0))
        self.assertEqual(by_effect['smoke'], ('rifle', 1, 1, None))      # no light: none is given
        self.assertEqual(fire, 4.0)                                      # idle is the players' own in the effects: no measure
        self.assertEqual(families['rifle'], (0.8, 0.4, {'front': (4, 1), 'idle': (1, 1)}))

    def test_no_family_or_no_one_factor_stops_the_build(self):
        with self.assertRaises(SystemExit):      # as many fits on one stock flash as on the other
            self.scale({'brake': effect(0.2)}, {'brake': collections.Counter(rifle=2, pistol=2)})
        with self.assertRaises(SystemExit):      # a family whose fire is brighter by another factor
            self.scale({'a': effect(0.2), 'b': effect(0.2)}, {'a': collections.Counter(rifle=1), 'b': collections.Counter(pistol=1)},
                       here=dict(self.HERE_, small=material(5)))
        with self.assertRaises(SystemExit):      # a light, and the family has none here
            self.scale({'a': effect(0.2)}, {'a': collections.Counter(rifle=1)}, stock=dict(self.STOCK, rifle=effect(None, ['front'])))

    def test_a_light_is_scaled_whole_and_a_disabled_one_is_none(self):
        tree = effect(0.2)
        mp.scale_lights(tree, 2.5)
        self.assertEqual(mp.light_peak(tree), 0.5)
        key = tree['LOD 0']['children']['light']['children']['l']['properties']['Intensity']['source']['data']['curve0']['key0']
        self.assertEqual((key['position'], key['left_tangent'], key['right_tangent']), ((0.1, 0.5), (0.0, 0.5), (0.2, 0.5)))
        self.assertIsNone(mp.light_peak(effect(0.2, enabled=False)))
        self.assertIsNone(mp.emissive(material(1, 'default_particle')))

    def test_the_table_is_the_packs(self):
        """effect_scale.tsv as checked in: every light the family's ratio times the donor's, the fire one factor."""
        rows = [line.split('\t') for line in (HERE / 'effect_scale.tsv').read_text(encoding='utf-8').splitlines() if not line.startswith('#')]
        ratio = {row[1]: float(row[4]) for row in rows if row[0] == 'family'}
        effects = [row for row in rows if row[0] == 'effect']
        self.assertEqual({row[1] for row in effects}, {d['shoot'] for d in dev.read_table() if (d['shoot'] or '').startswith(mp.DONOR_EFFECTS)})
        for _, name, family, _, before, after in effects:
            if before != '-':
                self.assertAlmostEqual(float(after), float(before) * ratio[family], delta=2e-3, msg=name)
        factors = {float(after) / float(before) for kind, _, before, after in (row for row in rows if row[0] == 'material')}
        self.assertEqual(len(factors), 1)


if __name__ == '__main__':
    unittest.main()
