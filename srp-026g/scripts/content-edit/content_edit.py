#!/usr/bin/env python3
"""content_edit.py: make a resources.db patch folder by editing game configs in place.

usage: python3 -I content_edit.py <edits.json> <live folder> <out folder> [--check]

<live folder>  the live game files, at their game paths (gameplay/items/...) or flattened as
               `dbgrab <server-game-data.db> get gameplay/ <dir>` writes them (gameplay__items__...).
               Take them from the live server game data, which holds the files the players' game
               mounts (stock resources.db with every offered patch over it).
<out folder>   gets every edited file at its game path, for `release/build-patch <out> <rev>`.
               Must not exist yet.
--check        edit in memory and report, write nothing.

A Vostok binary config has no header: the root value is at 0, every value is 24 bytes
(+0 data or the offset of its children, +8 offset of its name, +0x10 CRC32 of the name, +0x14 u16 type,
+0x16 u16 count), names and strings after the values. The game's lookup (0xF6B10 -> 0xF6D50)
binary-searches a table's children by name CRC and has no end check (srp-026g/docs/notes/replay_sur.md),
so the files are never re-made: every edit changes the fewest bytes possible and a renamed key's
table is re-sorted by CRC.

Edits (edits.json):
  "set":            [{"file", "path", "from", "to"}]   a number (float or int, its type kept); `from` must
                                                       be the live value (a stale base is refused); the
                                                       path's `name[i]` is item i of an indexed table
  "rename_key":     [{"files" (glob on the game path), "table", "from", "to"}]
                    renames a key of the named table in every matching file that has it; the new name
                    is appended after the file's last byte, the table's children re-sorted by CRC
  "set_prop":       [{"file", "prop_id", "from", "to"}] the lobby card's text of a stat (ui_desc/props_list:
                                                       1 damage x100, 3 armour piercing x100, 4 rate of fire)
  "replace_string": [{"files", "from", "to"}]          every string value equal to `from`, appended
  "edit_text":      [{"file", "path", "from", "to"}]   in one text (UTF-8, as the localization tables
                                                       keep it), its one `from` becomes `to`
  "add_entry":      [{"files", "list", "after", "like", "values", "only_if"}]
                    a new entry of the indexed table `list` (a lobby card's `ui_desc/props_list`, the
                    card lines' `ui_properties`) after the entry whose keys match `after`, made like
                    the entry matching `like` with `values` set; a value "{a/b}" is that number of
                    the same file, a list of numbers a vector (a position), a list of vectors an
                    indexed table of vectors (a wire's points) made anew; files without `only_if`,
                    the table or those entries are left alone
  "add_key":        [{"file", "table", "key", "text"}] a new text of a localization table
  "add_value":      [{"files" (a list of game paths), "table" ("" the root), "key", "value"}]
                    a new key of the named table in each file, in its place by name CRC: a bool,
                    an int, a float (a number with a point), a text, or an object (a named table of
                    those); a file that has the key already is refused
  "remove_item":    [{"file", "path", "value"}]        the one item equal to `value` of an indexed table of
                                                       numbers (an item's `default_modifications`) or of
                                                       strings (a weapon's `death` animations); the
                                                       later items move down in place, the count drops by 1
  "copy":           [{"files", "from", "to"}]          every live file the glob matches (its path starts
                                                       with `from`) is also a new file at `to` + the rest
                                                       of its path, a path no live file has; applied first,
                                                       the globs below see the copies
  "scale":          [{"files", "path", "by"}]          the number at `path` of every matching file times
                                                       `by`, as a float (a matching file without it is
                                                       refused)
  "prefix_string":  [{"files", "key", "prefix"}]       every string value named `key` of the matching files
                                                       gets `prefix` in front, appended
  "set_entry":      [{"file", "list", "where", "from", "to"}]
                    the one entry of the indexed table `list` whose keys match `where`: each key of
                    `from` must hold that value, then takes the value in `to` (a number keeps its type,
                    a string is appended)
  "remove_entry":   [{"file", "list", "where"}]        the one entry of the indexed table `list` whose keys
                                                       match `where` taken out (a scene's object, model or
                                                       collision); the later entries move down in place, the
                                                       count drops by 1. Applied last: the paths of the edits
                                                       above are the live file's
  "add_event_channel": [{"file", "from", "add"}]       an animation clip (`.b-spline`, not a config): `from`
                                                       is its live event channels, `add` one more after them,
                                                       each {"name", "type", "knots", "domains"}; the bone
                                                       tracks stay byte for byte (see clip_parse)
In `where` (and set_entry's `from`) a float, or a vector (a position: a list of numbers), matches
within 0.0005, as the editor's text (`client_project.lua`) prints them; ints and strings exactly.
A copy is read back against the file it was copied from: byte for byte when nothing edits it.
New records and arrays are appended (arrays at 8-byte boundaries, as the files keep them); a table
that grows gets a new array and the old one stays, unreferenced.

Every edited file is read back with an independent walk: only the intended values, names and order
may differ from the live file (the check prints each difference).
"""
import fnmatch
import json
import os
import struct
import sys
import zlib

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5
REC = struct.Struct('<QQIHH')


def rec(buf, off):
    return REC.unpack_from(buf, off)


def name_at(buf, off):
    return bytes(buf[off:buf.index(b'\0', off)])


def children(buf, off):
    data, _, _, t, cnt = rec(buf, off)
    assert t in (T_NAMED, T_INDEXED), 'not a table'
    return [data + 24 * i for i in range(cnt)]


def find(buf, path):
    """Offset of the value at a/b/c; `name[i]` is item i of an indexed table (states[0]/take_time)."""
    off = 0
    for part in path.split('/'):
        name, *indices = part.split('[')
        for c in children(buf, off):
            if name_at(buf, rec(buf, c)[1]) == name.encode():
                off = c
                break
        else:
            raise KeyError(path)
        for i in indices:
            items, i = children(buf, off), int(i.rstrip(']'))
            if rec(buf, off)[3] != T_INDEXED or i >= len(items):
                raise KeyError(path)
            off = items[i]
    return off


def walk(buf, off=0, path=''):
    """Every value as (path, type, value) in file order, names resolved; floats as bits."""
    data, idoff, crc, t, cnt = rec(buf, off)
    out = []
    if t in (T_NAMED, T_INDEXED):
        out.append((path, t, cnt))
        for i, c in enumerate(children(buf, off)):
            cd, cid, ccrc, ct, ccnt = rec(buf, c)
            if t == T_NAMED:
                nm = name_at(buf, cid)
                assert zlib.crc32(nm) & 0xffffffff == ccrc, (path, nm)
                out += walk(buf, c, path + '/' + nm.decode())
            else:
                out += walk(buf, c, '%s[%d]' % (path, i))
        if t == T_NAMED:
            crcs = [rec(buf, c)[2] for c in children(buf, off)]
            assert crcs == sorted(crcs), 'children not sorted by CRC at ' + (path or '/')
    elif t == T_STR:
        out.append((path, t, bytes(buf[data:data + cnt - 1])))
    elif t in (T_BOOL, T_INT, T_FLOAT):
        out.append((path, t, data & 0xffffffff))
    else:
        out.append((path, t, bytes(buf[data:data + 4 * (t - 4)]) if t > 5 else data))
    return out


def as_number(t, raw):
    if t == T_FLOAT:
        return struct.unpack('<f', struct.pack('<I', raw))[0]
    if t == T_INT:
        return struct.unpack('<i', struct.pack('<I', raw))[0]
    raise TypeError('not a number')


def set_number(buf, path, frm, to, as_float=False):
    """An int value keeps its type unless `as_float` (the edit's "as_float": true) makes it a float:
    the game's reader takes either for a number (the same key is an int in some configs, a float in
    others), but a fraction is never silently cut to an int."""
    off = find(buf, path)
    data, idoff, crc, t, cnt = rec(buf, off)
    old = as_number(t, data & 0xffffffff)
    if abs(old - frm) > 1e-6:
        raise ValueError('%s is %r, not %r: the base is not the live data' % (path, old, frm))
    if t == T_INT and as_float:
        t = T_FLOAT
    if t == T_INT and to != int(to):
        raise ValueError('%s is an int; %r needs "as_float": true' % (path, to))
    raw = struct.unpack('<I', struct.pack('<f' if t == T_FLOAT else '<i', to if t == T_FLOAT else int(to)))[0]
    REC.pack_into(buf, off, (data & ~0xffffffff) | raw, idoff, crc, t, cnt)


def append(buf, blob):
    off = len(buf)
    buf.extend(blob)
    return off


def rename_key(buf, table, frm, to):
    try:
        toff = find(buf, table)
    except KeyError:
        return False
    kids = children(buf, toff)
    hit = [c for c in kids if name_at(buf, rec(buf, c)[1]) == frm.encode()]
    if not hit:
        return False
    assert not any(name_at(buf, rec(buf, c)[1]) == to.encode() for c in kids), 'both names in ' + table
    noff = append(buf, to.encode() + b'\0')
    d, _, _, t, n = rec(buf, hit[0])
    REC.pack_into(buf, hit[0], d, noff, zlib.crc32(to.encode()) & 0xffffffff, t, n)
    recs = sorted((bytes(buf[c:c + 24]) for c in kids), key=lambda r: REC.unpack(r)[2])
    for c, r in zip(kids, recs):
        buf[c:c + 24] = r
    return True


def value_offsets(buf, off=0):
    """Every value's offset, the root first."""
    out = [off]
    t = rec(buf, off)[3]
    if t in (T_NAMED, T_INDEXED):
        for c in children(buf, off):
            out += value_offsets(buf, c)
    return out


def replace_string(buf, frm, to):
    n = 0
    noff = None
    for off in value_offsets(buf):
        d, i, c, t, cnt = rec(buf, off)
        if t == T_STR and bytes(buf[d:d + cnt - 1]) == frm.encode():
            if noff is None:
                noff = append(buf, to.encode() + b'\0')
            REC.pack_into(buf, off, noff, i, c, t, len(to.encode()) + 1)
            n += 1
    return n


def edit_text(buf, path, frm, to):
    """A text (a description in a localization table): its one `frm` becomes `to`, the new text
    appended. A text without `frm`, or with it twice, is refused."""
    off = find(buf, path)
    d, i, c, t, cnt = rec(buf, off)
    if t != T_STR:
        raise ValueError('%s is not a text' % path)
    old = bytes(buf[d:d + cnt - 1])
    if old.count(frm.encode()) != 1:
        raise ValueError('%s has %r %d times, not once: the base is not the live data'
                         % (path, frm, old.count(frm.encode())))
    new = old.replace(frm.encode(), to.encode())
    REC.pack_into(buf, off, append(buf, new + b'\0'), i, c, t, len(new) + 1)


def append_array(buf, recs):
    """24-byte records appended at an 8-byte boundary, as the game's files keep their arrays."""
    buf.extend(bytes(-len(buf) % 8))
    return append(buf, b''.join(recs))


def child(buf, table_off, name):
    for c in children(buf, table_off):
        if name_at(buf, rec(buf, c)[1]) == name.encode():
            return c
    return None


def value_of(buf, off):
    d, _, _, t, cnt = rec(buf, off)
    if t == T_STR:
        return bytes(buf[d:d + cnt - 1]).decode()
    if t > T_STR:
        return list(struct.unpack('<%df' % (t - 4), bytes(buf[d:d + 4 * (t - 4)])))
    return as_number(t, d & 0xffffffff) if t != T_BOOL else d & 1


def same(got, want):
    """A float or a vector within 0.0005 of `want` (the editor prints three decimals); else equal."""
    if isinstance(got, list):
        return isinstance(want, list) and len(got) == len(want) and all(same(g, w) for g, w in zip(got, want))
    if isinstance(got, float) and isinstance(want, (int, float)) and not isinstance(want, bool):
        return abs(got - want) <= 5e-4
    return got == want


def matches(buf, entry, want):
    for k, v in want.items():
        c = child(buf, entry, k)
        if c is None or not same(value_of(buf, c), v):
            return False
    return True


def filled(buf, text):
    """`{a/b}` in a value: that number of the same file, as the game's cards write numbers."""
    if not (isinstance(text, str) and text.startswith('{') and text.endswith('}')):
        return text
    v = value_of(buf, find(buf, text[1:-1]))
    return str(int(v)) if v == int(v) else '%g' % v


def add_entry(buf, path, after, like, values):
    """A new entry of the indexed table `path` after the one matching `after`: the entry matching
    `like` copied (its keys, types and order), the keys of `values` set. Its records and the table's
    new array are appended; the old array stays, unreferenced. Returns the new entry's index, or None
    when the file has no such table or entry."""
    try:
        lst = find(buf, path)
    except KeyError:
        return None
    kids = children(buf, lst)
    at = [i for i, e in enumerate(kids) if matches(buf, e, after)]
    model = [e for e in kids if matches(buf, e, like)]
    if not at or not model:
        return None
    assert len(at) == 1 and len(model) == 1, (path, after, like)
    names = {name_at(buf, rec(buf, c)[1]).decode(): c for c in children(buf, model[0])}
    assert set(values) <= set(names), ('keys the model entry lacks', set(values) - set(names))
    recs = []
    for c in children(buf, model[0]):
        d, i, crc, t, cnt = rec(buf, c)
        name = name_at(buf, i).decode()
        if name in values:
            v = filled(buf, values[name])
            if t == T_INDEXED:
                items = children(buf, c)
                if not items or not isinstance(v, list) or not all(isinstance(x, list) for x in v):
                    raise ValueError('%s: %s takes a list of vectors' % (path, name))
                _, ii, icrc, it, icnt = rec(buf, items[0])
                if it <= T_STR or any(len(x) != it - 4 for x in v):
                    raise ValueError('%s: %s holds vectors of %d numbers' % (path, name, it - 4))
                d = append_array(buf, [REC.pack(append(buf, struct.pack('<%df' % len(x), *x)), ii, icrc, it, icnt)
                                       for x in v])
                cnt = len(v)
            elif t > T_STR:
                if not isinstance(v, list) or len(v) != t - 4:
                    raise ValueError('%s: %s takes %d numbers' % (path, name, t - 4))
                d = append(buf, struct.pack('<%df' % len(v), *v))
            elif t == T_STR:
                v = v.encode()
                d, cnt = append(buf, v + b'\0'), len(v) + 1
            elif t == T_INT:
                if v != int(v):
                    raise ValueError('%s: %s is an int; %r is not' % (path, name, v))
                d = (d & ~0xffffffff) | (int(v) & 0xffffffff)
            elif t == T_FLOAT:
                d = (d & ~0xffffffff) | struct.unpack('<I', struct.pack('<f', v))[0]
            else:
                d = (d & ~0xffffffff) | (1 if v else 0)
        recs.append(REC.pack(d, i, crc, t, cnt))
    d, i, crc, t, cnt = rec(buf, model[0])
    entry = REC.pack(append_array(buf, recs), i, crc, t, cnt)
    old = [bytes(buf[c:c + 24]) for c in kids]
    new = old[:at[0] + 1] + [entry] + old[at[0] + 1:]
    d, i, crc, t, cnt = rec(buf, lst)
    REC.pack_into(buf, lst, append_array(buf, new), i, crc, t, cnt + 1)
    return at[0] + 1


def add_key(buf, table, key, text):
    """A new text `key` of the named table `table`, in its place by name CRC."""
    toff = find(buf, table)
    kids = [bytes(buf[c:c + 24]) for c in children(buf, toff)]
    crc = zlib.crc32(key.encode()) & 0xffffffff
    assert all(REC.unpack(r)[2] != crc for r in kids), (table, key, 'its CRC is taken')
    name = append(buf, key.encode() + b'\0')
    t = text.encode()
    new = sorted(kids + [REC.pack(append(buf, t + b'\0'), name, crc, T_STR, len(t) + 1)],
                 key=lambda r: REC.unpack(r)[2])
    d, i, c, ty, cnt = rec(buf, toff)
    REC.pack_into(buf, toff, append_array(buf, new), i, c, ty, cnt + 1)


def value_record(buf, name_off, key, value):
    """A 24-byte record of `value` named `key` (its name at `name_off`); a table's children and
    texts are appended."""
    crc = zlib.crc32(key.encode()) & 0xffffffff
    if isinstance(value, dict):
        kids = sorted((value_record(buf, append(buf, k.encode() + b'\0'), k, v) for k, v in value.items()),
                      key=lambda r: REC.unpack(r)[2])
        return REC.pack(append_array(buf, kids) if kids else 0, name_off, crc, T_NAMED, len(kids))
    if isinstance(value, bool):
        return REC.pack(int(value), name_off, crc, T_BOOL, 4)
    if isinstance(value, int):
        return REC.pack(value & 0xffffffff, name_off, crc, T_INT, 4)
    if isinstance(value, float):
        return REC.pack(struct.unpack('<I', struct.pack('<f', value))[0], name_off, crc, T_FLOAT, 4)
    text = value.encode()
    return REC.pack(append(buf, text + b'\0'), name_off, crc, T_STR, len(text) + 1)


def add_value(buf, table, key, value):
    """A new key of the named table `table` ('' the root), in its place by name CRC."""
    toff = find(buf, table) if table else 0
    assert rec(buf, toff)[3] == T_NAMED, (table, 'not a named table')
    kids = [bytes(buf[c:c + 24]) for c in children(buf, toff)]
    crc = zlib.crc32(key.encode()) & 0xffffffff
    assert all(REC.unpack(r)[2] != crc for r in kids), (table, key, 'its CRC is taken')
    record = value_record(buf, append(buf, key.encode() + b'\0'), key, value)
    new = sorted(kids + [record], key=lambda r: REC.unpack(r)[2])
    d, i, c, ty, cnt = rec(buf, toff)
    REC.pack_into(buf, toff, append_array(buf, new), i, c, ty, cnt + 1)


def items_of(buf, path):
    """The items of an indexed table of numbers or strings, as numbers and (UTF-8) strings."""
    off = find(buf, path)
    if rec(buf, off)[3] != T_INDEXED:
        raise ValueError('%s is not an indexed table' % path)
    items = []
    for c in children(buf, off):
        d, _, _, t, cnt = rec(buf, c)
        if t in (T_INT, T_FLOAT):
            items.append(as_number(t, d & 0xffffffff))
        elif t == T_STR:
            items.append(bytes(buf[d:d + cnt - 1]).decode())
        else:
            raise ValueError('%s holds an item that is neither a number nor a string' % path)
    return items


def remove_item(buf, path, value):
    """The one item equal to `value` of the indexed table of numbers or strings `path` taken out: the later
    items' records move down one place in the table's own array, its count drops by one (the last
    record stays, unreferenced). A value the table holds not exactly once is refused."""
    items = items_of(buf, path)
    if items.count(value) != 1:
        raise ValueError('%s holds %r %d times, not once: the base is not the live data'
                         % (path, value, items.count(value)))
    off = find(buf, path)
    kids = children(buf, off)
    at = items.index(value)
    for dst, src in zip(kids[at:], kids[at + 1:]):
        buf[dst:dst + 24] = buf[src:src + 24]
    d, i, crc, t, cnt = rec(buf, off)
    REC.pack_into(buf, off, d, i, crc, t, cnt - 1)


def shifted(path, lst, at):
    """A path of the file before an entry was put at `at` of the table `lst`, after it."""
    head = lst + '['
    if not path.startswith(head):
        return path
    k, rest = path[len(head):].split(']', 1)
    return '%s%d]%s' % (head, int(k) + (int(k) >= at), rest)


def set_prop(buf, prop_id, frm, to):
    """The lobby card's text of a stat: `ui_desc/props_list` entry `prop_id` (1 damage x100, 3 armour
    piercing x100, 4 rate of fire, ...), its `prop_value` string; the card shows this text, it does not
    compute it from `parameters`. Returns the changed value's path."""
    lst = find(buf, 'ui_desc/props_list')
    for i, entry in enumerate(children(buf, lst)):
        kids = {name_at(buf, rec(buf, c)[1]): c for c in children(buf, entry)}
        d, _, _, t, _ = rec(buf, kids[b'prop_id'])
        if as_number(t, d & 0xffffffff) != prop_id:
            continue
        off = kids[b'prop_value']
        d, i2, c, t, cnt = rec(buf, off)
        old = bytes(buf[d:d + cnt - 1]).decode()
        if old != frm:
            raise ValueError('prop %d says %r, not %r: the base is not the live data' % (prop_id, old, frm))
        noff = append(buf, to.encode() + bytes(1))
        REC.pack_into(buf, off, noff, i2, c, t, len(to.encode()) + 1)
        return '/ui_desc/props_list[%d]/prop_value' % i
    raise KeyError('prop %d' % prop_id)


def prefix_strings(buf, key, prefix):
    """Every string value named `key` gets `prefix` in front (the new strings appended). Returns
    how many."""
    def named(off):
        """(offset, name) of every value under a table; an indexed table's items have no name."""
        t = rec(buf, off)[3]
        out = []
        if t in (T_NAMED, T_INDEXED):
            for c in children(buf, off):
                out.append((c, name_at(buf, rec(buf, c)[1]) if t == T_NAMED else None))
                out += named(c)
        return out
    n = 0
    for off, name in named(0):
        d, i, c, t, cnt = rec(buf, off)
        if t == T_STR and name == key.encode():
            new = prefix.encode() + bytes(buf[d:d + cnt - 1])
            REC.pack_into(buf, off, append(buf, new + b'\0'), i, c, t, len(new) + 1)
            n += 1
    return n


def set_entry(buf, path, where, frm, to):
    """The one entry of the indexed table `path` whose keys match `where`: each key of `frm` must
    hold that value; then each key of `to` takes its value (a number keeps its type, a string is
    appended). Returns the entry's index."""
    lst = find(buf, path)
    if rec(buf, lst)[3] != T_INDEXED:
        raise ValueError('%s is not an indexed table' % path)
    hit = [i for i, e in enumerate(children(buf, lst)) if matches(buf, e, where)]
    if len(hit) != 1:
        raise ValueError('%s: %d entries match %r, not 1' % (path, len(hit), where))
    at = hit[0]
    entry = children(buf, lst)[at]
    for k, v in frm.items():
        c = child(buf, entry, k)
        if c is None or not same(value_of(buf, c), v):
            raise ValueError('%s[%d]/%s is %r, not %r: the base is not the live data'
                             % (path, at, k, None if c is None else value_of(buf, c), v))
    for k, v in to.items():
        c = child(buf, entry, k)
        if c is None:
            raise KeyError('%s[%d]/%s' % (path, at, k))
        d, i, crc, t, cnt = rec(buf, c)
        if t == T_STR:
            new = v.encode()
            REC.pack_into(buf, c, append(buf, new + b'\0'), i, crc, t, len(new) + 1)
        else:
            set_number(buf, '%s[%d]/%s' % (path, at, k), value_of(buf, c), v)
    return at


def remove_entry(buf, path, where):
    """The one entry of the indexed table `path` whose keys match `where` taken out: the later entries'
    records move down one place in the table's own array, its count drops by one (the last record and
    the entry's own values stay, unreferenced). Returns the entry's index; an entry matched not exactly
    once is refused."""
    lst = find(buf, path)
    if rec(buf, lst)[3] != T_INDEXED:
        raise ValueError('%s is not an indexed table' % path)
    kids = children(buf, lst)
    hit = [i for i, e in enumerate(kids) if rec(buf, e)[3] == T_NAMED and matches(buf, e, where)]
    if len(hit) != 1:
        raise ValueError('%s: %d entries match %r, not 1: the base is not the live data' % (path, len(hit), where))
    at = hit[0]
    for dst, src in zip(kids[at:], kids[at + 1:]):
        buf[dst:dst + 24] = buf[src:src + 24]
    d, i, crc, t, cnt = rec(buf, lst)
    REC.pack_into(buf, lst, d, i, crc, t, cnt - 1)
    return at


def unshifted(path, lst, at):
    """A path of the file before entry `at` of the table `lst` was taken out, after it; None for a
    path of the entry itself."""
    head = lst + '['
    if not path.startswith(head):
        return path
    k, rest = path[len(head):].split(']', 1)
    if int(k) == at:
        return None
    return '%s%d]%s' % (head, int(k) - (int(k) > at), rest)


CLIP_HEAD = struct.Struct('<HBB')  # bones, event channels, animation type
CLIP_BONE, CLIP_EVENT = 0x48, 0x10
CLIP_NAME_MAX = 31  # the engine copies a channel's name into 32 bytes (0x3A84CA)


def clip_parse(b):
    """A `.b-spline` clip, the memory image the engine fixes up on load (vostok's
    create_baked_animation_in_place): u16 bones, u8 event channels, u8 type; per bone nine 8-byte
    track pointers, per event channel a 16-byte header (8-byte knots pointer, u16 knots, u8 domains,
    u8 type, 4 unused bytes); every bone's nine tracks (u32 n, n knot/value float pairs); then per
    event channel its knots (f32), its domains (u8) and its NUL-ended name, padded to 4. Every pointer
    is set on load from the counts, never read; the unused and padding bytes are whatever the cooker's
    memory held (kept as they are). Returns (the part up to the event headers, the headers, the
    tracks, the channels, their padding); clip_bytes makes the same bytes again."""
    bones, events, _ = CLIP_HEAD.unpack_from(b, 0)
    heads_at = CLIP_HEAD.size + bones * CLIP_BONE
    heads = [bytes(b[heads_at + CLIP_EVENT * i:heads_at + CLIP_EVENT * (i + 1)]) for i in range(events)]
    off = tracks_at = heads_at + CLIP_EVENT * events
    for _ in range(bones * 9):
        off += 4 + 8 * struct.unpack_from('<I', b, off)[0]
    tracks = bytes(b[tracks_at:off])
    channels, pads = [], []
    for head in heads:
        knots, domains, kind = struct.unpack_from('<HBB', head, 8)
        values = list(struct.unpack_from('<%df' % knots, b, off))
        off += 4 * knots
        end = b.index(b'\0', off + domains)
        name = bytes(b[off + domains:end]).decode()
        size = (domains + len(name) + 1 + 3) // 4 * 4
        channels.append({'name': name, 'type': kind, 'knots': values, 'domains': list(b[off:off + domains])})
        pads.append(bytes(b[end + 1:off + size]))
        off += size
    if off != len(b):
        raise ValueError('a clip ends at %d, its file at %d' % (off, len(b)))
    return bytes(b[:heads_at]), heads, tracks, channels, pads


def clip_bytes(start, heads, tracks, channels, pads):
    """A clip's bytes; a channel past `pads` is padded with zeros."""
    out = bytearray(start)
    out[2] = len(channels)
    for head in heads:
        out += head
    out += tracks
    for i, c in enumerate(channels):
        tail = struct.pack('<%df' % len(c['knots']), *c['knots']) + bytes(c['domains']) + c['name'].encode() + b'\0'
        out += tail + (pads[i] if i < len(pads) else bytes(-len(tail) % 4))
    return out


def clip_end(tracks):
    """The clip's length in frames: the last knot of its first track."""
    n = struct.unpack_from('<I', tracks, 0)[0]
    return struct.unpack_from('<f', tracks, 4 + 8 * (n - 1))[0]


def f32(values):
    return [struct.unpack('<f', struct.pack('<f', v))[0] for v in values]


def add_event_channel(buf, frm, add):
    """`buf` with the event channel `add` after the channels `frm` (its live ones, checked)."""
    start, heads, tracks, channels, pads = clip_parse(buf)
    if clip_bytes(start, heads, tracks, channels, pads) != buf:
        sys.exit('add_event_channel: the clip does not read back byte for byte')
    want = [dict(c, knots=f32(c['knots'])) for c in frm]
    if channels != want:
        sys.exit('add_event_channel: the live channels are %s, not %s' % (channels, frm))
    new = dict(add, knots=f32(add['knots']))
    end = clip_end(tracks)
    if new['name'] in [c['name'] for c in channels] or len(new['name'].encode()) > CLIP_NAME_MAX \
            or not new['knots'] or len(new['knots']) != len(new['domains']) \
            or new['knots'] != sorted(new['knots']) or not 0 <= new['knots'][0] <= new['knots'][-1] < end \
            or not all(0 <= d < 255 for d in new['domains']) or not 0 <= new['type'] < 256:
        sys.exit('add_event_channel: %s does not fit a clip of %g frames with %s' % (add, end, channels))
    head = bytes(8) + struct.pack('<HBB', len(new['knots']), len(new['domains']), new['type']) + bytes(4)
    out = clip_bytes(start, heads + [head], tracks, channels + [new], pads)
    s2, h2, t2, c2, p2 = clip_parse(out)  # read back
    if s2[:2] + s2[3:] != start[:2] + start[3:] or s2[2] != len(heads) + 1 or h2 != heads + [head] \
            or t2 != tracks or c2 != channels + [new] or p2[:-1] != pads:
        sys.exit('add_event_channel: the edited clip reads back otherwise')
    buf[:] = out


def game_path(name):
    return name.replace('__', '/')


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    check = '--check' in sys.argv
    if len(args) != 3:
        sys.exit(__doc__)
    edits_file, live, out = args
    edits = json.load(open(edits_file, encoding='utf-8'))
    if not check and os.path.exists(out):
        sys.exit('%s: already there' % out)
    files = {}
    for root, _, names in os.walk(live):
        for n in names:
            rel = os.path.relpath(os.path.join(root, n), live).replace(os.sep, '/')
            files[game_path(rel)] = os.path.join(root, n)
    bufs = {}

    def buf(p):
        if p not in bufs:
            if p not in files:
                sys.exit('%s: not in the live folder' % p)
            bufs[p] = bytearray(open(files[p], 'rb').read())
        return bufs[p]

    allowed = {}
    copies = {}
    for e in edits.get('copy', []):
        n = 0
        for p in sorted(fnmatch.filter(files, e['files'])):
            if not p.startswith(e['from']):
                sys.exit('copy: %s does not start with %s' % (p, e['from']))
            q = e['to'] + p[len(e['from']):]
            if q in files or q in copies:
                sys.exit('copy: %s is already there' % q)
            copies[q] = p
            bufs[q] = bytearray(open(files[p], 'rb').read())
            n += 1
        print('copy %s -> %s: %d files' % (e['files'], e['to'], n))
    prefixed = {}
    for e in edits.get('scale', []):
        n = 0
        for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
            b = buf(p)
            off = find(b, e['path'])
            d, _, _, t, _ = rec(b, off)
            old = as_number(t, d & 0xffffffff)
            set_number(b, e['path'], old, round(old * e['by'], 6), as_float=True)
            allowed.setdefault(p, set()).add('/' + e['path'])
            n += 1
        print('scale %s x %g: %d files' % (e['path'], e['by'], n))
    for e in edits.get('prefix_string', []):
        n = 0
        for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
            k = prefix_strings(buf(p), e['key'], e['prefix'])
            if k:
                prefixed.setdefault(p, []).append((e['key'].encode(), e['prefix'].encode()))
                n += k
        print('prefix %s with %s: %d values' % (e['key'], e['prefix'], n))
    for e in edits.get('set', []):
        set_number(buf(e['file']), e['path'], e['from'], e['to'], e.get('as_float', False))
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('set_entry', []):
        at = set_entry(buf(e['file']), e['list'], e['where'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).update('/%s[%d]/%s' % (e['list'], at, k) for k in e['to'])
    for e in edits.get('set_prop', []):
        q = set_prop(buf(e['file']), e['prop_id'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add(q)
    for e in edits.get('edit_text', []):
        edit_text(buf(e['file']), e['path'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('rename_key', []):
        n = 0
        for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
            b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
            if rename_key(b, e['table'], e['from'], e['to']):
                bufs[p] = b
                n += 1
        print('rename %s -> %s: %d files' % (e['from'], e['to'], n))
    for e in edits.get('replace_string', []):
        n = 0
        for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
            b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
            k = replace_string(b, e['from'], e['to'])
            if k:
                bufs[p] = b
                n += k
        print('string %s -> %s: %d values' % (e['from'], e['to'], n))
    shifts, added, grown = {}, {}, {}  # grown: file -> table -> entries added (less those removed)
    for e in edits.get('add_entry', []):
        n = 0
        for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
            b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
            if e.get('only_if'):
                try:
                    find(b, e['only_if'])
                except KeyError:
                    continue
            at = add_entry(b, e['list'], e['after'], e['like'], e['values'])
            if at is None:
                continue
            bufs[p] = b
            n += 1
            lst = '/' + e['list']
            shifts.setdefault(p, []).append((lst, at))
            allowed[p] = {shifted(q, lst, at) for q in allowed.get(p, ())}
            added[p] = [shifted(q, lst, at) for q in added.get(p, ())]
            added.setdefault(p, []).append('%s[%d]' % (lst, at))
            grown.setdefault(p, {}).setdefault(lst, 0)
            grown[p][lst] += 1
        print('entry %s after %s: %d files' % (e['list'], e['after'], n))
    for e in edits.get('add_key', []):
        add_key(buf(e['file']), e['table'], e['key'], e['text'])
        added.setdefault(e['file'], []).append('/%s/%s' % (e['table'], e['key']))
        grown.setdefault(e['file'], {}).setdefault('/' + e['table'], 0)
        grown[e['file']]['/' + e['table']] += 1
    for e in edits.get('add_value', []):
        for p in e['files']:
            add_value(buf(p), e['table'], e['key'], e['value'])
            table = '/' + e['table'] if e['table'] else ''
            added.setdefault(p, []).append('%s/%s' % (table, e['key']))
            grown.setdefault(p, {}).setdefault(table, 0)
            grown[p][table] += 1
        print('value %s/%s: %d files' % (e['table'], e['key'], len(e['files'])))
    removed = {}
    for e in edits.get('remove_item', []):
        remove_item(buf(e['file']), e['path'], e['value'])
        removed.setdefault(e['file'], {}).setdefault(e['path'], []).append(e['value'])
    dropped = {}
    for e in edits.get('remove_entry', []):
        at = remove_entry(buf(e['file']), e['list'], e['where'])
        lst = '/' + e['list']
        print('remove %s[%d] %s' % (e['list'], at, json.dumps(e['where'], ensure_ascii=False)))
        dropped.setdefault(e['file'], []).append((lst, at))
        grown.setdefault(e['file'], {}).setdefault(lst, 0)
        grown[e['file']][lst] -= 1
        allowed[e['file']] = {q for q in (unshifted(q, lst, at) for q in allowed.get(e['file'], ())) if q is not None}
        if any(unshifted(a, lst, at) is None for a in added.get(e['file'], ())):
            sys.exit('remove_entry: %s[%d] is an entry this list adds' % (lst, at))
        added[e['file']] = [unshifted(a, lst, at) for a in added.get(e['file'], ())]
    clips = {}  # animation clips: not configs, so add_event_channel reads them back itself
    for e in edits.get('add_event_channel', []):
        p = e['file']
        if p in bufs:
            sys.exit('add_event_channel: %s is edited as a config too' % p)
        if p not in clips:
            if p not in files:
                sys.exit('%s: not in the live folder' % p)
            clips[p] = bytearray(open(files[p], 'rb').read())
        add_event_channel(clips[p], e['from'], e['add'])
        print('event channel %s: %s knots %s domains %s' % (p, e['add']['name'], e['add']['knots'], e['add']['domains']))

    # read back: compare every value with the live file, by path
    renames = [('/%s/%s' % (e['table'], e['from']), '/%s/%s' % (e['table'], e['to']), e['from'], e['to'])
               for e in edits.get('rename_key', [])]
    strings = [(e['from'].encode(), e['to'].encode()) for e in edits.get('replace_string', [])]
    bad = same_copies = 0
    for p, b in sorted(bufs.items()):
        source = bytearray(open(files[copies.get(p, p)], 'rb').read())
        if p in copies and b == source:
            same_copies += 1
            continue
        old = {q: (t, v) for q, t, v in walk(source)}
        new = {q: (t, v) for q, t, v in walk(b)}
        for ro, rn, _, _ in renames:
            if ro in old and rn in new and rn not in old:
                old[rn] = old.pop(ro)
        for lst, at in shifts.get(p, ()):
            old = {shifted(q, lst, at): v for q, v in old.items()}
        for lst, at in dropped.get(p, ()):
            old = {unshifted(q, lst, at): v for q, v in old.items() if unshifted(q, lst, at) is not None}
        for lst, values in removed.get(p, {}).items():
            was, now = items_of(source, lst), items_of(b, lst)
            want = list(was)
            for v in values:
                want.remove(v)
            ok = now == want
            bad += not ok
            print('%s%s /%s: %s -> %s' % ('' if ok else 'UNEXPECTED ', p, lst, was, now))
            for d in (old, new):
                for q in [q for q in d if q == '/' + lst or q.startswith('/%s[' % lst)]:
                    del d[q]
        for q in sorted(set(old) | set(new)):
            o, n = old.get(q), new.get(q)
            if o == n:
                continue
            ok = (q in allowed.get(p, ()) and o and n and n[0] in (o[0], T_FLOAT)) or \
                (o and n and o[0] == n[0] == T_STR and (o[1], n[1]) in strings) or \
                (o and n and o[0] == n[0] == T_STR and any(q.endswith('/' + k.decode()) and n[1] == x + o[1]
                                                           for k, x in prefixed.get(p, ()))) or \
                (o is None and any(q == a or q.startswith(a + '/') for a in added.get(p, ()))) or \
                (o and n and o[0] == n[0] and n[1] - o[1] == grown.get(p, {}).get(q, 0) != 0)
            bad += not ok
            print('%s%s %s: %s -> %s' % ('' if ok else 'UNEXPECTED ', p, q, show(o), show(n)))
    if copies:
        print('%d new files (copies), %d of them byte for byte' % (len(copies), same_copies))
    print('%d files edited, %d unexpected differences' % (len(bufs) + len(clips), bad))
    if bad:
        sys.exit(1)
    if check:
        return
    for p, b in list(bufs.items()) + list(clips.items()):
        dst = os.path.join(out, *p.split('/'))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        with open(dst, 'wb') as f:
            f.write(b)
    print('written to', out)


def show(v):
    if v is None:
        return None
    return round(as_number(v[0], v[1]), 6) if v[0] in (T_INT, T_FLOAT) else v[1]


if __name__ == '__main__':
    main()
