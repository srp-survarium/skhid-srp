"""A map's spawns at the owner's picks: only position and rotation change, new points copy a solo
spawn, the left-over last go."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import make_picks_edits as m

sys.path.insert(0, str(m.base.HERE.parent / 'content-edit'))
from test_content_edit import config


def rows(moved=(), added=(), roof=(), removed=()):
    return {v: dict(points) for v, points in zip(m.VERDICTS, (moved, added, roof, removed))}


TEST = m.Map(Path('test-picks.tsv'), Path('ffa-test-picks-1.json'), 'ffa_test_', 'a test')


class Picks(unittest.TestCase):
    def test_checked_in_recipes_are_their_picks_on_free_for_all_projects_only(self):
        for name, map in m.MAPS.items():
            with self.subTest(map=name):
                self.recipe_is_its_picks(map)

    def recipe_is_its_picks(self, map):
        recipe = json.loads(map.output.read_text())
        picks = m.picks(map)
        count = lambda *verdicts: sum(len(by[v]) for by in picks.values() for v in verdicts)
        self.assertEqual(len(recipe.get('set_entry', [])), count('moved'))
        self.assertEqual(len(recipe.get('add_entry', [])), count('added', 'roof'))
        self.assertEqual(len(recipe.get('remove_entry', [])), count('removed'))
        for row in recipe.get('set_entry', []):
            self.assertRegex(row['file'], r'^projects/[a-z0-9_]+/[a-z]+_8/client_project$')
            self.assertEqual(row['where']['team'], 2)
            position, yaw = picks[row['file']]['moved'][row['where']['point_id']]
            self.assertEqual(row['to'], {'position': position, 'rotation': [0., yaw, 0.]})
        for row in recipe.get('add_entry', []):
            self.assertRegex(row['files'], r'^projects/[a-z0-9_]+/[a-z]+_8/client_project$')
            by = picks[row['files']]
            position, yaw = {**by['added'], **by['roof']}[row['values']['point_id']]
            self.assertEqual((row['values']['position'], row['values']['rotation']), (position, [0., yaw, 0.]))
            self.assertEqual((row['values']['team'], row['values']['invulnerability_time_ms']), (2, 500))
            self.assertEqual(row['values']['name'], f"{map.prefix}{row['values']['point_id']}")

    def apply(self, root, path, recipe):
        out = Path(tempfile.mkdtemp(dir=root)) / 'out'
        recipe_path = out.parent / 'edits.json'
        recipe_path.write_text(json.dumps(recipe))
        done = subprocess.run([sys.executable, str(m.base.EDIT), str(recipe_path), str(root/'live'), str(out)],
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        return m.base.tree((out/path).read_bytes())

    def test_picks_move_add_and_remove_only_solo_spawns(self):
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
            # Two picks for three solo spawns: the last goes.
            fewer = rows(moved={1: ([100., 7., 50.], 0.5), 2: ([130., 7., 50.], -1.)}, removed={3: None})
            for wrong in (rows(moved={1: ([100., 7., 50.], 0.5), 3: ([130., 7., 50.], 0.)}, removed={2: None}),
                          rows(moved={1: ([100., 7., 50.], 0.5)}, removed={3: None}),
                          rows(moved={1: ([100., 7., 50.], 0.5), 2: ([130., 7., 50.], 0.), 3: ([160., 7., 50.], 0.)},
                               added={5: ([200., 7., 50.], 0.)})):
                with self.subTest(wrong=wrong), self.assertRaises(ValueError), \
                        patch.object(m, 'picks', return_value={path: wrong}):
                    m.edits(TEST, root / 'live')
            with patch.object(m, 'picks', return_value={path: fewer}):
                actual = self.apply(root, path, m.edits(TEST, root / 'live'))
            expected = json.loads(json.dumps(old))
            points = expected['server_objects']['respawn_points']
            points[1]['position'], points[1]['rotation'] = [100., 7., 50.], [0., 0.5, 0.]
            points[2]['position'], points[2]['rotation'] = [130., 7., 50.], [0., -1., 0.]
            del points[3]
            self.assertEqual(actual, expected)
            # Five: two new points, one a roof spawn 3 m over a pick (no spacing between points).
            more = rows(moved={1: ([100., 7., 50.], 0.5), 2: ([130., 7., 50.], -1.), 3: ([160., 7., 50.], 0.)},
                        added={4: ([200., 7., 50.], 1.)}, roof={5: ([101., 10., 50.], 2.)})
            with patch.object(m, 'picks', return_value={path: more}):
                actual = self.apply(root, path, m.edits(TEST, root / 'live'))
            got = actual['server_objects']['respawn_points']
            self.assertEqual([p['point_id'] for p in got], [0, 1, 2, 3, 4, 5])
            for i, (position, yaw) in [(4, ([200., 7., 50.], 1.)), (5, ([101., 10., 50.], 2.))]:
                self.assertEqual(got[i]['position'], position)
                self.assertEqual(got[i]['rotation'], [0., yaw, 0.])
                self.assertEqual((got[i]['name'], got[i]['team'], got[i]['invulnerability_time_ms'],
                                  got[i]['enemy_check_radius']), (f'ffa_test_{i}', 2, 500, 20.))
            # Point ids anew in each team (the STALKER maps): team 0's stay, team 2's last one goes.
            path = 'projects/stalker/day_8/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            source.write_bytes(config({'server_objects': {'respawn_points': [
                {**e, 'point_id': i, 'team': team} for e, (i, team) in zip(entries + entries[:1],
                                                                          [(0, 0), (1, 0), (0, 2), (1, 2), (2, 2)])]}}))
            old = m.base.tree(source.read_bytes())
            picked = rows(moved={0: ([100., 7., 50.], 0.5), 1: ([130., 7., 50.], -1.)}, removed={2: None})
            with patch.object(m, 'picks', return_value={path: picked}):
                actual = self.apply(root, path, m.edits(TEST, root / 'live'))
            expected = json.loads(json.dumps(old))
            points = expected['server_objects']['respawn_points']
            points[2]['position'], points[2]['rotation'] = [100., 7., 50.], [0., 0.5, 0.]
            points[3]['position'], points[3]['rotation'] = [130., 7., 50.], [0., -1., 0.]
            del points[4]
            self.assertEqual(actual, expected)
            # New points there (Fire Station's three more): the team names the entry after and the model.
            grown = rows(moved={0: ([100., 7., 50.], 0.5), 1: ([130., 7., 50.], -1.), 2: ([160., 7., 50.], 0.)},
                         added={3: ([200., 7., 50.], 1.)})
            with patch.object(m, 'picks', return_value={path: grown}):
                actual = self.apply(root, path, m.edits(TEST, root / 'live'))
            got = actual['server_objects']['respawn_points']
            self.assertEqual([(p['point_id'], p['team']) for p in got], [(0, 0), (1, 0), (0, 2), (1, 2), (2, 2), (3, 2)])
            self.assertEqual((got[5]['name'], got[5]['position'], got[5]['rotation']),
                             ('ffa_test_3', [200., 7., 50.], [0., 1., 0.]))

    def test_a_list_of_new_points_leaves_the_others_as_they_are(self):
        adds = TEST._replace(adds=True)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = 'projects/test/day_8/client_project'
            source = root / 'live' / path
            source.parent.mkdir(parents=True)
            # London (v0.32)'s: the ally radius an int.
            entries = [{'point_id': i, 'name': f'p{i}', 'full_name': f'logic/p{i}', 'team': team,
                'position': (float(40 * i), 6., 5.), 'rotation': (0., 1., 0.),
                'enemy_check_radius': 20., 'ally_check_radius': 1, 'invulnerability_time': 0,
                'invulnerability_time_ms': 500, 'ally_zones': {'collision_geometries': {}},
                'enemy_zones': {'collision_geometries': {}}}
                for i, team in enumerate([0, 2, 2])]
            source.write_bytes(config({'server_objects': {'respawn_points': entries}}))
            old = m.base.tree(source.read_bytes())
            # No solo spawn removed, none moved but a solo spawn, the new ones after the last.
            for wrong in (rows(moved={0: ([100., 7., 50.], 0.5)}, added={3: ([200., 7., 50.], 1.)}),
                          rows(removed={2: None}), rows(added={4: ([200., 7., 50.], 1.)})):
                with self.subTest(wrong=wrong), self.assertRaises(ValueError), \
                        patch.object(m, 'picks', return_value={path: wrong}):
                    m.edits(adds, root / 'live')
            new = rows(added={3: ([200., 7., 50.], 1.), 4: ([230., 7., 50.], -1.)})
            with patch.object(m, 'picks', return_value={path: new}):
                recipe = m.edits(adds, root / 'live')
            self.assertEqual(sorted(recipe), ['_about', 'add_entry'])
            got = self.apply(root, path, recipe)['server_objects']['respawn_points']
            self.assertEqual(got[:3], old['server_objects']['respawn_points'])
            self.assertEqual([(p['point_id'], p['team'], p['ally_check_radius'], p['enemy_check_radius'])
                              for p in got[3:]], [(3, 2, 1, 20.), (4, 2, 1, 20.)])
            self.assertEqual([p['position'] for p in got[3:]], [[200., 7., 50.], [230., 7., 50.]])
            # Past the respawn's 128 (Station's, picks --cap): a pick in a solo spawn's place, the
            # other solo spawn as it is.
            capped = rows(moved={2: ([130., 7., 50.], -1.)}, added={3: ([200., 7., 50.], 1.)})
            with patch.object(m, 'picks', return_value={path: capped}):
                recipe = m.edits(adds, root / 'live')
            got = self.apply(root, path, recipe)['server_objects']['respawn_points']
            self.assertEqual(got[:2], old['server_objects']['respawn_points'][:2])
            self.assertEqual((got[2]['name'], got[2]['position'], got[2]['rotation']), ('p2', [130., 7., 50.], [0., -1., 0.]))
            self.assertEqual([(p['point_id'], p['position']) for p in got[3:]], [(3, [200., 7., 50.])])


if __name__ == '__main__':
    unittest.main()
