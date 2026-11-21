"""The devices' rules (devices.py, rules.tsv) on a made-up game.options, and the checked-in table."""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'research-mode'))
import bcw  # noqa: E402
import devices as dev  # noqa: E402

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
        return dev.stats_of(g, 'm', RULES)[:2]

    def test_a_brake_keeps_its_two_numbers_at_the_bought_grade(self):
        self.assertEqual(self.stats(300, 403), ('parameters/recoil_modifier', [('parameters/aim_time', 0.03), ('recoil', -0.04)]))
        self.assertEqual(dev.modifiers([('recoil', -0.04)]), [(kick, -0.04) for kick in dev.KICKS])
        self.assertEqual(len(set(dev.KICKS)), 6)

    def test_a_flash_hider_takes_no_number_not_even_its_penalty(self):
        self.assertEqual(self.stats(315, 403), ('parameters/unmasking_radius', []))
        self.assertEqual(self.stats(315, 402), ('parameters/unmasking_radius', []))

    def test_the_slowdown_is_its_share_of_the_guns_own(self):
        self.assertEqual(self.stats(314, 403)[1], [('parameters/aim_time', 0.03), ('parameters/stopping_power', 0.2667)])
        two = {'gun': {'parameters': {'movement_speed_damage': 0.3}}, 'other': {'parameters': {'movement_speed_damage': 0.2}}}
        g = game({'m': module(1, 'x', ['thread'])}, {1: (314, 403)}, weapons=two, links=[('m', 'gun'), ('m', 'other')])
        with self.assertRaises(SystemExit):
            dev.stats_of(g, 'm', RULES)

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


if __name__ == '__main__':
    unittest.main()
