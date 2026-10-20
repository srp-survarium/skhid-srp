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
LOBBY = HERE.parent / 'lobby'


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
        with mock.patch.object(lobby.build_feature, 'build') as build, \
                mock.patch.object(lobby.movie_source, 'init_project') as init:
            with self.assertRaisesRegex(ValueError, "Not revision 24's lobby movie"):
                lobby.build(self.movie(b'some other movie'), None, self.root / 'project', report=silent)
            for data, name in [(b'revision 23', "revision 23's"), (b'retail', 'the retail')]:
                with mock.patch.dict(lobby.OLDER, {hashlib.sha256(data).hexdigest(): name}):
                    with self.assertRaisesRegex(ValueError, f'This is {name} lobby movie'):
                        lobby.build(self.movie(data), None, self.root / 'project', report=silent)
            build.assert_not_called()
            init.assert_not_called()

    def fake_build(self, sha):
        def build(tools, reconstruction, project, build_list, output, jobs, log):
            self.assertEqual((reconstruction, build_list), (lobby.RECONSTRUCTION, lobby.LOBBY))
            output.mkdir(parents=True)
            (output / 'feature.swf').write_bytes(b'the lobby from source')
            return {'feature_movie_sha256': sha, 'changed_methods': 1, 'added_methods': 2}
        return build

    def run_build(self, sha, **options):
        base = b'revision 24'
        project = self.root / 'project'
        project.mkdir(exist_ok=True)
        manifest = {'movie_sha256': hashlib.sha256(base).hexdigest(), 'release': lobby.RELEASE}
        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()), \
                mock.patch.object(lobby.movie_source, 'verify_project', return_value=manifest), \
                mock.patch.object(lobby.build_feature, 'build', self.fake_build(sha)):
            return lobby.build(self.movie(base), None, project, report=silent, **options)

    def test_the_result_must_be_the_pinned_one(self):
        movie, _ = self.run_build(lobby.RESULT_SHA256)
        self.assertEqual(movie, b'the lobby from source')
        with self.assertRaisesRegex(ValueError, 'not RESULT_SHA256'):
            self.run_build('0' * 64)
        movie, report = self.run_build('0' * 64, any_result=True)
        self.assertEqual(report['added_methods'], 2)

    def test_a_kept_work_directory_must_be_new(self):
        work = self.root / 'work'
        self.run_build(lobby.RESULT_SHA256, work=work)
        self.assertTrue((work / 'build' / 'feature.swf').is_file())
        with self.assertRaises(FileExistsError):
            self.run_build(lobby.RESULT_SHA256, work=work)

    def test_the_export_is_made_once_from_the_movie(self):
        base = b'revision 24'
        project = self.root / 'private' / 'rev24'
        manifest = {'movie_sha256': hashlib.sha256(base).hexdigest(), 'release': lobby.RELEASE}

        def init(movie, pending, tools, release):
            self.assertEqual((pending.name, release), ('rev24.partial', lobby.RELEASE))
            pending.mkdir()

        with mock.patch.object(lobby, 'BASE_SHA256', hashlib.sha256(base).hexdigest()), \
                mock.patch.object(lobby.movie_source, 'init_project', side_effect=init) as made, \
                mock.patch.object(lobby.movie_source, 'verify_project', return_value=manifest):
            self.assertEqual(lobby.project_for(self.movie(base), project, None, silent), project)
            self.assertEqual(lobby.project_for(self.movie(base), project, None, silent), project)
        self.assertEqual(made.call_count, 1)
        with mock.patch.object(lobby.movie_source, 'verify_project', return_value=dict(manifest, release='other')):
            with self.assertRaisesRegex(ValueError, "not revision 24's pinned export"):
                lobby.project_for(self.movie(base), project, None, silent)

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
    """The build list and the repository-authored classes."""

    def features(self):
        spec = json.loads((LOBBY / 'lobby.json').read_text())
        return spec, {name: json.loads((LOBBY / 'features' / name / 'feature.json').read_text())
                      for name in spec['features']}

    def test_every_feature_and_class_source_is_built(self):
        # A feature or a class left out of the build list never reaches the players.
        spec, features = self.features()
        self.assertEqual(sorted(p.name for p in (LOBBY / 'features').iterdir()), sorted(spec['features']))
        sources = {entry['source'] for feature in features.values() for entry in feature['classes'] if 'source' in entry}
        self.assertEqual(sources, {f'src/{p.name}' for p in (LOBBY / 'src').glob('*.as')})
        for name, feature in features.items():
            for entry in feature['classes']:
                if 'patch' in entry:
                    self.assertTrue((LOBBY / 'features' / name / entry['patch']).is_file(), (name, entry))

    def test_the_build_list_is_the_pinned_releases(self):
        spec, _ = self.features()
        self.assertEqual((spec['release'], spec['movie_sha256']), (lobby.RELEASE, lobby.BASE_SHA256))
        self.assertEqual(spec['reconstruction_sha256'], hashlib.sha256(lobby.RECONSTRUCTION.read_bytes()).hexdigest())

    def test_no_class_defines_a_method_twice(self):
        # FFDec compiles these classes and keeps the last of two methods of one name.
        for path in (LOBBY / 'src').glob('*.as'):
            self.assertEqual(lobby.build_feature.duplicate_functions(path.read_text(encoding='utf-8')), [], path.name)

    def test_the_exchange_and_the_server_agree_on_the_buy_gold_mark(self):
        window = (LOBBY / 'src' / 'SrpExchange.as').read_text(encoding='utf-8')
        server = (HERE.parent / 'crates' / 'survarium' / 'src' / 'v026g' / 'lobby' / 'requests.rs').read_text(encoding='utf-8')
        sent = re.search(r'BUY_GOLD:uint = (0x[0-9A-Fa-f]+);', window)
        read = re.search(r'pub const BUY_PREMIUM: u32 = (0x[0-9A-Fa-f_]+);', server)
        self.assertIsNotNone(sent)
        self.assertIsNotNone(read)
        self.assertEqual(int(sent.group(1), 16), int(read.group(1).replace('_', ''), 16))

    def test_every_exchange_text_is_in_every_language(self):
        window = (LOBBY / 'src' / 'SrpExchange.as').read_text(encoding='utf-8')
        texts = re.search(r'TEXT:Object = \{(.*?)\};', window, re.S).group(1)
        rows = re.findall(r'"(\w+)": \[(.*?)\]', texts, re.S)
        self.assertGreater(len(rows), 5)
        for key, row in rows:
            self.assertEqual(len(re.findall(r'"[^"]+"', row)), 5, key)

    def test_get_gold_opens_the_exchange_over_the_dialog_on_the_buy_side(self):
        source = (LOBBY / 'src' / 'SrpBuyGold.as').read_text(encoding='utf-8')
        self.assertIn('Message.showMessage("exchange", {"label": "", "srpBuyGold": true}, true, 0, true);', source)


if __name__ == '__main__':
    unittest.main()
