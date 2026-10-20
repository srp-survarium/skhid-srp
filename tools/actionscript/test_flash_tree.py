import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import flash_tree
import source_patches


def sha(data):
    return hashlib.sha256(data).hexdigest()


class FlashTreeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = self.root = Path(self.temp.name)
        self.movie = root / 'flash' / 'm'
        files = {'A.as': b'a\r\n', 'pkg/B.as': b'b\r\n'}
        for relative, data in files.items():
            (self.movie / 'src' / relative).parent.mkdir(parents=True, exist_ok=True)
            (self.movie / 'src' / relative).write_bytes(data)
        self.spec = {'schema': 1, 'release': 'test', 'movie': 'flash_movies/m.swf', 'movie_sha256': sha(b'MOVIE'),
                     'reconstruction': 'r/manifest.json', 'reconstruction_sha256': 'x',
                     'sources': {relative: sha(data) for relative, data in files.items()}}
        (self.movie / 'movie.json').write_text(json.dumps(self.spec))

    def tearDown(self):
        self.temp.cleanup()

    def test_differences_against_the_baseline(self):
        self.assertEqual(flash_tree.differences(self.movie), ([], [], []))
        (self.movie / 'src/A.as').write_bytes(b'edited\r\n')
        (self.movie / 'src/New.as').write_bytes(b'new\n')
        (self.movie / 'src/pkg/B.as').unlink()
        self.assertEqual(flash_tree.differences(self.movie), (['A.as'], ['New.as'], ['pkg/B.as']))
        (self.movie / 'src/notes.txt').write_text('x')
        with self.assertRaisesRegex(ValueError, 'Not ActionScript'):
            flash_tree.differences(self.movie)

    def test_a_full_build_is_of_the_baseline_tree_only(self):
        (self.movie / 'src/A.as').write_bytes(b'edited\r\n')
        with self.assertRaisesRegex(ValueError, 'baseline tree'):
            flash_tree.full(self.movie, self.root / 'o.swf', None, self.root / 'work', 1)

    def test_the_original_is_cut_from_the_pinned_archive_entry(self):
        archive = self.root / 'resources.db.patch_9'
        archive.write_bytes(b'xxxx' + b'MOVIE' + b'yyyy')
        release = {'archive': archive.name, 'archive_offset': 4, 'movie_size': 5}
        with mock.patch.object(flash_tree, 'pin', return_value=release):
            flash_tree.original(self.movie, archive, self.root / 'out.swf')
            self.assertEqual((self.root / 'out.swf').read_bytes(), b'MOVIE')
            with self.assertRaises(FileExistsError):
                flash_tree.original(self.movie, archive, self.root / 'out.swf')
            archive.write_bytes(b'z' * 13)
            with self.assertRaisesRegex(ValueError, 'does not hold'):
                flash_tree.original(self.movie, archive, self.root / 'other.swf')
            with self.assertRaisesRegex(ValueError, 'is in'):
                flash_tree.original(self.movie, self.root / 'resources.db', self.root / 'x.swf')

    def test_the_project_from_a_tree_pins_the_tree_files(self):
        (self.root / 'o.swf').write_bytes(b'MOVIE')
        project = flash_tree.project_from_tree(self.movie, self.root / 'o.swf', self.root / 'project')
        manifest = json.loads((project / 'manifest.json').read_text())
        self.assertEqual(manifest['sources'], self.spec['sources'])
        self.assertEqual((project / 'pristine/pkg/B.as').read_bytes(), b'b\r\n')
        (self.root / 'bad.swf').write_bytes(b'OTHER')
        with self.assertRaisesRegex(ValueError, 'pinned original'):
            flash_tree.project_from_tree(self.movie, self.root / 'bad.swf', self.root / 'project2')

    def test_compiler_profiles_keep_the_settings_and_drop_the_diffs(self):
        recon = self.root / 'manifest.json'
        recon.write_text(json.dumps({'schema': 1, 'release': 'test', 'movie_sha256': self.spec['movie_sha256'], 'classes': [
            {'class': 'A', 'compiler': 'ffdec', 'patch': 'A.patch', 'pristine_sha256': '1', 'patch_sha256': '2',
             'result_sha256': '3', 'method_reservations': {'A/m': {'stack': 2}}}]}))
        spec = dict(self.spec, reconstruction=recon.name, reconstruction_sha256=sha(recon.read_bytes()))
        with mock.patch.object(flash_tree, 'REPO', self.root):
            out = flash_tree.compiler_profiles(spec, self.root / 'profiles.json')
            self.assertEqual(json.loads(out.read_text())['classes'],
                             [{'class': 'A', 'compiler': 'ffdec', 'method_reservations': {'A/m': {'stack': 2}}}])
            with self.assertRaisesRegex(ValueError, 'differs'):
                flash_tree.compiler_profiles(dict(spec, reconstruction_sha256='stale'), self.root / 'p2.json')

    def test_a_reconstruction_entry_may_set_a_compiler_without_a_patch(self):
        project = self.root / 'project'
        (project / 'pristine').mkdir(parents=True)
        (project / 'pristine/A.as').write_bytes(b'a\n')
        pinned = {'release': 'test', 'movie_sha256': 'm', 'sources': {'A.as': sha(b'a\n')}}
        manifest = self.root / 'recon.json'
        manifest.write_text(json.dumps({'schema': 1, 'release': 'test', 'movie_sha256': 'm',
                                        'classes': [{'class': 'A', 'compiler': 'ffdec'}]}))
        sources, evidence = source_patches.load_sources(manifest, project, pinned)
        self.assertEqual((sources, evidence['classes']), ({}, [{'class': 'A', 'compiler': 'ffdec'}]))
        manifest.write_text(json.dumps({'schema': 1, 'release': 'test', 'movie_sha256': 'm',
                                        'classes': [{'class': 'A', 'compiler': 'ffdec', 'result_sha256': 'x'}]}))
        with self.assertRaisesRegex(ValueError, 'no patch hashes'):
            source_patches.load_sources(manifest, project, pinned)


if __name__ == '__main__':
    unittest.main()
