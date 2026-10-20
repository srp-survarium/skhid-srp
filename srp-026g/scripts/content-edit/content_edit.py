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
                    the same file; files without `only_if`, the table or those entries are left alone
  "add_key":        [{"file", "table", "key", "text"}] a new text of a localization table
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
    return as_number(t, d & 0xffffffff) if t != T_BOOL else d & 1


def matches(buf, entry, want):
    for k, v in want.items():
        c = child(buf, entry, k)
        if c is None or value_of(buf, c) != v:
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
            if t == T_STR:
                v = v.encode()
                d, cnt = append(buf, v + b'\0'), len(v) + 1
            elif t == T_INT:
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
    for e in edits.get('set', []):
        set_number(buf(e['file']), e['path'], e['from'], e['to'], e.get('as_float', False))
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('set_prop', []):
        q = set_prop(buf(e['file']), e['prop_id'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add(q)
    for e in edits.get('edit_text', []):
        edit_text(buf(e['file']), e['path'], e['from'], e['to'])
        allowed.setdefault(e['file'], set()).add('/' + e['path'])
    for e in edits.get('rename_key', []):
        n = 0
        for p in sorted(fnmatch.filter(files, e['files'])):
            b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
            if rename_key(b, e['table'], e['from'], e['to']):
                bufs[p] = b
                n += 1
        print('rename %s -> %s: %d files' % (e['from'], e['to'], n))
    for e in edits.get('replace_string', []):
        n = 0
        for p in sorted(fnmatch.filter(files, e['files'])):
            b = bytearray(open(files[p], 'rb').read()) if p not in bufs else bufs[p]
            k = replace_string(b, e['from'], e['to'])
            if k:
                bufs[p] = b
                n += k
        print('string %s -> %s: %d values' % (e['from'], e['to'], n))
    shifts, added, grown = {}, {}, {}
    for e in edits.get('add_entry', []):
        n = 0
        for p in sorted(fnmatch.filter(files, e['files'])):
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
            added.setdefault(p, []).append('%s[%d]' % (lst, at))
            grown.setdefault(p, set()).add(lst)
        print('entry %s after %s: %d files' % (e['list'], e['after'], n))
    for e in edits.get('add_key', []):
        add_key(buf(e['file']), e['table'], e['key'], e['text'])
        added.setdefault(e['file'], []).append('/%s/%s' % (e['table'], e['key']))
        grown.setdefault(e['file'], set()).add('/' + e['table'])

    # read back: compare every value with the live file, by path
    renames = [('/%s/%s' % (e['table'], e['from']), '/%s/%s' % (e['table'], e['to']), e['from'], e['to'])
               for e in edits.get('rename_key', [])]
    strings = [(e['from'].encode(), e['to'].encode()) for e in edits.get('replace_string', [])]
    bad = 0
    for p, b in sorted(bufs.items()):
        old = {q: (t, v) for q, t, v in walk(bytearray(open(files[p], 'rb').read()))}
        new = {q: (t, v) for q, t, v in walk(b)}
        for ro, rn, _, _ in renames:
            if ro in old and rn in new and rn not in old:
                old[rn] = old.pop(ro)
        for lst, at in shifts.get(p, ()):
            old = {shifted(q, lst, at): v for q, v in old.items()}
        for q in sorted(set(old) | set(new)):
            o, n = old.get(q), new.get(q)
            if o == n:
                continue
            ok = (q in allowed.get(p, ()) and o and n and n[0] in (o[0], T_FLOAT)) or \
                (o and n and o[0] == n[0] == T_STR and (o[1], n[1]) in strings) or \
                (o is None and any(q == a or q.startswith(a + '/') for a in added.get(p, ()))) or \
                (q in grown.get(p, ()) and o and n and o[0] == n[0] and n[1] == o[1] + 1)
            bad += not ok
            print('%s%s %s: %s -> %s' % ('' if ok else 'UNEXPECTED ', p, q, show(o), show(n)))
    print('%d files edited, %d unexpected differences' % (len(bufs), bad))
    if bad:
        sys.exit(1)
    if check:
        return
    for p, b in bufs.items():
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
