"""A binary config laid out again from its record tree, without the bytes no record reaches.

content_edit.py edits a config in place: a value that grows (a table with a new child, a longer
string) is written again at the file's end (its `append`), and the old copy stays where it was,
reached by nothing. The game reads a config whole into the resources' unmanaged arena, so those
bytes stay in the arena while the config is loaded: rev75's gameplay/db_static_dictionaries is
65,017,848 bytes of which its tree reaches 947,429, and each of Vector Laboratory's
(level_07) projects about 29 MiB of which about 1.4 MiB.

`compact` writes the same tree in the layout the game's own files have (binary-config-parser's
write.rs): the root record at 0, every table's children side by side in the order a breadth-first
walk reaches the tables, then, slot by slot, a named child's name and a string's or float vector's
bytes, one copy of each (the game's own files share them too: 624 of stock level_07/day_2's
15,531 strings and vectors point at bytes another points at; none is aligned). Every field of every record is kept as the file has it (a
scalar's 64-bit data, an indexed child's id, every name's CRC, every type and count) except the
offsets, which point at the new places. `tree` reads a file into a value with no offset in it,
and `compact` refuses any result whose tree is not the source's, so nothing the game reads
changes: the same keys, values, types and order.

Pure: the same bytes give the same bytes.
"""
import struct

REC = struct.Struct('<QQIHH')   # data, name offset (an indexed child's id), name CRC, type, count
NAMED, INDEXED, STRING = 3, 4, 5


def _tail(buf, data, kind, count):
    """The bytes a string (its count, NUL included) or a float vector (types 6-8) points at."""
    if kind == STRING:
        return bytes(buf[data:data + count])
    if kind > STRING:
        return bytes(buf[data:data + 4 * (kind - 4)])
    return None


def tree(buf):
    """The file's value tree with every offset resolved: per record (name or id, CRC, type,
    count, the scalar's data or the bytes it points at, its children)."""
    def value(off, named_child):
        data, ident, crc, kind, count = REC.unpack_from(buf, off)
        if named_child:
            end = buf.index(b'\0', ident)
            ident = bytes(buf[ident:end])
        if kind in (NAMED, INDEXED):
            return (ident, crc, kind, count, tuple(value(data + 24 * i, kind == NAMED) for i in range(count)))
        tail = _tail(buf, data, kind, count)
        return (ident, crc, kind, count, data if tail is None else tail)
    return value(0, False)


def relay(buf):
    """The tree of `buf` in the game's layout, keeping every field but the offsets."""
    slots = [(0, False)]            # (source offset, whether a named table's child)
    first = {}
    at = 0
    while at < len(slots):          # breadth first, as write.rs
        off, _ = slots[at]
        data, _, _, kind, count = REC.unpack_from(buf, off)
        if kind in (NAMED, INDEXED):
            first[at] = len(slots)
            slots.extend((data + 24 * i, kind == NAMED) for i in range(count))
        at += 1
    out = bytearray(24 * len(slots))
    names, tails = {}, {}
    for at, (off, named_child) in enumerate(slots):
        data, ident, crc, kind, count = REC.unpack_from(buf, off)
        if named_child:
            name = bytes(buf[ident:buf.index(b'\0', ident)])
            if name not in names:
                names[name] = len(out)
                out += name + b'\0'
            ident = names[name]
        if kind in (NAMED, INDEXED):
            data = 24 * first[at]
        else:
            tail = _tail(buf, data, kind, count)
            if tail is not None:
                if tail not in tails:
                    tails[tail] = len(out)
                    out += tail
                data = tails[tail]
        REC.pack_into(out, 24 * at, data, ident, crc, kind, count)
    return bytes(out)


def compact(path, buf):
    """`buf` laid out again; refused (ValueError) unless its tree is the source's."""
    out = relay(buf)
    if tree(out) != tree(buf):
        raise ValueError(f'{path}: the compacted tree differs from the source\'s')
    return out
