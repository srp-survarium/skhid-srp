"""Checks of the lobby movie driver without requiring the proprietary game assets."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import build_lobby_026g as lobby


class BuildLobbyTests(unittest.TestCase):
    def test_a_movie_other_than_revision_23s_is_refused_before_any_step(self):
        with mock.patch.object(lobby.STEPS[0][1], 'build_movie') as first:
            with self.assertRaisesRegex(ValueError, "Not revision 23's lobby movie"):
                lobby.build(b'some other movie', None, report=lambda _: None)
            first.assert_not_called()

    def test_the_retail_movie_is_named_as_such(self):
        retail = hashlib.sha256(b'retail').hexdigest()
        with mock.patch.object(lobby, 'RETAIL_SHA256', retail):
            with self.assertRaisesRegex(ValueError, 'retail lobby movie'):
                lobby.build(b'retail', None, report=lambda _: None)

    def test_every_step_runs_once_in_order_on_the_one_before(self):
        base = b'revision 23'
        calls = []

        def fake(name):
            def build_movie(original, ffdec, java, javac, work, known=True):
                calls.append((name, original, known))
                return original + b'+' + name.encode()
            return build_movie

        patches = [mock.patch.object(module, 'build_movie', fake(module.__name__)) for _, module in lobby.STEPS]
        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()):
            for p in patches:
                p.start()
            try:
                movie, hashes = lobby.build(base, None, report=lambda _: None, any_result=True)
            finally:
                for p in patches:
                    p.stop()
        names = [module.__name__ for _, module in lobby.STEPS]
        self.assertEqual([name for name, _, _ in calls], names)
        self.assertTrue(all(known is False for _, _, known in calls))
        expected = base
        for (name, original, _), step_name in zip(calls, names):
            self.assertEqual(original, expected)
            expected = expected + b'+' + step_name.encode()
        self.assertEqual(movie, expected)
        self.assertEqual(len(hashes), len(lobby.STEPS))

    def test_every_lobby_movie_builder_is_a_step(self):
        # A lobby movie patcher left out of STEPS would never reach the players.
        here = Path(lobby.__file__).resolve().parent
        builders = {path.stem for path in here.glob('*_026g.py')
                    if 'def build_movie' in path.read_text(encoding='utf-8')
                    and 'lobby' in path.read_text(encoding='utf-8')}
        # The exchange's first build and the login hint made revision 23's base.
        before_base = {'exchange_026g', 'login_hint_026g'}
        steps = {module.__name__ for _, module in lobby.STEPS}
        self.assertEqual(builders - before_base - steps, set())

    def test_a_result_other_than_the_checked_one_is_refused(self):
        base = b'revision 23'
        patches = [mock.patch.object(module, 'build_movie', lambda m, *a, **k: m + b'+')
                   for _, module in lobby.STEPS]
        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()):
            for p in patches:
                p.start()
            try:
                with self.assertRaisesRegex(ValueError, 'not RESULT_SHA256'):
                    lobby.build(base, None, report=lambda _: None)
            finally:
                for p in patches:
                    p.stop()

    def test_every_step_alone_refuses_to_write_a_movie(self):
        # A movie of one step lacks the others: only --step-only writes one.
        for _, module in lobby.STEPS:
            with self.subTest(module.__name__), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / 'lobby_menu.swf'
                with mock.patch('sys.argv', [module.__name__, 'unused', str(output), '--ffdec=unused']), \
                        mock.patch.object(module, 'build_movie') as build, \
                        self.assertRaises(SystemExit):
                    module.main()
                build.assert_not_called()
                self.assertFalse(output.exists())

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'lobby_menu.swf'
            output.write_bytes(b'existing movie')
            with mock.patch('sys.argv', ['build_lobby_026g.py', 'unused', str(output), '--ffdec=unused']):
                with self.assertRaises(SystemExit):
                    lobby.main()
            self.assertEqual(output.read_bytes(), b'existing movie')


if __name__ == '__main__':
    unittest.main()
