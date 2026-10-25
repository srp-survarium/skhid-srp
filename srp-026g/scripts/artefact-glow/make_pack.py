#!/usr/bin/env python3
"""make_pack.py: the content pack of the artefacts' first-person glow (2026-10-25;
../../docs/notes/artefacts.md, "Your own glow").

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/artefact-glow-<version>

--stock  the stock resources.db's files at their game paths (no Skhid patch carries them;
         `srp-game-data get --db <resources.db> --out <folder> <path>...`): every path of STOCK.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

An active artefact's glow is its config's `data/effect`, gameplay/effects/effect_artefact_<kind>:
a particle effect (type 114) playing fx/artifacts/human_<kind> on the carrier's body. The game
shows a particle effect through the presenters of its `view` (the cook 0x25A730 reads it, 1 when
absent): 1 the third-person presenters (everyone else's view of a player), 0 the first-person
presenter (the view of the player one's camera is in), 2 both. The stock glow has none, so the
carrier never sees it. Each effect becomes a composite (type 110, every child at once, as
effect_zone_radiation) of two particle children with the same time and the same end (the
artefact's effect_calculator: the effect ends when the artefact leaves its active state):

- the stock effect as it is (no `view`: the third person, what everyone else sees, unchanged);
- `view` 0 with fx/artifacts/human_<kind>_fp, a first-person copy of the particle system: the
  stock one, value for value (its emitters, sizes, rates, point lights, colours and haze; the
  owner, 2026-10-25: "use the original effects seen by other people, but with the head cleaned
  somewhat"), with Head, Face and Neck off in every emitter's animated domain, so nothing sits
  on the camera. The same materials (additive), so kinds add up as in the third person.
  view_cone.py measures each emitter against the middle of the first-person view.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

PACK_ID = 'artefact-glow'
VERSION = '4'
# kind -> (effect config, particle system); the stock files, by sha256.
KINDS = {
    'lifebone': 'fx/artifacts/human_zhivokost',
    'onyx': 'fx/artifacts/human_onix',
    'spring': 'fx/artifacts/human_spring',
}
STOCK = {
    'gameplay/effects/effect_artefact_lifebone.options': '8adfd0e768c8bec73fc42c551833d8b0121e1a16e76f103ed5e83d7fad943a5d',
    'gameplay/effects/effect_artefact_onyx.options': '194ad4db47f6a4da93f5ee3930c78059e69ba3b8671e6aac3ccebee0954c1f20',
    'gameplay/effects/effect_artefact_spring.options': 'ab563c39565ba6fedd57610dd7244cbcaca4fe4fb6c5fda25c926f4c5bfe29a7',
    'particles/fx/artifacts/human_zhivokost.particle': '8c313b13a128a29c68a0ccb8a5dac334e5564258a1bdf68768821bd04e561a56',
    'particles/fx/artifacts/human_onix.particle': 'eea6f174e326919d97d00c08a6dfe3a21dfddc590ec8f81328f5ab4a01914b75',
    'particles/fx/artifacts/human_spring.particle': '7fdccfa6d42126e1701b85849f50053c3b8c169c2923194491de45a178540e7b',
}
COMPOSITE, PARTICLE = 110, 114
FIRST_PERSON = 0
NEAR_EYES = ('Head', 'Face', 'Neck')     # the body parts at the camera: off in the twin
DOMAIN = 'particle_action_initial_animated_domain_type'

T_BOOL, T_INT, T_FLOAT, T_NAMED, T_INDEXED, T_STR = 0, 1, 2, 3, 4, 5
REC = struct.Struct('<QQIHH')


def sha256(data):
    return hashlib.sha256(data).hexdigest()


# A binary config as a tree: a value is (type, data); a named table's data a list of (name, value),
# an indexed one's a list of values, a string's its text, a vector's its floats, a number's or a
# boolean's the record's raw u64 (as srp-026g/scripts/lobby-bulb/make_pack.py).
def parse(buf, off=0):
    data, _, _, t, cnt = REC.unpack_from(buf, off)
    if t in (T_NAMED, T_INDEXED):
        kids = []
        for k in range(cnt):
            c = data + 24 * k
            v = parse(buf, c)
            if t == T_NAMED:
                i = REC.unpack_from(buf, c)[1]
                kids.append((bytes(buf[i:buf.index(b'\0', i)]).decode(), v))
            else:
                kids.append(v)
        return (t, kids)
    if t == T_STR:
        return (t, bytes(buf[data:data + cnt - 1]).decode())
    if t > T_STR:
        return (t, list(struct.unpack_from('<%df' % (t - 4), buf, data)))
    return (t, data)


def serialize(root):
    """The tree as a config: the records (a table's children side by side, the tables in the order
    they are reached), then the names, strings and vectors; a named table's children sorted by CRC,
    as the game's lookup needs."""
    recs, heap, strings = [], bytearray(), {}

    def text(s):
        if s not in strings:
            strings[s] = len(heap)
            heap.extend(s.encode() + b'\0')
        return strings[s]

    slots = [[root, None]]
    queue = [0]
    first = {}
    while queue:
        at = queue.pop(0)
        t, v = slots[at][0]
        if t in (T_NAMED, T_INDEXED):
            kids = v
            if t == T_NAMED:
                kids = sorted(v, key=lambda kv: zlib.crc32(kv[0].encode()) & 0xffffffff)
            first[at] = len(slots)
            for kid in kids:
                if t == T_NAMED:
                    slots.append([kid[1], kid[0]])
                else:
                    slots.append([kid, None])
                queue.append(len(slots) - 1)
    heap_base = 24 * len(slots)
    out = bytearray()
    for at, (value, name) in enumerate(slots):
        t, v = value
        name_off = heap_base + text(name) if name is not None else 0
        crc = zlib.crc32(name.encode()) & 0xffffffff if name is not None else 0
        if t in (T_NAMED, T_INDEXED):
            data, cnt = 24 * first[at], len(v)
        elif t == T_STR:
            data, cnt = heap_base + text(v), len(v.encode()) + 1
        elif t > T_STR:
            data, cnt = heap_base + len(heap), 4 * len(v)
            heap.extend(struct.pack('<%df' % len(v), *v))
        else:
            data, cnt = v, 4
        out += REC.pack(data, name_off, crc, t, cnt)
    return bytes(out + heap)


def get(tree, path):
    for part in path.split('/'):
        tree = dict(tree[1])[part]
    return tree


def has(tree, name):
    return tree[0] == T_NAMED and any(k == name for k, _ in tree[1])


def sort_key(tree):
    """Named tables compared regardless of their children's order (the writer sorts them by CRC)."""
    t, v = tree
    if t == T_NAMED:
        return (t, sorted((k, sort_key(c)) for k, c in v))
    if t == T_INDEXED:
        return (t, [sort_key(c) for c in v])
    return (t, v)


def domains_off(tree):
    """`tree` with Head, Face and Neck off in every animated domain (an action whose
    particle_entity_type is DOMAIN), everything else as it is."""
    t, v = tree
    if t == T_NAMED:
        kids = dict(v)
        if kids.get('particle_entity_type') == (T_STR, DOMAIN):
            return (t, [(k, (T_NAMED, [(n, (T_BOOL, 0) if n in NEAR_EYES else c) for n, c in c[1]])
                         if k == 'properties' else c) for k, c in v])
        return (t, [(k, domains_off(c)) for k, c in v])
    if t == T_INDEXED:
        return (t, [domains_off(c) for c in v])
    return tree


def first_person_particle(tree):
    """The first-person copy of a human_<kind> particle system: the stock one with Head, Face and
    Neck off in every emitter's animated domain."""
    return domains_off(copy.deepcopy(tree))


def composite_effect(tree, fp_system):
    """effect_artefact_<kind>: the stock particle effect and its first-person twin, side by side."""
    stock = get(tree, 'effect')
    assert get(stock, 'type') == (T_INT, PARTICLE) and not has(stock, 'view'), 'not the stock particle effect'
    fp = [(k, (T_STR, fp_system) if k == 'particle_system' else c) for k, c in stock[1]] + [('view', (T_INT, FIRST_PERSON))]
    effect = (T_NAMED, [('type', (T_INT, COMPOSITE)), ('effects', (T_INDEXED, [stock, (T_NAMED, fp)]))])
    return (T_NAMED, [(k, effect if k == 'effect' else c) for k, c in tree[1]])


def differences(a, b, path=''):
    """The paths where two trees differ (named tables by name, regardless of order)."""
    if a[0] != b[0]:
        return [path]
    if a[0] == T_NAMED:
        x, y = dict(a[1]), dict(b[1])
        if len(x) != len(a[1]) or len(y) != len(b[1]):
            return [path + ' (a name twice)']
        out = [path + '/' + k for k in set(x) ^ set(y)]
        for k in set(x) & set(y):
            out += differences(x[k], y[k], path + '/' + k)
        return out
    if a[0] == T_INDEXED:
        if len(a[1]) != len(b[1]):
            return [path]
        return [d for i, (x, y) in enumerate(zip(a[1], b[1])) for d in differences(x, y, f'{path}[{i}]')]
    return [] if a[1] == b[1] else [path]


def check_particle(tree, stock):
    """The twin is the stock system exactly (emitters, sizes, rates, lights, colours, the haze),
    but for Head, Face and Neck off in the emitters' animated domains: the whole tree compared."""
    allowed = sorted(differences(domains_off(stock), stock))
    assert all(d.rsplit('/', 1)[-1] in NEAR_EYES and '/properties/' in d for d in allowed), allowed
    assert differences(tree, domains_off(stock)) == [], differences(tree, domains_off(stock))[:5]
    for d in allowed:                    # each of those is off in the twin
        node = tree
        for part in d.strip('/').split('/'):
            node = dict(node[1])[part]
        assert node == (T_BOOL, 0), d


def check_bites(stock):
    """check_particle refuses a twin that differs from the stock system otherwise: a rate, a
    body part, an emitter left out."""
    def refused(tree, what):
        try:
            check_particle(tree, stock)
        except AssertionError:
            return
        raise SystemExit(f'check_particle let {what} through')

    twin = first_person_particle(stock)
    lod = dict(twin[1])['LOD 0']
    guid, emitter = dict(lod[1])['children'][1][0]

    def with_emitter(new):
        kids = [(g, new if g == guid else e) for g, e in dict(lod[1])['children'][1]]
        new_lod = (T_NAMED, [(k, (T_NAMED, kids) if k == 'children' else c) for k, c in lod[1]])
        return (T_NAMED, [(k, new_lod if k == 'LOD 0' else c) for k, c in twin[1]])

    def bump(node):                       # the first curve key's value x2
        t, v = node
        if t == T_NAMED:
            for i, (k, c) in enumerate(v):
                if k == 'position' and c[0] == 6:
                    return (t, v[:i] + [(k, (6, [c[1][0], c[1][1] * 2 + 1]))] + v[i + 1:]), True
                new, done = bump(c)
                if done:
                    return (t, v[:i] + [(k, new)] + v[i + 1:]), True
        return node, False

    props = dict(emitter[1])['properties']
    rate, done = bump(dict(props[1])['Rate'])
    assert done
    new_props = (T_NAMED, [(k, rate if k == 'Rate' else c) for k, c in props[1]])
    refused(with_emitter((T_NAMED, [(k, new_props if k == 'properties' else c) for k, c in emitter[1]])), 'a rate')
    left_out = (T_NAMED, [(k, (T_NAMED, dict(lod[1])['children'][1][1:]) if k == 'children' else c)
                          for k, c in lod[1]])
    refused((T_NAMED, [(k, left_out if k == 'LOD 0' else c) for k, c in twin[1]]), 'an emitter left out')
    head_on = copy.deepcopy(twin)

    def head(node):
        t, v = node
        if t == T_NAMED:
            if dict(v).get('particle_entity_type') == (T_STR, DOMAIN):
                return (t, [(k, (T_NAMED, [(n, (T_BOOL, 1) if n == 'Head' else c) for n, c in c[1]])
                             if k == 'properties' else c) for k, c in v])
            return (t, [(k, head(c)) for k, c in v])
        if t == T_INDEXED:
            return (t, [head(c) for c in v])
        return node
    refused(head(head_on), 'the head on')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    stock = {}
    for path, sha in STOCK.items():
        data = (args.stock / path).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{path}: sha256 {sha256(data)}, not the stock file\'s {sha}')
        stock[path] = data

    files, operations = {}, []
    for kind, system in KINDS.items():
        effect_path = f'gameplay/effects/effect_artefact_{kind}.options'
        particle_path = f'particles/{system}.particle'
        fp_system = system + '_fp'
        particle = parse(stock[particle_path])
        fp_tree = first_person_particle(particle)
        check_particle(fp_tree, particle)
        check_bites(particle)
        effect = composite_effect(parse(stock[effect_path]), fp_system)
        for path, tree in ((effect_path, effect), (f'particles/{fp_system}.particle', fp_tree)):
            data = serialize(tree)
            assert sort_key(parse(data)) == sort_key(tree), path
            files[path] = data
        operations.append({'op': 'replace_file', 'path': effect_path, 'base_sha256': STOCK[effect_path]})
        operations.append({'op': 'add_file', 'path': f'particles/{fp_system}.particle'})
        # the stock child is the stock effect, value for value
        assert sort_key(get(effect, 'effect/effects')[1][0]) == sort_key(get(parse(stock[effect_path]), 'effect'))
    for path, sha in STOCK.items():   # the writer against the game's own files: the same trees
        assert sort_key(parse(serialize(parse(stock[path])))) == sort_key(parse(stock[path])), path

    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': "the artefacts' glow seen by their carrier: each effect_artefact_<kind> a composite of the "
                       'stock third-person particle effect and a first-person one (view 0) playing '
                       'fx/artifacts/human_<kind>_fp: the stock particle system, its point lights and haze '
                       'included, with Head, Face and Neck off in every emitter\'s animated domain',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/artefact-glow/make_pack.py from the stock resources.db\'s '
                  'artefact effects and particle systems (docs/notes/artefacts.md)'],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, pack.json sha256 {digest}')


if __name__ == '__main__':
    main()
