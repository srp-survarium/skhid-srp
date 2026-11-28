"""config_compact.py: a binary config laid out again keeps its tree and loses only unreached bytes."""
import struct
import sys
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import config_compact as cc  # noqa: E402

REC = cc.REC


class Config:
    """A binary config written record by record, as content_edit.py's appends leave one."""

    def __init__(self):
        self.buf = bytearray(24)            # the root, set by `root`

    def tail(self, data):
        at = len(self.buf)
        self.buf += data
        return at

    def records(self, recs):
        """Children side by side: each (name or None, kind, count, data)."""
        at = len(self.buf)
        self.buf += bytes(24 * len(recs))
        for i, (name, kind, count, data, ident) in enumerate(recs):
            crc = 0
            if name is not None:
                ident = self.tail(name + b'\0')
                crc = zlib.crc32(name) & 0xffffffff
            REC.pack_into(self.buf, at + 24 * i, data, ident, crc, kind, count)
        return at


def sample():
    c = Config()
    s = c.tail(b'hello\0')
    v = c.tail(struct.pack('<3f', 1, 2, 3))
    items = c.records([(None, 1, 4, 7, 0), (None, 1, 4, 8, 5)])     # an indexed child's id kept: 5, not 1
    old = c.records([(b'a', 1, 4, 1, 0)])                            # an earlier copy of the root's children
    c.tail(b'dead bytes, reached by nothing' * 10)
    names = sorted([b'a', b'name', b'pos', b'list', b'flag'], key=lambda n: zlib.crc32(n) & 0xffffffff)
    value = {b'a': (1, 4, 1 | 0xABCD << 32), b'name': (5, 6, s), b'pos': (7, 12, v), b'list': (4, 2, items),
             b'flag': (0, 4, 1)}
    root = c.records([(n, *value[n], 0) for n in names])
    REC.pack_into(c.buf, 0, root, 0, 0, 3, len(names))
    assert old
    return bytes(c.buf)


class Compact(unittest.TestCase):
    def test_same_tree_fewer_bytes(self):
        src = sample()
        out = cc.compact('x', src)
        self.assertEqual(cc.tree(out), cc.tree(src))
        self.assertLess(len(out), len(src))
        self.assertNotIn(b'dead bytes', out)

    def test_fields_kept(self):
        tree = cc.tree(cc.compact('x', sample()))
        children = {c[0]: c for c in tree[4]}
        self.assertEqual(children[b'a'][4], 1 | 0xABCD << 32)       # a scalar's whole 64 bits
        self.assertEqual([c[0] for c in children[b'list'][4]], [0, 5])  # indexed ids as they were
        self.assertEqual(children[b'name'][4], b'hello\0')
        self.assertEqual([c[0] for c in tree[4]], [c[0] for c in cc.tree(sample())[4]])  # the order

    def test_deterministic_and_settled(self):
        out = cc.compact('x', sample())
        self.assertEqual(cc.compact('x', sample()), out)
        self.assertEqual(cc.relay(out), out)

    def test_shared_bytes_once(self):
        c = Config()
        s1, s2 = c.tail(b'same\0'), c.tail(b'same\0')
        names = sorted([b'p', b'q'], key=lambda n: zlib.crc32(n) & 0xffffffff)
        root = c.records([(n, 5, 5, s1 if n == b'p' else s2, 0) for n in names])
        REC.pack_into(c.buf, 0, root, 0, 0, 3, 2)
        self.assertEqual(cc.compact('x', bytes(c.buf)).count(b'same\0'), 1)

    def test_a_changed_tree_is_seen(self):
        src = sample()
        broken = bytearray(src)
        at = src.index(b'hello')
        broken[at] = ord('j')
        self.assertNotEqual(cc.tree(bytes(broken)), cc.tree(src))

    def test_refused_when_the_tree_differs(self):
        relay = cc.relay
        try:
            cc.relay = lambda buf: relay(buf).replace(b'hello', b'jello')
            with self.assertRaises(ValueError):
                cc.compact('x', sample())
        finally:
            cc.relay = relay


if __name__ == '__main__':
    unittest.main()
