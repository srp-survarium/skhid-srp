import difflib
import json
from pathlib import Path
import tempfile
import unittest

import source_patches as patches


class SourcePatchTests(unittest.TestCase):
    def patch(self, before, after):
        return ''.join(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True), fromfile='a/C.as', tofile='b/C.as')).encode()

    def test_exact_positions_and_line_endings(self):
        before, after = 'a\nb\nc\n', 'a\nchanged\nc\nextra\n'
        diff = self.patch(before, after)
        self.assertEqual(patches.apply_patch(before.encode(), diff, 'C.as'), after.encode())
        self.assertEqual(patches.apply_patch(before.replace('\n', '\r\n').encode(), diff, 'C.as'), after.replace('\n', '\r\n').encode())
        for bad in (b'prefix\n' + before.encode(), b'a\nwrong\nc\n', b'a\r\nb\nc\n'):
            with self.assertRaises(ValueError):
                patches.apply_patch(bad, diff, 'C.as')
        for bad in (diff.replace(b'b/C.as', b'b/other.as'), diff.replace(b'+1,4', b'+2,4'), diff.replace(b'-1,3', b'-1,2'), diff + b'\\ No newline at end of file\n'):
            with self.assertRaises(ValueError):
                patches.apply_patch(before.encode(), bad, 'C.as')

    def test_empty_range_insertions_and_deletions(self):
        for before, after in [('a\n', 'first\na\n'), ('a\nb\n', 'a\n'), ('a\n', 'a\nlast\n')]:
            diff = ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='a/C.as', tofile='b/C.as', n=0)).encode()
            self.assertEqual(patches.apply_patch(before.encode(), diff, 'C.as'), after.encode())

    def test_manifest_authenticates_movie_source_patch_and_result(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'pristine').mkdir()
            before, after = b'class C {}\n', b'class C { public var value:int; }\n'
            source = root / 'pristine/C.as'
            source.write_bytes(before)
            diff = self.patch(before.decode(), after.decode())
            (root / 'C.patch').write_bytes(diff)
            pinned = {'release': 'test', 'movie_sha256': 'movie', 'sources': {'C.as': patches.sha(before)}}
            entry = {'class': 'C', 'patch': 'C.patch', 'patch_sha256': patches.sha(diff), 'pristine_sha256': patches.sha(before), 'result_sha256': patches.sha(after)}
            spec = {'schema': 1, 'release': 'test', 'movie_sha256': 'movie', 'classes': [entry]}
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps(spec))
            self.assertEqual(patches.load_sources(manifest, root, pinned)[0], {'C.as': after})
            self.assertEqual(source.read_bytes(), before)
            for field in ('patch_sha256', 'pristine_sha256', 'result_sha256'):
                broken = dict(spec, classes=[dict(entry, **{field: 'changed'})])
                manifest.write_text(json.dumps(broken))
                with self.assertRaises(ValueError):
                    patches.load_sources(manifest, root, pinned)
            for path in ('../outside.patch', '/tmp/outside.patch'):
                manifest.write_text(json.dumps(dict(spec, classes=[dict(entry, patch=path)])))
                with self.assertRaises(ValueError):
                    patches.load_sources(manifest, root, pinned)
            manifest.write_text(json.dumps(dict(spec, classes=[entry, entry])))
            with self.assertRaises(ValueError):
                patches.load_sources(manifest, root, pinned)
            manifest.write_text(json.dumps(dict(spec, movie_sha256='different')))
            with self.assertRaises(ValueError):
                patches.load_sources(manifest, root, pinned)


if __name__ == '__main__':
    unittest.main()
