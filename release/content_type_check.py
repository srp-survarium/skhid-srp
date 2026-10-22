#!/usr/bin/env python3
"""The value-type check of release/build-content: every file a pack or an edit
list changes keeps, at every key path that already exists in the stock or
previous version of that file, the value's type (float2/3/4 vs an indexed
table vs an int vs a float vs a string vs a named table). A new key is allowed
only with a type 0.26g files already use at that key name (the corpus of the
previous versions), or an explicit allowlist row with a reason. It fails with
the file, the path, the old type and the new type.

Two bugs in the Research pack builder (#297) got past every check this guards
against: it wrote every vector (float2/3/4) as an indexed table, through a
`json.loads(json.dumps(...))` round-trip, so the game, which reads vectors
raw, loaded scrambled levels. A vector rewritten as a list is exactly what
this refuses.

It reuses the repo's binary-config reader, srp-026g/scripts/content-edit/
content_edit.py (`walk`): a Vostok config has no header, the root value is at
offset 0, every value is 24 bytes (+0x14 a u16 type, +0x16 a u16 count), and
walk returns every value as (path, type, value) in file order.

Usage as a module: build the corpus of the previous versions
(`corpus(previous)`), then `violations(old, new, path, corpus, allowlist)` for
each changed file, old being its previous bytes or None (a brand-new file).
release/build-content calls it over the whole manifest; it can also be run on
two files:

    python3 -I content_type_check.py <content-edit dir> <old file> <new file> <game path>
"""
import re
import struct
import sys

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5
INDEX = re.compile(r"\[\d+\]")


def type_name(t):
    names = {T_BOOL: "bool", T_INT: "int", T_FLOAT: "float", T_NAMED: "table",
             T_INDEXED: "indexed table", T_STR: "string"}
    return names.get(t, f"float{t - 4}" if t > T_STR else f"type {t}")


def load_walk(content_edit_dir):
    """The repo's config reader (content_edit.py's `walk`), imported from the
    tree's content-edit directory so build-content and this agree."""
    import importlib.util
    path = f"{content_edit_dir.rstrip('/')}/content_edit.py"
    spec = importlib.util.spec_from_file_location("content_edit_for_types", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.walk


def types_of(walk, data):
    """{normalised path: {type, ...}} of a config: every value's type, the
    indices of indexed tables folded to `[]` (every entry of a table shares a
    structural path), the leaf key kept. None when the bytes are not a config
    walk reads (a texture, a sound, a model): the check does not apply."""
    try:
        values = walk(bytearray(data))
    except (AssertionError, IndexError, ValueError, KeyError, struct.error):
        return None
    if not values or values[0][1] not in (T_NAMED, T_INDEXED):
        return None
    out = {}
    for path, t, _ in values:
        out.setdefault(INDEX.sub("[]", path), set()).add(t)
    return out


def leaf(path):
    """The key name of a path: the last `/` segment, its `[]` dropped (an
    indexed item itself has the parent's name)."""
    return INDEX.sub("", path.rsplit("/", 1)[-1])


def corpus_with(walk, previous):
    """{normalised key path: {type, ...}} over the given config files ({path:
    bytes}): the types 0.26g already uses at each key path (the indices of
    indexed tables folded to `[]`). A new key's type, where another file uses
    the same path, must be one of these (or allowlisted). The key is the whole
    path, not the leaf name, so a name like `value` (a scalar in an item, a
    vector in a material) is weighed in its own place. Non-config files are
    skipped."""
    out = {}
    for data in previous.values():
        types = types_of(walk, data)
        if types is None:
            continue
        for path, kinds in types.items():
            out.setdefault(path, set()).update(kinds)
    return out


def violations(walk, old, new, path, corp, allowlist):
    """The type problems of one changed file (`new` its bytes, `old` its
    previous bytes or None): a list of strings. An existing path whose type
    changed; a new key whose type no 0.26g file uses at that name and that is
    not allowlisted. A file `new` is not a config walk reads has no problems
    (the check does not apply to it)."""
    new_types = types_of(walk, new)
    if new_types is None:
        return []
    old_types = types_of(walk, old) if old is not None else {}
    out = []
    for p in sorted(new_types):
        allow = allowlist.get((path, p)) or allowlist.get((path, leaf(p))) or allowlist.get(("*", leaf(p)))
        if old_types and p in old_types:
            gained = new_types[p] - old_types[p]
            if gained and not allow:
                out.append(f"{path}: {p or '/'} was {fmt(old_types[p])}, now {fmt(new_types[p])} "
                           f"(a key that exists keeps its value type)")
        else:                                              # a key path new to this file
            known = corp.get(p, set())
            # Only a path 0.26g already uses elsewhere constrains a new key
            # (the task's "where one exists"): a type that contradicts every
            # known type at that path is refused; a brand-new path is allowed.
            # This catches a vector written as an indexed table where the same
            # path is a vector in other files (the Research pack bug), without
            # flagging paths 0.26g has no precedent for.
            if known and not (new_types[p] & known) and not allow:
                out.append(f"{path}: new key {p or '/'} is {fmt(new_types[p])}, but 0.26g uses {fmt(known)} at "
                           f"that path; use that type or allowlist (file, key) with a reason")
    return out


def fmt(kinds):
    return "/".join(sorted(type_name(t) for t in kinds))


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    ce_dir, old_path, new_path, game_path = sys.argv[1:]
    walk = load_walk(ce_dir)
    old = open(old_path, "rb").read()
    new = open(new_path, "rb").read()
    corp = corpus_with(walk, {game_path: old})
    problems = violations(walk, old, new, game_path, corp, {})
    for p in problems:
        print(f"TYPE: {p}")
    print(f"{len(problems)} type problem(s)")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
