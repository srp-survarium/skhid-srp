#!/usr/bin/env python3
"""content_edit.py: make a resources.db patch folder by editing game configs in place.

usage: python3 -I content_edit.py <edits.json | <table>.tsv> <live folder> <out folder> [--check]

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
  "set_vector":     [{"file", "path", "from", "to"}]   a float3 (a model's `bounding_box/min`): `from` the
                                                       live three numbers, `to` the new ones, in place
  "set_bool":       [{"file", "path", "from", "to"}]   a native boolean; both values must be JSON booleans,
                                                       type and every other byte are preserved
  "rename_key":     [{"files" (glob on the game path), "table", "from", "to"}]
                    renames a key of the named table in every matching file that has it; the new name
                    is appended after the file's last byte, the table's children re-sorted by CRC
  "remove_key":     [{"files" (glob on the game path), "keys"}]
                    each key path (`parameters/arp`) taken out of every matching file that has it: its
                    table's later records move down in place, the count drops by 1; a key no matching
                    file has is refused
  "replace_string": [{"files", "from", "to"}]          every string value equal to `from`, appended
  "edit_text":      [{"file", "path", "from", "to"}]   in one text (UTF-8, as the localization tables
                                                       keep it), its one `from` becomes `to`
  "add_entry":      [{"files", "list", "after", "like", "values", "only_if"}]
                    a new entry of the indexed table `list` (a lobby card's `ui_desc/props_list`, the
                    card lines' `ui_properties`) after the entry whose keys match `after`, made like
                    the entry matching `like` with `values` set; a value "{a/b}" is that number of
                    the same file, a list of numbers a vector (a position), a list of vectors an
                    indexed table of vectors (a wire's points) made anew; a key of `values` may be a
                    path in the entry (`spawned_item/position`): the tables on its way are copied
                    anew, the like entry's own stay as they are; files without `only_if`, the table
                    or those entries are left alone
  "add_key":        [{"file", "table", "key", "text"}] a new text of a localization table
  "add_value":      [{"files" (a list of game paths), "table" ("" the root), "key", "value"}]
                    a new key of the named table in each file, in its place by name CRC: a bool,
                    an int, a float (a number with a point), a text, a vector (a list of 2-4
                    numbers: a float2..float4, as a position), or an object (a named table of
                    those); a file that has the key already is refused
  "remove_item":    [{"file", "path", "value"}]        the one item equal to `value` of an indexed table of
                                                       numbers (an item's `default_modifications`) or of
                                                       strings (a weapon's `death` animations); the
                                                       later items move down in place, the count drops by 1
  "promote_rolled_defaults": [{"categories", "from_top"}]
                    every item of those `items_dict` categories: each of its `default_modifications`
                    of a kind that rolls (a row of the kind with a drop weight that changes more than
                    wear, as the server's Modification::rolls) weaker than that kind's `from_top`-th
                    strongest row (1 the rarest: the rows by drop weight, commonest first, as the
                    server and the wiki order them) becomes it; a stronger one stays (it only raises);
                    kinds that never roll stay; applied as `set` edits, before them
  "append_item":    [{"file", "path", "value"}]        `value` after the items of the indexed table of
                                                       numbers or strings `path` (an item's
                                                       `default_modifications`), made like its first item;
                                                       a value the table holds already is refused; applied
                                                       after remove_item
  "copy":           [{"files", "from", "to"}]          every live file the glob matches (its path starts
                                                       with `from`) is also a new file at `to` + the rest
                                                       of its path, a path no live file has; applied first,
                                                       the globs below see the copies
  "localization":   "relative/path/to/tsv/folder"
                    compile every registered language from its complete TSV dictionary;
                    preserve all source keys, check formats and the native text buffer limit;
                    map each of srp's own texts (st_srp_*) to itself in the UI mapping of the
                    movies that read them (localization_026g.FLASH_MOVIES: each movie its
                    prefix's), after their rows.
  "scale":          [{"files", "path", "by"}]          the number at `path` of every matching file times
                                                       `by`, as a float (a matching file without it is
                                                       refused)
  "prefix_string":  [{"files", "key", "prefix"}]       every string value named `key` of the matching files
                                                       gets `prefix` in front, appended
  "set_entry":      [{"file", "list", "where", "from", "to"}]
                    the one entry of the indexed table `list` whose keys match `where`: each key of
                    `from` must hold that value, then takes the value in `to` (a number keeps its type,
                    a string or a vector, a list of 2-4 numbers, is appended); a key of `from` and `to`
                    may be a path in the entry (`spawned_item/position`, `collision_geometries[0]/name`)
  "append_rows":    [{"file", "path", "rows"}]         append rows of strings to an indexed UI table
  "add_item":       [{"file", "list", "where", "key", "value"}]
                    the one entry of the indexed table `list` whose keys match `where`: its indexed
                    table `key` (a path in the entry) of strings or numbers gets `value` after its
                    items, made like its first item (an anomaly's `artefact_containers`); applied after
                    add_entry, before remove_entry
  "remove_entry":   [{"file", "list", "where"}]        the one entry of the indexed table `list` whose keys
                                                       match `where` taken out (a scene's object, model or
                                                       collision); the later entries move down in place, the
                                                       count drops by 1. Applied last: the paths of the edits
                                                       above are the live file's
  "add_event_channel": [{"file", "from", "add"}]       an animation clip (`.b-spline`, not a config): `from`
                                                       is its live event channels, `add` one more after them,
                                                       each {"name", "type", "knots", "domains"}; the bone
                                                       tracks stay byte for byte (see clip_parse)
  "make_held_flower": [{"files": ["four model resources"], "sha256": {"path": "hash"},
                        "source": "models/...model/", "output": "models/...held.model/", "minimum_y": 0.425}],
  "make_pose_clip": [{"files": ["clip.b-spline", "clip.bones_names"], "source_sha256", "names_sha256",
                       "output", "frames", "start", "end"}]
                    new full-pose animation pair, from a hash-pinned stock rig. `output` is a new
                    animations/ path without an extension; start/end map bone names to local XYZ
                    rotations in radians on the shortest continuous branch. Other channels keep
                    the source's zero-time pose. The clamped cubic eases in/out with zero endpoint
                    velocity; no events, no edits to stock clips. Not shipped: the carry's pose
                    source (scripts/artefact-carry/pose.json) runs it to compile its curves.
  "runtime_vegetation": [{"files" (glob)}]             remove only .veg editor chunks 0x4d/0x4e;
                                                       preserve runtime positions/normals byte for byte;
                                                       reject unknown, duplicate or malformed chunks
  "require_headwear_bind": [{"file"}]                 verify the character config’s body_parts/bind_pose
                                                       names one of this list’s append_headwear_bind files
  "append_headwear_bind": [{"file"}]                   append the render-only Head copy to a native bind
  "rigid_headwear": [{"file", "vertices"[,            bind a checked Head/Neck surface to that render bone;
                       "allow_bones", "why"]}]         geometry, UVs and indices stay byte for byte;
                                                       allow_bones: this surface's other native bones
                                                       (indices), which then follow the head too
A table (<table>.tsv, see STATS): its cells are written into its configs, then their card lines.
`--dump-stats <table> <folder>` prints the table of the configs there.
A key of `where` may be a path in the entry too (`path[1]`: a modifier's target key).
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
import hashlib
import math
import os
import re
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


def find(buf, path, off=0):
    """Offset of the value at a/b/c; `name[i]` is item i of an indexed table (states[0]/take_time);
    from the value at `off` (the root by default)."""
    for part in path.split('/'):
        name, *indices = part.split('[')
        if name or not indices:
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


def set_vector(buf, path, frm, to):
    """A float3 value's three numbers, in place."""
    at, _, _, t, _ = rec(buf, find(buf, path))
    if t != 7 or len(frm) != 3 or len(to) != 3:
        raise TypeError('%s: a float3 and three numbers each' % path)
    old = struct.unpack_from('<3f', buf, at)
    if any(abs(a - b) > 1e-6 for a, b in zip(old, frm)):
        raise ValueError('%s is %r, not %r: the base is not the live data' % (path, old, frm))
    struct.pack_into('<3f', buf, at, *to)


def set_bool(buf, path, frm, to):
    """Set a native boolean without interpreting it as an integer or changing its type."""
    if type(frm) is not bool or type(to) is not bool:
        raise TypeError('set_bool requires JSON booleans for from and to')
    off = find(buf, path)
    data, name, crc, kind, count = rec(buf, off)
    if kind != T_BOOL:
        raise TypeError('%s is not a boolean' % path)
    old = data & 0xffffffff
    if old not in (0, 1) or old != int(frm):
        raise ValueError('%s is %r, not %r: the base is not the live data' % (path, old, frm))
    REC.pack_into(buf, off, (data & ~0xffffffff) | int(to), name, crc, kind, count)


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


def remove_key(buf, path):
    """The key at `path` taken out of its named table: the later records move down one place in the
    table's own array (the CRC order holds), its count drops by one. False when there is no such key."""
    table, _, key = path.rpartition('/')
    try:
        toff = find(buf, table) if table else 0
    except KeyError:
        return False
    if rec(buf, toff)[3] != T_NAMED:
        return False
    kids = children(buf, toff)
    hit = [i for i, c in enumerate(kids) if name_at(buf, rec(buf, c)[1]) == key.encode()]
    if not hit:
        return False
    for dst, src in zip(kids[hit[0]:], kids[hit[0] + 1:]):
        buf[dst:dst + 24] = buf[src:src + 24]
    d, i, crc, t, cnt = rec(buf, toff)
    REC.pack_into(buf, toff, d, i, crc, t, cnt - 1)
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
    """Whether each key of `want`, a path in the entry (`id`, `path[1]`), holds its value."""
    for k, v in want.items():
        try:
            c = find(buf, k, entry)
        except (KeyError, AssertionError):
            c = None
        if c is None or not same(value_of(buf, c), v):
            return False
    return True


def number_text(v):
    """A number as the game's cards write it: 50, 0.45, 3.167."""
    return str(int(v)) if v == int(v) else '%g' % v


def filled(buf, text):
    """`{a/b}` in a value: that number of the same file, as the game's cards write numbers."""
    if not (isinstance(text, str) and text.startswith('{') and text.endswith('}')):
        return text
    return number_text(value_of(buf, find(buf, text[1:-1])))


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
    for key in values:
        try:
            find(buf, key, model[0])
        except KeyError:
            raise AssertionError(('a key the model entry lacks', path, key))
    d, i, crc, t, cnt = rec(buf, model[0])
    entry = REC.pack(copied_table(buf, model[0], values, path), i, crc, t, cnt)
    old = [bytes(buf[c:c + 24]) for c in kids]
    new = old[:at[0] + 1] + [entry] + old[at[0] + 1:]
    d, i, crc, t, cnt = rec(buf, lst)
    REC.pack_into(buf, lst, append_array(buf, new), i, crc, t, cnt + 1)
    return at[0] + 1


def copied_table(buf, table, values, path):
    """A new array of the records of the table at `table`, the keys of `values` set; a key `a/b` (or
    `a[0]/b`) sets b in a copy of the table a. Returns the array's offset."""
    recs = []
    named = rec(buf, table)[3] == T_NAMED
    for k, c in enumerate(children(buf, table)):
        d, i, crc, t, cnt = rec(buf, c)
        name = name_at(buf, i).decode() if named else '[%d]' % k
        subs = {key[len(name):].lstrip('/'): v for key, v in values.items()
                if key.startswith(name + '/') or key.startswith(name + '[')}
        if subs:
            if name in values:
                raise ValueError('%s: %s is set and has keys set' % (path, name))
            recs.append(REC.pack(copied_table(buf, c, subs, path), i, crc, t, cnt))
            continue
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
    return append_array(buf, recs)


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
    if isinstance(value, dict) and value and all(isinstance(k, int) for k in value):
        if sorted(value) != list(range(len(value))):
            raise ValueError('%s: items %s, not 0 to %d' % (key, sorted(value), len(value) - 1))
        empty = append(buf, b'\0')
        kids = [value_record(buf, empty, '', value[k]) for k in range(len(value))]
        return REC.pack(append_array(buf, kids), name_off, crc, T_INDEXED, len(kids))
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
    if isinstance(value, list):
        if not 2 <= len(value) <= 4 or not all(isinstance(x, (int, float)) and not isinstance(x, bool)
                                              for x in value):
            raise ValueError('%s: a vector is a list of 2 to 4 numbers, not %r' % (key, value))
        floats = struct.pack('<%df' % len(value), *value)
        return REC.pack(append(buf, floats), name_off, crc, T_INDEXED + len(value), len(floats))
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


def append_item(buf, path, value):
    """`value` after the items of the indexed table of numbers or strings `path`, a record like its
    first item; the table's records move to a new array with it. A value the table holds already is
    refused (a stale base, or a second bonus of one modification)."""
    items = items_of(buf, path)
    if value in items:
        raise ValueError('%s holds %r already' % (path, value))
    off = find(buf, path)
    kids = children(buf, off)
    if not kids:
        raise ValueError('%s has no item to make the new one like' % path)
    _, ii, icrc, it, icnt = rec(buf, kids[0])
    if it == T_STR and isinstance(value, str):
        text = value.encode()
        new = REC.pack(append(buf, text + b'\0'), ii, icrc, it, len(text) + 1)
    elif it == T_INT and isinstance(value, int) and not isinstance(value, bool):
        new = REC.pack(value & 0xffffffff, ii, icrc, it, icnt)
    elif it == T_FLOAT and isinstance(value, (int, float)) and not isinstance(value, bool):
        new = REC.pack(struct.unpack('<I', struct.pack('<f', value))[0], ii, icrc, it, icnt)
    else:
        raise ValueError('%s holds items of type %d, not %r' % (path, it, value))
    old = [bytes(buf[c:c + 24]) for c in kids]
    d, i, crc, t, cnt = rec(buf, off)
    REC.pack_into(buf, off, append_array(buf, old + [new]), i, crc, t, cnt + 1)


DICTIONARY = 'gameplay/db_static_dictionaries'
MODIFICATIONS = 'gameplay/items/modifications/modifications.options'
# What a "reliability" modification changes, and nothing else (the server's WEAR_PARAMETERS).
WEAR = {'parameters/degrade_per_match', 'parameters/degrade_per_use', 'parameters/degrade_per_death'}


def rolled_strengths(mods):
    """For each kind (type_mask's set character) that rolls, its rolling rows' ids by drop weight,
    commonest first (then by id), as the server's Modification::rolls and its order have them."""
    rows = {}
    for i, _ in enumerate(children(mods, find(mods, 'modifications'))):
        at = 'modifications[%d]' % i
        paths = ['/'.join(items_of(mods, '%s/modifiers[%d]/path' % (at, j)))
                 for j in range(len(children(mods, find(mods, at + '/modifiers'))))]
        weight = value_of(mods, find(mods, at + '/drop_weight'))
        if weight > 0 and not (paths and all(p in WEAR for p in paths)):
            kind = value_of(mods, find(mods, at + '/type_mask')).index('1')
            rows.setdefault(kind, []).append((-weight, value_of(mods, find(mods, at + '/id'))))
    return {kind: [i for _, i in sorted(r)] for kind, r in rows.items()}


def promoted_defaults(read, categories, from_top):
    """The `set` edits of promote_rolled_defaults: (file, path, from, to) for each default of a
    rolling kind of an item of `categories` weaker than that kind's `from_top`-th strongest row
    (it only raises: a default at that strength or stronger stays)."""
    mods = read(MODIFICATIONS)
    kinds = {}
    for i, _ in enumerate(children(mods, find(mods, 'modifications'))):
        at = 'modifications[%d]' % i
        kinds[value_of(mods, find(mods, at + '/id'))] = value_of(mods, find(mods, at + '/type_mask')).index('1')
    strengths = rolled_strengths(mods)
    dictionary = read(DICTIONARY)
    edits = []
    for i, _ in enumerate(children(dictionary, find(dictionary, 'items_dict'))):
        at = 'items_dict[%d]' % i
        if value_of(dictionary, find(dictionary, at + '/item_category')) not in categories:
            continue
        cfg = value_of(dictionary, find(dictionary, at + '/cfg_name'))
        item = read(cfg)
        try:
            defaults = items_of(item, 'default_modifications')
        except KeyError:
            continue
        for j, mid in enumerate(defaults):
            ranked = strengths.get(kinds.get(mid))
            if not ranked:
                continue
            at = max(0, len(ranked) - from_top)
            # Strength is the place in `ranked` (commonest first); a row of the kind that never
            # rolls has no place, and is set like a weaker one.
            if mid not in ranked or ranked.index(mid) < at:
                edits.append((cfg, 'default_modifications[%d]' % j, mid, ranked[at]))
    return edits


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


# Weapon card lines (ui_desc/props_list) that show a config value: prop_id: (key path, factor).
WEAPON_CARD = {
    1: ('parameters/bullet_damage', 100), 2: ('dispersion/base_dispersion', 1),
    3: ('parameters/bullet_pierce', 100), 24: ('parameters/min_damage_distance', 1),
    105: ('parameters/movement_speed_modifier', 1), 132: ('parameters/stopping_power', 100),
    133: ('parameters/aim_time', 1), 134: ('parameters/magazine_capacity', 1),
    138: ('parameters/reload_time', 1), 139: ('parameters/max_damage_distance', 1),
    140: ('parameters/min_damage_distance', 1), 141: ('parameters/min_damage_amount', 1),
    142: ('parameters/show_time', 1), 143: ('parameters/hide_time', 1),
    144: ('parameters/aim_zoom_factor', 1), 145: ('parameters/stamina_damage', 1),
    146: ('recoil/min_top_recoil', 1), 147: ('recoil/max_top_recoil', 1),
    148: ('recoil/min_left_recoil', 1), 149: ('recoil/max_left_recoil', 1),
    150: ('recoil/min_right_recoil', 1), 151: ('recoil/max_right_recoil', 1),
    152: ('dispersion/from_the_hip_multiplier', 1), 153: ('dispersion/one_shoot_dispersion_amount', 1),
    154: ('dispersion/speed_of_aiming', 1), 155: ('dispersion/growth_speed', 1),
    156: ('breath_vibration/base_horizontal_amplitude', 1), 157: ('breath_vibration/base_vertical_amplitude', 1),
    158: ('breath_vibration/base_period', 1), 159: ('dispersion/max_dispersion', 1),
    160: ('parameters/chamber_a_round_time', 1), 161: ('ui_desc/combat_log_icon', 1),
}


def weapon_card(buf):
    """{prop_id: text} of the card lines the weapon config `buf` makes (WEAPON_CARD)."""
    def number(path):
        try:
            return value_of(buf, find(buf, path))
        except KeyError:
            return None
    out = {p: number_text(number(k) * times) for p, (k, times) in WEAPON_CARD.items() if number(k) is not None}
    rpm, chamber = number('parameters/rounds_per_minute'), number('parameters/chamber_a_round_time')
    if rpm:
        # the stock cards count a bolt or pump gun's chambering in its rate of fire
        out[4] = number_text(rpm if chamber is None else math.floor(60 / (chamber + 60 / rpm) + 1e-3))
    if number('parameters/weight') is not None:
        out[15] = '%.1f' % number('parameters/weight')
    return out


# <table>.tsv, a row each of what `configs` names, a column a stat:
#   'dir/x_*.ext'      a row per name the first `*` takes (x_name); the rest of the glob may match several
#                      files, each the row's (a level's projects of every weather)
#   'file#list[key]'   a row per entry of the indexed table `list`, named by its `key`
#   'file'             no `columns`: a key/value table (`key  value`), a row a key path; the config is
#                      made anew when no archive has it
# `columns`: their key paths; `[*]` every item, `[field=cell]` the entries whose field reads so, `[i]`.
# A cell takes its key's type (an int, a float, true/false, a text); an empty cell: no such key.
# `match`: the columns (of a key/value table: keys) a match computes with: a change needs the `next`
# row of simulation_changes.tsv. `cards`: the lobby card lines a config's values make.
STATS = {
    'weapon-stats': {
        'configs': 'gameplay/items/weapons/wpn_*.options',
        'columns': {
            'damage': 'parameters/bullet_damage', 'pierce': 'parameters/bullet_pierce',
            'rpm': 'parameters/rounds_per_minute', 'magazine': 'parameters/magazine_capacity',
            'reload': 'parameters/reload_time', 'reload_start': 'parameters/reload_start_time',
            'reload_end': 'parameters/reload_end_time', 'chamber': 'parameters/chamber_a_round_time',
            'aim_time': 'parameters/aim_time', 'show_time': 'parameters/show_time',
            'hide_time': 'parameters/hide_time', 'zoom': 'parameters/aim_zoom_factor',
            'weight': 'parameters/weight', 'movement': 'parameters/movement_speed_modifier',
            'stopping': 'parameters/stopping_power', 'stamina_damage': 'parameters/stamina_damage',
            'near_m': 'parameters/max_damage_distance', 'far_m': 'parameters/min_damage_distance',
            'far_damage': 'parameters/min_damage_amount',
            'level': 'parameters/item_level', 'mm_level': 'parameters/match_making_level',
            'spread': 'dispersion/base_dispersion', 'shot_spread': 'dispersion/one_shoot_dispersion_amount',
            'max_spread': 'dispersion/max_dispersion', 'spread_growth': 'dispersion/growth_speed',
            'aim_speed': 'dispersion/speed_of_aiming', 'hip_spread': 'dispersion/from_the_hip_multiplier',
            'aim_spread': 'dispersion/aim_multiplier', 'reload_spread': 'dispersion/reload_dispersion_amount',
            'recoil': 'recoil/recoil_power', 'top_min': 'recoil/min_top_recoil', 'top_max': 'recoil/max_top_recoil',
            'left_min': 'recoil/min_left_recoil', 'left_max': 'recoil/max_left_recoil',
            'right_min': 'recoil/min_right_recoil', 'right_max': 'recoil/max_right_recoil',
            'pid_p': 'recoil/compensation_proportional_gain', 'pid_i': 'recoil/compensation_integral_gain',
            'pid_d': 'recoil/compensation_derivational_gain',
            'shake_min': 'recoil/min_shake_recoil', 'shake_max': 'recoil/max_shake_recoil',
            'shake_time': 'recoil/shake_time_scale',
            'sway_h': 'breath_vibration/base_horizontal_amplitude', 'sway_v': 'breath_vibration/base_vertical_amplitude',
            'sway_period': 'breath_vibration/base_period', 'melee': 'melee/data/k_damage',
        },
        'match': {'damage', 'pierce', 'rpm', 'magazine', 'reload', 'reload_start', 'reload_end', 'chamber',
                  'aim_time', 'show_time', 'hide_time', 'weight', 'movement', 'stopping', 'stamina_damage',
                  'near_m', 'far_m', 'far_damage', 'spread', 'shot_spread', 'max_spread', 'spread_growth',
                  'aim_speed', 'hip_spread', 'aim_spread', 'reload_spread', 'recoil', 'top_min', 'top_max',
                  'left_min', 'left_max', 'right_min', 'right_max', 'pid_p', 'pid_i', 'pid_d', 'shake_min',
                  'shake_max', 'shake_time', 'sway_h', 'sway_v', 'sway_period', 'melee'},
        'cards': weapon_card,
    },
    # srp's lobby reads it, and the lobby movie through exe patch 94: nothing a match computes.
    'rooms': {'configs': 'gameplay/srp/rooms.options', 'match': set()},
}


def stats_category(path):
    name = os.path.basename(path)
    if not name.endswith('.tsv') or name[:-len('.tsv')] not in STATS:
        raise ValueError('%s: not <name>.tsv of a table of STATS (%s)' % (name, ', '.join(STATS)))
    return STATS[name[:-len('.tsv')]]


def stats_config(category, name):
    """The glob of a row's files (a path when the glob has one `*`)."""
    glob = category['configs']
    star = glob.index('*')
    return glob[:glob[:star].rindex('/') + 1] + name + glob[star + 1:]


def stats_paths(category, paths):
    """The game paths of `paths` a category reads."""
    configs = category['configs'].split('#')[0]
    return sorted(fnmatch.filter(paths, configs) if '*' in configs else {configs} & set(paths))


def stats_targets(category, files):
    """{row: [(path, offset)]}: the values each row's cells are under, of `files` ({path: buf})."""
    configs, out = category['configs'], {}
    if '#' in configs:
        path, (lst, key) = configs.split('#')[0], configs.split('#')[1][:-1].split('[')
        for _, entry in resolve(files[path], lst + '[*]') if path in files else []:
            out.setdefault(cell_text(files[path], find(files[path], key, entry)), []).append((path, entry))
    elif '*' in configs:
        star = configs.index('*')
        head = configs[:configs[:star].rindex('/') + 1]
        pattern = re.compile(re.escape(head) + '(' + re.escape(configs[len(head):star]) + '[^/]*)' +
                             '.*'.join(map(re.escape, configs[star + 1:].split('*'))))
        for path in sorted(files):
            if pattern.fullmatch(path):
                out.setdefault(pattern.fullmatch(path)[1], []).append((path, 0))
    elif configs in files:
        out[''] = [(configs, 0)]
    return out


def stats_label(category, row, path=None):
    """A row's config in messages: its file (one of a glob's), or the list entry."""
    configs = category['configs']
    if '#' in configs:
        return '%s=%s]' % (configs[:-1], row)
    return path or (stats_config(category, row) if '*' in configs else configs)


def read_stats(path):
    """(category, {column: key path}, {row: {column: cell}}) of a stats table; a key/value table
    (`key  value`) is one row '', a column a key."""
    category = stats_category(path)
    with open(path, encoding='utf-8') as f:
        lines = [l.split('\t') for l in f.read().splitlines() if l.strip() and not l.startswith('#')]
    head, body = lines[0], lines[1:]
    for fields in body:
        if len(fields) != len(head):
            raise ValueError('%s: %s: %d fields, not %d' % (path, fields[0], len(fields), len(head)))
    names = [fields[0] for fields in body]
    if len(set(names)) != len(names):
        raise ValueError('%s: %s is there twice' % (path, sorted(n for n in names if names.count(n) > 1)[0]))
    if 'columns' not in category:
        if head != ['key', 'value']:
            raise ValueError('%s: a key/value table: key, value' % path)
        return category, {k: k for k in names}, {'': {k: v for k, v in body}}
    columns, known = head[1:], category['columns']
    unknown, missing = [c for c in columns if c not in known], [c for c in known if c not in columns]
    if unknown or missing or len(set(columns)) != len(columns):
        raise ValueError('%s: unknown columns %s, missing %s' % (path, unknown, missing))
    return category, known, {fields[0]: dict(zip(columns, fields[1:])) for fields in body}


def resolve(buf, path, off=0):
    """[(walk path, offset)] of every value `path` selects under the value at `off`: `name` a key,
    `[i]` item i, `[*]` every item, `[field=cell]` every entry whose field reads as that cell."""
    out = [('', off)]
    for part in path.split('/'):
        name, *selectors = part.split('[')
        if name:
            out = [(at + '/' + name, c) for at, o in out if rec(buf, o)[3] == T_NAMED
                   for c in [child(buf, o, name)] if c is not None]
        for s in (s.rstrip(']') for s in selectors):
            field, _, cell = s.partition('=')
            out = [('%s[%d]' % (at, i), c) for at, o in out if rec(buf, o)[3] == T_INDEXED
                   for i, c in enumerate(children(buf, o))
                   if s in ('*', str(i)) or (cell and resolve(buf, field, c) and
                                             cell_text(buf, resolve(buf, field, c)[0][1]) == cell)]
    return out


def cell_text(buf, off):
    """A value as a table's cell: an int, the shortest float text, true/false, the text."""
    d, _, _, t, cnt = rec(buf, off)
    if t == T_BOOL:
        return 'true' if d & 1 else 'false'
    if t == T_STR:
        return bytes(buf[d:d + cnt - 1]).decode()
    if t == T_INT:
        return str(as_number(t, d & 0xffffffff))
    if t == T_FLOAT:
        raw = d & 0xffffffff
        for digits in range(6, 10):
            text = '%.*g' % (digits, as_number(t, raw))
            if struct.unpack('<I', struct.pack('<f', float(text)))[0] == raw:
                return text
    return '(a table)'


NUMBER = re.compile(r'-?\d+(\.\d+)?(e-?\d+)?')


def same_cell(cell, have):
    if cell == have or not (NUMBER.fullmatch(cell) and NUMBER.fullmatch(have)):
        return cell == have
    return struct.pack('<f', float(cell)) == struct.pack('<f', float(have))


def write_cell(buf, off, cell):
    """The value at `off` becomes `cell`, its type kept (a fraction makes an int a float)."""
    d, i, crc, t, cnt = rec(buf, off)
    if t == T_STR:
        REC.pack_into(buf, off, append(buf, cell.encode() + b'\0'), i, crc, t, len(cell.encode()) + 1)
    elif t == T_BOOL and cell in ('true', 'false'):
        REC.pack_into(buf, off, (d & ~0xffffffff) | (cell == 'true'), i, crc, t, cnt)
    elif t in (T_INT, T_FLOAT) and NUMBER.fullmatch(cell):
        number = float(cell)
        t = T_INT if t == T_INT and number == int(number) else T_FLOAT
        raw = int(number) & 0xffffffff if t == T_INT else struct.unpack('<I', struct.pack('<f', number))[0]
        REC.pack_into(buf, off, (d & ~0xffffffff) | raw, i, crc, t, cnt)
    else:
        raise ValueError('%r does not fit a value of type %d' % (cell, t))


def stats_values(buf, off, columns, row):
    """{column: [cell of each value its key path selects under `off`, in order]}; a path whose selector
    selects nothing is refused."""
    out = {}
    for c, key in columns.items():
        out[c] = [cell_text(buf, o) for _, o in resolve(buf, key, off)]
        if not out[c] and '[' in key:
            raise ValueError('row %s: %s matches nothing' % (row, key))
    return out


def stats_cells(buf, off, columns, row):
    """{column: cell} of the values under `off` ('' none; values that differ: a|b)."""
    return {c: '|'.join(sorted(set(v))) for c, v in stats_values(buf, off, columns, row).items()}


def stats_differences(buf, off, columns, cells, row):
    """[(column, key path, cell, config value)] where the values under `off` disagree with the row."""
    have = stats_cells(buf, off, columns, row)
    return [(c, key, cells[c], have[c]) for c, key in columns.items() if not same_cell(cells[c], have[c])]


def stats_unmatched(category, rows, targets, table):
    """A config without its row, a row without its config."""
    return (['%s: no row in %s' % (stats_label(category, row, p), table)
             for row, places in targets.items() if row not in rows for p, _ in places] +
            ['%s: a row of %s, no such config' % (stats_label(category, row), table) for row in rows if row not in targets])


def stats_problems(category, columns, rows, targets, files, table):
    """Why the configs are not the table: stats_unmatched, and each cell its values are not."""
    out = stats_unmatched(category, rows, targets, table)
    for row, places in targets.items():
        for p, off in places if row in rows else ():
            try:
                out += ['%s: %s (%s) is %s, its row says %s' % (stats_label(category, row, p), column, key,
                                                                have or 'absent', cell or 'nothing')
                        for column, key, cell, have in stats_differences(files[p], off, columns, rows[row], row)]
            except ValueError as why:
                out.append('%s: %s' % (stats_label(category, row, p), why))
    return out


def stats_write(category, columns, rows, data, table):
    """The rows' cells and the card lines they make written into `data` ({path: buf}; a key/value
    table's config made when it is not there): ({path: [walk paths written]}, {paths made})."""
    made = set()
    if 'columns' not in category and category['configs'] not in data:
        data[category['configs']] = new_config(rows[''])
        made.add(category['configs'])
    targets = stats_targets(category, data)
    unmatched = stats_unmatched(category, rows, targets, table)
    if unmatched:
        raise ValueError(unmatched[0])
    written = {p: [] for p in made}
    for row, places in sorted(targets.items()):
        for p, off in places:
            b = data[p]
            for column, key, cell, have in stats_differences(b, off, columns, rows[row], row):
                spots = resolve(b, key, off)
                if not cell or not spots:
                    raise ValueError('%s %s (%s): the row says %r, the config %r (an empty cell: no such key)'
                                     % (stats_label(category, row, p), column, key, cell, have))
                for at, o in spots:
                    if not same_cell(cell, cell_text(b, o)):
                        write_cell(b, o, cell)
                        written.setdefault(p, []).append(at)
                print('%s %s: %s -> %s' % (stats_label(category, row, p), column, have, cell))
            if stats_differences(b, off, columns, rows[row], row):
                raise ValueError('%s: does not read back as its row' % stats_label(category, row, p))
            for prop_id, text, want in card_differences(b, category['cards']) if category.get('cards') else ():
                written.setdefault(p, []).append(set_prop(b, prop_id, text, want))
                print('card %s %d: %s -> %s' % (p, prop_id, text, want))
    print('%s: %d rows, %d files written' % (table, len(rows), len(written)))
    return written, made


def stats_moved(category, columns, players, now):
    """[label key] of each match column whose values (each selected one, in order) differ between
    players' configs and the new ones ({path: buf}; a config players lack has none)."""
    match = {c: key for c, key in columns.items() if c in category['match']}

    def values(files):
        return {(row, p): stats_values(files[p], off, match, row)
                for row, places in stats_targets(category, files).items() for p, off in places}
    was, have = values(players), values(now)
    return ['%s %s' % (stats_label(category, row, p), key) for row, p in sorted(was.keys() | have.keys())
            for c, key in match.items() if was.get((row, p), {}).get(c, []) != have.get((row, p), {}).get(c, [])]


def card_differences(buf, card):
    """[(prop_id, card text, text from the config)] of the card lines that disagree with `card(buf)`."""
    try:
        lines = children(buf, find(buf, 'ui_desc/props_list'))
    except KeyError:
        return []
    want, out = card(buf), []
    for entry in lines:
        prop_id, text = (value_of(buf, find(buf, k, entry)) for k in ('prop_id', 'prop_value'))
        if prop_id in want and want[prop_id] != text:
            out.append((prop_id, text, want[prop_id]))
    return sorted(out)


def new_config(cells):
    """A config of key paths and cells: `/` a named table, `[i]` an indexed one (items from 0); a cell
    an int, a float (with a point), true/false or a text."""
    root = {}
    for key, cell in cells.items():
        steps = [int(i) if i else name for i, name in re.findall(r'\[(\d+)\]|([^/\[\]]+)', key)]
        node = root
        for step in steps[:-1]:
            node = node.setdefault(step, {})
        node[steps[-1]] = (cell == 'true' if cell in ('true', 'false') else int(cell) if re.fullmatch(r'-?\d+', cell)
                           else float(cell) if NUMBER.fullmatch(cell) else cell)
    buf = bytearray(24)
    buf[:24] = value_record(buf, append(buf, b'\0'), '', root)
    return buf


def live_files(live):
    """{game path: file} of a folder of files at their game paths or flattened (a__b)."""
    return {game_path(os.path.relpath(os.path.join(root, n), live).replace(os.sep, '/')): os.path.join(root, n)
            for root, _, names in os.walk(live) for n in names}


def dump_stats(name, live):
    category = STATS[name]
    files = live_files(live)
    data = {p: bytearray(open(files[p], 'rb').read()) for p in stats_paths(category, files)}
    if 'columns' not in category:
        buf = data.get(category['configs'], bytearray(24))
        print('key\tvalue')
        for q, t, _ in walk(buf)[1:]:
            if t not in (T_NAMED, T_INDEXED):
                print('%s\t%s' % (q[1:], cell_text(buf, find(buf, q[1:]))))
        return
    lines, mixed = ['\t'.join([name] + list(category['columns']))], []
    for row, places in sorted(stats_targets(category, data).items()):
        cells = {c: set() for c in category['columns']}
        for p, off in places:
            for c, v in stats_values(data[p], off, category['columns'], row).items():
                cells[c].update(v or [''])
        mixed += ['row %s %s: %s' % (row, c, ' | '.join(sorted(v))) for c, v in cells.items() if len(v) > 1]
        lines.append('\t'.join([row] + [v.pop() for v in cells.values()]))
    if mixed:
        sys.exit('values that differ within a row (a cell unifies them: pick one):\n' + '\n'.join(mixed))
    print('\n'.join(lines))


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
    def at_key(k):
        try:
            return find(buf, k, entry)
        except KeyError:
            return None
    for k, v in frm.items():
        c = at_key(k)
        if c is None or not same(value_of(buf, c), v):
            raise ValueError('%s[%d]/%s is %r, not %r: the base is not the live data'
                             % (path, at, k, None if c is None else value_of(buf, c), v))
    for k, v in to.items():
        c = at_key(k)
        if c is None:
            raise KeyError('%s[%d]/%s' % (path, at, k))
        d, i, crc, t, cnt = rec(buf, c)
        if t == T_STR:
            new = v.encode()
            REC.pack_into(buf, c, append(buf, new + b'\0'), i, crc, t, len(new) + 1)
        elif t > T_STR:
            if not isinstance(v, list) or len(v) != t - 4:
                raise ValueError('%s[%d]/%s takes %d numbers' % (path, at, k, t - 4))
            REC.pack_into(buf, c, append(buf, struct.pack('<%df' % len(v), *v)), i, crc, t, cnt)
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


def add_item(buf, path, where, key, value):
    """`value` after the items of the indexed table `key` of the one entry of `path` whose keys match
    `where`, a record like its first item. Returns (the entry's index, the new item's index)."""
    lst = find(buf, path)
    hit = [i for i, e in enumerate(children(buf, lst)) if matches(buf, e, where)]
    if len(hit) != 1:
        raise ValueError('%s: %d entries match %r, not 1' % (path, len(hit), where))
    table = find(buf, key, children(buf, lst)[hit[0]])
    d, i, crc, t, cnt = rec(buf, table)
    items = children(buf, table)
    if t != T_INDEXED or not items:
        raise ValueError('%s[%d]/%s is not an indexed table with items' % (path, hit[0], key))
    _, ii, icrc, it, icnt = rec(buf, items[0])
    if it == T_STR and isinstance(value, str):
        text = value.encode()
        new = REC.pack(append(buf, text + b'\0'), ii, icrc, it, len(text) + 1)
    elif it == T_INT and isinstance(value, int) and not isinstance(value, bool):
        new = REC.pack(value & 0xffffffff, ii, icrc, it, icnt)
    elif it == T_FLOAT and isinstance(value, (int, float)) and not isinstance(value, bool):
        new = REC.pack(struct.unpack('<I', struct.pack('<f', value))[0], ii, icrc, it, icnt)
    else:
        raise ValueError('%s[%d]/%s holds items of type %d, not %r' % (path, hit[0], key, it, value))
    old = [bytes(buf[c:c + 24]) for c in items]
    REC.pack_into(buf, table, append_array(buf, old + [new]), i, crc, t, cnt + 1)
    return hit[0], cnt


def append_rows(buf, path, rows):
    """`rows` (lists of strings) after the items of the indexed table `path`, each an indexed table
    of strings made like its first item (a UI mapping's [ui key, dictionary key]). Returns the index
    of the first new item."""
    lst = find(buf, path)
    d, i, crc, t, cnt = rec(buf, lst)
    items = children(buf, lst)
    if t != T_INDEXED or not items or rec(buf, items[0])[3] != T_INDEXED:
        raise ValueError('%s is not an indexed table of indexed tables' % path)
    _, ri, rcrc, _, _ = rec(buf, items[0])
    first = children(buf, items[0])
    if not first or rec(buf, first[0])[3] != T_STR:
        raise ValueError('%s[0] holds no string' % path)
    _, si, scrc, _, _ = rec(buf, first[0])
    new = []
    for row in rows:
        texts = [REC.pack(append(buf, v.encode() + b'\0'), si, scrc, T_STR, len(v.encode()) + 1) for v in row]
        new.append(REC.pack(append_array(buf, texts), ri, rcrc, T_INDEXED, len(row)))
    old = [bytes(buf[c:c + 24]) for c in items]
    REC.pack_into(buf, lst, append_array(buf, old + new), i, crc, t, cnt + len(new))
    return cnt


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


def make_pose_clip(source, bone_names, spec):
    """Author a full pose clip from a pinned stock rig, with cubic ease in/out.

    Zero-time full-pose channels supply translations, scales and unedited
    bones. Four clamped controls (a,a,b,b), then the final unused control,
    give a + (b-a)*(3*t*t-2*t*t*t) in Vostok's cubic spline basis. Rotations
    are local XYZ radians, deliberately authored on one continuous branch.
    This emits animation assets only; it never edits a stock weapon clip.
    """
    if hashlib.sha256(source).hexdigest() != spec['source_sha256'] \
            or hashlib.sha256(bone_names).hexdigest() != spec['names_sha256']:
        raise ValueError('make_pose_clip: source animation/rig differs from the authored version')
    if source[3] != 0:
        raise ValueError('make_pose_clip: a full pose is required, not an additive clip')
    frames = spec['frames']
    if not isinstance(frames, (int, float)) or not math.isfinite(frames) or not 0 < frames <= 3600:
        raise ValueError('make_pose_clip: frames must be finite and in (0, 3600]')
    names = []
    for at in children(bone_names, find(bone_names, 'bones_names')):
        data, _, _, kind, count = rec(bone_names, at)
        if kind != T_STR:
            raise ValueError('make_pose_clip: bone name is not a string')
        names.append(bytes(bone_names[data:data + count - 1]).decode())
    if len(set(names)) != len(names) or len(names) != CLIP_HEAD.unpack_from(source)[0]:
        raise ValueError('make_pose_clip: bone names do not match the clip')
    for pose in (spec['start'], spec['end']):
        for name, angles in pose.items():
            if name not in names or len(angles) != 3 \
                    or any(not isinstance(a, (int, float)) or not math.isfinite(a) for a in angles):
                raise ValueError('make_pose_clip: unknown bone or invalid XYZ rotation: ' + name)
    tracks = clip_parse(source)[2]
    out = bytearray(CLIP_HEAD.pack(len(names), 0, 0) + bytes(len(names) * CLIP_BONE))
    offset = 0
    for name in names:
        for axis in range(9):
            count = struct.unpack_from('<I', tracks, offset)[0]
            pairs = [struct.unpack_from('<ff', tracks, offset + 4 + i * 8) for i in range(count)]
            offset += 4 + count * 8
            if count < 5 or any(k != 0 for k, _ in pairs[:4]) or pairs[4][0] <= 0:
                raise ValueError('make_pose_clip: source must have a clamped zero-time pose')
            start = end = pairs[0][1]
            if axis in (3, 4, 5):
                start = spec['start'].get(name, [start] * 3)[axis - 3]
                end = spec['end'].get(name, [end] * 3)[axis - 3]
                if abs(end - start) > math.pi:
                    raise ValueError('make_pose_clip: author rotations on the shortest branch: ' + name)
            if not math.isfinite(start) or not math.isfinite(end):
                raise ValueError('make_pose_clip: nonfinite source channel')
            out += struct.pack('<I10f', 5, 0, start, 0, start, 0, end, 0, end, frames, -1e9)
    clip_parse(out)
    return out


def make_held_flower(sources, spec):
    """A separate, pinned static model: discard the stem and centre the flower.

    0.26g static meshes have chunks 3 (u32 count, 32-byte vertices) and
    4 (u32 count, u16 indices). Packed bases, UVs and colours stay byte exact.
    The source model is never overwritten; only the held renderer uses this.
    """
    prefix, output = spec['source'], spec['output']
    if (not output.startswith('models/') or not output.endswith('.model/')
            or '..' in output.split('/') or output == prefix):
        raise ValueError('held flower: expected a distinct model resource path')
    if set(sources) != set(spec['sha256']):
        raise ValueError('held flower: source file set changed')
    for path, data in sources.items():
        if hashlib.sha256(data).hexdigest() != spec['sha256'][path]:
            raise ValueError('held flower: source changed: ' + path)
    mesh_path = next(p for p in sources if p.endswith('/converted_model'))
    mesh = sources[mesh_path]
    kind, size, count = struct.unpack_from('<III', mesh)
    end = 8 + size
    ikind, isize, icount = struct.unpack_from('<III', mesh, end)
    if (kind != 3 or size != 4 + count * 32 or ikind != 4
            or isize != 4 + icount * 2 or end + 8 + isize != len(mesh) or icount % 3):
        raise ValueError('held flower: unexpected static mesh layout')
    positions = [struct.unpack_from('<3f', mesh, 12 + i * 32) for i in range(count)]
    indices = struct.unpack_from('<%dH' % icount, mesh, end + 12)
    if any(i >= count for i in indices) or any(not math.isfinite(x) for p in positions for x in p):
        raise ValueError('held flower: invalid geometry')
    floor = spec['minimum_y']
    kept = [indices[i:i+3] for i in range(0, icount, 3)
            if all(positions[j][1] >= floor for j in indices[i:i+3])]
    used = sorted({j for tri in kept for j in tri})
    if not used or len(used) == count:
        raise ValueError('held flower: crop must remove stem and retain flower')
    lo = [min(positions[j][i] for j in used) for i in range(3)]
    hi = [max(positions[j][i] for j in used) for i in range(3)]
    centre = [(a+b)/2 for a,b in zip(lo,hi)]
    vertices = bytearray()
    for j in used:
        v = bytearray(mesh[12+j*32:12+(j+1)*32])
        struct.pack_into('<3f', v, 0, *(x-c for x,c in zip(positions[j],centre)))
        vertices.extend(v)
    mapping = {old:new for new,old in enumerate(used)}
    ix = [mapping[j] for tri in kept for j in tri]
    result = {output + p[len(prefix):]: bytearray(data) for p,data in sources.items()}
    result[output + mesh_path[len(prefix):]] = (struct.pack('<III',3,4+len(vertices),len(used)) + vertices
        + struct.pack('<III',4,4+len(ix)*2,len(ix)) + struct.pack('<%dH'%len(ix),*ix))
    # Update both aggregate and surface bounds, and the surface mesh counts.
    for path, data in result.items():
        if not path.endswith('/export_properties'): continue
        for name, values in [('bounding_box/min',[a-c for a,c in zip(lo,centre)]),
                             ('bounding_box/max',[a-c for a,c in zip(hi,centre)])]:
            at,_,_,kind,_ = rec(data,find(data,name))
            if kind != 7: raise ValueError('held flower: expected float3 bounds')
            struct.pack_into('<3f',data,at,*values)
        if '/render/export_properties' in path: continue
        at,_,_,kind,_ = rec(data,find(data,'bounding_sphere/center'))
        if kind != 7: raise ValueError('held flower: expected float3 centre')
        struct.pack_into('<3f',data,at,0,0,0)
        radius = max(math.sqrt(sum((x-c)**2 for x,c in zip(positions[j],centre))) for j in used)
        for name, value in [('bounding_sphere/radius',radius),('source/vertices',len(used)),
                            ('source/indices',len(ix)),('source/primitives',len(kept))]:
            raw,_,_,kind,_=rec(data,find(data,name))
            set_number(data,name,as_number(kind,raw & 0xffffffff),value)
    return result


def vegetation_chunks(data):
    """The 0.26g sequential chunk format; validate before dropping editor data."""
    chunks = {}
    offset = 0
    while offset < len(data):
        if len(data) - offset < 8:
            raise ValueError('vegetation: truncated chunk header')
        kind, size = struct.unpack_from('<II', data, offset)
        end = offset + 8 + size
        if kind not in (0x4d, 0x4e, 0x58, 0x59):
            raise ValueError('vegetation: unknown chunk %#x' % kind)
        if kind in chunks or end > len(data):
            raise ValueError('vegetation: duplicate or truncated chunk %#x' % kind)
        chunks[kind] = data[offset:end]
        offset = end
    if 0x58 not in chunks or 0x59 not in chunks:
        raise ValueError('vegetation: missing positions or normals')
    positions, normals = chunks[0x58][8:], chunks[0x59][8:]
    if len(positions) != len(normals) or len(positions) % 12:
        raise ValueError('vegetation: positions/normals must be matching float3 arrays')
    for payload in (positions, normals):
        if any(not math.isfinite(v) for (v,) in struct.iter_unpack('<f', payload)):
            raise ValueError('vegetation: non-finite position or normal')
    return chunks


def runtime_vegetation(data):
    """Keep complete runtime chunks in original order, including their headers."""
    original = vegetation_chunks(data)
    expected = {kind: chunk for kind, chunk in original.items() if kind in (0x58, 0x59)}
    result = b''.join(expected.values())
    if vegetation_chunks(result) != expected:
        raise ValueError('vegetation: runtime chunks changed on read-back')
    return result


def require_headwear_bind(config, paths):
    """The character cook must use one of the render binds this edit extends."""
    data, _, _, kind, count = rec(config, find(config, 'body_parts/bind_pose'))
    if kind != T_STR:
        raise TypeError('body_parts/bind_pose must be a resource path string')
    reference = bytes(config[data:data + count - 1]).decode('utf-8')
    prefix = 'resources/'
    if not reference.startswith(prefix) or reference[len(prefix):] not in paths:
        raise ValueError('character bind is not extended by append_headwear_bind: ' + reference)


def main():
    if sys.argv[1:2] == ['--dump-stats']:
        return dump_stats(*sys.argv[2:4])
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    check = '--check' in sys.argv
    if len(args) != 3:
        sys.exit(__doc__)
    edits_file, live, out = args
    stats = read_stats(edits_file) if edits_file.endswith('.tsv') else None
    edits = json.load(open(edits_file, encoding='utf-8')) if stats is None else {}
    if not check and os.path.exists(out):
        sys.exit('%s: already there' % out)
    files = live_files(live)
    bufs, made = {}, set()

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
    for e in edits.get('promote_rolled_defaults', []):
        changed = promoted_defaults(buf, set(e['categories']), e['from_top'])
        for cfg, path, frm, to in changed:
            set_number(buf(cfg), path, frm, to)
            allowed.setdefault(cfg, set()).add('/' + path)
            print('built-in %s %s: %s -> %s' % (cfg, path, frm, to))
        print('promote_rolled_defaults: %d defaults' % len(changed))
    for e in edits.get('set', []):
        set_number(buf(e['file']), e['path'], e['from'], e['to'], e.get('as_float', False))
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('set_vector', []):
        set_vector(buf(e['file']), e['path'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('set_bool', []):
        set_bool(buf(e['file']), e['path'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('set_entry', []):
        at = set_entry(buf(e['file']), e['list'], e['where'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).update('/%s[%d]/%s' % (e['list'], at, k) for k in e['to'])
    if stats is not None:
        category, columns, rows = stats
        data = {p: bytearray(open(files[p], 'rb').read()) for p in stats_paths(category, files)}
        written, made = stats_write(category, columns, rows, data, os.path.basename(edits_file))
        bufs.update({p: data[p] for p in written})
        for p, paths in written.items():
            allowed.setdefault(p, set()).update(paths)
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
    gone = {}
    for e in edits.get('remove_key', []):
        for key in e['keys']:
            n = 0
            for p in sorted(fnmatch.filter(list(files) + list(copies), e['files'])):
                b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
                if remove_key(b, key):
                    bufs[p] = b
                    gone.setdefault(p, []).append('/' + key)
                    table = '/' + key.rpartition('/')[0] if '/' in key else ''
                    grown.setdefault(p, {}).setdefault(table, 0)
                    grown[p][table] -= 1
                    n += 1
            if not n:
                sys.exit('remove_key: no file of %s has %s' % (e['files'], key))
            print('remove %s: %d files' % (key, n))
    if 'localization' in edits:
        from pathlib import Path
        sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'client-patches'))
        from build_localization_026g import compile_catalog
        from localization_026g import FLASH_MOVIES, LANGUAGES, flash_rows, read_config, read_translations
        directory = Path(edits_file).resolve().parent / edits['localization']
        languages = [language for language, *_ in LANGUAGES] + ['original']
        # New languages inherit opaque support files (EULA/profanity data) from
        # English. Every dictionary, old or new, is compiled from its own TSV.
        for language in languages:
            p = f'localization/{language}/localization'
            if p not in files:
                for source in sorted(fnmatch.filter(files, 'localization/english/*')):
                    target = source.replace('localization/english/', f'localization/{language}/', 1)
                    if target not in files:
                        copies[target] = source
                        bufs[target] = bytearray(open(files[source], 'rb').read())
        sources = {language: bytes(buf(f'localization/{language}/localization'))
                   for language in languages}
        compiled = compile_catalog(directory, sources)
        mapping = 'localization/flash_movies_localizations.options'
        if mapping in files:
            config, errors = read_config(bytes(buf(mapping)))
            if errors:
                raise ValueError('UI mapping contains invalid UTF-8')
            keys = read_translations(directory / 'english.tsv').keys()
            for movie, prefix in FLASH_MOVIES.items():
                table = movie + '_strings'
                have = {row[0]: row[1] for row in config[table]}
                if any(have.get(key, ui) != ui for ui, key in flash_rows(keys, prefix)):
                    raise ValueError(f'{table}: an srp text key mapped to another text')
                rows = [row for row in flash_rows(keys, prefix) if row[0] not in have]
                if rows:
                    at = append_rows(buf(mapping), table, rows)
                    for k in range(at, at + len(rows)):
                        added.setdefault(mapping, []).extend(
                            ['/%s[%d]' % (table, k), '/%s[%d][0]' % (table, k), '/%s[%d][1]' % (table, k)])
                    grown.setdefault(mapping, {}).setdefault('/' + table, 0)
                    grown[mapping]['/' + table] += len(rows)
                    config[table] += rows
                print('UI mapping %s: %d texts of srp mapped to themselves' % (table, len(rows)))
            referenced = {row[1] for rows in config.values() for row in rows}
            missing = referenced - keys
            if missing:
                raise ValueError(f'UI mapping keys missing from TSV: {sorted(missing)}')
        for language, result in compiled.items():
            p = f'localization/{language}/localization'
            source = sources[language]
            bufs[p] = bytearray(result)
            old_keys = {q for q, t, _ in walk(source) if t == T_STR}
            new_keys = {q for q, t, _ in walk(result) if t == T_STR}
            allowed.setdefault(p, set()).update(old_keys)
            added.setdefault(p, []).extend(sorted(new_keys - old_keys))
            grown.setdefault(p, {})['/strings'] = len(new_keys) - len(old_keys)
            print('localization %s: %d keys, no fallback' % (p, len(new_keys)))
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
    for e in edits.get('append_rows', []):
        k = append_rows(buf(e['file']), e['path'], e['rows'])
        table = '/' + e['path']
        added.setdefault(e['file'], []).extend('%s[%d]' % (table, i) for i in range(k, k + len(e['rows'])))
        grown.setdefault(e['file'], {}).setdefault(table, 0)
        grown[e['file']][table] += len(e['rows'])
    for e in edits.get('add_item', []):
        at, k = add_item(buf(e['file']), e['list'], e['where'], e['key'], e['value'])
        table = '/%s[%d]/%s' % (e['list'], at, e['key'])
        added.setdefault(e['file'], []).append('%s[%d]' % (table, k))
        grown.setdefault(e['file'], {}).setdefault(table, 0)
        grown[e['file']][table] += 1
        print('item %s += %s' % (table, json.dumps(e['value'], ensure_ascii=False)))
    removed = {}
    before_removals = {}  # includes newly copied tables and appended items
    for e in edits.get('remove_item', []):
        before_removals.setdefault(e['file'], {}).setdefault(
            e['path'], items_of(buf(e['file']), e['path']))
        remove_item(buf(e['file']), e['path'], e['value'])
        removed.setdefault(e['file'], {}).setdefault(e['path'], []).append(e['value'])
    for e in edits.get('append_item', []):
        k = len(items_of(buf(e['file']), e['path']))
        append_item(buf(e['file']), e['path'], e['value'])
        # The check below expects the new item and the table's count one more.
        table = '/' + e['path']
        added.setdefault(e['file'], []).append('%s[%d]' % (table, k))
        grown.setdefault(e['file'], {}).setdefault(table, 0)
        grown[e['file']][table] += 1
        print('item %s += %s' % (e['path'], json.dumps(e['value'], ensure_ascii=False)))
    dropped = {}
    for e in edits.get('remove_entry', []):
        at = remove_entry(buf(e['file']), e['list'], e['where'])
        lst = '/' + e['list']
        print('remove %s[%d] %s' % (e['list'], at, json.dumps(e['where'], ensure_ascii=False)))
        dropped.setdefault(e['file'], []).append((lst, at))
        grown[e['file']] = {(unshifted(q, lst, at) or q): n for q, n in grown.get(e['file'], {}).items()}
        grown[e['file']].setdefault(lst, 0)
        grown[e['file']][lst] -= 1
        allowed[e['file']] = {q for q in (unshifted(q, lst, at) for q in allowed.get(e['file'], ())) if q is not None}
        if any(unshifted(a, lst, at) is None for a in added.get(e['file'], ())):
            sys.exit('remove_entry: %s[%d] is an entry this list adds' % (lst, at))
        added[e['file']] = [unshifted(a, lst, at) for a in added.get(e['file'], ())]
    clips = {}  # animation clips: not configs, so add_event_channel reads them back itself
    for e in edits.get('make_held_flower', []):
        made = make_held_flower({p: open(files[p], 'rb').read() for p in e['files']}, e)
        for p, data in made.items():
            if p in files or p in bufs or p in clips:
                raise ValueError('held flower: destination exists: ' + p)
            clips[p] = data
        print('held flower: %d separate model resources' % len(made))
    for e in edits.get('make_pose_clip', []):
        source, names = e['files']
        stem = e['output']
        if not stem.startswith('animations/') or '..' in stem.split('/') or '\\' in stem:
            raise ValueError('make_pose_clip: output must be a new animations/ resource path')
        for suffix in ('.b-spline', '.bones_names'):
            if stem + suffix in files or stem + suffix in bufs or stem + suffix in clips:
                raise ValueError('make_pose_clip: output already exists: ' + stem + suffix)
        source_bytes = open(files[source], 'rb').read()
        names_bytes = open(files[names], 'rb').read()
        clips[stem + '.b-spline'] = make_pose_clip(source_bytes, names_bytes, e)
        clips[stem + '.bones_names'] = names_bytes
        print('pose clip %s: %g frames, %d authored bones' % (stem, e['frames'], len(e['end'])))
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

    vegetation = {}
    for e in edits.get('runtime_vegetation', []):
        paths = sorted(fnmatch.filter(files, e['files']))
        if not paths:
            sys.exit('runtime_vegetation: no files match ' + e['files'])
        for p in paths:
            if not p.endswith('.veg') or p in bufs or p in clips:
                sys.exit('runtime_vegetation: invalid or conflicting edit of ' + p)
            source = open(files[p], 'rb').read()
            try:
                vegetation[p] = runtime_vegetation(source)
            except ValueError as why:
                sys.exit('%s: %s' % (p, why))
            print('runtime vegetation %s: %d -> %d bytes; positions/normals unchanged' %
                  (p, len(source), len(vegetation[p])))

    for e in edits.get('require_headwear_bind', []):
        source = open(files[e['file']], 'rb').read()
        require_headwear_bind(source, {b['file'] for b in edits.get('append_headwear_bind', [])})
        print('character bind reference verified: ' + e['file'])

    # Binary render assets: each operation checks its format and unchanged bytes.
    if edits.get('append_headwear_bind') or edits.get('rigid_headwear'):
        import importlib.util
        spec = importlib.util.spec_from_file_location('headwear', os.path.join(os.path.dirname(__file__), 'headwear.py'))
        headwear = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(headwear)
        for operation, transform in (
            ('append_headwear_bind', lambda b, e: headwear.append_headwear_bind(b)),
            ('rigid_headwear', lambda b, e: headwear.rigid_headwear(b, e['vertices'], e.get('allow_bones', ()))),
        ):
            for e in edits.get(operation, []):
                p = e['file']
                if p in bufs or p in clips:
                    sys.exit('%s: %s already edited' % (operation, p))
                if p not in files:
                    sys.exit('%s: not in the live folder' % p)
                source = open(files[p], 'rb').read()
                clips[p] = transform(source, e)
                print('%s %s: %d -> %d bytes' % (operation, p, len(source), len(clips[p])))

    # read back: compare every value with the live file, by path
    renames = [('/%s/%s' % (e['table'], e['from']), '/%s/%s' % (e['table'], e['to']), e['from'], e['to'])
               for e in edits.get('rename_key', [])]
    strings = [(e['from'].encode(), e['to'].encode()) for e in edits.get('replace_string', [])]
    bad = same_copies = 0
    for p, b in sorted(bufs.items()):
        if p in made:
            print('%s: a new config, %d values' % (p, len(walk(b))))
            continue
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
            # A copied entry has no path in the original file. Compare with its
            # table just before removals, then follow any removed parent entries.
            end = '/' + lst
            for l2, at in dropped.get(p, ()):
                end = unshifted(end, l2, at)
                if end is None:
                    break
            if end is None:
                continue
            was, now = before_removals[p][lst], items_of(b, end[1:])
            want = list(was)
            for v in values:
                want.remove(v)
            ok = now == want
            bad += not ok
            print('%s%s %s: %s -> %s' % ('' if ok else 'UNEXPECTED ', p, end, was, now))
            for d in (old, new):
                for q in [q for q in d if q == end or q.startswith(end + '[')]:
                    del d[q]
        for q in sorted(set(old) | set(new)):
            o, n = old.get(q), new.get(q)
            if o == n:
                continue
            ok = (q in allowed.get(p, ()) and o and n and n[0] in (o[0], T_FLOAT)) or \
                (o and n and o[0] == n[0] == T_STR and (o[1], n[1]) in strings) or \
                (o and n and o[0] == n[0] == T_STR and any(q.endswith('/' + k.decode()) and n[1] == x + o[1]
                                                           for k, x in prefixed.get(p, ()))) or \
                (o is None and any(q == a or q.startswith(a + '/') or q.startswith(a + '[') for a in added.get(p, ()))) or \
                (n is None and any(q == g or q.startswith(g + '/') or q.startswith(g + '[') for g in gone.get(p, ()))) or \
                (o and n and o[0] == n[0] and n[1] - o[1] == grown.get(p, {}).get(q, 0) != 0)
            bad += not ok
            print('%s%s %s: %s -> %s' % ('' if ok else 'UNEXPECTED ', p, q, show(o), show(n)))
    if copies:
        print('%d new files (copies), %d of them byte for byte' % (len(copies), same_copies))
    print('%d files edited, %d unexpected differences' % (len(bufs) + len(clips) + len(vegetation), bad))
    if bad:
        sys.exit(1)
    if check:
        return
    for p, b in list(bufs.items()) + list(clips.items()) + list(vegetation.items()):
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
