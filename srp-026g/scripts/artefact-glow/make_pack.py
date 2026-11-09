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

Version 5 adds Forest Protection's glow (docs/notes/artefact-skills.md, "Forest Protection"; exe
patch 111 plays it while the skill protects a player): gameplay/effects/effect_forest_protection,
a particle effect (the stock Onyx glow's effect, `view` 2: everyone's view and the player's own)
playing fx/artifacts/human_forest_protection, the stock Onyx glow (its emitters, sizes, rates and
haze) grey and on the legs only:

- every animated domain with its leg parts alone on (LEGS); an emitter left with none is left out
  (the hands' strokes and the chest's rays);
- the orange materials' grey copies, fx/artifacts/<name>_grey (the stock material with its
  constant_emissive grey, as the stock kinds differ only there), and the point light's colour grey.

Version 6 adds the leg quarters (docs/notes/artefact-skills.md, "Leg quarters"; exe patch 111 plays
them): for each kind and n in 1..4, gameplay/effects/effect_artefact_leg_<kind>_<n> (the kind's stock
effect, `view` 2) playing fx/artifacts/human_<kind>_leg_<n>: the kind's stock glow, its own colours,
with only quarter n's body parts on (QUARTERS: 1 the left lower leg and foot, 2 the right ones, 3
the left thigh, 4 the right one), the emitters with none of them left out, and the point light's
emitter (up_light) too.

Version 7: each kept emitter's Rate (its curve and its data source) and MaxParticles (rounded up,
at least 1) times the share of the stock emitter's body parts it keeps. Version 6 kept the stock
rate on fewer parts, so a quarter was several times as dense as the stock glow on that leg (the
owner, 2026-11-06: they looked different from the activation glow); now a quarter is the stock
glow's own density on its parts, and four quarters are the stock emitters' leg share.

Version 8: the point light's emitter (up_light) is back in every quarter (the owner, 2026-11-06),
like the others: its domain cut to the quarter's parts, its Rate and MaxParticles times its kept
share (a shin quarter one light), its intensity, colour and range the stock's.

Version 9: Forest Protection's glow on the hands instead of the legs (the owner, 2026-11-06): the
stock Onyx glow, grey, made as a quarter is (`quarter_particle`) with HANDS (both forearms and
hands) for its parts: the hands' strokes and the point light kept, every kept emitter at the stock
density on its parts. It no longer lies on the leg quarters; everyone sees it, the carrier too in
the first person (its own hands).

Version 10 (the owner, 2026-11-06): Forest Protection's glow is the legs' system of versions 5 to 8
(its grey puffs and strokes, rays, haze and light, value for value) with its leg parts moved to the
arms (LEG_TO_ARM: upper leg to upper arm, lower leg to forearm, foot to hand); and it and the leg
quarters made as effect_artefact_<kind> is: a
composite of the system with no `view` (everyone else's third person, unchanged: its light and
haze) and a first-person twin (`view` 0, <system>_fp) without the point light's emitter and the
haze (FIRST_PERSON_LEFT_OUT), which washed the carrier's own view grey.

Version 11 pushed that system's puffs, strokes and haze out of the arm on every axis; still no fog
like the stock activation's (the owner, 2026-11-07). Version 12 (the owner's pick, "dark grey smoke
no light is good"): Forest Protection's glow is a dark-grey smoke around the arms. The stock
activation's fog beyond the body is its `up` puffs and `body` strokes, which have no arm part; so:

- the stock Onyx glow cut to the six arm parts (ARMS), every kept emitter at its full stock Rate
  and MaxParticles (its strokes, rays and haze), without the point light (FOREST_LEFT_OUT);
- plus the stock `up` and `body` (FOREST_PUFFS), full rate, every animated domain part off but the
  six arm parts, and their spawn outward on every axis: each Scale axis `up`'s x curve (an animated
  domain puts a particle at a random point of its body part's shape times Scale per axis in the
  part's own frame, 0x369030; the stock axes were made for the legs);
- grey (`greyed`: the rays' grey copy, rays_grey), and the puffs and the arm strokes
  (FOREST_SMOKE) drawn with fx/artifacts/smoke_dark_grey: the stock alpha-blended
  explosions/dark_smoke (blend_mode 1; the glow's own materials are additive, 2, and a dark grey
  additive barely shows) with its constant_emissive DARK_GREY.

Its first-person effect plays FOREST_FIRST_PERSON only, so nothing of the smoke drifts into the
carrier's own view.

Version 13 (the owner, 2026-11-07): the leg quarters look the same to everyone as to their carrier
("it should be like on yourself for everyone"): each quarter is one particle effect seen in both
views (`view` 2) playing its system without the point light and the haze (FIRST_PERSON_LEFT_OUT),
no composite and no first-person twin; and Forest Protection's smoke a tiny bit thicker: the
FOREST_SMOKE emitters' Rate (curve and data source) and MaxParticles (rounded up) times
SMOKE_THICKER, their colour and sizes as they were; the rays and the haze unchanged.

Version 14 (the owner, 2026-11-09: frame rate first, "let's start with particles"): the same glows
drawn with less. Every glow above is made from the kinds' stock systems made lighter first (`lean`),
and the stock systems themselves are replaced by those lighter ones (what everyone sees of an
activation):

- the sprites (SPARSE: the rays, the chest's rays and the up puffs) at SPARSE_RATE of their Rate
  (curve and data source) and MaxParticles, each SPARSE_SIZE as large (InitSize, every axis): half
  the sprites, each twice the area, so the same lit area (the glow's materials are additive);
- the point lights (`up_light`, a light per particle: about 34 alive per glow) at LIGHT_RATE of
  their Rate and MaxParticles, each LIGHT_INTENSITY as bright and LIGHT_RADIUS as wide (Intensity
  and Radius curves): about a quarter as many lights giving about the light they gave;
- the carrier's own first-person twin (human_<kind>_fp) the lean system without the head, as it
  was the stock one (version 3, approved by the owner; versions 1 and 2, lighter, showed too little
  in the first person): its lights fewer and brighter as everyone else's;
- the leg quarters cumulative: effect_artefact_leg_<kind>_<n> plays quarters 1..n of the legs in
  one system (CUMULATIVE), and exe patch 111 plays only the one of the kind's count (it played
  quarters 1..count, each a system of its own): a kind on a player is one system, not up to four;
- Forest Protection's smoke on the lean puffs: SMOKE_THICKER of SPARSE_RATE, each puff
  SPARSE_SIZE as large (version 13's lit area, half its puffs).

Sizes, colours, materials, lifetimes, body parts and every other emitter are as before.
"""
import argparse
import copy
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import zlib

PACK_ID = 'artefact-glow'
VERSION = '14'
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
    'material_instances/fx/artifacts/rays_orange.material': 'b7abda4163c0fee2d57bd3bc0198a956c51430a30d97e61326bb2b71289ad7d8',
    'material_instances/fx/artifacts/stroke_scroll_orange.material': 'bbbca74606ecc318bd7582eda42a9974d9330b977842f5f04a6a96c56b4cea89',
    'material_instances/fx/artifacts/up_orange.material': '9a9640e924f0911c8c1bc5a0c51ac9d2d20bbadb0ee5a7f170750fc2f30d658c',
    'material_instances/fx/explosions/dark_smoke.material': '82bc6ffe831595734cbc0fac15eb69aaac9edce00ed0469f6b03d3b34088fae3',
}
# Forest Protection's glow: made from the Onyx glow (its effect, particle system and materials).
FOREST_FROM = 'onyx'
FOREST_EFFECT = 'gameplay/effects/effect_forest_protection.options'
FOREST_SYSTEM = 'fx/artifacts/human_forest_protection'
ORANGE_MATERIALS = ('rays', 'stroke_scroll', 'up')
LEGS = ('LeftUpLeg', 'LeftLeg', 'LeftFoot', 'RightUpLeg', 'RightLeg', 'RightFoot')
# Forest Protection's glow since version 12: a dark-grey smoke around the six arm parts.
ARMS = ('LeftArm', 'LeftForeArm', 'LeftHand', 'RightArm', 'RightForeArm', 'RightHand')
FOREST_PUFFS = ('up', 'body')                       # the stock fog, moved onto the arms
FOREST_SMOKE = ('up', 'body', 'left_hend', 'right_hend')
SMOKE_THICKER = 1.5                                 # version 13: FOREST_SMOKE's Rate and MaxParticles
FOREST_LEFT_OUT = ('up_light',)
FOREST_FIRST_PERSON = ('rays',)                     # what the carrier sees of it on his own arms
DARK_SMOKE_FROM = 'material_instances/fx/explosions/dark_smoke.material'
DARK_SMOKE = 'fx/artifacts/smoke_dark_grey'
DARK_GREY = [0.30000001192092896] * 3 + [1.0]   # 0.3 as an f32 (the render the owner picked)
# What a first-person twin of Forest Protection's glow or a leg quarter leaves out (version 10).
FIRST_PERSON_LEFT_OUT = ('up_light', 'distortion')
BODY_PARTS = LEGS + ('BackChest', 'BackSpin', 'Face', 'FrontChest', 'FrontSpin', 'Head', 'LeftArm',
                     'LeftForeArm', 'LeftHand', 'Neck', 'RightArm', 'RightForeArm', 'RightHand')
ALPHA_BLENDED = 1                        # a material's blend_mode: 1 alpha-blended, 2 additive
GREY = [0.5, 0.5, 0.5, 1.0]              # the materials' emissive and the light: a neutral grey (exact in f32)
BOTH_VIEWS = 2
# The leg quarters (version 6): a kind's carried copies counting for a player light this many
# quarters, in this order (lower legs first, left then right, then the thighs).
QUARTERS = (('LeftLeg', 'LeftFoot'), ('RightLeg', 'RightFoot'), ('LeftUpLeg',), ('RightUpLeg',))
# Version 14: effect n lights quarters 1..n (patch 111 plays the one of the count).
CUMULATIVE = tuple(sum(QUARTERS[:n], ()) for n in range(1, len(QUARTERS) + 1))
# Version 14's lean systems (`lean`).
SPARSE = ('rays', 'rays_chest', 'up')
SPARSE_RATE = 0.5
SPARSE_SIZE = math.sqrt(2.0)
LIGHT = 'up_light'
LIGHT_RATE = 0.25
LIGHT_INTENSITY = 2.5
LIGHT_RADIUS = 1.25
SIZE_ACTION = 'particle_action_initial_size_type'
LIGHT_ACTION = 'particle_action_light_type'


def quarter_names(kind, n):
    """Effect n (1..4) of `kind`, quarters 1..n lit: its effect config's path and its particle system."""
    return (f'gameplay/effects/effect_artefact_leg_{kind}_{n}.options', f'{KINDS[kind]}_leg_{n}')
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
    """The first-person copy of a human_<kind> particle system: the stock one (since version 14 the
    lean one, what everyone else sees) with Head, Face and Neck off in every emitter's animated
    domain."""
    return domains_off(copy.deepcopy(tree))


def parts_only(tree, parts):
    """`tree` with every body part but `parts` off in each animated domain."""
    t, v = tree
    if t == T_NAMED:
        kids = dict(v)
        if kids.get('particle_entity_type') == (T_STR, DOMAIN):
            return (t, [(k, (T_NAMED, [(n, (T_BOOL, 0) if n in BODY_PARTS and n not in parts else c)
                                       for n, c in c[1]]) if k == 'properties' else c) for k, c in v])
        return (t, [(k, parts_only(c, parts)) for k, c in v])
    if t == T_INDEXED:
        return (t, [parts_only(c, parts) for c in v])
    return tree


def emitter_name(emitter):
    return dict(dict(emitter[1])['properties'][1])['Name'][1]


def keep_emitters(tree, keep):
    """`tree` with only the emitters `keep(emitter)` says (and its children_order so)."""
    lod = dict(tree[1])['LOD 0']
    kept = [(g, e) for g, e in dict(lod[1])['children'][1] if keep(e)]
    names = {g for g, _ in kept}
    order = [c for c in dict(lod[1])['children_order'][1] if c[1] in names]
    new_lod = (T_NAMED, [(k, (T_NAMED, kept) if k == 'children' else (T_INDEXED, order) if k == 'children_order'
                          else c) for k, c in lod[1]])
    return (T_NAMED, [(k, new_lod if k == 'LOD 0' else c) for k, c in tree[1]])


def domain_parts(emitter):
    """The body parts on in an emitter's animated domains."""
    on = set()
    for _, child in dict(emitter[1])['children'][1]:
        if dict(child[1]).get('particle_entity_type') == (T_STR, DOMAIN):
            on |= {n for n, c in dict(child[1])['properties'][1] if n in BODY_PARTS and c == (T_BOOL, 1)}
    return on


def greyed(tree, path=''):
    """`tree` with its orange materials' grey copies and the colours of its light (LightColor) and
    of its colour sources (data_sources/color) grey: where the stock kinds differ."""
    t, v = tree
    if t == T_NAMED:
        return (t, [(k, greyed(c, f'{path}/{k}')) for k, c in v])
    if t == T_INDEXED:
        return (t, [greyed(c, path) for c in v])
    under_colour = '/LightColor/' in path or path.startswith('/data_sources/color/')
    if t == T_STR and v.endswith('_orange') and v[len('fx/artifacts/'):-len('_orange')] in ORANGE_MATERIALS:
        return (t, v[:-len('_orange')] + '_grey')
    if t == 8 and under_colour:
        return (t, list(GREY))
    return tree


def emitters_by_name(tree):
    return {emitter_name(e): (g, e) for g, e in dict(dict(tree[1])['LOD 0'][1])['children'][1]}


def domain_child(emitter):
    """The properties (name -> value) of an emitter's animated domain."""
    for _, child in dict(emitter[1])['children'][1]:
        if dict(child[1]).get('particle_entity_type') == (T_STR, DOMAIN):
            return dict(dict(child[1])['properties'][1])
    raise KeyError(emitter_name(emitter))


AXES = ('source0', 'source1', 'source2')


def every_axis(scale, curve):
    """A Scale property (or its data source) with `curve` on each axis."""
    return (T_NAMED, [(n, (T_NAMED, [(k, curve if k == 'data' else v) for k, v in c[1]]) if n in AXES else c)
                      for n, c in scale[1]])


def on_arms(emitter, curve):
    """`emitter` with every animated domain part off but ARMS and each Scale axis `curve`."""
    def child(c):
        if dict(c[1]).get('particle_entity_type') != (T_STR, DOMAIN):
            return c
        props = [(n, (T_BOOL, int(n in ARMS)) if n in BODY_PARTS else every_axis(v, curve) if n == 'Scale' else v)
                 for n, v in dict(c[1])['properties'][1]]
        return (T_NAMED, [(k, (T_NAMED, props) if k == 'properties' else v) for k, v in c[1]])
    return (T_NAMED, [(k, (T_NAMED, [(g, child(c)) for g, c in v[1]]) if k == 'children' else v)
                      for k, v in emitter[1]])


def with_material(emitter, material):
    return (T_NAMED, [(k, (T_NAMED, [(n, (T_STR, material) if n == 'Material' else c) for n, c in v[1]])
                       if k == 'properties' else v) for k, v in emitter[1]])


def forest_particle(stock):
    """human_forest_protection (version 12): the stock Onyx glow on ARMS at full rate without the
    point light, plus FOREST_PUFFS on ARMS spawning outward, grey, FOREST_SMOKE in DARK_SMOKE."""
    arms = parts_only(copy.deepcopy(stock), ARMS)
    stock_lod = dict(stock[1])['LOD 0']
    stock_emitters = dict(dict(stock_lod[1])['children'][1])
    outward = dict(dict(domain_child(emitters_by_name(stock)['up'][1])['Scale'][1])['source0'][1])['data']
    kept, scales = {}, set()
    for guid, emitter in dict(dict(dict(arms[1])['LOD 0'][1])['children'][1]).items():
        name = emitter_name(emitter)
        if name in FOREST_PUFFS:
            kept[guid] = on_arms(stock_emitters[guid], outward)
            scales.add(dict(domain_child(stock_emitters[guid])['Scale'][1])['selected_source'][1])
        elif domain_parts(emitter) and name not in FOREST_LEFT_OUT:
            kept[guid] = emitter
    children = [(g, kept[g]) for g, _ in dict(stock_lod[1])['children'][1] if g in kept]
    order = [c for c in dict(stock_lod[1])['children_order'][1] if c[1] in kept]
    lod = (T_NAMED, [(k, (T_NAMED, children) if k == 'children' else (T_INDEXED, order) if k == 'children_order'
                      else c) for k, c in dict(arms[1])['LOD 0'][1]])
    data = (T_NAMED, [(n, (T_NAMED, [(f, every_axis(c, outward) if f in scales else c) for f, c in kinds[1]])
                       if n == 'float3' else kinds) for n, kinds in dict(arms[1])['data_sources'][1]])
    tree = greyed((T_NAMED, [(k, lod if k == 'LOD 0' else data if k == 'data_sources' else c) for k, c in arms[1]]))
    lod = dict(tree[1])['LOD 0']
    children, sources = [], set()
    for g, e in dict(lod[1])['children'][1]:
        if emitter_name(e) in FOREST_SMOKE:
            e, source = thicker(with_material(e, DARK_SMOKE))
            sources.add(source)
        children.append((g, e))
    users = [dict(dict(dict(e[1])['properties'][1])['Rate'][1])['selected_source'][1] for _, e in children]
    assert all(users.count(s) == 1 for s in sources), 'a smoke Rate source another emitter shares'
    lod = (T_NAMED, [(k, (T_NAMED, children) if k == 'children' else c) for k, c in lod[1]])
    data = (T_NAMED, [(n, (T_NAMED, [(f, (T_NAMED, [(m, rate_scaled(x, SMOKE_THICKER) if m == 'source' else x)
                                                     for m, x in c[1]]) if f in sources else c) for f, c in kinds[1]])
                       if n == 'float' else kinds) for n, kinds in dict(tree[1])['data_sources'][1]])
    return (T_NAMED, [(k, lod if k == 'LOD 0' else data if k == 'data_sources' else c) for k, c in tree[1]])


def thicker(emitter):
    """A smoke emitter SMOKE_THICKER as thick (version 13): its Rate curve and MaxParticles (rounded
    up) times SMOKE_THICKER; and the name of its Rate's data source, which the caller scales too."""
    props, source = [], None
    for name, value in dict(emitter[1])['properties'][1]:
        if name == 'Rate':
            source = dict(value[1])['selected_source'][1]
            value = (T_NAMED, [(n, rate_scaled(c, SMOKE_THICKER) if n == 'source' else c) for n, c in value[1]])
        elif name == 'MaxParticles':
            value = (value[0], scaled_maximum(value[1], SMOKE_THICKER))
        props.append((name, value))
    return (T_NAMED, [(n, (T_NAMED, props) if n == 'properties' else c) for n, c in emitter[1]]), source


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def rate_scaled(node, k):
    """A Rate curve (its source) with every key's value (a vector's second number: the key's
    position and both tangents) times `k`, as f32."""
    t, v = node
    if t == T_NAMED:
        return (t, [(n, rate_scaled(c, k)) for n, c in v])
    if t == T_INDEXED:
        return (t, [rate_scaled(c, k) for c in v])
    if t == 6:
        return (t, [v[0], f32(v[1] * k)])
    return node


def share(emitter, stock_emitter):
    """The share of the stock emitter's body parts a quarter's emitter keeps."""
    return len(domain_parts(emitter)) / len(domain_parts(stock_emitter))


def scaled_maximum(maximum, k):
    return max(1, math.ceil(maximum * k))


def selected_sources(tree):
    """How many properties of `tree` select each data source (selected_source -> count)."""
    used = {}

    def walk(node):
        t, v = node
        if t == T_NAMED:
            for k, c in v:
                if k == 'selected_source' and c[0] == T_STR:
                    used[c[1]] = used.get(c[1], 0) + 1
                else:
                    walk(c)
        elif t == T_INDEXED:
            for c in v:
                walk(c)
    walk(tree)
    return used


def lean_scales(emitter):
    """What `lean` scales in an emitter: {(action type or None for the emitter's own properties,
    property): factor}; empty for an emitter it leaves as it is."""
    name = emitter_name(emitter)
    if name in SPARSE:
        return {(None, 'Rate'): SPARSE_RATE, (None, 'MaxParticles'): SPARSE_RATE,
                (SIZE_ACTION, 'InitSize'): SPARSE_SIZE}
    if name == LIGHT:
        return {(None, 'Rate'): LIGHT_RATE, (None, 'MaxParticles'): LIGHT_RATE,
                (LIGHT_ACTION, 'Intensity'): LIGHT_INTENSITY, (LIGHT_ACTION, 'Radius'): LIGHT_RADIUS}
    return {}


def lean(stock):
    """A kind's stock glow drawn with less (version 14): for each emitter `lean_scales` names, its
    MaxParticles (rounded up) and each named curve (Rate, InitSize, Intensity, Radius: every value of
    the property's own sources and of the data source it selects, which no other property may share)
    times the factor. Everything else is the stock's."""
    tree = copy.deepcopy(stock)
    sources, done = {}, []

    def properties(props, scales, action):
        out = []
        for n, v in props:
            k = scales.get((action, n))
            if k is not None and n == 'MaxParticles':
                v = (v[0], scaled_maximum(v[1], k))
                done.append((action, n))
            elif k is not None:
                sources[dict(v[1])['selected_source'][1]] = k
                v = (T_NAMED, [(m, rate_scaled(c, k) if m in ('source',) + AXES else c) for m, c in v[1]])
                done.append((action, n))
            out.append((n, v))
        return (T_NAMED, out)

    def child(c, scales):
        action = dict(c[1])['particle_entity_type'][1]
        return (T_NAMED, [(k, properties(v[1], scales, action) if k == 'properties' else v) for k, v in c[1]])

    def emitter(e):
        scales = lean_scales(e)
        if not scales:
            return e
        done.clear()
        e = (T_NAMED, [(k, properties(v[1], scales, None) if k == 'properties'
                        else (T_NAMED, [(g, child(c, scales)) for g, c in v[1]]) if k == 'children' else v)
                       for k, v in e[1]])
        assert sorted(done, key=str) == sorted(scales, key=str), (emitter_name(e), done)
        return e

    lod = dict(tree[1])['LOD 0']
    lod = (T_NAMED, [(k, (T_NAMED, [(g, emitter(e)) for g, e in v[1]]) if k == 'children' else v) for k, v in lod[1]])
    used = selected_sources(stock)
    assert all(used[s] == 1 for s in sources), 'a lean data source another property shares'
    tables = dict(tree[1])['data_sources'][1]
    assert all(sum(s in dict(t[1]) for _, t in tables) == 1 for s in sources), 'a lean data source not in one table'
    data = (T_NAMED, [(kind, (T_NAMED, [(n, rate_scaled(c, sources[n]) if n in sources else c) for n, c in t[1]]))
                      for kind, t in tables])
    return (T_NAMED, [(k, lod if k == 'LOD 0' else data if k == 'data_sources' else v) for k, v in tree[1]])


def check_lean(tree, stock):
    """The lean system is the stock one but for `lean_scales`' properties of its emitters and the
    data sources they select: each value there (a curve key's value and tangents) the stock's times
    its factor, as f32, and MaxParticles the stock's scaled; every emitter `lean_scales` names
    changed. Every other value is the stock's."""
    factor = {}                    # a path prefix -> its factor
    expected_max = {}
    for guid, e in dict(dict(dict(stock[1])['LOD 0'][1])['children'][1]).items():
        scales = lean_scales(e)
        if not scales:
            continue
        base = f'/LOD 0/children/{guid}'
        actions = {dict(c[1])['particle_entity_type'][1]: g for g, c in dict(e[1])['children'][1]}
        for (action, prop), k in scales.items():
            owner = dict(e[1]) if action is None else dict(dict(dict(e[1])['children'][1])[actions[action]][1])
            at = base + ('' if action is None else f'/children/{actions[action]}') + f'/properties/{prop}'
            if prop == 'MaxParticles':
                expected_max[at] = scaled_maximum(dict(owner['properties'][1])[prop][1], k)
                continue
            factor[at + '/'] = k
            source = dict(dict(owner['properties'][1])[prop][1])['selected_source'][1]
            for kind, table in dict(stock[1])['data_sources'][1]:
                if source in dict(table[1]):
                    factor[f'/data_sources/{kind}/{source}/'] = k
    changed = set()
    for d in differences(tree, stock):
        if d in expected_max:
            assert get_path(tree, d)[1] == expected_max[d], d
            changed.add(d)
            continue
        k = next((k for prefix, k in factor.items() if d.startswith(prefix)), None)
        assert k is not None, d
        new, old = get_path(tree, d), get_path(stock, d)
        assert old[0] == 6 and new == (6, [old[1][0], f32(old[1][1] * k)]), (d, old, new)
        changed.add(next(prefix for prefix in factor if d.startswith(prefix)))
    assert changed == set(factor) | set(expected_max), sorted(set(factor) | set(expected_max) - changed)


def get_path(tree, path):
    for part in path.strip('/').split('/'):
        tree = dict(tree[1])[part]
    return tree


def quarter_particle(stock, parts):
    """A kind's leg quarter (artefact-skills.md, "Leg quarters"): the kind's stock glow, its
    colours as they are, with only the quarter's `parts` on; the emitters with none of them left
    out (the point light's, up_light, kept like the others since version 8). Each kept emitter's Rate (its curve and its data source) and MaxParticles (rounded up, at least
    1) times its kept share of the stock emitter's parts: a quarter is as dense on its parts as the
    stock glow is, and the four quarters together are the stock emitters' leg share."""
    tree = keep_emitters(parts_only(copy.deepcopy(stock), parts),
                         domain_parts)
    stock_emitters = dict(dict(dict(stock[1])['LOD 0'][1])['children'][1])
    lod = dict(tree[1])['LOD 0']
    sources = {}
    emitters = []
    for guid, emitter in dict(lod[1])['children'][1]:
        k = share(emitter, stock_emitters[guid])
        props = []
        for name, value in dict(emitter[1])['properties'][1]:
            if name == 'Rate':
                rate = dict(value[1])
                sources[rate['selected_source'][1]] = k
                value = (T_NAMED, [(n, rate_scaled(c, k) if n == 'source' else c) for n, c in value[1]])
            elif name == 'MaxParticles':
                value = (value[0], scaled_maximum(value[1], k))
            props.append((name, value))
        emitters.append((guid, (T_NAMED, [(n, (T_NAMED, props) if n == 'properties' else c) for n, c in emitter[1]])))
    lod = (T_NAMED, [(n, (T_NAMED, emitters) if n == 'children' else c) for n, c in lod[1]])

    def source(name, value):
        if name not in sources:
            return value
        return (T_NAMED, [(n, rate_scaled(c, sources[name]) if n == 'source' else c) for n, c in value[1]])
    data = dict(tree[1])['data_sources']
    data = (T_NAMED, [(n, (T_NAMED, [(f, source(f, c)) for f, c in kinds[1]]) if n == 'float' else kinds)
                      for n, kinds in data[1]])
    return (T_NAMED, [(n, lod if n == 'LOD 0' else data if n == 'data_sources' else c) for n, c in tree[1]])


def check_quarter(tree, stock, parts):
    """A quarter is the stock system but for its body parts (the quarter's alone on), the emitters
    left out (those with none of them), and each kept emitter's Rate values and
    MaxParticles scaled by its kept share of the stock emitter's parts (in its curve and its data
    source alike): every other value the stock's."""
    lod, stock_lod = dict(tree[1])['LOD 0'], dict(stock[1])['LOD 0']
    emitters, stock_emitters = dict(dict(lod[1])['children'][1]), dict(dict(stock_lod[1])['children'][1])
    dropped = {g for g, e in stock_emitters.items()
               if not domain_parts(e) & set(parts)}
    assert set(emitters) == set(stock_emitters) - dropped and emitters, sorted(dropped)
    for guid, emitter in emitters.items():
        assert domain_parts(emitter) == domain_parts(stock_emitters[guid]) & set(parts), guid
    scaled = {}
    for guid, emitter in emitters.items():
        stock_props = dict(dict(stock_emitters[guid][1])['properties'][1])
        props = dict(dict(emitter[1])['properties'][1])
        k = share(emitter, stock_emitters[guid])
        assert props['MaxParticles'][1] == scaled_maximum(stock_props['MaxParticles'][1], k), guid
        rate, stock_rate = dict(props['Rate'][1]), dict(stock_props['Rate'][1])
        assert rate['source'] == rate_scaled(stock_rate['source'], k), guid
        scaled[rate['selected_source'][1]] = k
    stock_floats = dict(dict(dict(stock[1])['data_sources'][1])['float'][1])
    for name, value in dict(dict(dict(tree[1])['data_sources'][1])['float'][1]).items():
        expected = stock_floats[name] if name not in scaled else \
            (T_NAMED, [(n, rate_scaled(c, scaled[name]) if n == 'source' else c) for n, c in stock_floats[name][1]])
        assert sort_key(value) == sort_key(expected), name
    for d in differences(tree, stock):
        last = d.rsplit('/', 1)[-1]
        assert (last in BODY_PARTS and '/properties/' in d) or last in dropped or d.endswith('/children_order') or \
            '/properties/Rate/source/' in d or last == 'MaxParticles' or \
            (d.startswith('/data_sources/float/') and d.split('/')[3] in scaled), d


def grey_material(stock, grey=GREY):
    """A stock material's grey copy: its constant_emissive `grey`, nothing else."""
    out = copy.deepcopy(stock)
    value = get(out, 'material/forward/effect/constant_emissive/value')
    assert value[0] == 8 and len(value[1]) == 4, value
    value[1][:] = grey
    return out


def forest_effect(stock_effect, system=FOREST_SYSTEM):
    """effect_forest_protection (or a quarter's), as effect_artefact_<kind> is made (version 10): a
    composite of the stock artefact effect playing `system` (no `view`: everyone else's third
    person, its light and haze included) and its first-person twin (`view` 0) playing
    `system`_fp, with the same time and end."""
    stock = get(stock_effect, 'effect')
    assert get(stock, 'type') == (T_INT, PARTICLE) and not has(stock, 'view'), 'not the stock particle effect'
    third = (T_NAMED, [(k, (T_STR, system) if k == 'particle_system' else c) for k, c in stock[1]])
    tree = (T_NAMED, [(k, third if k == 'effect' else c) for k, c in stock_effect[1]])
    return composite_effect(tree, system + '_fp')


def quarter_effect(stock_effect, system):
    """A leg quarter's effect (version 13): the stock artefact particle effect playing `system`
    and seen in both views (`view` 2): everyone sees what the carrier sees on himself."""
    stock = get(stock_effect, 'effect')
    assert get(stock, 'type') == (T_INT, PARTICLE) and not has(stock, 'view'), 'not the stock particle effect'
    both = (T_NAMED, [(k, (T_STR, system) if k == 'particle_system' else c) for k, c in stock[1]]
            + [('view', (T_INT, BOTH_VIEWS))])
    return (T_NAMED, [(k, both if k == 'effect' else c) for k, c in stock_effect[1]])


def first_person_twin(tree):
    """A glow's first-person twin (version 10): the same system without its point light's emitter
    and its haze (FIRST_PERSON_LEFT_OUT), which washed the carrier's own view grey."""
    return keep_emitters(tree, lambda e: emitter_name(e) not in FIRST_PERSON_LEFT_OUT)


def check_twin(twin, tree):
    """The twin is exactly `tree` without the emitters FIRST_PERSON_LEFT_OUT names."""
    emitters = dict(dict(dict(tree[1])['LOD 0'][1])['children'][1])
    dropped = {g for g, e in emitters.items() if emitter_name(e) in FIRST_PERSON_LEFT_OUT}
    assert dropped and {emitter_name(emitters[g]) for g in dropped} == set(FIRST_PERSON_LEFT_OUT), dropped
    for d in differences(twin, tree):
        assert d.rsplit('/', 1)[-1] in dropped or d.endswith('/children_order'), d
    kept = dict(dict(dict(twin[1])['LOD 0'][1])['children'][1])
    assert set(kept) == set(emitters) - dropped


def check_forest(tree, stock):
    """The forest system is the stock Onyx glow, grey (`greyed`), with exactly these differences:
    the emitters with no arm part, and FOREST_LEFT_OUT, left out; each kept emitter's domain parts
    those of ARMS it had (FOREST_PUFFS: all of ARMS, nothing else); FOREST_PUFFS' Scale, in the
    emitter and in its data source, `up`'s x curve on every axis; FOREST_SMOKE's Material
    DARK_SMOKE, and their Rate (curve and data source) and MaxParticles SMOKE_THICKER times the
    stock's (version 13). Every other rate, size, colour and value is the stock's."""
    ref = greyed(copy.deepcopy(stock))
    ref_emitters, emitters = emitters_by_name(ref), emitters_by_name(tree)
    expected = {n for n, (_, e) in ref_emitters.items()
                if n in FOREST_PUFFS or (domain_parts(e) & set(ARMS) and n not in FOREST_LEFT_OUT)}
    assert set(emitters) == expected, sorted(emitters)
    outward = dict(dict(domain_child(ref_emitters['up'][1])['Scale'][1])['source0'][1])['data']
    scales, thick = set(), set()
    for name, (guid, emitter) in emitters.items():
        assert guid == ref_emitters[name][0]
        stock_on = domain_parts(ref_emitters[name][1])
        for part in BODY_PARTS:
            on = part in ARMS and (name in FOREST_PUFFS or part in stock_on)
            assert domain_child(emitter)[part] == (T_BOOL, int(on)), (name, part)
        scale = domain_child(emitter)['Scale']
        if name in FOREST_PUFFS:
            assert all(dict(dict(scale[1])[a][1])['data'] == outward for a in AXES), name
            scales.add(dict(scale[1])['selected_source'][1])
        props, ref_props = dict(dict(emitter[1])['properties'][1]), dict(dict(ref_emitters[name][1][1])['properties'][1])
        material = props['Material']
        assert material == (T_STR, DARK_SMOKE) if name in FOREST_SMOKE else material[1] != DARK_SMOKE, name
        k = SMOKE_THICKER if name in FOREST_SMOKE else 1
        assert props['MaxParticles'][1] == (scaled_maximum(ref_props['MaxParticles'][1], k) if k != 1
                                            else ref_props['MaxParticles'][1]), name
        rate, ref_rate = dict(props['Rate'][1]), dict(ref_props['Rate'][1])
        assert rate['source'] == (rate_scaled(ref_rate['source'], k) if k != 1 else ref_rate['source']), name
        if k != 1:
            thick.add(rate['selected_source'][1])
        for d in differences(emitter, ref_emitters[name][1]):
            last = d.rsplit('/', 1)[-1]
            assert (last in BODY_PARTS or (name in FOREST_PUFFS and '/properties/Scale/' in d)
                    or (name in FOREST_SMOKE and (d.endswith('/properties/Material') or last == 'MaxParticles'
                                                  or '/properties/Rate/source/' in d))), (name, d)
    ref_floats = dict(dict(dict(ref[1])['data_sources'][1])['float'][1])
    for s in thick:
        source = dict(dict(dict(tree[1])['data_sources'][1])['float'][1])[s]
        expected = (T_NAMED, [(n, rate_scaled(c, SMOKE_THICKER) if n == 'source' else c) for n, c in ref_floats[s][1]])
        assert sort_key(source) == sort_key(expected), s
    for d in differences(dict(tree[1])['data_sources'], dict(ref[1])['data_sources']):
        assert any(d.startswith(f'/float3/{s}/') for s in scales) or any(d.startswith(f'/float/{s}/') for s in thick), d
    for s in scales:
        source = dict(dict(dict(tree[1])['data_sources'][1])['float3'][1])[s]
        assert all(dict(dict(source[1])[a][1])['data'] == outward for a in AXES), s
    for d in differences(tree, ref):
        assert d.startswith(('/LOD 0/children', '/data_sources/float3/', '/data_sources/float/')), d


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
    """The twin is the stock system (since version 14 the lean one) exactly (emitters, sizes, rates,
    lights, colours, the haze), but for Head, Face and Neck off in the emitters' animated domains:
    the whole tree compared."""
    allowed = sorted(differences(domains_off(stock), stock))
    assert all(d.rsplit('/', 1)[-1] in NEAR_EYES and '/properties/' in d for d in allowed), allowed
    assert differences(tree, domains_off(stock)) == [], differences(tree, domains_off(stock))[:5]
    for d in allowed:                    # each of those is off in the twin
        assert get_path(tree, d) == (T_BOOL, 0), d


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
        stock_particle = parse(stock[particle_path])
        particle = lean(stock_particle)             # version 14: what everyone sees, lighter
        check_lean(particle, stock_particle)
        fp_tree = first_person_particle(particle)
        check_particle(fp_tree, particle)
        check_bites(particle)
        effect = composite_effect(parse(stock[effect_path]), fp_system)
        for path, tree in ((effect_path, effect), (particle_path, particle), (f'particles/{fp_system}.particle', fp_tree)):
            data = serialize(tree)
            assert sort_key(parse(data)) == sort_key(tree), path
            files[path] = data
        operations.append({'op': 'replace_file', 'path': effect_path, 'base_sha256': STOCK[effect_path]})
        operations.append({'op': 'replace_file', 'path': particle_path, 'base_sha256': STOCK[particle_path]})
        operations.append({'op': 'add_file', 'path': f'particles/{fp_system}.particle'})
        # the stock child is the stock effect, value for value
        assert sort_key(get(effect, 'effect/effects')[1][0]) == sort_key(get(parse(stock[effect_path]), 'effect'))
    # Forest Protection's glow: its effect, its particle system and the grey materials.
    onyx = lean(parse(stock[f'particles/{KINDS[FOREST_FROM]}.particle']))
    forest = forest_particle(onyx)
    check_forest(forest, onyx)
    forest_twin = keep_emitters(forest, lambda e: emitter_name(e) in FOREST_FIRST_PERSON)
    assert set(emitters_by_name(forest_twin)) == set(FOREST_FIRST_PERSON)
    forest_files = {FOREST_EFFECT: forest_effect(parse(stock[f'gameplay/effects/effect_artefact_{FOREST_FROM}.options'])),
                    f'particles/{FOREST_SYSTEM}.particle': forest,
                    f'particles/{FOREST_SYSTEM}_fp.particle': forest_twin}
    used = {dict(dict(e[1])['properties'][1])['Material'][1] for _, e in emitters_by_name(forest).values()}
    for name in ORANGE_MATERIALS:            # the grey copies the forest system still draws with
        if f'fx/artifacts/{name}_grey' not in used:
            continue
        orange = parse(stock[f'material_instances/fx/artifacts/{name}_orange.material'])
        grey = grey_material(orange)
        assert differences(grey, orange) == ['/material/forward/effect/constant_emissive/value']
        forest_files[f'material_instances/fx/artifacts/{name}_grey.material'] = grey
    smoke = parse(stock[DARK_SMOKE_FROM])
    dark = grey_material(smoke, DARK_GREY)
    assert differences(dark, smoke) == ['/material/forward/effect/constant_emissive/value']
    assert get(smoke, 'material/forward/effect/blend_mode/value') == (T_INT, ALPHA_BLENDED)
    forest_files[f'material_instances/{DARK_SMOKE}.material'] = dark
    # The leg quarters: each kind's stock glow, its colours, a quarter of the legs each.
    for kind, system in KINDS.items():
        kind_stock = lean(parse(stock[f'particles/{system}.particle']))
        kind_effect = parse(stock[f'gameplay/effects/effect_artefact_{kind}.options'])
        for n, parts in enumerate(CUMULATIVE, 1):
            effect_path, quarter_system = quarter_names(kind, n)
            scaled = quarter_particle(kind_stock, parts)
            check_quarter(scaled, kind_stock, parts)
            quarter = first_person_twin(scaled)      # version 13: no point light, no haze, for everyone
            check_twin(quarter, scaled)
            forest_files[effect_path] = quarter_effect(kind_effect, quarter_system)
            forest_files[f'particles/{quarter_system}.particle'] = quarter
    for path, tree in forest_files.items():
        data = serialize(tree)
        assert sort_key(parse(data)) == sort_key(tree), path
        files[path] = data
        operations.append({'op': 'add_file', 'path': path})
    for path, sha in STOCK.items():   # the writer against the game's own files: the same trees
        assert sort_key(parse(serialize(parse(stock[path])))) == sort_key(parse(stock[path])), path

    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': "the artefacts' glows, drawn with less (version 14): each kind's stock glow "
                       'fx/artifacts/human_<kind> lighter (its rays, chest rays and up puffs at half the rate, '
                       'each sqrt 2 as large; its point lights at a quarter of the rate, each 2.5 times as bright '
                       'and 1.25 times as wide); each effect_artefact_<kind> a composite of the '
                       'stock third-person particle effect and a first-person one (view 0) playing '
                       'fx/artifacts/human_<kind>_fp: that lighter system, its point lights and haze included, '
                       'with Head, Face and Neck off in every emitter\'s animated domain; and '
                       'Forest Protection\'s glow, effect_forest_protection (a composite: third person, and a first-person one with only its rays) playing '
                       'fx/artifacts/human_forest_protection: a dark-grey smoke around the arms (the lighter Onyx glow on the arms with its up puffs and body strokes moved onto them, no light, drawn with fx/artifacts/smoke_dark_grey, its smoke 1.5 times that rate); '
                       'and the leg quarters, effect_artefact_leg_<kind>_<1..4> (one particle effect each, seen in both views) playing '
                       'fx/artifacts/human_<kind>_leg_<1..4>: each kind\'s lighter glow on quarters 1..n of the '
                       'legs (left lower, right lower, left upper, right upper; exe patch 111 plays the one of '
                       'the count), each emitter at that glow\'s density on the parts, without the point light and the haze',
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
