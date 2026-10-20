"""Checks of the lobby movie's source build without the proprietary game movie or the compilers."""
import hashlib
import json
from pathlib import Path
import re
import tempfile
import unittest
from unittest import mock

import build_lobby_026g as lobby

HERE = Path(__file__).resolve().parent
TREE = HERE.parent / 'flash' / 'lobby_menu'


def silent(_):
    pass


class BuildLobbyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def movie(self, data):
        path = self.root / 'lobby_menu.swf'
        path.write_bytes(data)
        return path

    def test_a_movie_other_than_revision_24s_is_refused_before_any_compiler(self):
        with mock.patch.object(lobby.flash_tree, 'build') as build:
            with self.assertRaisesRegex(ValueError, "Not revision 24's lobby movie"):
                lobby.build(self.movie(b'some other movie'), None, self.root / 'project', report=silent)
            for data, name in [(b'revision 23', "revision 23's"), (b'retail', 'the retail')]:
                with mock.patch.dict(lobby.OLDER, {hashlib.sha256(data).hexdigest(): name}):
                    with self.assertRaisesRegex(ValueError, f'This is {name} lobby movie'):
                        lobby.build(self.movie(data), None, self.root / 'project', report=silent)
            build.assert_not_called()

    def run_build(self, built, **options):
        base = b'revision 24'
        calls = []

        def fake(tree, movie, tools, project, work, jobs, report):
            calls.append((tree, movie))
            return built, {'changed_methods': 1, 'added_methods': 2}

        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()), \
                mock.patch.object(lobby.flash_tree, 'build', side_effect=fake):
            result = lobby.build(self.movie(base), None, self.root / 'project', report=silent, **options)
        self.assertEqual(calls[0][0], lobby.TREE)
        return result

    def test_the_result_must_be_the_pinned_one(self):
        movie = b'the lobby from source'
        with mock.patch.object(lobby, 'RESULT_SHA256', hashlib.sha256(movie).hexdigest()):
            self.assertEqual(self.run_build(movie)[0], movie)
            with self.assertRaisesRegex(ValueError, 'not RESULT_SHA256'):
                self.run_build(b'another movie')
            self.assertEqual(self.run_build(b'another movie', any_result=True)[1]['added_methods'], 2)

    def test_existing_output_is_not_overwritten(self):
        output = self.root / 'lobby_menu.swf'
        output.write_bytes(b'existing movie')
        with mock.patch('sys.argv', ['build_lobby_026g.py', 'unused', str(output)]), \
                mock.patch.object(lobby, 'build') as build:
            with self.assertRaises(SystemExit):
                lobby.main()
            build.assert_not_called()
        self.assertEqual(output.read_bytes(), b'existing movie')


class LobbySourceTests(unittest.TestCase):
    """The lobby's source tree and srp's own classes in it."""

    def test_the_tree_is_revision_24s(self):
        spec = json.loads((TREE / 'movie.json').read_text())
        self.assertEqual(spec['movie_sha256'], lobby.BASE_SHA256)

    def test_every_srp_class_is_in_the_tree_and_every_new_file_is_an_srp_class(self):
        # A new file not named in srp.json would be refused by the build;
        # one named but missing would never reach the players.
        srp = [entry['class'] for entry in json.loads((TREE / 'srp.json').read_text())['classes']]
        _, added, removed = lobby.flash_tree.differences(TREE)
        self.assertEqual(removed, [])
        self.assertEqual(sorted(p[:-3].replace('/', '.') for p in added),
                         sorted(c for c in srp if (TREE / 'src' / (c.replace('.', '/') + '.as')).exists()
                                and c.replace('.', '/') + '.as' not in json.loads((TREE / 'movie.json').read_text())['sources']))
        for cls in srp:
            self.assertTrue((TREE / 'src' / (cls.replace('.', '/') + '.as')).is_file(), cls)

    def test_no_srp_class_defines_a_method_twice(self):
        # FFDec compiles them and keeps the last of two methods of one name.
        import build_feature
        for entry in json.loads((TREE / 'srp.json').read_text())['classes']:
            path = TREE / 'src' / (entry['class'].replace('.', '/') + '.as')
            self.assertEqual(build_feature.duplicate_functions(path.read_text(encoding='utf-8')), [], path.name)

    def test_the_exchange_and_the_server_agree_on_the_buy_gold_mark(self):
        window = (TREE / 'src' / 'SrpExchange.as').read_text(encoding='utf-8')
        server = (HERE.parent / 'crates' / 'survarium' / 'src' / 'v026g' / 'lobby' / 'requests.rs').read_text(encoding='utf-8')
        sent = re.search(r'BUY_GOLD:uint = (0x[0-9A-Fa-f]+);', window)
        read = re.search(r'pub const BUY_PREMIUM: u32 = (0x[0-9A-Fa-f_]+);', server)
        self.assertIsNotNone(sent)
        self.assertIsNotNone(read)
        self.assertEqual(int(sent.group(1), 16), int(read.group(1).replace('_', ''), 16))

    def test_every_exchange_text_is_in_every_language(self):
        window = (TREE / 'src' / 'SrpExchange.as').read_text(encoding='utf-8')
        texts = re.search(r'TEXT:Object = \{(.*?)\};', window, re.S).group(1)
        rows = re.findall(r'"(\w+)": \[(.*?)\]', texts, re.S)
        self.assertGreater(len(rows), 5)
        for key, row in rows:
            self.assertEqual(len(re.findall(r'"[^"]+"', row)), 5, key)

    def test_get_gold_opens_the_exchange_over_the_dialog_on_the_buy_side(self):
        source = (TREE / 'src' / 'survarium' / 'messages' / 'MessageMoneyExchange.as').read_text(encoding='utf-8')
        body = re.search(r'public static function buyGold\(\) : void\s*\{(.*?)\n      \}', source, re.S).group(1)
        self.assertRegex(body, r'Message\.showMessage\("exchange",\{\s*label:"",\s*srpBuyGold:true\s*\},true,0,true\);')


if __name__ == '__main__':
    unittest.main()
