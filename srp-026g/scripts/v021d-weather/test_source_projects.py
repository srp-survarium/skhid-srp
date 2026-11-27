import tempfile
import unittest
from pathlib import Path

import source_projects as source


class DaySourceTest(unittest.TestCase):
    def test_only_map_append_guards_follow_excluded_weather_rows(self):
        original = '{"append_entry": [{"list": "maps_dict", "expected_count": 157}, {"list": "items", "expected_count": 18}]}\n'
        shifted = source.shift_map_counts(original, -20)
        self.assertEqual(shifted, original.replace('157', '137'))
        self.assertEqual(source.shift_map_counts(shifted, 20), original)

    def test_entries_of_the_removed_files_and_their_copies_are_left_out(self):
        removed = {'projects/w/rain_2/client_project', 'projects/w/rain_4/client_project'}
        edits = {
            '_about': 'kept',
            'copy': [{'files': 'projects/w/rain_4/client_project', 'from': 'projects/w/rain_4/', 'to': 'projects/w/rain_ah/'},
                     {'files': 'projects/w/day_4/client_project', 'from': 'projects/w/day_4/', 'to': 'projects/w/day_ah/'}],
            'add_entry': [{'files': 'projects/w/rain_ah/client_project', 'list': 'render_visuals'},
                          {'files': 'projects/w/*/client_project', 'expected_files': 5},
                          {'files': 'projects/w/day_ah/client_project', 'list': 'render_visuals'}],
            'set': [{'file': 'projects/w/rain_2/client_project', 'path': 'a', 'from': 1, 'to': 2}],
            'append_entry': [{'files': ['projects/w/day_2/client_project', 'projects/w/rain_2/client_project'],
                              'expected_counts': [3, 4]}],
        }
        kept = source.without_files(edits, removed)
        self.assertEqual(kept['_about'], 'kept')
        self.assertEqual(kept['copy'], edits['copy'][1:])
        # rain_2, rain_4 and the copy rain_ah were three of the glob's five.
        self.assertEqual(kept['add_entry'], [{'files': 'projects/w/*/client_project', 'expected_files': 2},
                                             edits['add_entry'][2]])
        self.assertEqual(kept['set'], [])
        self.assertEqual(kept['append_entry'], [{'files': ['projects/w/day_2/client_project'], 'expected_counts': [3]}])
        self.assertIsNone(source.without_files({'set': [{'file': 'projects/w/day_2/client_project'}]}, removed))

    def test_view_removes_only_new_weather_rows_and_keeps_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'root'
            edits = root / 'srp-026g/scripts/content-edit'
            edits.mkdir(parents=True)
            (root / 'srp-026g/client-patches').mkdir()
            modes = ('modes\tmap_id\n'
                     'v021d_level_01_day_2\t29\n'
                     'v021d_level_01_rain_2\t29\n'
                     'v021d_level_05_evn_4\t26\n'
                     'level_05_evn_4\t5\n'
                     'v021d_level_02_day_8\t25\n')
            (edits / 'modes.tsv').write_text(modes)
            (edits / 'keep.json').write_text('{}')
            removed = {'gameplay/game_modes/v021d_level_01_rain_2.options',
                       'gameplay/game_modes/v021d_level_05_evn_4.options'}
            view = source.day_edit_tree(Path(tmp) / 'work', [('edit', 'keep.json')], removed, root)
            actual = view / 'srp-026g/scripts/content-edit'
            self.assertEqual((edits / 'modes.tsv').read_text(), modes)
            self.assertEqual((actual / 'modes.tsv').read_text(),
                             'modes\tmap_id\nv021d_level_01_day_2\t29\nlevel_05_evn_4\t5\nv021d_level_02_day_8\t25\n')
            self.assertEqual((actual / 'keep.json').resolve(), edits / 'keep.json')
            self.assertEqual((view / 'srp-026g/client-patches').resolve(), root / 'srp-026g/client-patches')


if __name__ == '__main__':
    unittest.main()
