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
"""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'artefact-glow'))
import make_pack as glow  # noqa: E402  (the artefact-glow pack's reader, writer and helpers)
from make_pack import T_NAMED, T_STR, parse, serialize, sha256, sort_key  # noqa: E402

PACK_ID = 'freeze-tag-glow'
VERSION = '1'
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
}
SYSTEM = 'fx/artifacts/human_frozen'
# Spring is (0, 0.40, 1), a deep blue; the frozen glow ice blue, fainter (the owner's pick, "a").
ICE = (0.62, 0.88, 1.0)
EMISSIVE = 0.6
RATE = 0.6
LIGHT = 0.5
# The thaw fades it (the owner, 2026-11-21: "would be nice if the glow was reducing as the player
# was unfrozen", "so others could deem the freeze state by look"): the particle instances the
# engine plays take no colour or emission scale once playing (scene_renderer has play, stop,
# remove and update of the transform only), so the glow comes at four strengths, one a quarter of
# the thaw, the patch swapping them: each a system and its materials, the ice emissive, the
# rate and the light all times the strength. The suffix of each one's paths, and its strength.
STRENGTHS = (('', 1.0), ('_70', 0.7), ('_40', 0.4), ('_15', 0.15))


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



def frozen_files(stock):
    """The glow's particle systems and their ice materials at each strength, by path."""
    spring = parse(stock[f'particles/{SPRING_SYSTEM}.particle'])
    files = {}
    for suffix, strength in STRENGTHS:
        materials = {f'fx/artifacts/{name}_blue': f'fx/artifacts/{name}_ice{suffix}' for name in BLUE}
        tree = scaled(coloured(spring, ICE, materials), RATE * strength, LIGHT * strength)
        assert len(glow.emitters_by_name(tree)) == len(glow.emitters_by_name(spring))
        files[f'particles/{SYSTEM}{suffix}.particle'] = tree
        for name in BLUE:
            blue = parse(stock[f'material_instances/fx/artifacts/{name}_blue.material'])
            files[f'material_instances/fx/artifacts/{name}_ice{suffix}.material'] = \
                ice_material(blue, ICE, EMISSIVE * strength)
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
            f'{LIGHT} of its intensity; at four strengths (the thaw fades it): '
            + ', '.join(f'{SYSTEM}{suffix} {strength:g}' for suffix, strength in STRENGTHS)
            + ' (emissive, rate and light times the strength)'),
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
