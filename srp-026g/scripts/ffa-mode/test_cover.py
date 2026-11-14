"""Spawns moved or turned by the spawn rule keep everything but their position and rotation."""
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
        rows = json.loads(m.OUTPUT.read_text())['set_entry']
        points = m.changed()
        self.assertEqual(len(rows), sum(len(p) for p in points.values()))
        for row in rows:
            self.assertRegex(row['file'], r'^projects/[^/]+/[a-z]+_8/client_project$')
            self.assertFalse(row['file'].startswith(m.NOT_HERE))
            self.assertEqual(row['where']['team'], 2)
            verdict, position, yaw = points[row['file']][row['where']['point_id']]
            if verdict == 'replaced':
                self.assertEqual(row['to'], {'position': position, 'rotation': [0., yaw, 0.]})
            else:
                self.assertEqual(row['to'], {'rotation': [0., yaw, 0.]})
            self.assertEqual(set(row['from']), set(row['to']))

    def test_only_the_changed_solo_spawns_change(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = 'projects/test/day_8/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            entries = [{'point_id': i, 'name': f'p{i}', 'team': team,
                'position': (float(40 * i), 6., 5.), 'rotation': (3.142, 1., 3.142),
                'enemy_check_radius': 20., 'invulnerability_time': 0}
                for i, team in enumerate([0, 2, 2, 2])]
            source.write_bytes(config({'server_objects': {'respawn_points': entries}}))
            old = m.base.tree(source.read_bytes())
            changes = {path: {2: ('replaced', [100., 7., 50.], 0.5), 3: ('turned', [0., 0., 0.], -1.)}}
            with patch.object(m, 'changed', return_value=changes):
                recipe = m.edits(root / 'live')
            self.assertEqual(len(recipe['set_entry']), 2)
            # No distance between spawns is kept: beside point 3 is a place too.
            close = {path: {2: ('replaced', [119., 6., 5.], 0.5)}}
            with patch.object(m, 'changed', return_value=close):
                self.assertEqual(len(m.edits(root / 'live')['set_entry']), 1)
            # A point the project does not have is refused.
            with patch.object(m, 'changed', return_value={path: {9: ('turned', [0., 0., 0.], 0.)}}), \
                    self.assertRaises(ValueError):
                m.edits(root / 'live')
            recipe_path = root / 'edits.json'
            recipe_path.write_text(json.dumps(recipe))
            subprocess.run([sys.executable, str(m.base.EDIT), str(recipe_path),
                            str(root/'live'), str(root/'out')], check=True, stdout=subprocess.DEVNULL)
            actual = m.base.tree((root/'out'/path).read_bytes())
            points = old['server_objects']['respawn_points']
            points[2]['position'] = [100., 7., 50.]
            points[2]['rotation'] = [0., 0.5, 0.]
            points[3]['rotation'] = [0., -1., 0.]
            self.assertEqual(actual, old)


if __name__ == '__main__':
    unittest.main()
