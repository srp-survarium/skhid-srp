import json
from pathlib import Path
import tempfile
import unittest

import feature_source
import source_patches as patches


class FeatureSourceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = self.root = Path(self.temp.name)
        project = self.project = root / 'project'
        (project / 'pristine/pkg').mkdir(parents=True)
        pristine = {'pkg/Game.as': b'a\r\nb\r\nc\r\n', 'Fixed.as': b'broken\r\n'}
        for relative, data in pristine.items():
            (project / 'pristine' / relative).write_bytes(data)
        (project / 'original.swf').write_bytes(b'movie')
        (project / 'manifest.json').write_text(json.dumps({
            'release': 'test', 'movie_sha256': patches.sha(b'movie'),
            'sources': {relative: patches.sha(data) for relative, data in pristine.items()}}))
        fix = b'--- a/Fixed.as\n+++ b/Fixed.as\n@@ -1 +1 @@\n-broken\n+fixed\n'
        (root / 'reconstruction').mkdir()
        (root / 'reconstruction/Fixed.patch').write_bytes(fix)
        self.reconstruction = root / 'reconstruction/manifest.json'
        self.reconstruction.write_text(json.dumps({
            'schema': 1, 'release': 'test', 'movie_sha256': patches.sha(b'movie'),
            'classes': [{'class': 'Fixed', 'patch': 'Fixed.patch', 'pristine_sha256': patches.sha(b'broken\r\n'),
                         'patch_sha256': patches.sha(fix), 'result_sha256': patches.sha(b'fixed\r\n')}]}))
        self.lobby = root / 'lobby'
        self.lobby.mkdir()
        self.build_list = self.lobby / 'lobby.json'
        self.build_list.write_text(json.dumps({'schema': 2, 'features': ['one', 'two']}))

    def tearDown(self):
        self.temp.cleanup()

    def record(self, feature, cls, text):
        edited = self.root / f'{feature}.as'
        edited.write_bytes(text)
        relative = cls.replace('.', '/') + '.as'
        before = feature_source.source_before(self.build_list, self.project, self.reconstruction, cls, feature)
        folder = self.lobby / 'features' / feature
        folder.mkdir(parents=True, exist_ok=True)
        name = cls.rsplit('.', 1)[-1] + '.patch'
        (folder / name).write_bytes(feature_source.diff(before, text, relative))
        (folder / 'feature.json').write_text(json.dumps({'schema': 2, 'classes': [
            {'class': cls, 'patch': name, 'methods': ['m']}]}))

    def test_show_applies_the_reconstruction_then_every_earlier_feature(self):
        show = lambda cls, stop=None: feature_source.source_before(
            self.build_list, self.project, self.reconstruction, cls, stop)
        self.assertEqual(show('Fixed', 'one'), b'fixed\r\n')
        self.record('one', 'pkg.Game', b'a\nB\nc\n')
        self.record('two', 'pkg.Game', b'a\nB\nC\nd\n')
        self.assertEqual(show('Fixed'), b'fixed\r\n')
        self.assertEqual(show('pkg.Game', 'one'), b'a\r\nb\r\nc\r\n')
        self.assertEqual(show('pkg.Game', 'two'), b'a\r\nB\r\nc\r\n')
        self.assertEqual(show('pkg.Game'), b'a\r\nB\r\nC\r\nd\r\n')
        with self.assertRaisesRegex(ValueError, 'no feature'):
            show('pkg.Game', 'three')
        with self.assertRaisesRegex(ValueError, 'not a class of the release'):
            show('New')

    def test_a_diff_has_the_applicator_headers_and_refuses_no_change(self):
        patch = feature_source.diff(b'a\r\nb\r\n', b'a\nc\n', 'pkg/Game.as')
        self.assertTrue(patch.startswith(b'--- a/pkg/Game.as\n+++ b/pkg/Game.as\n@@ '))
        self.assertEqual(patches.apply_patch(b'a\r\nb\r\n', patch, 'pkg/Game.as'), b'a\r\nc\r\n')
        with self.assertRaisesRegex(ValueError, 'same'):
            feature_source.diff(b'a\n', b'a\r\n', 'pkg/Game.as')


if __name__ == '__main__':
    unittest.main()
