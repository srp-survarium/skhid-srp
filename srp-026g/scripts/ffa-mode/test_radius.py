"""The final FFA radius retune preserves everything except solo enemy distance."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_radius_edits as m

sys.path.insert(0, str(m.base.HERE.parent / 'content-edit'))
from test_content_edit import config


class RadiusRetune(unittest.TestCase):
    def test_checked_in_recipe_covers_every_spawn_above_twenty_metres(self):
        rows = json.loads(m.OUTPUT.read_text())['set_entry']
        self.assertEqual(len(rows), 696)
        self.assertEqual(sum(row['from']['enemy_check_radius'] == 50 for row in rows), 3)
        expected = set(m.projects()) - {
            f'projects/level_08/{weather}_8/client_project' for weather in ('day', 'evening', 'prerain')}
        self.assertEqual({row['file'] for row in rows}, expected)
        self.assertEqual(len({(row['file'], row['where']['point_id']) for row in rows}), len(rows))
        for row in rows:
            self.assertEqual(row['where']['team'], 2)
            self.assertEqual(row['to'], {'enemy_check_radius': 20})
            self.assertEqual(set(row['from']), {'enemy_check_radius'})

    def test_only_solo_radius_changes_including_the_existing_integer_radius(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = 'projects/test/day_4/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            entries = [{'point_id': i, 'name': f'p{i}', 'team': team,
                'position': (float(i), 6., 5.), 'enemy_check_radius': radius,
                'invulnerability_time': 2} for i, (team, radius) in enumerate([
                    (0, 30.), (1, 50), (2, 30.), (2, 50), (2, 20.)])]
            source.write_bytes(config({'server_objects': {'respawn_points': entries}}))
            old = m.base.tree(source.read_bytes())
            with patch.object(m, 'projects', return_value=[path]):
                recipe = m.edits(root / 'live')
            self.assertEqual(len(recipe['set_entry']), 2)
            recipe_path = root / 'edits.json'
            recipe_path.write_text(json.dumps(recipe))
            subprocess.run([sys.executable, str(m.base.EDIT), str(recipe_path),
                            str(root/'live'), str(root/'out')], check=True, stdout=subprocess.DEVNULL)
            actual = m.base.tree((root/'out'/path).read_bytes())
            for entry in old['server_objects']['respawn_points']:
                if entry['team'] == 2:
                    entry['enemy_check_radius'] = 20
            self.assertEqual(actual, old)


if __name__ == '__main__':
    unittest.main()
