#!/usr/bin/env python3
"""make_pack.py: the content pack of Freeze Tag's frozen glow (../../docs/notes/freeze-tag.md, "The
frozen glow"; exe patch 160 draws it on every frozen body).

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/freeze-tag-glow-<version>

--stock  the stock resources.db's files at their game paths (`srp-game-data get --db <resources.db>
         --out <folder> --paths <list>`): every path of STOCK.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

The owner (2026-11-21): "effect can be anomalous artifact spring activation but of slightly
different hue and less intense"; from the previews' variants: "a is good". The glow is a particle
system, fx/artifacts/human_frozen, which the patch plays on each frozen body as scene particles
(each game for itself, never in the simulation; the frozen player sees his own body from the
orbit camera, so there is no first-person twin). It is the stock Spring glow, fx/artifacts/
human_spring (the activation glow of effect_artefact_spring, unchanged), with:

- its three blue materials (fx/artifacts/<name>_blue) drawn with ice copies
  (fx/artifacts/<name>_ice: the stock material, its constant_emissive ICE times EMISSIVE; the
  materials are additive, so a smaller emissive is a fainter glow);
- every emitter's Rate (its curve and its data source) and MaxParticles (rounded up, at least 1)
  times RATE;
- its point light (up_light) ICE in colour, its Intensity curve times LIGHT.

Version 2 adds the plume (the owner, 2026-11-22: "in freeze tag there it is hard to know what was
frozen from enemy team, so effect should be visible higher too"): one more emitter, `plume`, a
column of soft ice-blue smoke rising about 4 m from the body, so a frozen body shows over low
cover and at range, to either team. The patch plays the system with the body's matrix and no
skeleton, so the body-shaped (animated) domains spawn at the matrix's origin, the feet: what the
stock glow shows there is a small light at the feet, and its `up` streaks (star_trail, a thin line
texture) a hair-thin line. The plume is `up` with:

- its upward speed (InitVelocity's y, 0.1 to 0.2 m/s) PLUME_SPEED[0] times plus PLUME_SPEED[1]
  (1.4 to 2.2 m/s), its width and height PLUME_SIZE times, its life PLUME_LIFE and its rate
  PLUME_RATE (MaxParticles their product, rounded up), each changed property (curve and data
  source) with a data source of its own; it fades in and out as `up` does (its ColorLife);
- drawn facing the camera (PLUME_ALIGNMENT) with fx/artifacts/smoke_ice: the Spring emergence's
  smoke (smoke_blue, additive) with its constant_emissive ICE times PLUME_EMISSIVE.

The four strengths of version 1 (human_frozen_70, _40, _15 and their materials) are no longer
made: the patch scales one system's matrix through the thaw.
"""
import argparse
import json
import math
from pathlib import Path
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'artefact-glow'))
import make_pack as glow  # noqa: E402  (the artefact-glow pack's reader, writer and helpers)
from make_pack import T_NAMED, T_STR, parse, serialize, sha256, sort_key  # noqa: E402

PACK_ID = 'freeze-tag-glow'
VERSION = '2'
SPRING_SYSTEM = 'fx/artifacts/human_spring'
BLUE = ('rays', 'stroke_scroll', 'up')
STOCK = {
    f'particles/{SPRING_SYSTEM}.particle': glow.STOCK[f'particles/{SPRING_SYSTEM}.particle'],
    'material_instances/fx/artifacts/rays_blue.material':
        '2cb0b26d282b1870f04bd2372cf9c5fbf5234c3b2ed1dd61ba051d605dff0d10',
    'material_instances/fx/artifacts/stroke_scroll_blue.material':
        'b75df7286a56b32cf9d4704ef39298e9a58c9d2713ef375347f59892b53fd810',
    'material_instances/fx/artifacts/up_blue.material':
        '1229867293729394f2e3642d27db9225b6cfc0caebb8f50f510769deb8f46bd2',
    'material_instances/fx/artifacts/smoke_blue.material':
        'd93a7a95db68c39f917560ff1b65ed19d9b19cb3a434c6a1b8b9e2056bf7f65d',
}
SYSTEM = 'fx/artifacts/human_frozen'
# Spring is (0, 0.40, 1), a deep blue; the frozen glow ice blue, fainter (the owner's pick, "a").
ICE = (0.62, 0.88, 1.0)
EMISSIVE = 0.6
RATE = 0.6
LIGHT = 0.5
# The plume (version 2): its upward speed a x + b of `up`'s (m/s); its width and height times
# `up`'s; its life (s) and rate (a second); drawn camera-facing with the Spring emergence's smoke
# (smoke_blue) in ice, PLUME_EMISSIVE times the ice blue.
PLUME_SPEED = (8.0, 0.6)
PLUME_SIZE = (45.0, 16.0)
PLUME_LIFE = 2.4
PLUME_RATE = 30.0
PLUME_ALIGNMENT = 'Rectangle'
PLUME_SMOKE = 'smoke'
PLUME_EMISSIVE = 1.0
PLUME_MATERIAL = f'fx/artifacts/{PLUME_SMOKE}_ice'


def f32(x):
    return glow.f32(x)


def ice_material(stock, colour, emissive):
    """A blue material's ice copy: its constant_emissive `colour` times `emissive`, nothing else."""
    rgba = [f32(c * emissive) for c in colour] + [1.0]
    out = glow.grey_material(stock, rgba)
    assert glow.differences(out, stock) == ['/material/forward/effect/constant_emissive/value']
    return out


def coloured(tree, colour, materials, path=''):
    """`tree` with its blue materials renamed by `materials` and its light's colour (LightColor and
    the colour data sources) `colour`."""
    t, v = tree
    if t == T_NAMED:
        return (t, [(k, coloured(c, colour, materials, f'{path}/{k}')) for k, c in v])
    if t == glow.T_INDEXED:
        return (t, [coloured(c, colour, materials, path) for c in v])
    if t == T_STR and v in materials:
        return (t, materials[v])
    if t == 8 and ('/LightColor/' in path or path.startswith('/data_sources/color/')):
        return (t, [f32(c) for c in colour] + [1.0])
    return tree


def scaled(tree, rate, light):
    """Every emitter's Rate (curve and data source) and MaxParticles times `rate`; the point light's
    Intensity (curve and data source) times `light`."""
    lod = dict(tree[1])['LOD 0']
    sources, emitters = {}, []
    for guid, emitter in dict(lod[1])['children'][1]:
        def props(node, scale_rate):
            out = []
            for name, value in node[1]:
                if name == 'Rate' and scale_rate:
                    sources[dict(value[1])['selected_source'][1]] = rate
                    value = (T_NAMED, [(n, glow.rate_scaled(c, rate) if n == 'source' else c) for n, c in value[1]])
                elif name == 'MaxParticles' and scale_rate:
                    value = (value[0], glow.scaled_maximum(value[1], rate))
                elif name == 'Intensity':
                    sources[dict(value[1])['selected_source'][1]] = light
                    value = (T_NAMED, [(n, glow.rate_scaled(c, light) if n == 'source' else c) for n, c in value[1]])
                out.append((name, value))
            return (T_NAMED, out)

        def walk(node, top):
            kids = []
            for k, c in node[1]:
                if k == 'properties':
                    c = props(c, top)
                elif k == 'children':
                    c = (T_NAMED, [(g, walk(x, False)) for g, x in c[1]])
                kids.append((k, c))
            return (T_NAMED, kids)
        emitters.append((guid, walk(emitter, True)))
    users = [dict(dict(dict(e[1])['properties'][1])['Rate'][1])['selected_source'][1] for _, e in emitters]
    assert len(set(users)) == len(users), 'two emitters share a Rate source'
    assert len(sources) == len(users) + 1, 'a light Intensity source shared with a Rate'
    lod = (T_NAMED, [(n, (T_NAMED, emitters) if n == 'children' else c) for n, c in lod[1]])

    def source(name, value):
        if name not in sources:
            return value
        return (T_NAMED, [(n, glow.rate_scaled(c, sources[name]) if n == 'source' else c) for n, c in value[1]])
    data = (T_NAMED, [(n, (T_NAMED, [(f, source(f, c)) for f, c in kinds[1]]) if n == 'float' else kinds)
                      for n, kinds in dict(tree[1])['data_sources'][1]])
    return (T_NAMED, [(n, lod if n == 'LOD 0' else data if n == 'data_sources' else c) for n, c in tree[1]])


def remapped(node, f):
    """A curve with every key's value (a vector's second number: the key's position and both
    tangents) f of it, as f32."""
    t, v = node
    if t == T_NAMED:
        return (t, [(n, remapped(c, f)) for n, c in v])
    if t == glow.T_INDEXED:
        return (t, [remapped(c, f) for c in v])
    if t == 6:
        return (t, [v[0], f32(f(v[1]))])
    return node


def values(node):
    t, v = node
    if t == T_NAMED:
        return [x for _, c in v for x in values(c)]
    return [v[1]] if t == 6 else []


def set_props(node, changes):
    """`node` (an emitter or an action) with its properties' values replaced by `changes`."""
    return (T_NAMED, [(k, (T_NAMED, [(n, changes.get(n, c)) for n, c in v[1]]) if k == 'properties' else v)
                      for k, v in node[1]])


def plume(tree):
    """`tree` with the plume: a copy of its `up` emitter, a column of ice smoke rising from the body."""
    _, up = glow.emitters_by_name(tree)['up']
    kinds = {n: dict(k[1]) for n, k in dict(tree[1])['data_sources'][1]}
    added = {}

    def fresh(kind, prop, fs):
        """`prop` with each curve of `fs` (by name) f of it and a new data source, the old one's so too."""
        name = dict(prop[1])['selected_source'][1]
        n = 1 + max(int(k[len(kind):]) for k in kinds[kind] if k[len(kind):].isdigit())
        new = f'{kind}{n}'
        kinds[kind][new] = (T_NAMED, [(a, remapped(c, fs[a]) if a in fs else c) for a, c in kinds[kind][name][1]])
        added.setdefault(kind, []).append(new)
        return (T_NAMED, [(a, remapped(c, fs[a]) if a in fs else (T_STR, new) if a == 'selected_source' else c)
                          for a, c in prop[1]])

    def constant(prop, value):
        assert len(set(values(dict(prop[1])['source']))) == 1, 'not a flat curve'
        return fresh('float', prop, {'source': lambda _: value})

    def guid(old):
        return str(uuid.uuid5(uuid.NAMESPACE_URL, f'{SYSTEM}/plume/{old}'))

    a, b = PLUME_SPEED
    children = []
    for old, child in dict(up[1])['children'][1]:
        field = dict(child[1])['node_config_field'][1]
        props = dict(dict(child[1])['properties'][1])
        if field == 'InitVelocity':
            child = set_props(child, {field: fresh('float3', props[field], {'source1': lambda y: a * y + b})})
        elif field == 'Billboard':
            child = set_props(child, {'ScreenAlignment': (T_STR, PLUME_ALIGNMENT)})
        elif field == 'InitSize':
            w, h = PLUME_SIZE
            child = set_props(child, {field: fresh('float3', props[field], {'source0': lambda x: x * w,
                                                                            'source1': lambda y: y * h})})
        child = (T_NAMED, [(k, (T_STR, guid(old)) if k == 'id' else v) for k, v in child[1]])
        children.append((guid(old), child))
    props = dict(dict(up[1])['properties'][1])
    emitter = set_props(up, {
        'Name': (T_STR, 'plume'),
        'Material': (T_STR, PLUME_MATERIAL),
        'Rate': constant(props['Rate'], PLUME_RATE),
        'LifeTime': constant(props['LifeTime'], PLUME_LIFE),
        'MaxParticles': (props['MaxParticles'][0], math.ceil(PLUME_RATE * PLUME_LIFE)),
    })
    me = guid('emitter')
    emitter = (T_NAMED, [(k, (T_STR, me) if k == 'id' else (T_NAMED, children) if k == 'children'
                          else (glow.T_INDEXED, [(T_STR, guid(c[1])) for c in v[1]]) if k == 'children_order'
                          else v) for k, v in emitter[1]])
    assert sorted(dict(dict(emitter[1])['children'][1])) == sorted(c[1] for c in dict(emitter[1])['children_order'][1])
    lod = dict(tree[1])['LOD 0']
    lod = (T_NAMED, [(k, (T_NAMED, v[1] + [(me, emitter)]) if k == 'children'
                      else (glow.T_INDEXED, v[1] + [(T_STR, me)]) if k == 'children_order' else v) for k, v in lod[1]])
    data = (T_NAMED, [(n, (T_NAMED, list(kinds[n].items()))) for n, _ in dict(tree[1])['data_sources'][1]])
    assert sorted(added) == ['float', 'float3'] and len(added['float']) == len(added['float3']) == 2
    return (T_NAMED, [(n, lod if n == 'LOD 0' else data if n == 'data_sources' else c) for n, c in tree[1]])


def frozen_files(stock):
    """The glow's particle system and its ice materials, by path."""
    spring = parse(stock[f'particles/{SPRING_SYSTEM}.particle'])
    materials = {f'fx/artifacts/{name}_blue': f'fx/artifacts/{name}_ice' for name in BLUE}
    tree = plume(scaled(coloured(spring, ICE, materials), RATE, LIGHT))
    assert len(glow.emitters_by_name(tree)) == len(glow.emitters_by_name(spring)) + 1
    files = {f'particles/{SYSTEM}.particle': tree}
    for name, emissive in [(name, EMISSIVE) for name in BLUE] + [(PLUME_SMOKE, PLUME_EMISSIVE)]:
        blue = parse(stock[f'material_instances/fx/artifacts/{name}_blue.material'])
        files[f'material_instances/fx/artifacts/{name}_ice.material'] = ice_material(blue, ICE, emissive)
    return files


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
    for path, tree in frozen_files(stock).items():
        data = serialize(tree)
        assert sort_key(parse(data)) == sort_key(tree), path
        files[path] = data
        operations.append({'op': 'add_file', 'path': path})
    for path in STOCK:   # the writer against the game's own files: the same trees
        assert sort_key(parse(serialize(parse(stock[path])))) == sort_key(parse(stock[path])), path
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': (
            "Freeze Tag's frozen glow (exe patch 160 draws it on each frozen body as scene particles): "
            'fx/artifacts/human_frozen, the Spring activation glow fx/artifacts/human_spring (unchanged) in ice '
            f"blue {ICE} and fainter: its blue materials' ice copies fx/artifacts/<name>_ice (constant_emissive "
            f'the ice blue x {EMISSIVE}), every emitter at {RATE} of its rate, its point light ice blue at '
            f'{LIGHT} of its intensity; and its plume, a column of ice smoke (fx/artifacts/smoke_ice, smoke_blue '
            f'with the ice blue x {PLUME_EMISSIVE}) rising about 4 m from the body ({PLUME_RATE:g} a second, '
            f'{PLUME_LIFE} s), so that a frozen body shows over low cover and at range'),
        'operations': operations,
        'notes': ['made by srp-026g/scripts/freeze-tag-glow/make_pack.py from the stock resources.db\'s '
                  'Spring glow (docs/notes/freeze-tag.md)'],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{args.out}: {len(files)} files, pack.json sha256 {sha256((args.out / "pack.json").read_bytes())}')


if __name__ == '__main__':
    main()
