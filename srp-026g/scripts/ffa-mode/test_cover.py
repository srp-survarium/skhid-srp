"""Spawns moved, turned or added by the spawn rule: only those change, the new ones copies of a solo spawn."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_cover_edits as m

sys.path.insert(0, str(m.base.HERE.parent / 'content-edit'))
from test_content_edit import config


class CoverMoves(unittest.TestCase):
    def test_checked_in_recipe_is_cover_maps_tsv_on_free_for_all_projects_only(self):
        recipe = json.loads(m.OUTPUT.read_text())
        points = m.changed()
        count = lambda *verdicts: sum(v[0] in verdicts for p in points.values() for v in p.values())
        self.assertEqual(len(recipe['set_entry']), count('replaced', 'turned'))
        self.assertEqual(len(recipe['add_entry']), count('added'))
        for row in recipe['set_entry']:
            self.assertRegex(row['file'], r'^projects/[^/]+/[a-z]+_8/client_project$')
            self.assertFalse(row['file'].startswith(m.NOT_HERE))
            self.assertEqual(row['where']['team'], 2)
            verdict, position, yaw = points[row['file']][row['where']['point_id']]
            if verdict == 'replaced':
                self.assertEqual(row['to'], {'position': position, 'rotation': [0., yaw, 0.]})
            else:
                self.assertEqual(row['to'], {'rotation': [0., yaw, 0.]})
            self.assertEqual(set(row['from']), set(row['to']))
        for row in recipe['add_entry']:
            self.assertRegex(row['files'], r'^projects/[^/]+/[a-z]+_8/client_project$')
            verdict, position, yaw = points[row['files']][row['values']['point_id']]
            self.assertEqual(verdict, 'added')
            self.assertEqual((row['values']['position'], row['values']['rotation']), (position, [0., yaw, 0.]))
            self.assertEqual((row['values']['team'], row['values']['invulnerability_time_ms'],
                              row['values']['enemy_check_radius']), (2, 500, 20.))

    def test_only_the_changed_solo_spawns_change_and_new_ones_follow_the_last(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = 'projects/test/day_8/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            entries = [{'point_id': i, 'name': f'p{i}', 'full_name': f'logic/p{i}', 'team': team,
                'position': (float(40 * i), 6., 5.), 'rotation': (3.142, 1., 3.142),
                'enemy_check_radius': 20., 'ally_check_radius': 1., 'invulnerability_time': 0,
                'invulnerability_time_ms': 500, 'ally_zones': {'collision_geometries': {}},
                'enemy_zones': {'collision_geometries': {}}}
                for i, team in enumerate([0, 2, 2, 2])]
            source.write_bytes(config({'server_objects': {'respawn_points': entries}}))
            old = m.base.tree(source.read_bytes())
            # A new point right beside point 3: no distance between spawns is kept.
            changes = {path: {2: ('replaced', [100., 7., 50.], 0.5), 3: ('turned', [0., 0., 0.], -1.),
                              4: ('added', [121., 6., 5.], 1.5)}}
            with patch.object(m, 'changed', return_value=changes):
                recipe = m.edits(root / 'live')
            self.assertEqual((len(recipe['set_entry']), len(recipe['add_entry'])), (2, 1))
            # A point the project does not have, or a new one not numbered after the last, is refused.
            for wrong in ({9: ('turned', [0., 0., 0.], 0.)}, {5: ('added', [0., 0., 0.], 0.)}):
                with self.subTest(wrong=wrong), patch.object(m, 'changed', return_value={path: wrong}), \
                        self.assertRaises(ValueError):
                    m.edits(root / 'live')
            recipe_path = root / 'edits.json'
            recipe_path.write_text(json.dumps(recipe))
            subprocess.run([sys.executable, str(m.base.EDIT), str(recipe_path),
                            str(root/'live'), str(root/'out')], check=True, stdout=subprocess.DEVNULL)
            actual = m.base.tree((root/'out'/path).read_bytes())['server_objects']['respawn_points']
            expected = json.loads(json.dumps(old))['server_objects']['respawn_points']
            expected[2]['position'], expected[2]['rotation'] = [100., 7., 50.], [0., 0.5, 0.]
            expected[3]['rotation'] = [0., -1., 0.]
            self.assertEqual(actual[:4], expected)
            self.assertEqual(len(actual), 5)
            new = actual[4]
            self.assertEqual((new['point_id'], new['name'], new['team'], new['position'], new['rotation'],
                              new['invulnerability_time_ms'], new['enemy_check_radius']),
                             (4, 'ffa_cover_4', 2, [121., 6., 5.], [0., 1.5, 0.], 500, 20.))


if __name__ == '__main__':
    unittest.main()
