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
            with self.assertRaisesRegex(ValueError, 'baseline'):
                build_feature.feature_inputs(path, pinned, {}, evidence, root)
            path.write_text(json.dumps(dict(spec, reconstruction_sha256='stale')))
            with self.assertRaisesRegex(ValueError, 'reconstruction'):
                build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)
            path.write_text(json.dumps(dict(spec, classes=[entry, entry])))
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                build_feature.feature_inputs(path, pinned, {'C.as': baseline}, evidence, root)


if __name__ == '__main__':
    unittest.main()
