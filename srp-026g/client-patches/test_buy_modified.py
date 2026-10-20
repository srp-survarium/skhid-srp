"""Checks of the buy-modified builder without requiring the proprietary game assets."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import buy_modified_026g as patch

HERE = Path(__file__).resolve().parent


class BuyModifiedTests(unittest.TestCase):
    def test_unknown_movie_is_rejected_before_invoking_compiler(self):
        with mock.patch.object(patch.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'Not a known live lobby movie'):
                patch.build_movie(b'unknown movie', None, None, None, None)
            run.assert_not_called()

    def test_retail_movie_is_named_as_such(self):
        retail = hashlib.sha256(b'retail movie').hexdigest()
        with mock.patch.object(patch, 'RETAIL_SHA256', retail), \
                mock.patch.object(patch.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'retail lobby movie'):
                patch.build_movie(b'retail movie', None, None, None, None, known=False)
            run.assert_not_called()

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'lobby_menu.swf'
            output.write_bytes(b'existing movie')
            with mock.patch('sys.argv', ['buy_modified_026g.py', 'unused', str(output), '--ffdec=unused', '--step-only']):
                with self.assertRaises(SystemExit):
                    patch.main()
            self.assertEqual(output.read_bytes(), b'existing movie')


if __name__ == '__main__':
    unittest.main()
