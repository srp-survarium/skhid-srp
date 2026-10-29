"""Map-editor edits preserve neighboring entries and reject stale input.

Run with: python3 -m unittest discover -s srp-026g/scripts/content-edit
"""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib

import content_edit as ce


def config(value):
    """Encode a small independent binary-config fixture (tuples are vectors)."""
    data = bytearray(24)

    def put(raw):
        offset = len(data)
        data.extend(raw)
        return offset

    def record(name, value):
        name_offset = put(name.encode() + b'\0')
        crc = zlib.crc32(name.encode())
        count = 1
        if isinstance(value, dict):
            records = [record(k, v) for k, v in value.items()]
            records.sort(key=lambda row: struct.unpack_from('<I', row, 16)[0])
            offset, kind, count = put(b''.join(records)), 3, len(records)
        elif isinstance(value, list):
            records = [record('', v) for v in value]
            offset, kind, count = put(b''.join(records)), 4, len(records)
        elif isinstance(value, tuple):
            offset, kind = put(struct.pack('<%df' % len(value), *value)), 4 + len(value)
        elif isinstance(value, str):
            offset, kind, count = put(value.encode() + b'\0'), 5, len(value.encode()) + 1
        elif isinstance(value, int):
            offset, kind = value & 0xffffffff, 1
        else:
            offset, kind = struct.unpack('<I', struct.pack('<f', value))[0], 2
        return struct.pack('<QQIHH', offset, name_offset, crc, kind, count)

    data[:24] = record('', value)
    return data


def fixture():
    return config({'objects': [
        {'full_name': 'first', 'position': (1.0, 2.0, 3.0),
         'spawned_item': {'position': (4.0, 5.0, 6.0), 'count': 2},
         'geometries': [{'name': 'original'}], 'containers': ['a', 'b']},
        {'full_name': 'second', 'position': (7.0, 8.0, 9.0)},
    ]})


def value(data, path):
    return ce.value_of(data, ce.find(data, path))


class MapEditorEdits(unittest.TestCase):
    def test_vectors_and_nested_keys_keep_other_values(self):
        data = fixture()
        before = dict((p, (t, v)) for p, t, v in ce.walk(data))
        changes = {'position': [3.5, 2.0, -1.0],
                   'spawned_item/position': [6.0, 5.0, 4.0],
                   'spawned_item/count': 3, 'geometries[0]/name': 'changed'}
        ce.set_entry(data, 'objects', {'full_name': 'first'},
                     {'position': [1.0001, 2, 3], 'spawned_item/count': 2}, changes)
        after = dict((p, (t, v)) for p, t, v in ce.walk(data))
        self.assertEqual({p for p in before if before[p] != after[p]},
                         {'/objects[0]/' + k for k in changes})
        for key, expected in changes.items():
            self.assertTrue(ce.same(value(data, 'objects[0]/' + key), expected))

    def test_stale_vector_and_wrong_dimensions_are_refused(self):
        for frm, to in [({'position': [9, 2, 3]}, {'position': [0, 0, 0]}),
                        ({}, {'position': [0, 0]})]:
            data = fixture()
            before = bytes(data)
            with self.assertRaises(ValueError):
                ce.set_entry(data, 'objects', {'full_name': 'first'}, frm, to)
            self.assertEqual(data, before)

    def test_nested_copy_does_not_change_the_model(self):
        data = fixture()
        ce.add_entry(data, 'objects', {'full_name': 'first'}, {'full_name': 'first'},
                     {'full_name': 'copy', 'spawned_item/position': [0, 1, 2],
                      'geometries[0]/name': 'copy-geometry'})
        self.assertEqual(value(data, 'objects[0]/spawned_item/position'), [4, 5, 6])
        self.assertEqual(value(data, 'objects[0]/geometries[0]/name'), 'original')
        self.assertEqual(value(data, 'objects[1]/spawned_item/position'), [0, 1, 2])
        self.assertEqual(value(data, 'objects[1]/geometries[0]/name'), 'copy-geometry')
        self.assertEqual(value(data, 'objects[2]/full_name'), 'second')
        ce.walk(data)  # Names, CRC ordering and every reachable value still parse.

    def test_modifier_retarget_by_its_path(self):
        """A modification's modifiers differ only in their `path`: `where` takes a path in the entry."""
        def modifier(key):
            return {'path': ['recoil', key], 'value': -0.02, 'type': 2}
        data = config({'modifications': [{'id': 2080, 'modifiers': [modifier('a'), modifier('b'), modifier('c')]}]})
        ce.set_entry(data, 'modifications', {'id': 2080}, {'modifiers[0]/path[1]': 'a'},
                     {'modifiers[0]/path[1]': 'max_top_recoil'})
        with self.assertRaises(ValueError):  # every modifier has this value and type
            ce.remove_entry(bytearray(data), 'modifications[0]/modifiers', {'type': 2})
        ce.remove_entry(data, 'modifications[0]/modifiers', {'path[1]': 'b'})
        with self.assertRaises(ValueError):  # gone: a second removal is refused
            ce.remove_entry(data, 'modifications[0]/modifiers', {'path[1]': 'b'})
        self.assertEqual([value(data, 'modifications[0]/modifiers[%d]/path[1]' % i) for i in range(2)],
                         ['max_top_recoil', 'c'])
        self.assertTrue(ce.same(value(data, 'modifications[0]/modifiers[1]/value'), -0.02))
        ce.walk(data)

    def test_cli_checks_add_remove_and_entry_index_shifts(self):
        edits = {
            'add_entry': [{'files': 'project', 'list': 'objects',
                           'after': {'full_name': 'first'}, 'like': {'full_name': 'first'},
                           'values': {'full_name': 'copy', 'spawned_item/position': [0, 1, 2]}}],
            'add_item': [{'file': 'project', 'list': 'objects', 'where': {'full_name': 'copy'},
                          'key': 'containers', 'value': 'c'}],
            'remove_item': [{'file': 'project', 'path': 'objects[1]/containers', 'value': 'a'}],
            'remove_entry': [{'file': 'project', 'list': 'objects', 'where': {'full_name': 'first'}}],
        }
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'live').mkdir()
            (root / 'live/project').write_bytes(fixture())
            (root / 'edits.json').write_text(json.dumps(edits))
            result = subprocess.run([sys.executable, str(Path(ce.__file__).resolve()),
                                     str(root / 'edits.json'), str(root / 'live'), str(root / 'out')],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            data = bytearray((root / 'out/project').read_bytes())
            self.assertEqual(ce.items_of(data, 'objects[0]/containers'), ['b', 'c'])
            self.assertEqual(value(data, 'objects[1]/full_name'), 'second')


if __name__ == '__main__':
    unittest.main()
