"""Reader and writer of the game's binary configs (client projects,
particles, materials, options), as patch_14 (London) has them. (The same
file as scripts/dynamic-anomalies/bcw.py of #225, which is not merged.)

A value is a 24-byte record `u64 data, u64 name offset, u32 crc32(name),
u16 type, u16 count`, the root first. A node here is (type, payload, count):

    0 bool (u32)        1 int (u32 bits)     2 float (u32 bits)
    3 named table [(name bytes, node)]       4 indexed table [node]
    5 string (bytes)    6/7/8 float2/3/4 (raw bytes)

`write` lays a tree out breadth first, a named table's children sorted by the
CRC-32 of their names (the game finds a key by binary search on it), names
written once each when their table is reached, string and vector bytes
written once each (strings and vectors apart): byte for byte the layout of patch_14's
London projects and particles (test_bcw.py). The original game files are
laid out otherwise (by their editor); written again they read the same.

Python view (load/dump): dict = named table (key order free, the writer
sorts), list = indexed table, bool, int (i32), float (f32), str (cp1251),
tuple of 2/3/4 floats = float2/3/4.
"""
import collections
import struct
import zlib

RECORD = struct.Struct("<QQIHH")
BOOL, INT, FLOAT, NAMED, INDEXED, STRING = range(6)


def read(data):
    """The tree of a binary config, as nodes."""
    def name_at(at):
        return bytes(data[at:data.index(b"\0", at)])

    def value(at):
        payload, _, _, kind, count = RECORD.unpack_from(data, at)
        if kind in (BOOL, INT, FLOAT):
            return (kind, payload & 0xFFFFFFFF, count)
        if kind == STRING:
            return (kind, bytes(data[payload:payload + count - 1]), count)
        if kind in (6, 7, 8):
            return (kind, bytes(data[payload:payload + 4 * (kind - 4)]), count)
        if kind == NAMED:
            return (kind, [(name_at(RECORD.unpack_from(data, payload + 24 * i)[1]), value(payload + 24 * i))
                           for i in range(count)], count)
        if kind == INDEXED:
            return (kind, [value(payload + 24 * i) for i in range(count)], count)
        raise ValueError(f"value type {kind}")
    return value(0)


def count(node):
    """The number of values in a tree."""
    if node[0] == NAMED:
        return 1 + sum(count(v) for _, v in node[1])
    if node[0] == INDEXED:
        return 1 + sum(count(v) for v in node[1])
    return 1


def write(root):
    """The bytes of a tree (see the module's text for the layout)."""
    out = bytearray(24 * count(root))
    names, blobs = {}, {}

    def name(key):
        if key not in names:
            names[key] = (len(out), zlib.crc32(key))
            out.extend(key + b"\0")
        return names[key]

    def blob(data, table):
        if (table, data) not in blobs:
            blobs[(table, data)] = len(out)
            out.extend(data)
        return blobs[(table, data)]

    queue = collections.deque([(root, 0, 0)])
    index = 0
    while queue:
        node, name_offset, crc = queue.popleft()
        kind = node[0]
        if kind == BOOL:
            data, size = (1 if node[1] else 0), (node[2] if len(node) > 2 else 1)
        elif kind in (INT, FLOAT):
            data, size = node[1], 4
        elif kind == STRING:
            data, size = blob(node[1] + b"\0", "string"), len(node[1]) + 1
        elif kind in (6, 7, 8):
            data, size = blob(node[1], "vector"), 4 * (kind - 4)
        elif kind == NAMED:
            data, size = (index + len(queue) + 1) * 24, len(node[1])
            for key, child in sorted(node[1], key=lambda item: zlib.crc32(item[0])):
                queue.append((child, *name(key)))
        else:
            data, size = (index + len(queue) + 1) * 24, len(node[1])
            for i, child in enumerate(node[1]):
                queue.append((child, i, 0))
        RECORD.pack_into(out, 24 * index, data, name_offset, crc, kind, size)
        index += 1
    return bytes(out)


def to_py(node):
    kind, payload = node[0], node[1]
    if kind == BOOL:
        return bool(payload)
    if kind == INT:
        return struct.unpack("<i", struct.pack("<I", payload))[0]
    if kind == FLOAT:
        return struct.unpack("<f", struct.pack("<I", payload))[0]
    if kind == STRING:
        return payload.decode("cp1251")
    if kind in (6, 7, 8):
        return tuple(struct.unpack(f"<{kind - 4}f", payload))
    if kind == NAMED:
        return {key.decode("cp1251"): to_py(child) for key, child in payload}
    return [to_py(child) for child in payload]


def from_py(value):
    if isinstance(value, bool):
        return (BOOL, int(value), 4)
    if isinstance(value, int):
        return (INT, value & 0xFFFFFFFF, 4)
    if isinstance(value, float):
        return (FLOAT, struct.unpack("<I", struct.pack("<f", value))[0], 4)
    if isinstance(value, str):
        data = value.encode("cp1251")
        return (STRING, data, len(data) + 1)
    if isinstance(value, tuple):
        return (4 + len(value), struct.pack(f"<{len(value)}f", *value), 4 * len(value))
    if isinstance(value, dict):
        return (NAMED, [(key.encode("cp1251"), from_py(child)) for key, child in value.items()], len(value))
    if isinstance(value, list):
        return (INDEXED, [from_py(child) for child in value], len(value))
    raise TypeError(type(value))


def loads(data):
    return to_py(read(data))


def load(path):
    with open(path, "rb") as file:
        return loads(file.read())


def dump(value):
    return write(from_py(value))
