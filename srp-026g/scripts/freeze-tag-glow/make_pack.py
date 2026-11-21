#!/usr/bin/env python3
"""make_pack.py: the content pack of Freeze Tag's frozen glow (../../docs/notes/freeze-tag.md, "The
frozen glow"; exe patch 160 plays it on every frozen body).

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/freeze-tag-glow-<version>
       python3 -I make_pack.py --stock <folder> --out <folder> --preview

--stock    the stock resources.db's files at their game paths (`srp-game-data get --db <resources.db>
           --out <folder> --paths <list>`): every path of STOCK.
--out      the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
           manifest's `pack` row (release/content.tsv).
--preview  a pack for the previews only (never a manifest row): the stock Spring glow under its own
           name (effect_freeze_tag_frozen_stock) and every variant of VARIANTS as
           effect_freeze_tag_frozen_<variant>, each with its own system and materials.

The owner (2026-11-21): "effect can be anomalous artifact spring activation but of slightly
different hue and less intense". The base is the Spring artefact's activation glow,
gameplay/effects/effect_artefact_spring (a particle effect playing fx/artifacts/human_spring on the
body). The frozen glow, gameplay/effects/effect_freeze_tag_frozen, is made as the artefact-glow
pack makes an artefact's (../artefact-glow/make_pack.py): a composite of the third-person effect
playing fx/artifacts/human_frozen and its first-person twin (`view` 0) playing
fx/artifacts/human_frozen_fp: the same system with Head, Face and Neck off and without the point
light and the haze, so nothing washes the frozen player's own view. Spring's own files are not
touched: its activation is unchanged.

human_frozen is the stock Spring system with, for the variant chosen (SHIPPED):

- its three blue materials (fx/artifacts/<name>_blue) drawn with ice copies
  (fx/artifacts/<name>_ice: the stock material, its constant_emissive the variant's colour times its
  `emissive`; the materials are additive, so a smaller emissive is a fainter glow);
- every emitter's Rate (its curve and its data source) and MaxParticles (rounded up, at least 1)
  times its `rate`;
- its point light (up_light): its colour the variant's, its Intensity curve times `light`; or, with
  `light` None, the emitter left out.
"""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'artefact-glow'))
import make_pack as glow  # noqa: E402  (the artefact-glow pack's reader, writer and helpers)
from make_pack import T_NAMED, T_STR, get, parse, serialize, sha256, sort_key  # noqa: E402

PACK_ID = 'freeze-tag-glow'
VERSION = '1'
SPRING_EFFECT = 'gameplay/effects/effect_artefact_spring.options'
SPRING_SYSTEM = 'fx/artifacts/human_spring'
BLUE = ('rays', 'stroke_scroll', 'up')
STOCK = {
    SPRING_EFFECT: glow.STOCK[SPRING_EFFECT],
    f'particles/{SPRING_SYSTEM}.particle': glow.STOCK[f'particles/{SPRING_SYSTEM}.particle'],
    'material_instances/fx/artifacts/rays_blue.material':
        '2cb0b26d282b1870f04bd2372cf9c5fbf5234c3b2ed1dd61ba051d605dff0d10',
    'material_instances/fx/artifacts/stroke_scroll_blue.material':
        'b75df7286a56b32cf9d4704ef39298e9a58c9d2713ef375347f59892b53fd810',
    'material_instances/fx/artifacts/up_blue.material':
        '1229867293729394f2e3642d27db9225b6cfc0caebb8f50f510769deb8f46bd2',
}
EFFECT = 'gameplay/effects/effect_freeze_tag_frozen.options'
SYSTEM = 'fx/artifacts/human_frozen'
# Spring is (0, 0.40, 1): a deep blue. Each variant: its colour (the materials' and the light's),
# the materials' emissive (times the colour), the emitters' rate and the light's intensity (None:
# no light).
VARIANTS = {
    'a': {'colour': (0.62, 0.88, 1.0), 'emissive': 0.6, 'rate': 0.6, 'light': 0.5},
    'b': {'colour': (0.40, 0.84, 1.0), 'emissive': 0.6, 'rate': 0.6, 'light': 0.5},
    'c': {'colour': (0.85, 0.95, 1.0), 'emissive': 0.45, 'rate': 0.5, 'light': None},
}
SHIPPED = 'a'


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
    Intensity (curve and data source) times `light`, or its emitter left out with `light` None."""
    if light is None:
        tree = glow.keep_emitters(tree, lambda e: glow.emitter_name(e) != 'up_light')
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
                elif name == 'Intensity' and light is not None:
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
    assert len(sources) == len(users) + (light is not None), 'a light Intensity source shared with a Rate'
    lod = (T_NAMED, [(n, (T_NAMED, emitters) if n == 'children' else c) for n, c in lod[1]])

    def source(name, value):
        if name not in sources:
            return value
        return (T_NAMED, [(n, glow.rate_scaled(c, sources[name]) if n == 'source' else c) for n, c in value[1]])
    data = (T_NAMED, [(n, (T_NAMED, [(f, source(f, c)) for f, c in kinds[1]]) if n == 'float' else kinds)
                      for n, kinds in dict(tree[1])['data_sources'][1]])
    return (T_NAMED, [(n, lod if n == 'LOD 0' else data if n == 'data_sources' else c) for n, c in tree[1]])


def frozen_files(stock, variant, effect, system, suffix):
    """The variant's effect, its two systems and its ice materials, by path."""
    params = VARIANTS[variant]
    materials = {f'fx/artifacts/{name}_blue': f'fx/artifacts/{name}_ice{suffix}' for name in BLUE}
    spring = parse(stock[f'particles/{SPRING_SYSTEM}.particle'])
    tree = scaled(coloured(spring, params['colour'], materials), params['rate'], params['light'])
    assert len(glow.emitters_by_name(tree)) == len(glow.emitters_by_name(spring)) - (params['light'] is None)
    twin = glow.first_person_twin(glow.first_person_particle(tree))
    files = {
        effect: glow.forest_effect(parse(stock[SPRING_EFFECT]), system),
        f'particles/{system}.particle': tree,
        f'particles/{system}_fp.particle': twin,
    }
    for name in BLUE:
        blue = parse(stock[f'material_instances/fx/artifacts/{name}_blue.material'])
        files[f'material_instances/fx/artifacts/{name}_ice{suffix}.material'] = \
            ice_material(blue, params['colour'], params['emissive'])
    return files


def stock_files(stock):
    """The preview's stock Spring glow under its own name: Spring's effect, value for value."""
    return {'gameplay/effects/effect_freeze_tag_frozen_stock.options': parse(stock[SPRING_EFFECT])}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--preview', action='store_true')
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    stock = {}
    for path, sha in STOCK.items():
        data = (args.stock / path).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{path}: sha256 {sha256(data)}, not the stock file\'s {sha}')
        stock[path] = data
    if args.preview:
        trees = stock_files(stock)
        for variant in VARIANTS:
            trees |= frozen_files(stock, variant, f'gameplay/effects/effect_freeze_tag_frozen_{variant}.options',
                                  f'{SYSTEM}_{variant}', f'_{variant}')
        version, description = f'{VERSION}-preview', 'previews of the frozen glow, never a manifest row'
    else:
        trees = frozen_files(stock, SHIPPED, EFFECT, SYSTEM, '')
        params = VARIANTS[SHIPPED]
        version = VERSION
        description = (
            "Freeze Tag's frozen glow (exe patch 160): effect_freeze_tag_frozen, a composite of "
            'fx/artifacts/human_frozen (third person) and its first-person twin human_frozen_fp (no head, '
            'face or neck, no point light or haze): the Spring activation glow fx/artifacts/human_spring '
            f"in ice blue (its materials' ice copies fx/artifacts/<name>_ice, constant_emissive "
            f"{params['colour']} x {params['emissive']}), every emitter at {params['rate']} of its rate, "
            + (f"its light the same colour at {params['light']} of its intensity" if params['light'] is not None
               else 'no point light'))
    files, operations = {}, []
    for path, tree in trees.items():
        data = serialize(tree)
        assert sort_key(parse(data)) == sort_key(tree), path
        files[path] = data
        operations.append({'op': 'add_file', 'path': path})
    for path in STOCK:   # the writer against the game's own files: the same trees
        assert sort_key(parse(serialize(parse(stock[path])))) == sort_key(parse(stock[path])), path
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': version,
        'target_build': '0.26g',
        'description': description,
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
