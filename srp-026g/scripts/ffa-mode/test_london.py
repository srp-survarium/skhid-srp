"""Additional London spawns preserve existing records and native relocation safety."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_london_edits as m

sys.path.insert(0, str(m.HERE.parent / 'content-edit'))
from test_content_edit import config


class LondonCenter(unittest.TestCase):
    def fixture(self, root):
        entries = []
        for i in range(24):
            entries.append({'point_id': i, 'team': 0 if i < 2 else 2,
                'name': f'original_{i}', 'full_name': f'original_{i}',
                'position': (100. + i*20, 6., 100.), 'rotation': (0., 0., 0.),
                'enemy_check_radius': 30., 'ally_check_radius': 1.,
                'invulnerability_time': 2, 'priority': 1, 'is_temporary': False,
                'ally_zones': {'collision_geometries': []},
                'enemy_zones': {'collision_geometries': []}})
        target = root / m.PATH
        target.parent.mkdir(parents=True)
        target.write_bytes(config({'server_objects': {'respawn_points': entries}}))
        return target

    def test_recipe_matches_reviewed_points(self):
        edits = json.loads(m.OUTPUT.read_text())['add_entry']
        self.assertEqual(len(edits), 6)
        for edit, row in zip(edits, m.points()):
            self.assertEqual(edit['files'], m.PATH)
            self.assertEqual(edit['values']['position'], [float(row[k]) for k in ('x', 'y', 'z')])
            self.assertEqual(edit['values']['rotation'], [float(row[k]) for k in ('rx', 'ry', 'rz')])

    def test_existing_spawns_unchanged_and_28_solo_records_relocate_once(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            live = root / 'live'
            source = self.fixture(live)
            old = m.base.tree(source.read_bytes())['server_objects']['respawn_points']
            recipe = root / 'edits.json'
            recipe.write_text(json.dumps(m.edits(live)))
            out = root / 'out'
            subprocess.run([sys.executable, str(m.HERE.parent/'content-edit/content_edit.py'),
                            str(recipe), str(live), str(out)], check=True, stdout=subprocess.DEVNULL)
            blob = (out/m.PATH).read_bytes()
            result = m.base.tree(blob)['server_objects']['respawn_points']
            self.assertEqual(result[:len(old)], old)
            self.assertEqual(sum(p['team'] == 2 for p in result), 28)
            for p in result[len(old):]:
                self.assertEqual((p['team'], p['invulnerability_time'], p['enemy_check_radius']), (2, 2, 30.))
                self.assertFalse(p['ally_zones']['collision_geometries'])
                self.assertFalse(p['enemy_zones']['collision_geometries'])
            offsets = m.base.c.value_offsets(blob)
            self.assertEqual(len(offsets), len(set(offsets)))

    def test_unchecked_or_overlapping_additions_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.fixture(root / 'live')
            points = root / 'points.tsv'
            raw = m.POINTS.read_text()
            points.write_text(raw.replace('kept', 'dropped', 1))
            with patch.object(m, 'POINTS', points), self.assertRaises(ValueError):
                m.edits(root / 'live')
            tree = m.base.tree(source.read_bytes())
            first = m.points()[0]
            tree['server_objects']['respawn_points'][2]['position'] = tuple(float(first[k]) for k in ('x', 'y', 'z'))
            source.write_bytes(config(tree))
            with self.assertRaisesRegex(ValueError, 'too close'):
                m.edits(root / 'live')


if __name__ == '__main__':
    unittest.main()
