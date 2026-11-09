#!/usr/bin/env python3
"""The value-type check of release/build-content: every file a pack or an edit
list changes keeps, value by value, at every key path that already exists in
the previous version of that file, the value's type (float2/3/4 vs an indexed
table vs an int vs a float vs a string vs a named table). A value at a path
new to the file has one of the types 0.26g already uses at that structural
path (the corpus of every base config), or an allowlist row with a reason
(new keys only). It fails with the file, the path, the old type and the new
type; build-content also refuses a file at a config path the reader refuses.

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
(`corpus(previous)`), then `violations(old, new, path, corpus, allowlist, keys)` for
each changed file, old being its previous bytes or None (a brand-new file).
release/build-content calls it over the whole manifest; it can also be run on
two files:

    python3 -I content_type_check.py <content-edit dir> <old file> <new file> <game path> [<edit list>...]

An entry of a list is weighed against the entry of the same identity: in a
list the edit lists key (append_entry's `unique_key`: list_keys, `keys` of
violations; build-content passes the manifest's edit lists, the CLI the
lists named after the game path) the one with the same key, else the one of
the same name or index (raw_types).
"""
import re
import struct
import sys

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5
INDEX = re.compile(r"\[[^\]]*\]")


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


RAMP_KEY = re.compile(r"/key\d+(?=/|$)")
ENTRY = re.compile(r"\[(\d+)\]")
ENTRY_OF = re.compile(r"(.*)\[\d+\]")


def normalise(path, kinds_of_entries):
    """A value's structural path, as Research's pack builder names them
    (scripts/research-mode/make_pack.py, project_paths): an indexed table's
    entry is `[<its game_object_type>]` when it has one (a project's
    client_objects hold objects of many types, each with its own keys) and
    `[]` otherwise; a colour ramp's `key0`, `key1`, ... are one `key#` (the
    game reads `key%d` from 0 while there is one, so a ramp may have more
    keys than 0.26g's have)."""
    out, at = [], 0
    for match in ENTRY.finditer(path):
        out.append(path[at:match.start()])
        prefix = path[:match.end()]
        kind = kinds_of_entries.get(prefix)
        out.append(f"[{kind}]" if kind else "[]")
        at = match.end()
    out.append(path[at:])
    return RAMP_KEY.sub("/key#", "".join(out))


# The paths whose files are configs walk must read (build-content's corpus
# and check): a file at one that walk refuses stops the build, never skips
# the check.
CONFIG_SUFFIXES = ('.options', '.raw_options', '.material', '/settings', '/export_properties',
                   '/hit_targets', '/client_project')


class NotAConfig(Exception):
    pass


def is_config_path(path):
    return path.endswith(CONFIG_SUFFIXES)


def parse(walk, data, config_path=True):
    """[(path, type, value)] of a config (walk's), or NotAConfig with why.
    At a config path any root walk reads is a config (the ported textures'
    `.options` of base patch 19 are a lone `true`); a file elsewhere is taken
    for one only with a table at its root, so a texture or a sound is never
    read as a config by chance."""
    try:
        values = walk(bytearray(data))
    except (AssertionError, IndexError, ValueError, KeyError, struct.error, RecursionError) as error:
        raise NotAConfig(f"{type(error).__name__}: {error}") from None
    if not values or (not config_path and values[0][1] not in (T_NAMED, T_INDEXED)):
        raise NotAConfig("its root is not a table")
    return values


def entry_kinds(values):
    """{an indexed table's entry path: its game_object_type}."""
    out = {}
    for path, t, value in values:
        if t == T_STR and path.endswith("]/game_object_type"):
            out[path[:-len("/game_object_type")]] = value.decode("utf-8", "replace")
    return out


def entry_names(values):
    """{an indexed table's entry path: its own name}: the entry's `full_name`
    or else `name` text, when no other entry of its table has it."""
    named = {}
    for path, t, value in values:
        for key in ("/full_name", "/name"):
            if t == T_STR and path.endswith("]" + key):
                entry = path[:-len(key)]
                named.setdefault(entry, {})[key] = value.decode("utf-8", "replace")
    out, seen = {}, {}
    for entry, keys in named.items():
        name = keys.get("/full_name") or keys.get("/name")
        table = entry[:entry.rindex("[")]
        seen.setdefault((table, name), []).append(entry)
        out[entry] = name
    return {e: n for e, n in out.items() if len(seen[(e[:e.rindex("[")], n)]) == 1}


def list_keys(edit_lists):
    """{game path: {a list's path (walk's, `/skills_tree`): the key that names its entries}}: the
    identity the edit lists give a list's entries, append_entry's `unique_key` (content_edit.py, which
    refuses a new entry without it or with a value another entry has), so the check knows each entry
    of that list by its key, as the edit that made it does. Two edits naming two keys for one list are
    refused."""
    out = {}
    for edits in edit_lists:
        for e in edits.get('append_entry', []):
            for file in e.get('files', [e.get('file')]):
                lists = out.setdefault(file, {})
                path, key = '/' + e['list'], e['unique_key']
                if lists.setdefault(path, key) != key:
                    raise ValueError(f"{file}: {path} is keyed by {lists[path]} and by {key} in the edit lists")
    return out


def key_text(t, value):
    """A scalar key's value as the own path shows it (`id=211`), or None for a table or a vector."""
    if t == T_STR:
        return value.decode("utf-8", "replace")
    if t == T_INT:
        return str(struct.unpack("<i", struct.pack("<I", value))[0])
    if t == T_FLOAT:
        return repr(struct.unpack("<f", struct.pack("<I", value))[0])
    if t == T_BOOL:
        return "true" if value else "false"
    return None


def keyed_entries(values, keys):
    """({an entry path of a keyed list (`keys`, list_keys' for the file): `<key>=<its value>`},
    [problems]): every entry of a keyed list has the key, a scalar, with a value no other entry of
    the list has; one that has not is a problem, never an entry silently known by its index."""
    entries = {}
    for path, _, _ in values:
        m = ENTRY_OF.fullmatch(path)
        if m and m.group(1) in keys:
            entries[path] = m.group(1)
    named, problems = {}, []
    for path, t, value in values:
        entry, _, name = path.rpartition("/")
        if entry in entries and name == keys[entries[entry]]:
            text = key_text(t, value)
            if text is None:
                problems.append(f"{path} is {type_name(t)}: the key of {entries[entry]}'s entries is a scalar")
            else:
                named[entry] = f"{name}={text}"
    seen = {}
    for entry, name in named.items():
        seen.setdefault((entries[entry], name), []).append(entry)
    for (list_path, name), at in sorted(seen.items()):
        if len(at) > 1:
            problems.append(f"{', '.join(at)} share {name}: the entries of {list_path} are known by "
                            f"{keys[list_path]}")
    for entry in sorted(set(entries) - set(named)):
        problems.append(f"{entry} has no {keys[entries[entry]]}: the entries of {entries[entry]} are known by it")
    return {e: n for e, n in named.items() if len(seen[(entries[e], n)]) == 1}, problems


def raw_types(values, keyed=None):
    """{a value's own path: (its structural path, its type)}. In the own path
    an entry of a list is itself: `[<key>=<its value>]` in a list the edit
    lists key (`keyed`, keyed_entries': `/skills_tree[id=211]/ui`), else
    `[<game_object_type>:<its name>]` when it has a name of its own
    (`/client_objects[light:bulb_2]/intensity`), else its index with its type
    (`[47:light]`), so an entry an edit moved or took out is never taken for
    the one now at its index."""
    kinds, names, keyed = entry_kinds(values), entry_names(values), keyed or {}

    def own(path):
        out, at = [], 0
        for m in ENTRY.finditer(path):
            entry = path[:m.end()]
            out.append(path[at:m.start()])
            if entry in keyed:
                out.append(f"[{keyed[entry]}]")
            elif entry in names:
                out.append(f"[{kinds.get(entry, '')}:{names[entry]}]")
            else:
                out.append(f"[{m.group(1)}" + (f":{kinds[entry]}" if entry in kinds else "") + "]")
            at = m.end()
        return "".join(out) + path[at:]
    return {own(path): (normalise(path, kinds), t) for path, t, _ in values}


def types_of(walk, data, path=""):
    """{normalised path: {type, ...}} of a config: every value's type at its
    structural path (normalise). None when the bytes are not a config walk
    reads (a texture, a sound, a model)."""
    try:
        values = parse(walk, data, is_config_path(path))
    except NotAConfig:
        return None
    kinds = entry_kinds(values)
    out = {}
    for path, t, _ in values:
        out.setdefault(normalise(path, kinds), set()).add(t)
    return out


def leaf(path):
    """The key name of a path: the last `/` segment, its `[]` dropped (an
    indexed item itself has the parent's name)."""
    return INDEX.sub("", path.rsplit("/", 1)[-1])


def corpus_with(walk, previous):
    """{normalised key path: {type, ...}} over the given config files ({path:
    bytes}): the types 0.26g already uses at each key path. A new key's type,
    where another file uses the same path, must be one of these (or
    allowlisted). The key is the whole path, not the leaf name, so a name like
    `value` (a scalar in an item, a vector in a material) is weighed in its
    own place. Non-config files are skipped."""
    out = {}
    for path, data in previous.items():
        types = types_of(walk, data, path)
        if types is None:
            continue
        for path, kinds in types.items():
            out.setdefault(path, set()).update(kinds)
    return out


def number_either(was, now, known):
    """An int that becomes a float or the reverse, at a structural path where 0.26g's own configs
    already have both: the game's reader takes either for a number there (content_edit.set_number;
    patch 19's weapons' melee/data/k_damage is an int where every native weapon's is a float). Never
    a vector, a table or a string: only these two scalar types, and only where the base mixes them."""
    return {was, now} == {T_INT, T_FLOAT} and {T_INT, T_FLOAT} <= known


def violations(walk, old, new, path, corp, allowlist, keys=None):
    """The type problems of one changed file (`new` its bytes, `old` its
    previous bytes or None), a list of strings; value by value:
    - a value at a path the previous version has (its own path: an entry of
      a list `keys` keys (list_keys' for the file) by its key, any other
      entry by its name or its index and object type) has the type it had
      there, but for a number that turns int or float where the corpus has
      both (number_either); a keyed entry only one version has (new, or
      taken out) has no previous types to keep;
    - an entry of a keyed list without its key, or with one another entry
      has, is a problem (keyed_entries);
    - a value at a path new to the file, whose structural path 0.26g already
      uses (the corpus, and the file's own previous version), has one of the
      types used there, unless `allowlist` has (file, structural path): a
      new key only, never a key that exists.
    A `new` that is not a config walk reads has no problems here (build-
    content refuses one at a config path before)."""
    try:
        new_values = parse(walk, new, is_config_path(path))
    except NotAConfig:
        return []
    try:
        old_values = parse(walk, old, is_config_path(path)) if old is not None else []
    except NotAConfig:
        old_values = []
    keys = keys or {}
    old_keyed, old_bad = keyed_entries(old_values, keys)
    new_keyed, new_bad = keyed_entries(new_values, keys)
    out = [f"{path} (previous): {p}" for p in old_bad] + [f"{path}: {p}" for p in new_bad]
    old_raw, new_raw = raw_types(old_values, old_keyed), raw_types(new_values, new_keyed)
    own = {}
    for structural, t in old_raw.values():
        own.setdefault(structural, set()).add(t)
    for key in sorted(new_raw):
        structural, t = new_raw[key]
        if key in old_raw:
            was = old_raw[key][1]
            if was != t and not number_either(was, t, corp.get(structural, set())):
                out.append(f"{path}: {key or '/'} was {type_name(was)}, now {type_name(t)} "
                           f"(a key that exists keeps its value type)")
            continue
        known = corp.get(structural, set()) | own.get(structural, set())
        if known and t not in known and (path, structural) not in allowlist:
            out.append(f"{path}: new key {key or '/'} is {type_name(t)}, but 0.26g uses {fmt(known)} at "
                       f"{structural}; use that type, or allowlist (file, path) with a reason")
    return out


def fmt(kinds):
    return "/".join(sorted(type_name(t) for t in kinds))


def main():
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    ce_dir, old_path, new_path, game_path = sys.argv[1:5]
    import json
    edit_lists = [json.load(open(p, encoding="utf-8")) for p in sys.argv[5:]]
    keys = list_keys(edit_lists).get(game_path, {})
    walk = load_walk(ce_dir)
    old = open(old_path, "rb").read()
    new = open(new_path, "rb").read()
    corp = corpus_with(walk, {game_path: old})
    problems = violations(walk, old, new, game_path, corp, {}, keys)
    for p in problems:
        print(f"TYPE: {p}")
    print(f"{len(problems)} type problem(s)")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
