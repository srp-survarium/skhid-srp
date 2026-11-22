"""tools/testpatch.py's choices, without a game or srp-game-data."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import testpatch  # noqa: E402


class TestPatchTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def pack(self, version, ops):
        folder = self.root / f'p-{version}'
        folder.mkdir()
        (folder / 'pack.json').write_text(json.dumps({'id': 'p', 'version': str(version), 'operations': ops}))
        return folder

    def test_only_what_differs_is_packed_and_what_edit_lists_lay_on_is_left_out(self):
        lines = ['same a/x resources.db', 'differs a/y resources.db.patch_5', 'new b/z',
                 'differs gameplay/g resources.db server', 'new projects/p/client_project server']
        packed, left = testpatch.chosen(lines, {'projects/p/client_project': ['maps.tsv']})
        self.assertEqual(packed, [('a/y', False), ('b/z', False), ('gameplay/g', True)])
        self.assertEqual(left, [('projects/p/client_project', ['maps.tsv'])])

    def test_a_pack_whose_other_operations_changed_is_refused(self):
        files = [{'op': 'add_file', 'path': 'a/x'}]
        table = {'op': 'dictionary_add', 'table': 'maps_dict', 'entries': [{'id': 1}]}
        self.pack(1, files + [table])
        self.pack(3, [])                                   # newer: not what version 2 is held against
        two = self.pack(2, [{'op': 'replace_file', 'path': 'a/y', 'base_sha256': '0'}, table])
        self.assertEqual(testpatch.earlier_version(two, {'id': 'p', 'version': '2'}), self.root / 'p-1')
        self.assertIsNone(testpatch.other_ops_changed(two))
        self.assertEqual(sorted(testpatch.pack_files(two)[1]), ['a/y'])
        four = self.pack(4, files + [dict(table, entries=[{'id': 2}])])
        self.assertIn("dictionary_add operations differ from p-3's", testpatch.other_ops_changed(four))
        first = self.root / 'q-1'
        first.mkdir()
        (first / 'pack.json').write_text(json.dumps({'id': 'q', 'version': '1', 'operations': [table]}))
        self.assertIn('no earlier version', testpatch.other_ops_changed(first))


if __name__ == '__main__':
    unittest.main()
