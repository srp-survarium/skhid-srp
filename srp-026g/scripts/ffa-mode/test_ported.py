"""The ported-map source edits preserve TDM and construct valid solo spawn records."""
import csv
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_ported_edits as m

sys.path.insert(0, str(m.HERE.parent / 'content-edit'))
from test_content_edit import config


class PortedMaps(unittest.TestCase):
    def test_checked_in_recipe_matches_the_reviewed_point_source(self):
        authored = m.points()
        recipe = json.loads(m.OUTPUT.read_text())
        copied = {row['to'] for row in recipe['copy']}
        self.assertEqual(copied, {f'gameplay/game_modes/ffa/{name}_day_8.options' for name, _ in m.MAPS})
        self.assertEqual(len(recipe['add_entry']), 110)
        by_name = {(row['files'], row['values']['name']): row['values'] for row in recipe['add_entry']}
        for project, points in authored.items():
            for point in points:
                emitted = by_name[(m.base.copy_of('projects/' + project + '/client_project'), 'ffa_' + point['point'])]
                self.assertEqual(emitted['position'], [float(point[k]) for k in ('x', 'y', 'z')])
                self.assertEqual(emitted['rotation'], [float(point[k]) for k in ('rx', 'ry', 'rz')])
                self.assertEqual((emitted['team'], emitted['invulnerability_time']), (2, 2))

    def fixture(self, root):
        live = root / 'live'
        paths = m.paths()
        rows = []
        for i, (name, _) in enumerate(m.MAPS):
            mode = {'mode': 4, 'name': name + '_4', 'rules': [93, 91, 90, 94],
                    'ui': {'icon': 1, 'mode_name': 'st_mode_04_name', 'description': 'st_mode_04_description'},
                    'match_making_params': {'map_name': name + '/day_4', 'map_id': 10+i, 'map_weather_id': 1},
                    'parameters': {'match_time': 900}}
            spawn = {'point_id': 0, 'team': 0, 'name': 'original', 'full_name': 'original',
                     'priority': 1, 'is_temporary': 0, 'position': (1., 2., 3.), 'rotation': (0., 1., 0.),
                     'invulnerability_time': 6,
                     'ally_zones': {'collision_geometries': [{'name': 'original-ally'}]},
                     'enemy_zones': {'collision_geometries': [{'name': 'original-enemy'}]}}
            if not name.startswith('v011e'):
                spawn.update(enemy_check_radius=1.2, ally_check_radius=0.8)
            for path, value in [(paths[1+i*2], {'mode': mode}),
                                (paths[2+i*2], {'server_objects': {'respawn_points': [spawn]}})]:
                p = live / path
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_bytes(config(value))
            rows.append({'id': 180+i, 'cfg_mode_name': name + '_day_4.options'})
        p = live / m.base.DICTIONARY
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(config({'maps_dict': rows}))
        points = root / 'points.tsv'
        with points.open('w', newline='') as stream:
            fields = ['project', 'point', 'x', 'y', 'z', 'rx', 'ry', 'rz', 'enemy_check_radius',
                      'invulnerability_time', 'verdict']
            out = csv.DictWriter(stream, fields, delimiter='\t')
            out.writeheader()
            for name, _ in m.MAPS:
                for i in range(22):
                    out.writerow(dict(zip(fields, [name+'/day_4', f'p{i}', (i%6)*20, 0, (i//6)*20,
                                                    0, 0, 0, 30, 2, 'kept'])))
        return live, points

    def test_complete_edit_list_preserves_originals_and_has_safe_solo_records(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            live, points = self.fixture(root)
            with patch.object(m, 'POINTS', points):
                edits = m.edits(live)
            recipe = root / 'edits.json'
            recipe.write_text(json.dumps(edits))
            out = root / 'out'
            subprocess.run([sys.executable, str(m.HERE.parent/'content-edit/content_edit.py'),
                            str(recipe), str(live), str(out)], check=True, stdout=subprocess.DEVNULL)
            dictionary = m.base.tree((out/m.base.DICTIONARY).read_bytes())['maps_dict']
            self.assertEqual([r['id'] for r in dictionary[-5:]], [i for _, i in m.MAPS])
            for name, _ in m.MAPS:
                mode = m.base.tree((out/f'gameplay/game_modes/ffa/{name}_day_8.options').read_bytes())['mode']
                self.assertEqual(mode['mode'], 8)
                self.assertEqual(mode['rules'], [93, 91, 94])
                self.assertEqual(mode['parameters']['kills_to_win'], 30)
                self.assertEqual(mode['match_making_params']['max_players'], 10)
                path = f'projects/{name}/day_8/client_project'
                old = m.base.tree((live/path).read_bytes())['server_objects']['respawn_points']
                blob = (out/path).read_bytes()
                spawns = m.base.tree(blob)['server_objects']['respawn_points']
                self.assertEqual(spawns[:1], old)
                self.assertEqual(len(spawns), 23)
                for point in spawns[1:]:
                    self.assertEqual((point['team'], point['invulnerability_time'], point['enemy_check_radius'],
                                      point['ally_check_radius']), (2, 2, 30., 1.))
                    self.assertFalse(point['ally_zones']['collision_geometries'])
                    self.assertFalse(point['enemy_zones']['collision_geometries'])
                offsets = m.base.c.value_offsets(blob)
                self.assertEqual(len(offsets), len(set(offsets)), 'native relocation needs unique records')

    def test_unchecked_points_and_occupied_map_ids_are_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            live, points = self.fixture(Path(temp))
            with patch.object(m, 'POINTS', points):
                raw = points.read_text()
                points.write_text(raw.replace('kept', 'dropped', 1))
                with self.assertRaises(ValueError):
                    m.edits(live)
                points.write_text(raw)
                dictionary = live / m.base.DICTIONARY
                value = m.base.tree(dictionary.read_bytes())
                value['maps_dict'][0]['id'] = m.MAPS[0][1]
                dictionary.write_bytes(config(value))
                with self.assertRaises(ValueError):
                    m.edits(live)


if __name__ == '__main__':
    unittest.main()
