"""The movie build: one compile of the whole tree, its code in place of the original's.

`SRP_AS_TOOLS=/path/to/tools.json` adds a real compc build of a small tree.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

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


def png(width, height, rgba, kind=0, colour=6):
    """A PNG of the pixels, every row filtered by `kind` (0 none, 1 sub, 2 up, 3 average, 4 Paeth)."""
    channels = 4 if colour == 6 else 3
    stride = width * channels
    raw, previous = b'', bytes(stride)
    for y in range(height):
        line = rgba[y * stride:(y + 1) * stride]
        out = bytearray()
        for x in range(stride):
            left = line[x - channels] if x >= channels else 0
            up, corner = previous[x], previous[x - channels] if x >= channels else 0
            p = left + up - corner
            paeth = left if abs(p - left) <= abs(p - up) and abs(p - left) <= abs(p - corner) \
                else up if abs(p - up) <= abs(p - corner) else corner
            out.append((line[x] - (0, left, up, (left + up) // 2, paeth)[kind]) & 0xff)
        raw += bytes([kind]) + bytes(out)
        previous = line

    def chunk(name, body):
        return struct.pack('>I', len(body)) + name + body + struct.pack('>I', zlib.crc32(name + body))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, colour, 0, 0, 0)) \
        + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')


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

    def test_a_png_is_read_through_every_filter(self):
        rgba = bytes(range(2 * 3 * 4))
        for kind in range(5):
            self.assertEqual(swf.png(png(2, 3, rgba, kind)), (2, 3, rgba))
        self.assertEqual(swf.png(png(1, 1, b'\x01\x02\x03', 0, colour=2)), (1, 1, b'\x01\x02\x03\xff'))
        with self.assertRaisesRegex(ValueError, 'bad CRC'):
            swf.png(png(1, 1, b'abcd')[:-5] + b'\0' + png(1, 1, b'abcd')[-4:])

    def test_a_bitmap_is_added_bound_to_its_class(self):
        shape = (2, tag(2, struct.pack('<H', 7) + b'shape'))
        original = movie([shape, do_abc('a', 'A'), symbols((7, 'A')), (1, tag(1, b''))])
        built = flash_tree.assemble(original, [do_abc('x', 'A', 'Icon')], [flash_tree.Lossless('Icon', 1, 1, b'\x10\x20\x30\x80')])
        _, tags = swf.split(built)
        self.assertEqual([k for k, _ in tags], [2, 36, 82, 76, 76, 1, 0])
        body = swf.payload(tags[1][1])
        self.assertEqual(struct.unpack_from('<HBHH', body), (8, 5, 1, 1))
        self.assertEqual(zlib.decompress(body[7:]), bytes((0x80, 0x08, 0x10, 0x18)))   # ARGB, premultiplied
        self.assertEqual(swf.symbol_classes(tags), [(8, 'Icon'), (7, 'A')])
        self.assertEqual(flash_tree.check(built), 2)
        with self.assertRaisesRegex(ValueError, 'already binds'):
            flash_tree.assemble(original, [], [flash_tree.Lossless('A', 1, 1, b'\0\0\0\0')])

    def test_a_jpeg_is_added_whole_and_must_be_baseline(self):
        # a 3 x 2 baseline JPEG, and the same progressive (ImageMagick)
        baseline = zlib.decompress(base64.b64decode(
            'eNqVjjsSwjAMRFf+kI/tYAU7pKbnDBTMZIZcjUtRcAg+BdzE2CWfhqetdlZapUt6ws/TYQIRQHmQ7thDClGUUVm61lop3VbVorattaY1xnW9dx13xvjB8yrEGO1yPQ5h7EMM5QjJvKN0o3UTnHHhb9IJXENASmIIJsmUzgjlyTeq4vpPF8Wln1n+zt5gJeUSydhhsz0+0vUFtV0f5g=='))
        progressive = zlib.decompress(base64.b64decode(
            'eNqVj0sOwjAMRO24KW2TFgfSwhJO0gUSElyql2KPhDgEnwXcJDgLkPhWjL0aPdvjcAhX4PVytQREAJSCcIYFkFKxRYm0zrROEl0MBmlmC2tNYUxZjbisXGUMN+zGvq5rO5xMGz8d+drHJUgyk+hc69yXpvR/K+zAZaCACB0oh+Qw7MHHkE9Ko4vv7gksISgmltdmm23E+BUDwTIZRq26CLjPACG2+AtQD+DribRVvwFs5909NZD4fOmJxH2RuO+iAMcb7BxDDA=='))
        self.assertEqual(swf.jpeg(9, baseline)[1], (3, 2))
        with self.assertRaisesRegex(ValueError, 'baseline'):
            swf.jpeg(9, progressive)
        with self.assertRaisesRegex(ValueError, 'not a JPEG'):
            swf.jpeg(9, baseline[:-2])
        shape = (2, tag(2, struct.pack('<H', 7) + b'shape'))
        original = movie([shape, do_abc('a', 'A'), symbols((7, 'A')), (1, tag(1, b''))])
        built = flash_tree.assemble(original, [do_abc('x', 'A', 'Picture')], [flash_tree.Jpeg('Picture', baseline)])
        _, tags = swf.split(built)
        self.assertEqual([k for k, _ in tags], [2, 21, 82, 76, 76, 1, 0])
        self.assertEqual(swf.payload(tags[1][1]), struct.pack('<H', 8) + baseline)
        self.assertEqual(swf.symbol_classes(tags), [(8, 'Picture'), (7, 'A')])

    def test_every_bound_class_must_be_defined_once(self):
        original = movie([do_abc('a', 'A'), symbols((1, 'A'), (2, 'pkg.B'))])
        with self.assertRaisesRegex(ValueError, 'does not have: pkg.B'):
            flash_tree.check(flash_tree.assemble(original, [do_abc('x', 'A')]))
        with self.assertRaisesRegex(ValueError, 'more than once: A'):
            flash_tree.check(flash_tree.assemble(original, [do_abc('x', 'A', 'pkg.B'), do_abc('y', 'A')]))


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

    def table(self, text, path='pkg/data/Rows.tsv'):
        (self.tree / 'tables' / path).parent.mkdir(parents=True, exist_ok=True)
        (self.tree / 'tables' / path).write_text(text)

    def test_a_table_is_a_class_of_its_lists(self):
        self.table('# a comment\nlist\ticon:int\ttip:String\tkind:pkg.Kind\n'
                   'FIRST\t3\tfilter_a\tA_KIND\nSECOND\t-1\tsay "b"\tB_KIND\nFIRST\t0\t\tA_KIND\n')
        self.assertEqual(flash_tree.tables(self.tree), {Path('pkg/data/Rows.as'): '''package pkg.data
{
   import pkg.Kind;
   // Made by the build (tools/actionscript/flash_tree.py) from tables/pkg/data/Rows.tsv: edit that.
   public class Rows
   {
      public static const FIRST:Array = [
         {"icon":3,"tip":"filter_a","kind":Kind.A_KIND},
         {"icon":0,"tip":"","kind":Kind.A_KIND}
      ];
      public static const SECOND:Array = [
         {"icon":-1,"tip":"say \\"b\\"","kind":Kind.B_KIND}
      ];
   }
}
'''})
        for text, error in (('list\ticon:int\nA\tx\n', "icon 'x' is not a int"),
                            ('list\tkind:pkg.Kind\nA\tB.C\n', "is not a pkg.Kind"),
                            ('list\ticon\nA\t1\n', 'the header is list'),
                            ('list\ticon:int\nA\t1\t2\n', 'rows of as many columns'),
                            ('list\ticon:int\nnot a name\t1\n', 'is not a name')):
            self.table(text)
            with self.assertRaisesRegex(ValueError, error):
                flash_tree.tables(self.tree)
        self.table('list\ticon:int\nA\t1\n')
        (self.tree / 'src/pkg/data').mkdir(parents=True)
        (self.tree / 'src/pkg/data/Rows.as').write_text('')
        with self.assertRaisesRegex(ValueError, 'a class of the tree already'):
            flash_tree.tables(self.tree)

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
