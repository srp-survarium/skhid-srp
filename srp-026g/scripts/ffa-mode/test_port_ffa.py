"""Each port's Free for All list is its picks: three configs and copies, the owner's points in each, no zones."""
import json
import unittest

import make_port_ffa_edits as m


class Ports(unittest.TestCase):
    def test_each_list_is_its_picks_in_every_weather(self):
        ids = [i for port in m.PORTS.values() for _, _, i in port.maps]
        self.assertEqual(len(ids), len(set(ids)))
        for key, port in m.PORTS.items():
            with self.subTest(port=key):
                self.recipe_is_its_picks(port)

    def recipe_is_its_picks(self, port):
        recipe = json.loads(port.output.read_text())
        by = m.picks(port)
        self.assertEqual(sorted((c['from'], c['to']) for c in recipe['copy']), sorted(
            [(m.config(port, w, 4), m.config(port, w, 8)) for w, _, _ in port.maps]
            + [(f'projects/{port.level}/{weather}_4/', f'projects/{port.level}/{weather}_8/')
               for _, weather, _ in port.maps]))
        rows = [e for e in recipe['add_entry'] if e['list'] == 'maps_dict']
        self.assertEqual([(e['values']['id'], e['values']['cfg_mode_name']) for e in rows],
                         [(i, m.config(port, w, 8).removeprefix('gameplay/game_modes/')) for w, _, i in port.maps])
        for w, weather, _ in port.maps:
            with self.subTest(weather=weather):
                ffa, copy = m.config(port, w, 8), m.project(port, weather, 8)
                self.assertIn({'file': ffa, 'path': 'mode/mode', 'from': 4, 'to': 8}, recipe['set'])
                self.assertIn({'file': ffa, 'path': 'mode/rules', 'value': m.TDM_RULE}, recipe['remove_item'])
                self.assertIn({'files': ffa, 'from': f'{port.level}/{weather}_4', 'to': f'{port.level}/{weather}_8'},
                              recipe['replace_string'])
                points = [e for e in recipe['add_entry'] if e['files'] == copy]
                self.assertEqual([(e['values']['point_id'], e['values']['position'], e['values']['rotation'])
                                  for e in points],
                                 [(i, p, [0., yaw, 0.]) for i, p, yaw in by[f'{port.level}/{weather}_4']])
                for e in points:
                    self.assertEqual((e['values']['name'], e['values']['team'], e['values']['enemy_check_radius'],
                                      e['values']['invulnerability_time']),
                                     (f'{port.prefix}{e["values"]["point_id"]}', 2, 20., 0))
                protection = [v for v in recipe['add_value'] if v['files'] == [copy]]
                self.assertEqual({(v['key'], v['value']) for v in protection}, {('invulnerability_time_ms', 500)})
                self.assertEqual(len(protection), len(points))
                # A zone taken out is a new point's, never a team point's.
                first = len(points) and points[0]['values']['point_id']
                for e in recipe.get('remove_entry', []):
                    if '/collision_geometries' in e['list']:
                        index = int(e['list'].split('[')[1].split(']')[0])
                        self.assertGreaterEqual(index, first)
                # The Team Deathmatch files are only copied.
                for kind in ('set', 'remove_entry', 'remove_item'):
                    self.assertFalse([e for e in recipe.get(kind, [])
                                      if e['file'] in (m.config(port, w, 4), m.project(port, weather, 4))])


if __name__ == '__main__':
    unittest.main()
