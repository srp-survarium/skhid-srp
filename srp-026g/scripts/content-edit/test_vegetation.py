"""Runtime-only vegetation keeps plant bytes and fails closed on other formats."""
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

import content_edit as ce


def chunk(kind, data):
    return struct.pack('<II', kind, len(data)) + data


POSITION = chunk(0x58, struct.pack('<6f', -0.0, 2, -3, 4, 5, 6))
NORMAL = chunk(0x59, struct.pack('<6f', 0, 1, 0, 0, 0, 1))


class Vegetation(unittest.TestCase):
    def test_both_editor_formats_and_runtime_order(self):
        for editor in (0x4d, 0x4e):
            source = NORMAL + chunk(editor, b'\0' * 65536) + POSITION
            result = ce.runtime_vegetation(source)
            self.assertEqual(result, NORMAL + POSITION)
            self.assertEqual(ce.runtime_vegetation(result), result)

    def test_empty_layers_are_valid(self):
        runtime = chunk(0x58, b'') + chunk(0x59, b'')
        self.assertEqual(ce.runtime_vegetation(chunk(0x4d, b'paint') + runtime), runtime)

    def test_malformed_or_unknown_input_is_refused(self):
        valid = POSITION + NORMAL
        invalid = [b'', POSITION, valid[:-1], valid + b'\0', valid + POSITION,
                   chunk(0x58, b'1234') + chunk(0x59, b'1234'),
                   POSITION + chunk(0x59, b''),
                   valid + chunk(0x60, b'unknown'),
                   struct.pack('<II', 0x4d, 0xffffffff) + valid]
        for value in (float('nan'), float('inf'), -float('inf')):
            invalid.append(chunk(0x58, struct.pack('<3f', value, 0, 0)) +
                           chunk(0x59, struct.pack('<3f', 0, 1, 0)))
        for source in invalid:
            with self.subTest(source=source[:16]):
                with self.assertRaises(ValueError):
                    ce.runtime_vegetation(source)

    def test_cli_glob_check_and_fail_before_writing(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = Path('projects/v010b_test/vegetation/grass.veg')
            live = root / 'live' / path
            live.parent.mkdir(parents=True)
            source = chunk(0x4d, b'editor') + POSITION + NORMAL
            live.write_bytes(source)
            edits = root / 'edits.json'
            edits.write_text(json.dumps({'runtime_vegetation': [
                {'files': 'projects/*/vegetation/*.veg'}]}))
            command = [sys.executable, ce.__file__, str(edits), str(root / 'live'), str(root / 'out')]
            result = subprocess.run(command + ['--check'], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse((root / 'out').exists())
            bad = live.with_name('invalid.veg')
            bad.write_bytes(source + chunk(0xff, b'?'))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / 'out').exists())
            bad.unlink()
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((root / 'out' / path).read_bytes(), POSITION + NORMAL)
            self.assertEqual(live.read_bytes(), source)


if __name__ == '__main__':
    unittest.main()
