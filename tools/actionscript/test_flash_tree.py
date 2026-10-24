"""The movie build: one compile of the whole tree, its code in place of the original's.

`SRP_AS_TOOLS=/path/to/tools.json` adds a real compc build of a small tree.
"""
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest

import flash_tree
import swf


def sha(data):
    return hashlib.sha256(data).hexdigest()


def tag(kind, body):
    return swf.tag(kind, body)


def movie(tags, compressed=False):
    data = swf.join(b'FWS\x15\0\0\0\0\0\0\x18\x01\0', tags + [(0, struct.pack('<H', 0))])
    return swf.compressed(data, b'CWS' if compressed else b'FWS')


def u30(value):
    out = bytearray()
    while True:
        byte = value & 0x7f
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def abc(*classes):
    """A well-formed ABC block defining `classes` (qualified names), one script each."""
    strings, namespaces, names = [''], [], []
    for cls in classes:
        package, _, local = cls.rpartition('.')
        for text in (package, local):
            if text not in strings:
                strings.append(text)
        namespaces.append(strings.index(package))
        names.append((len(namespaces), strings.index(local)))
    pool = u30(0) + u30(0) + u30(0)
    pool += u30(len(strings)) + b''.join(u30(len(s.encode())) + s.encode() for s in strings[1:])
    pool += u30(len(namespaces) + 1) + b''.join(b'\x16' + u30(n) for n in namespaces)
    pool += u30(0)
    pool += u30(len(names) + 1) + b''.join(b'\x07' + u30(ns) + u30(name) for ns, name in names)
    n = len(classes)
    methods = u30(3 * n) + (u30(0) + u30(0) + u30(0) + b'\0') * (3 * n)
    instances = b''.join(u30(i + 1) + u30(0) + b'\0' + u30(0) + u30(i) + u30(0) for i in range(n))
    statics = b''.join(u30(n + i) + u30(0) for i in range(n))
    scripts = u30(n) + b''.join(u30(2 * n + i) + u30(1) + u30(i + 1) + b'\x04' + u30(0) + u30(i) for i in range(n))
    bodies = u30(3 * n) + b''.join(u30(m) + u30(1) + u30(1) + u30(0) + u30(1) + u30(1) + b'\x47' + u30(0) + u30(0)
                                   for m in range(3 * n))
    return struct.pack('<HH', 16, 46) + pool + methods + u30(0) + u30(n) + instances + statics + scripts + bodies


def do_abc(name, *classes):
    return (swf.DO_ABC, tag(swf.DO_ABC, struct.pack('<I', 1) + name.encode() + b'\0' + abc(*classes)))


def symbols(*entries):
    body = struct.pack('<H', len(entries)) + b''.join(struct.pack('<H', cid) + name.encode() + b'\0'
                                                       for cid, name in entries)
    return (swf.SYMBOL_CLASS, tag(swf.SYMBOL_CLASS, body))


class MovieTests(unittest.TestCase):
    def test_an_abc_block_is_read_through_and_names_its_classes(self):
        self.assertEqual(swf.abc_definitions(abc('pkg.A', 'B')), (['pkg.A', 'B'], 2))
        with self.assertRaises(ValueError):
            swf.abc_definitions(abc('pkg.A') + b'\0')
        with self.assertRaises((ValueError, IndexError, struct.error)):
            swf.abc_definitions(abc('pkg.A')[:-3])

    def test_the_code_replaces_all_of_the_original_s_where_it_began(self):
        shape = (2, tag(2, b'shape'))
        original = movie([(69, tag(69, b'\x08\0\0\0')), shape, do_abc('a', 'A'), symbols((1, 'A')),
                          do_abc('b', 'pkg.B'), symbols((2, 'pkg.B')), (1, tag(1, b''))], compressed=True)
        code = [do_abc('x', 'A'), do_abc('y', 'pkg.B', 'pkg.C')]
        built = flash_tree.assemble(original, code)
        self.assertEqual(built[:3], b'CWS')
        _, tags = swf.split(built)
        self.assertEqual([k for k, _ in tags], [69, 2, 82, 82, 76, 76, 1, 0])
        self.assertEqual([swf.do_abc(raw)[1] for k, raw in tags if k == swf.DO_ABC], ['x', 'y'])
        self.assertEqual(swf.symbol_classes(tags), [(1, 'A'), (2, 'pkg.B')])
        self.assertEqual(flash_tree.check(built), 3)

    def test_every_bound_class_must_be_defined_once(self):
        original = movie([do_abc('a', 'A'), symbols((1, 'A'), (2, 'pkg.B'))])
        with self.assertRaisesRegex(ValueError, 'does not have: pkg.B'):
            flash_tree.check(flash_tree.assemble(original, [do_abc('x', 'A')]))
        with self.assertRaisesRegex(ValueError, 'more than once: A'):
            flash_tree.check(flash_tree.assemble(original, [do_abc('x', 'A', 'pkg.B'), do_abc('y', 'A')]))


    def test_images_are_defined_before_the_code_and_bound_after_it(self):
        original = movie([(2, tag(2, struct.pack('<H', 7) + b'shape')), do_abc('old', 'A')])
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, 'images').mkdir()
            Path(tmp, 'images', 'map.jpg').write_bytes(b'\xff\xd8jpeg\xff\xd9')
            Path(tmp, 'images', 'map.alpha').write_bytes(b'zlib-alpha')
            built = flash_tree.assemble(original, [do_abc('x', 'A', 'MapBitmap')], {'MapBitmap': 'images/map'}, Path(tmp))
        _, tags = swf.split(built)
        kinds = [kind for kind, _ in tags]
        self.assertEqual(kinds, [2, swf.DEFINE_BITS_JPEG3, swf.DO_ABC, swf.SYMBOL_CLASS, 0])
        self.assertEqual(swf.symbol_classes(tags), [(8, 'MapBitmap')])
        body = swf.payload(tags[1][1])
        self.assertEqual(struct.unpack_from('<HI', body), (8, 8))
        self.assertEqual(body[6:14], b'\xff\xd8jpeg\xff\xd9')
        self.assertEqual(body[14:], b'zlib-alpha')
        self.assertEqual(flash_tree.check(built), 2)


class TreeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tree = self.root / 'flash' / 'm'
        (self.tree / 'src').mkdir(parents=True)

    def spec(self, **values):
        (self.tree / 'movie.json').write_text(json.dumps(dict({'schema': 2, 'movie': 'flash_movies/m.swf',
                                                                'archive': 'resources.db.patch_9'}, **values)))

    def test_the_original_is_cut_from_its_archive_entry(self):
        archive = self.root / 'resources.db.patch_9'
        archive.write_bytes(b'xxxx' + b'MOVIE' + b'yyyy')
        self.spec(archive_offset=4, movie_size=5, movie_sha256=sha(b'MOVIE'))
        flash_tree.original(self.tree, archive, self.root / 'out.swf')
        self.assertEqual((self.root / 'out.swf').read_bytes(), b'MOVIE')
        with self.assertRaises(FileExistsError):
            flash_tree.original(self.tree, archive, self.root / 'out.swf')
        archive.write_bytes(b'z' * 13)
        with self.assertRaisesRegex(ValueError, 'does not hold'):
            flash_tree.original(self.tree, archive, self.root / 'other.swf')
        with self.assertRaisesRegex(ValueError, 'is in'):
            flash_tree.original(self.tree, self.root / 'resources.db', self.root / 'x.swf')
        self.spec(movie_sha256=sha(b'MOVIE'))
        with self.assertRaisesRegex(ValueError, 'no offset'):
            flash_tree.original(self.tree, archive, self.root / 'y.swf')

    def test_a_build_needs_the_original_movie_json_names(self):
        self.spec(movie_sha256=sha(b'MOVIE'))
        (self.root / 'o.swf').write_bytes(b'OTHER')
        with self.assertRaisesRegex(ValueError, 'Not the original'):
            flash_tree.build(self.tree, self.root / 'o.swf', None)

    @unittest.skipUnless(os.environ.get('SRP_AS_TOOLS'), 'set SRP_AS_TOOLS to run the real compiler')
    def test_a_tree_builds_whole_in_one_compiler_run(self):
        # A method put in the middle of a class, a try in a method, classes
        # naming each other and a changed signature its callers use: an
        # ordinary compile of the whole tree.
        tools = Path(os.environ['SRP_AS_TOOLS']).resolve()
        (self.tree / 'src/pkg').mkdir()
        (self.tree / 'src/Menu.as').write_text('''package {
    import flash.display.MovieClip;
    import pkg.Helper;
    public class Menu extends MovieClip {
        public var count:int = 1;
        public function show(a:int, owner:Object = null):int { return a + count; }
        public function inserted():int { try { return Helper.twice(count); } catch (e:Error) { } return 0; }
        public function last():String { return "last"; }
    }
}''')
        (self.tree / 'src/pkg/Helper.as').write_text('''package pkg {
    public class Helper {
        public static function twice(x:int):int { return Other.add(x, x); }
    }
}''')
        (self.tree / 'src/pkg/Other.as').write_text('''package pkg {
    public class Other {
        public static function add(a:int, b:int):int { return new Menu().show(a, Helper) + b; }
    }
}''')
        original = movie([(2, tag(2, b'shape')), do_abc('Menu', 'Menu'), symbols((1, 'Menu'))], compressed=True)
        (self.root / 'o.swf').write_bytes(original)
        self.spec(movie_sha256=sha(original))
        built = flash_tree.build(self.tree, self.root / 'o.swf', tools, report=lambda _: None)
        self.assertEqual(flash_tree.check(built), 3)
        self.assertEqual([k for k, _ in swf.split(built)[1]], [2, 82, 82, 82, 76, 0])
        self.assertEqual(built, flash_tree.build(self.tree, self.root / 'o.swf', tools, report=lambda _: None))
        (self.tree / 'src/pkg/Other.as').write_text('package pkg { public class Other { x } }')
        with self.assertRaisesRegex(ValueError, 'does not compile'):
            flash_tree.build(self.tree, self.root / 'o.swf', tools, report=lambda _: None)


if __name__ == '__main__':
    unittest.main()
