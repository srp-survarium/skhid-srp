"""Checks of the squads-of-two lobby step without requiring the proprietary game assets."""
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock

import build_lobby_026g as lobby
import login_hint_026g as hint
import squad_two_026g as patch

HERE = Path(__file__).resolve().parent
HEADER = b'\x00' + struct.pack('<HH', 0x1800, 1)


def tag(kind, payload):
    return kind, struct.pack('<H', kind << 6 | len(payload)) + payload


def movie(*abcs, background=b'\x01\x02\x03'):
    tags = [tag(9, background)] + [tag(82, abc) for abc in abcs] + [tag(0, b'')]
    return hint.pack(10, HEADER, tags)


def editor_writing(edited):
    """subprocess.run as javac (nothing) and as the Java patcher (writes `edited`)."""
    def run(command, check):
        if 'PatchSquadTwo' in command and command[-1].endswith('edited.swf'):
            Path(command[-1]).write_bytes(edited)
    return run


class SquadTwoTests(unittest.TestCase):
    def build(self, original, edited):
        with tempfile.TemporaryDirectory() as temporary, \
                mock.patch.object(patch.subprocess, 'run', side_effect=editor_writing(edited)):
            return patch.build_movie(original, Path('ffdec.jar'), 'java', 'javac', Path(temporary), known=False)

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
            with mock.patch('sys.argv', ['squad_two_026g.py', 'unused', str(output), '--ffdec=unused', '--step-only']):
                with self.assertRaises(SystemExit):
                    patch.main()
            self.assertEqual(output.read_bytes(), b'existing movie')

    def test_only_the_edited_script_block_is_taken_and_the_graphics_kept(self):
        original = movie(b'squad list', b'other')
        # The editor's movie lost the graphics' bytes: only its ABC is used.
        result = self.build(original, movie(b'squad two', b'other', background=b'\x09\x09\x09'))
        self.assertEqual(hint.unpack(result)[2], hint.unpack(movie(b'squad two', b'other'))[2])

    def test_a_second_changed_script_block_is_refused(self):
        with self.assertRaisesRegex(ValueError, 'squad panel'):
            self.build(movie(b'squad list', b'other'), movie(b'squad two', b'changed'))

    def test_an_unchanged_movie_is_refused(self):
        with self.assertRaisesRegex(ValueError, 'squad panel'):
            self.build(movie(b'squad list'), movie(b'squad list'))

    def test_it_is_a_step_of_the_driver_built_on_the_movie_before_it(self):
        self.assertIn(patch, [module for _, module in lobby.STEPS])
        # Its known input is what the steps before it made when it was added.
        self.assertIn('88fa657a48250bd2729c875ff86f56c20a8acdc42c7ba06bd41fbf11161db1c9', patch.MOVIE_SHA256S)

    def test_the_patcher_edits_the_squad_panel_alone(self):
        java = (HERE / 'squad_two' / 'PatchSquadTwo.java').read_text(encoding='utf-8')
        for method in ('"set_squad_player"', '"updateSize"', '"addFriend"'):
            self.assertIn(method, java)
        self.assertIn('blockOf(swf, "survarium.friends.SquadList")', java)
        self.assertIn('changed != 3', java)


if __name__ == '__main__':
    unittest.main()
