import difflib
import json
from pathlib import Path
import tempfile
import unittest

import build_feature
import source_patches as patches
from test_movie_source import swf, tag


class FeatureTests(unittest.TestCase):
    def test_container_gate_preserves_assets_and_abc_loading_attributes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            original, candidate = root / 'original.swf', root / 'candidate.swf'
            assets = (1000, tag(1000, b'opaque Scaleform bytes', long=True))
            code = (82, tag(82, b'\x01\0\0\0name\0old'))
            end = (0, tag(0, b''))
            original.write_bytes(swf([assets, code, end]))
            candidate.write_bytes(swf([assets, (82, tag(82, b'\x01\0\0\0name\0' + b'new' * 100)), end]))
            build_feature.verify_container(original, candidate)
            for tags in [
                [assets, (82, tag(82, b'\x00\0\0\0name\0new')), end],
                [assets, (82, tag(82, b'\x01\0\0\0renamed\0new')), end],
                [(1000, tag(1000, b'changed asset', long=True)), code, end],
                [code, assets, end],
            ]:
                candidate.write_bytes(swf(tags))
                with self.assertRaises(ValueError):
                    build_feature.verify_container(original, candidate)

    def test_feature_applies_to_the_pinned_reconstruction(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'pristine').mkdir()
            (root / 'pristine/C.as').write_bytes(b'pristine\n')
            baseline, changed = b'reconstructed\n', b'feature\n'
            diff = ''.join(difflib.unified_diff(baseline.decode().splitlines(True), changed.decode().splitlines(True),
                          fromfile='a/C.as', tofile='b/C.as')).encode()
            (root / 'C.patch').write_bytes(diff)
            pinned = {'release': 'test', 'movie_sha256': 'movie', 'sources': {'C.as': patches.sha(b'pristine\n')}}
            evidence = {'manifest_sha256': 'reconstruction', 'classes': [{'class': 'C', 'compiler': 'ffdec'}]}
            entry = {'class': 'C', 'patch': 'C.patch', 'baseline_sha256': patches.sha(baseline),
                     'patch_sha256': patches.sha(diff), 'result_sha256': patches.sha(changed),
                     'methods': [{'owner': 'C/method'}]}
            spec = {'schema': 1, 'release': 'test', 'movie_sha256': 'movie', 'reconstruction_sha256': 'reconstruction',
                    'classes': [entry]}
            path = root / 'feature.json'
            path.write_text(json.dumps(spec))
            _, inputs = build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)
            self.assertEqual(inputs[0]['baseline'], baseline)
            self.assertEqual(inputs[0]['changed'], changed)
            self.assertEqual(inputs[0]['compiler'], 'ffdec')
            evidence['classes'][0]['method_reservations'] = {'C/method': {'stack': 3}}
            _, inputs = build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)
            self.assertEqual(inputs[0]['reservations'], b'C/method\t3\t0\n')
            with self.assertRaisesRegex(ValueError, 'baseline'):
                build_feature.feature_inputs(path, pinned, {}, evidence, root)
            path.write_text(json.dumps(dict(spec, reconstruction_sha256='stale')))
            with self.assertRaisesRegex(ValueError, 'reconstruction'):
                build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)
            path.write_text(json.dumps(dict(spec, classes=[entry, entry])))
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)


GAME = """package pkg
{
   public class Game
   {
      public var a:int;

      public function two() : void
      {
         trace(1);
      }

      public function three() : int
      {
         return 3;
      }
   }
}
"""


class TreeInputsTests(unittest.TestCase):
    """A movie's source tree against its baseline: what is compiled, with which permissions."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = self.root = Path(self.temp.name)
        baseline = {'pkg/Game.as': GAME.replace('\n', '\r\n').encode(), 'Owned.as': b'old owned\r\n',
                    'Anchor.as': b'anchor\r\n'}
        (root / 'project/pristine/pkg').mkdir(parents=True)
        for relative, data in baseline.items():
            (root / 'project/pristine' / relative).write_bytes(data)
        self.pinned = {'release': 'test', 'movie_sha256': 'movie',
                       'sources': {relative: patches.sha(data) for relative, data in baseline.items()}}
        self.evidence = {'manifest_sha256': 'reconstruction',
                         'classes': [{'class': 'Owned', 'compiler': 'ffdec-source'}]}
        self.tree = root / 'flash' / 'm'
        (self.tree / 'src/pkg').mkdir(parents=True)
        lf = {relative: data.replace(b'\r\n', b'\n') for relative, data in baseline.items()}
        for relative, data in lf.items():
            (self.tree / 'src' / relative).write_bytes(data)
        (self.tree / 'movie.json').write_text(json.dumps({
            'release': 'test', 'movie_sha256': 'movie', 'movie': 'flash_movies/m.swf',
            'reconstruction_sha256': 'reconstruction',
            'sources': {relative: patches.sha(data) for relative, data in lf.items()}}))
        self.srp([{'class': 'Owned'}])

    def srp(self, classes):
        (self.tree / 'srp.json').write_text(json.dumps({'classes': classes}))

    def tearDown(self):
        self.temp.cleanup()

    def inputs(self):
        return build_feature.tree_inputs(self.tree, self.pinned, {}, self.evidence, self.root / 'project')[1]

    def write(self, relative, text):
        (self.tree / 'src' / relative).write_text(text)

    def test_the_baseline_tree_builds_nothing(self):
        with self.assertRaisesRegex(ValueError, 'nothing to build'):
            self.inputs()

    def test_changed_files_compile_new_then_owned_then_game_with_derived_permissions(self):
        self.srp([{'class': 'Owned'}, {'class': 'Helper', 'abc_with': 'Anchor'}])
        self.write('Helper.as', 'function help() {}\n')
        self.write('Owned.as', 'function a() {}\n')
        self.write('pkg/Game.as', GAME.replace('trace(1);', 'trace(2);').replace(
            '      public function three', '      public var b:int;\n\n      public function four() : void\n      {\n      }\n\n      public function three'))
        helper, owned, game = self.inputs()
        self.assertEqual([(i['class'], i['kind']) for i in (helper, owned, game)],
                         [('Helper', 'new'), ('Owned', 'owned'), ('pkg.Game', 'game')])
        self.assertEqual((helper['anchor'], helper['compiler'], helper['permissions']), ('Anchor', 'ffdec', b'class\tHelper\n'))
        self.assertEqual((owned['compiler'], owned['permissions'], owned['baseline']), ('ffdec-source', b'class\tOwned\n', b'old owned\n'))
        self.assertEqual(game['permissions'].decode().splitlines(),
                         ['edit\tpkg.Game\ttwo\t?\t?', 'member\tpkg.Game\tb', 'member\tpkg.Game\tfour'])
        self.assertIn(b'+         trace(2);', game['patch'])

    def test_refusals(self):
        for relative, text, message in [
            ('New.as', 'x\n', 'name it'),
            ('pkg/Game.as', GAME.replace('      public function three() : int\n      {\n         return 3;\n      }\n', ''), 'removed'),
            ('Owned.as', 'function a() {}\nfunction a() {}\n', 'more than once'),
            ('pkg/Game.as', GAME.replace('package pkg', 'package  pkg'), 'no method or declaration'),
        ]:
            with self.subTest(message):
                original = (self.tree / 'src' / relative).read_bytes() if (self.tree / 'src' / relative).exists() else None
                self.write(relative, text)
                with self.assertRaisesRegex(ValueError, message):
                    self.inputs()
                if original is None:
                    (self.tree / 'src' / relative).unlink()
                else:
                    (self.tree / 'src' / relative).write_bytes(original)
        (self.tree / 'src/Anchor.as').unlink()
        with self.assertRaisesRegex(ValueError, 'removed'):
            self.inputs()

    def test_the_tree_must_be_of_the_pinned_export(self):
        self.write('Owned.as', 'changed\n')
        self.evidence['manifest_sha256'] = 'other'
        with self.assertRaisesRegex(ValueError, 'another reconstruction'):
            self.inputs()


if __name__ == '__main__':
    unittest.main()
