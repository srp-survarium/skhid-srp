"""London (v0.42)'s Free for All list is its picks: three configs and copies, the owner's points in each."""
import json
import unittest

import make_london_v042_edits as m


class LondonV042(unittest.TestCase):
    def test_the_list_is_its_picks_in_every_weather(self):
        recipe = json.loads(m.OUTPUT.read_text())
        by = m.picks()
        self.assertEqual(sorted((c['from'], c['to']) for c in recipe['copy']), sorted(
            [(m.config(w, 4), m.config(w, 8)) for w, _, _ in m.MAPS]
            + [(f'projects/{m.LEVEL}/{weather}_4/', f'projects/{m.LEVEL}/{weather}_8/') for _, weather, _ in m.MAPS]))
        rows = [e for e in recipe['add_entry'] if e['list'] == 'maps_dict']
        self.assertEqual([(e['values']['id'], e['values']['cfg_mode_name']) for e in rows],
                         [(i, m.config(w, 8).removeprefix('gameplay/game_modes/')) for w, _, i in m.MAPS])
        for w, weather, _ in m.MAPS:
            with self.subTest(weather=weather):
                ffa = m.config(w, 8)
                self.assertIn({'file': ffa, 'path': 'mode/mode', 'from': 4, 'to': 8}, recipe['set'])
                self.assertIn({'file': ffa, 'path': 'mode/rules', 'value': m.TDM_RULE}, recipe['remove_item'])
                self.assertIn({'files': ffa, 'from': f'{m.LEVEL}/{weather}_4', 'to': f'{m.LEVEL}/{weather}_8'},
                              recipe['replace_string'])
                points = [e for e in recipe['add_entry'] if e['files'] == m.project(weather, 8)]
                self.assertEqual([(e['values']['point_id'], e['values']['position'], e['values']['rotation'])
                                  for e in points],
                                 [(i, p, [0., yaw, 0.]) for i, p, yaw in by[f'{m.LEVEL}/{weather}_4']])
                for e in points:
                    self.assertEqual((e['values']['team'], e['values']['enemy_check_radius'],
                                      e['values']['invulnerability_time']), (2, 20., 0))
                protection = [v for v in recipe['add_value'] if v['files'] == [m.project(weather, 8)]]
                self.assertEqual({(v['key'], v['value']) for v in protection}, {('invulnerability_time_ms', 500)})
                self.assertEqual(len(protection), len(points))
                # The Team Deathmatch files are only copied.
                for kind in ('set', 'remove_entry', 'remove_item'):
                    self.assertFalse([e for e in recipe.get(kind, []) if e['file'] in (m.config(w, 4), m.project(weather, 4))])


if __name__ == '__main__':
    unittest.main()
