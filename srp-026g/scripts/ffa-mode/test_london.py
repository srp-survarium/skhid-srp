"""Additional London spawns preserve existing records and native relocation safety."""
import dataclasses
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import make_london_edits as m

sys.path.insert(0, str(m.HERE.parent / 'content-edit'))
from test_content_edit import config


class LondonAdditions(unittest.TestCase):
    def fixture(self, root, region):
        entries = []
        for i in range(region.existing + 2):
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
        for region in m.REGIONS.values():
            edits = json.loads(region.output.read_text())['add_entry']
            self.assertEqual(len(edits), region.count)
            for edit, row in zip(edits, m.points(region)):
                self.assertEqual(edit['files'], m.PATH)
                self.assertEqual(edit['values']['position'], [float(row[k]) for k in ('x', 'y', 'z')])
                self.assertEqual(edit['values']['rotation'], [float(row[k]) for k in ('rx', 'ry', 'rz')])
                self.assertEqual(edit['values']['enemy_check_radius'], region.radius)

    def test_existing_spawns_unchanged_and_solo_records_relocate_once(self):
        for region in m.REGIONS.values():
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                live = root / 'live'
                source = self.fixture(live, region)
                old = m.base.tree(source.read_bytes())['server_objects']['respawn_points']
                recipe = root / 'edits.json'
                recipe.write_text(json.dumps(m.edits(region, live)))
                out = root / 'out'
                subprocess.run([sys.executable, str(m.HERE.parent/'content-edit/content_edit.py'),
                                str(recipe), str(live), str(out)], check=True, stdout=subprocess.DEVNULL)
                blob = (out/m.PATH).read_bytes()
                result = m.base.tree(blob)['server_objects']['respawn_points']
                self.assertEqual(result[:len(old)], old)
                self.assertEqual(sum(p['team'] == 2 for p in result), region.existing + region.count)
                for p in result[len(old):]:
                    self.assertEqual((p['team'], p['invulnerability_time'], p['enemy_check_radius']),
                                     (2, 2, region.radius))
                    self.assertFalse(p['ally_zones']['collision_geometries'])
                    self.assertFalse(p['enemy_zones']['collision_geometries'])
                offsets = m.base.c.value_offsets(blob)
                self.assertEqual(len(offsets), len(set(offsets)))

    def test_unchecked_or_overlapping_additions_rejected(self):
        for region in m.REGIONS.values():
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                source = self.fixture(root / 'live', region)
                points = root / 'points.tsv'
                raw = region.points.read_text()
                points.write_text(raw.replace('kept', 'dropped', 1))
                with self.assertRaises(ValueError):
                    m.edits(dataclasses.replace(region, points=points), root / 'live')
                tree = m.base.tree(source.read_bytes())
                first = m.points(region)[0]
                tree['server_objects']['respawn_points'][2]['position'] = tuple(float(first[k]) for k in ('x', 'y', 'z'))
                source.write_bytes(config(tree))
                with self.assertRaisesRegex(ValueError, 'too close'):
                    m.edits(region, root / 'live')

    def test_underpass_spacing_is_3d(self):
        """A spawn of the rotunda's storey 13 m over an underpass point leaves it clear; one on the
        street 5 m over it and 10 m off in plan does not."""
        region = m.UNDERPASS
        first = [float(m.points(region)[0][k]) for k in ('x', 'y', 'z')]
        for over, clear in (((0., 13.4, 0.), True), ((10., 5.3, 0.), False)):
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                source = self.fixture(root / 'live', region)
                tree = m.base.tree(source.read_bytes())
                tree['server_objects']['respawn_points'][2]['position'] = tuple(a + d for a, d in zip(first, over))
                source.write_bytes(config(tree))
                if clear:
                    self.assertEqual(len(m.edits(region, root / 'live')['add_entry']), region.count)
                else:
                    with self.assertRaisesRegex(ValueError, 'too close'):
                        m.edits(region, root / 'live')


if __name__ == '__main__':
    unittest.main()
