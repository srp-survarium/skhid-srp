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
                lobby.build(self.movie(b'some other movie'), None, report=silent)
            for data, name in [(b'revision 23', "revision 23's"), (b'retail', 'the retail')]:
                with mock.patch.dict(lobby.OLDER, {hashlib.sha256(data).hexdigest(): name}):
                    with self.assertRaisesRegex(ValueError, f'This is {name} lobby movie'):
                        lobby.build(self.movie(data), None, report=silent)
            build.assert_not_called()

    def test_the_tree_is_built_over_revision_24s_movie(self):
        base = b'revision 24'
        calls = []

        def fake(tree, movie, tools, report):
            calls.append((tree, movie.read_bytes()))
            return b'the lobby from source'

        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()), \
                mock.patch.object(lobby.flash_tree, 'build', side_effect=fake):
            self.assertEqual(lobby.build(self.movie(base), None, report=silent), b'the lobby from source')
        self.assertEqual(calls, [(lobby.TREE, base)])

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
    """The lobby's source tree."""

    def test_the_tree_is_of_revision_24s_movie(self):
        spec = json.loads((TREE / 'movie.json').read_text())
        self.assertEqual(spec['movie_sha256'], lobby.BASE_SHA256)

    def test_the_exchange_and_the_server_agree_on_the_buy_gold_mark(self):
        window = (TREE / 'src' / 'survarium' / 'messages' / 'MessageMoneyExchange.as').read_text(encoding='utf-8')
        server = (HERE.parent / 'crates' / 'survarium' / 'src' / 'v026g' / 'lobby' / 'requests.rs').read_text(encoding='utf-8')
        sent = re.search(r'BUY_GOLD:uint = (0x[0-9A-Fa-f]+);', window)
        read = re.search(r'pub const BUY_PREMIUM: u32 = (0x[0-9A-Fa-f_]+);', server)
        self.assertIsNotNone(sent)
        self.assertIsNotNone(read)
        self.assertEqual(int(sent.group(1), 16), int(read.group(1).replace('_', ''), 16))

    def test_every_exchange_text_is_in_every_language(self):
        window = (TREE / 'src' / 'survarium' / 'messages' / 'MessageMoneyExchange.as').read_text(encoding='utf-8')
        texts = re.search(r'TEXT:Object = \{(.*?)\};', window, re.S).group(1)
        rows = re.findall(r'"(\w+)": ?\[(.*?)\]', texts, re.S)
        self.assertGreater(len(rows), 5)
        for key, row in rows:
            self.assertEqual(len(re.findall(r'"[^"]+"', row)), 5, key)

    def test_get_gold_opens_the_exchange_over_the_dialog_on_the_buy_side(self):
        source = (TREE / 'src' / 'survarium' / 'messages' / 'MessageMoneyExchange.as').read_text(encoding='utf-8')
        body = re.search(r'public static function buyGold\(\) : void\s*\{(.*?)\n      \}', source, re.S).group(1)
        self.assertRegex(body, r'Message\.showMessage\("exchange",\{\s*label:"",\s*srpBuyGold:true\s*\},true,0,true\);')


if __name__ == '__main__':
    unittest.main()
