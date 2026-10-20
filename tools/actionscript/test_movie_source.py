import csv
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import movie_source as movie


def tag(kind, payload, long=False):
    if long or len(payload) >= 63:
        return struct.pack("<HI", kind << 6 | 63, len(payload)) + payload
    return struct.pack("<H", kind << 6 | len(payload)) + payload


def swf(tags):
    # RECT with zero-width fields, frame rate, frame count.
    return movie.join_movie(b"FWS\x0b\0\0\0\0\0\0\x18\x01\0", tags)


class MovieTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def file(self, name, data):
        path = self.root / name
        path.write_bytes(data)
        return path

    def test_transplant_preserves_unknown_scaleform_tags_and_encoding(self):
        tags = [(69, tag(69, b"\x08\0\0\0")),
                (1000, tag(1000, b"opaque Scaleform data", long=True)),
                (82, tag(82, b"original abc", long=True)), (0, tag(0, b""))]
        original = self.file("original.swf", swf(tags))
        code = self.root / "code.swf"
        movie.code_movie(original, code)
        self.assertEqual([k for k, _ in movie.split_movie(code)[1]], [69, 82, 0])
        compiled = self.file("compiled.swf", swf([(82, tag(82, b"new abc")), (0, tag(0, b""))]))
        output = self.root / "output.swf"
        movie.transplant(original, compiled, output)
        actual = movie.split_movie(output)[1]
        self.assertEqual(actual[0], tags[0])
        self.assertEqual(actual[1], tags[1])
        self.assertEqual(actual[2], (82, tag(82, b"new abc")))
        self.assertEqual(actual[3], tags[3])

    def test_transplant_refuses_missing_abc(self):
        original = self.file("original", swf([(82, tag(82, b"abc")), (0, tag(0, b""))]))
        compiled = self.file("compiled", swf([(0, tag(0, b""))]))
        with self.assertRaisesRegex(ValueError, "block count"):
            movie.transplant(original, compiled, self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_compression_is_reported_separately(self):
        raw = swf([(0, tag(0, b""))])
        plain = self.file("plain", raw)
        compressed = self.file("compressed", b"CWS" + raw[3:8] + zlib.compress(raw[8:]))
        self.assertEqual(movie.uncompressed(plain), movie.uncompressed(compressed))
        self.assertNotEqual(plain.read_bytes(), compressed.read_bytes())
        with self.assertRaises(ValueError):
            movie.uncompressed(self.file("trailing", compressed.read_bytes() + b"trailer"))

    def snapshot(self, name, abc, record="same"):
        folder = self.root / name
        (folder / "abc-0000").mkdir(parents=True)
        (folder / "abc-0000/block.abc").write_bytes(abc)
        fields = ["abc", "method", "owner", "code_size", "code_sha256", "record_sha256", "assembly_sha256"]
        with (folder / "methods.tsv").open("w") as stream:
            writer = csv.writer(stream, delimiter="\t")
            writer.writerow(fields)
            writer.writerow([0, 0, "C/m", 1, "same", record, "same"])
        return folder

    def test_unchanged_method_code_cannot_hide_changed_constant_pool(self):
        before = self.snapshot("before", b"pool: old")
        after = self.snapshot("after", b"pool: new")
        a = self.file("a", swf([(82, tag(82, b"pool: old")), (0, tag(0, b""))]))
        b = self.file("b", swf([(82, tag(82, b"pool: new")), (0, tag(0, b""))]))
        result = movie.compare(a, b, before, after)
        self.assertFalse(result["movie_exact"])
        self.assertEqual(result["method_changes"], [])
        self.assertEqual(result["changed_abc_blocks"], ["abc-0000"])

    def test_method_metadata_is_reported_even_with_identical_instructions(self):
        before = self.snapshot("before", b"old", "old-stack")
        after = self.snapshot("after", b"new", "new-stack")
        a = self.file("a", swf([(82, tag(82, b"old")), (0, tag(0, b""))]))
        b = self.file("b", swf([(82, tag(82, b"new")), (0, tag(0, b""))]))
        result = movie.compare(a, b, before, after)
        self.assertEqual(result["method_changes"][0]["fields"], ["record_sha256"])
        self.assertFalse(result["movie_exact"])

    def test_pristine_sources_cannot_be_silently_edited(self):
        (self.root / "pristine").mkdir()
        source = self.file("pristine/C.as", b"class C {}")
        original = self.file("original.swf", b"pinned bytes")
        movie.write_json(self.root / "manifest.json", {"movie_sha256": movie.digest(original),
                         "sources": {"C.as": movie.digest(source)}})
        movie.verify_project(self.root)
        source.write_text("class C { /* edit */ }")
        with self.assertRaisesRegex(ValueError, "Pristine"):
            movie.verify_project(self.root)


if __name__ == "__main__":
    unittest.main()
