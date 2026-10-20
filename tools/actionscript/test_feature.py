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


def diff_of(before, after, relative):
    return ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                                        fromfile=f'a/{relative}', tofile=f'b/{relative}')).encode()


class BuildListTests(unittest.TestCase):
    """Schema 2: ordered features over the reconstruction, with whole and new classes."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = self.root = Path(self.temp.name)
        (root / 'pristine/pkg').mkdir(parents=True)
        self.sources = {'pkg/Game.as': b'one\ntwo\nthree\n', 'Owned.as': b'old owned\n', 'Anchor.as': b'anchor\n'}
        for relative, text in self.sources.items():
            (root / 'pristine' / relative).write_bytes(text)
        self.pinned = {'release': 'test', 'movie_sha256': 'movie',
                       'sources': {relative: patches.sha(text) for relative, text in self.sources.items()}}
        self.evidence = {'manifest_sha256': 'reconstruction',
                         'classes': [{'class': 'Owned', 'compiler': 'ffdec'}, {'class': 'pkg.Game', 'compiler': 'flex-strict'}]}
        self.lobby = root / 'lobby'
        (self.lobby / 'src').mkdir(parents=True)
        (self.lobby / 'src/Owned.as').write_bytes(b'function a() {}\nfunction b() {}\n')
        (self.lobby / 'src/Helper.as').write_bytes(b'function help() {}\n')
        self.write_feature('first', [
            {'class': 'Helper', 'new': True, 'source': 'src/Helper.as', 'abc_with': 'Anchor'},
            {'class': 'pkg.Game', 'patch': 'Game.patch', 'methods': ['two']},
        ], {'Game.patch': diff_of('one\ntwo\nthree\n', 'one\nTWO\nthree\n', 'pkg/Game.as')})
        self.write_feature('second', [
            {'class': 'pkg.Game', 'patch': 'Game.patch', 'methods': ['three', 'two'], 'members': ['four']},
            {'class': 'Owned', 'source': 'src/Owned.as'},
        ], {'Game.patch': diff_of('one\nTWO\nthree\n', 'one\nTWO\nTHREE\nfour\n', 'pkg/Game.as')})
        self.spec = {'schema': 2, 'release': 'test', 'movie_sha256': 'movie', 'reconstruction_sha256': 'reconstruction',
                     'features': ['first', 'second'],
                     'reviewed_calls': {'pkg.Game.two': {'before': 'a' * 64, 'after': 'b' * 64}}}

    def tearDown(self):
        self.temp.cleanup()

    def write_feature(self, name, classes, files):
        folder = self.lobby / 'features' / name
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'feature.json').write_text(json.dumps({'schema': 2, 'classes': classes}))
        for filename, data in files.items():
            (folder / filename).write_bytes(data)

    def inputs(self, spec=None, reconstructed=None):
        path = self.lobby / 'lobby.json'
        path.write_text(json.dumps(spec or self.spec))
        return build_feature.build_list_inputs(path, self.pinned, reconstructed or {}, self.evidence, self.root)

    def test_features_chain_in_order_and_classes_compile_new_owned_then_game(self):
        _, inputs, manifests = self.inputs()
        self.assertEqual(list(manifests), ['first', 'second'])
        self.assertEqual([(i['class'], i['kind']) for i in inputs], [('Helper', 'new'), ('Owned', 'owned'), ('pkg.Game', 'game')])
        helper, owned, game = inputs
        self.assertEqual((helper['anchor'], helper['compiler'], helper['permissions']), ('Anchor', 'ffdec', b'class\tHelper\n'))
        self.assertEqual((owned['baseline'], owned['changed'], owned['permissions']),
                         (b'old owned\n', b'function a() {}\nfunction b() {}\n', b'class\tOwned\n'))
        self.assertEqual(game['changed'], b'one\nTWO\nTHREE\nfour\n')
        self.assertEqual(game['features'], ['first', 'second'])
        self.assertEqual(game['permissions'].decode().splitlines(), [
            'edit\tpkg.Game\ttwo\t' + 'a' * 64 + '\t' + 'b' * 64, 'edit\tpkg.Game\tthree\t-\t-', 'member\tpkg.Game\tfour'])

    def test_a_reconstructed_source_is_the_baseline(self):
        _, inputs, _ = self.inputs(reconstructed={'pkg/Game.as': b'one\r\ntwo\r\nthree\r\n'})
        self.assertEqual(inputs[2]['baseline'], b'one\r\ntwo\r\nthree\r\n')
        self.assertEqual(inputs[2]['changed'], b'one\r\nTWO\r\nTHREE\r\nfour\r\n')

    def test_review_mode_reports_unreviewed_methods(self):
        path = self.lobby / 'lobby.json'
        path.write_text(json.dumps(self.spec))
        _, inputs, _ = build_feature.build_list_inputs(path, self.pinned, {}, self.evidence, self.root, review=True)
        self.assertIn('edit\tpkg.Game\tthree\t?\t?', inputs[2]['permissions'].decode())

    def test_refusals(self):
        with self.assertRaisesRegex(ValueError, 'no feature edits'):
            self.inputs(dict(self.spec, reviewed_calls={'pkg.Game.five': {'before': 'a' * 64, 'after': 'b' * 64}}))
        with self.assertRaisesRegex(ValueError, 'different reconstruction'):
            self.inputs(dict(self.spec, reconstruction_sha256='stale'))
        with self.assertRaisesRegex(ValueError, 'one twice'):
            self.inputs(dict(self.spec, features=['first', 'first']))
        with self.assertRaisesRegex(ValueError, 'context differs'):
            self.inputs(dict(self.spec, features=['second']))
        (self.lobby / 'src/Owned.as').write_bytes(b'function a() {}\nfunction a() {}\n')
        with self.assertRaisesRegex(ValueError, 'more than once'):
            self.inputs()
        for classes, message in [
            ([{'class': 'Anchor', 'new': True, 'source': 'src/Helper.as', 'abc_with': 'Anchor'}], 'not a new class'),
            ([{'class': 'Helper2', 'new': True, 'source': 'src/Helper.as', 'abc_with': 'Anchor'}], 'file of its name'),
            ([{'class': 'Helper', 'new': True, 'source': '../Helper.as', 'abc_with': 'Anchor'}], 'Invalid Class source path'),
            ([{'class': 'Unknown', 'patch': 'Game.patch', 'methods': ['x']}], 'unknown class'),
            ([{'class': 'pkg.Game', 'patch': 'Game.patch'}], 'names no edited method'),
            ([{'class': 'pkg.Game', 'source': 'src/Owned.as', 'methods': ['x']}], 'whole source'),
        ]:
            self.write_feature('first', classes, {})
            with self.assertRaisesRegex(ValueError, message):
                self.inputs(dict(self.spec, features=['first'], reviewed_calls={}))


if __name__ == '__main__':
    unittest.main()
