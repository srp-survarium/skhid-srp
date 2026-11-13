"""Spawns moved to the centre keep everything but their position and rotation."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_center_edits as m

sys.path.insert(0, str(m.base.HERE.parent / 'content-edit'))
from test_content_edit import config


class CenterMoves(unittest.TestCase):
    def test_checked_in_recipe_is_center_tsv_on_free_for_all_projects_only(self):
        rows = json.loads(m.OUTPUT.read_text())['set_entry']
        points = m.moved()
        self.assertEqual(len(rows), sum(len(p) for p in points.values()))
        for row in rows:
            self.assertRegex(row['file'], r'^projects/[^/]+/[a-z]+_8/client_project$')
            self.assertEqual(row['where']['team'], 2)
            self.assertEqual(set(row['from']), {'position', 'rotation'})
            position, yaw = points[row['file']][row['where']['point_id']]
            self.assertEqual(row['to'], {'position': position, 'rotation': [0., yaw, 0.]})

    def test_only_the_moved_solo_spawn_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = 'projects/test/day_8/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            entries = [{'point_id': i, 'name': f'p{i}', 'team': team,
                'position': (float(40 * i), 6., 5.), 'rotation': (3.142, 1., 3.142),
                'enemy_check_radius': 20., 'invulnerability_time': 2}
                for i, team in enumerate([0, 2, 2, 2])]
            source.write_bytes(config({'server_objects': {'respawn_points': entries}}))
            old = m.base.tree(source.read_bytes())
            moves = {path: {2: ([100., 7., 50.], 0.5)}}
            with patch.object(m, 'moved', return_value=moves):
                recipe = m.edits(root / 'live')
                self.assertEqual(len(recipe['set_entry']), 1)
                with self.assertRaises(ValueError):  # within 12 m of point 3
                    with patch.object(m, 'moved', return_value={path: {2: ([118., 7., 5.], 0.5)}}):
                        m.edits(root / 'live')
            recipe_path = root / 'edits.json'
            recipe_path.write_text(json.dumps(recipe))
            subprocess.run([sys.executable, str(m.base.EDIT), str(recipe_path),
                            str(root/'live'), str(root/'out')], check=True, stdout=subprocess.DEVNULL)
            actual = m.base.tree((root/'out'/path).read_bytes())
            old['server_objects']['respawn_points'][2]['position'] = [100., 7., 50.]
            old['server_objects']['respawn_points'][2]['rotation'] = [0., 0.5, 0.]
            self.assertEqual(actual, old)


if __name__ == '__main__':
    unittest.main()
