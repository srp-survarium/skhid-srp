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

    def test_one_abc_block_is_replaced_and_found(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            blocks = [(82, tag(82, b'\x01\0\0\0' + name + b'\0code')) for name in (b'a', b'b', b'c')]
            base, other = root / 'base.swf', root / 'other.swf'
            base.write_bytes(swf([(69, tag(69, b'\0\0\0\0'))] + blocks + [(0, tag(0, b''))]))
            self.assertIsNone(build_feature.changed_block(base, base))
            raw = tag(82, b'\x01\0\0\0b\0new code')
            build_feature.with_block(base, 1, raw, other)
            self.assertEqual(build_feature.abc_blocks(other), [blocks[0][1], raw, blocks[2][1]])
            self.assertEqual(build_feature.changed_block(base, other), 1)
            build_feature.with_block(other, 2, raw, other)
            with self.assertRaisesRegex(ValueError, 'More than one'):
                build_feature.changed_block(base, other)


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
    """A movie's source tree against its baseline: what is compiled, whole."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = self.root = Path(self.temp.name)
        baseline = {'pkg/Game.as': GAME.encode(), 'Owned.as': b'old owned\n', 'Anchor.as': b'anchor\n'}
        self.tree = root / 'flash' / 'm'
        (self.tree / 'src/pkg').mkdir(parents=True)
        for relative, data in baseline.items():
            (self.tree / 'src' / relative).write_bytes(data)
        (self.tree / 'movie.json').write_text(json.dumps({
            'release': 'test', 'movie_sha256': 'movie', 'movie': 'flash_movies/m.swf',
            'reconstruction_sha256': 'reconstruction',
            'sources': {relative: patches.sha(data) for relative, data in baseline.items()}}))
        self.plans = {'Owned': ('ffdec-source', None, None)}

    def srp(self, classes):
        (self.tree / 'srp.json').write_text(json.dumps({'new_classes': classes}))

    def tearDown(self):
        self.temp.cleanup()

    def inputs(self):
        return build_feature.tree_inputs(self.tree, self.plans)[1]

    def write(self, relative, text):
        (self.tree / 'src' / relative).write_text(text)

    def test_the_baseline_tree_has_nothing_to_compile(self):
        self.assertEqual(self.inputs(), [])

    def test_every_changed_file_is_compiled_whole(self):
        # A method put in the middle of the class, and a try in an edited
        # method: the file is compiled whole, like any other edit.
        self.write('Owned.as', 'function a() {}\n')
        self.write('pkg/Game.as', GAME.replace('trace(1);', 'try { trace(2); } catch (e:Error) { }').replace(
            '      public function two', '      public function inserted() : void\n      {\n      }\n\n      public function two'))
        owned, game = self.inputs()
        self.assertEqual([(i['class'], i['kind'], i['compiler']) for i in (owned, game)],
                         [('Owned', 'changed', 'ffdec-source'), ('pkg.Game', 'changed', 'flex-strict')])
        self.assertEqual(game['source'], (self.tree / 'src/pkg/Game.as').read_bytes())

    def test_a_new_file_is_a_new_class_in_the_abc_srp_json_names(self):
        self.write('pkg/Helper.as', 'package pkg { public class Helper { } }\n')
        with self.assertRaisesRegex(ValueError, 'is new: name pkg.Helper'):
            self.inputs()
        self.srp([{'class': 'pkg.Helper', 'abc_with': 'Anchor'}])
        self.write('Owned.as', 'function a() {}\n')
        helper, owned = self.inputs()
        self.assertEqual((helper['class'], helper['kind'], helper['anchor'], helper['compiler'], helper['source_name']),
                         ('pkg.Helper', 'new', 'Anchor', 'ffdec', 'Helper.as'))
        self.assertEqual(owned['kind'], 'changed')
        for classes, message in [([{'class': 'pkg.Helper', 'abc_with': 'Missing'}], 'abc_with'),
                                 ([{'class': 'pkg.Helper'}], 'invalid'),
                                 ([{'class': 'pkg.Helper', 'abc_with': 'Anchor'}, {'class': 'Gone', 'abc_with': 'Anchor'}],
                                  'not a new file')]:
            with self.subTest(message):
                self.srp(classes)
                with self.assertRaisesRegex(ValueError, message):
                    self.inputs()
        self.srp([{'class': 'pkg.Helper', 'abc_with': 'Anchor'}])
        self.write('pkg/Helper.as', 'function a() {}\nfunction a() {}\n')
        with self.assertRaisesRegex(ValueError, 'more than once'):
            self.inputs()

    def test_a_file_gone_from_the_tree_is_taken_out_when_nothing_names_it(self):
        (self.tree / 'src/Owned.as').unlink()
        self.write('pkg/Game.as', GAME.replace('trace(1);', 'Owned.go();'))
        with self.assertRaisesRegex(ValueError, 'still names it: pkg/Game.as'):
            self.inputs()
        self.write('pkg/Game.as', GAME.replace('trace(1);', 'OwnedNot.go();'))
        removed, game = self.inputs()
        self.assertEqual((removed['class'], removed['kind'], game['kind']), ('Owned', 'removed', 'changed'))

    def test_srp_json_names_new_classes_only(self):
        (self.tree / 'srp.json').write_text(json.dumps({'classes': [], 'removed': ['Owned']}))
        with self.assertRaisesRegex(ValueError, 'new_classes'):
            self.inputs()

    def test_a_new_class_is_first_its_declaration_alone(self):
        source = b'''package survarium.rooms
{
    import flash.display.Sprite;
    import flash.events.Event;
    import survarium.controls.PlayerInfo;

    // The window.
    public class RoomsWindow extends Sprite
    {
        public static var me:RoomsWindow;
    }
}
'''
        self.assertEqual(build_feature.stub_source(source).decode(),
                         'package survarium.rooms\n{\n    import flash.display.Sprite;\n    public class RoomsWindow extends Sprite\n'
                         '    {\n    }\n}\n')
        self.assertEqual(build_feature.stub_source(b'package {\n public class A {\n }\n}\n').decode(),
                         'package\n{\n    public class A\n    {\n    }\n}\n')
        with self.assertRaises(ValueError):
            build_feature.stub_source(b'package { function f() {} }')

    def test_the_compiler_settings_are_the_tree_s_reconstruction(self):
        manifest = self.root / 'manifest.json'
        manifest.write_text(json.dumps({'classes': [{'class': 'Owned', 'compiler': 'ffdec-source'}, {'class': 'pkg.Game'}]}))
        spec = {'reconstruction_sha256': patches.sha(manifest.read_bytes())}
        self.assertEqual(build_feature.compiler_plans(manifest, spec),
                         {'Owned': ('ffdec-source', None, None), 'pkg.Game': ('flex-strict', None, None)})
        with self.assertRaisesRegex(ValueError, 'another reconstruction'):
            build_feature.compiler_plans(manifest, {'reconstruction_sha256': 'other'})


if __name__ == '__main__':
    unittest.main()
