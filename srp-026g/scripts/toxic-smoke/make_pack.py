#!/usr/bin/env python3
"""make_pack.py: the content pack of Toxic Smoke's green cloud (2026-11-08;
../../docs/notes/technical-skills.md, "Dense Smoke and Toxic Smoke").

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/toxic-smoke-<version>

--stock  the stock resources.db's files at their game paths (no Skhid patch carries them;
         `srp-game-data get --db <resources.db> --out <folder> <path>...`): every path of STOCK.
--out    the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
         manifest's `pack` row (release/content.tsv).

A smoke grenade's cloud is its config's `particle_explosion`, fx/explosions/smoke_grenade (rdg-m,
gameplay/items/weapons/smoke_grenade.options): two alpha-blended smoke emitters (`smoke`,
`smoke_centr`) drawn with fx/explosions/smoke_grenade_no_light, the first puff (spone) with
fx/explosions/smoke_spone_no_light, and the flash. Exe patch 180 asks every smoke grenade object
for a second instance, fx/explosions/smoke_grenade_toxic, and plays it for a thrower with Toxic
Smoke (441): the tell everyone sees. The pack adds, as the artefact glow's grey copies are made
(../artefact-glow/make_pack.py `grey_material`):

- fx/explosions/smoke_grenade_toxic: the stock particle system value for value, its smoke and
  first puff drawn with the green copies (the flash as it is);
- fx/explosions/smoke_grenade_no_light_toxic and smoke_spone_no_light_toxic: the stock materials
  with their constant_emissive a slight green (GREEN, about the stock grey's brightness).
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

PACK_ID = 'toxic-smoke'
VERSION = '1'
HERE = Path(__file__).resolve().parent
SYSTEM = 'fx/explosions/smoke_grenade'
TOXIC_SYSTEM = SYSTEM + '_toxic'
# the materials the smoke draws with -> their green copies
MATERIALS = {
    'fx/explosions/smoke_grenade_no_light': 'fx/explosions/smoke_grenade_no_light_toxic',
    'fx/explosions/smoke_spone_no_light': 'fx/explosions/smoke_spone_no_light_toxic',
}
STOCK = {
    f'particles/{SYSTEM}.particle': 'a69dc7d30bbf66e40e519f11e5959458f7a678b949bf8c6d4032656b300c545a',
    'material_instances/fx/explosions/smoke_grenade_no_light.material':
        '5937622dbc3131a5d857dbbda1c43f333cd5d25ad932b719bd100a4756a02383',
    'material_instances/fx/explosions/smoke_spone_no_light.material':
        '79d25d9a1cdd398543ef9f35149907dcce26ff5614bf32c0bd8dc70f452b1190',
}
# The stock smoke's emissive is a grey of 0.48 (the first puff's 0.50); the green keeps its
# brightness (Rec. 709 luminance 0.48) with a slight tint: my pick.
GREEN = [0.4000000059604645, 0.5199999809265137, 0.3799999952316284, 1.0]


def glow():
    """The artefact glow's pack maker: its binary config reader and writer and its material copy."""
    spec = importlib.util.spec_from_file_location('artefact_glow_make_pack', HERE.parent / 'artefact-glow' / 'make_pack.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def toxic_particle(g, tree):
    """The stock system with its smoke materials' green copies: every `Material` string of MATERIALS
    replaced, nothing else."""
    t, v = tree
    if t == g.T_NAMED:
        return (t, [(k, (g.T_STR, MATERIALS[c[1]]) if k == 'Material' and c[0] == g.T_STR and c[1] in MATERIALS
                     else toxic_particle(g, c)) for k, c in v])
    if t == g.T_INDEXED:
        return (t, [toxic_particle(g, c) for c in v])
    return tree


def materials_of(g, tree):
    """Every `Material` an emitter of `tree` draws with."""
    t, v = tree
    found = []
    if t == g.T_NAMED:
        for k, c in v:
            if k == 'Material' and c[0] == g.T_STR:
                found.append(c[1])
            else:
                found += materials_of(g, c)
    elif t == g.T_INDEXED:
        for c in v:
            found += materials_of(g, c)
    return found


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--stock', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    g = glow()
    stock = {}
    for path, sha in STOCK.items():
        data = (args.stock / path).read_bytes()
        if sha256(data) != sha:
            sys.exit(f'{path}: sha256 {sha256(data)}, not the stock file\'s {sha}')
        stock[path] = data

    trees = {}
    particle = g.parse(stock[f'particles/{SYSTEM}.particle'])
    toxic = toxic_particle(g, particle)
    # the same system but its smoke's materials: two smoke emitters and the first puff, the flash kept
    assert sorted(materials_of(g, particle)) == sorted(['fx/explosions/flash', *MATERIALS, 'fx/explosions/smoke_grenade_no_light'])
    assert sorted(materials_of(g, toxic)) == sorted(['fx/explosions/flash', *MATERIALS.values(),
                                                     'fx/explosions/smoke_grenade_no_light_toxic'])
    assert g.sort_key(toxic_particle(g, toxic)) == g.sort_key(toxic)
    trees[f'particles/{TOXIC_SYSTEM}.particle'] = toxic
    for name, green_name in MATERIALS.items():
        source = g.parse(stock[f'material_instances/{name}.material'])
        assert g.get(source, 'material/forward/effect/blend_mode/value') == (g.T_INT, g.ALPHA_BLENDED)
        green = g.grey_material(source, GREEN)
        assert g.differences(green, source) == ['/material/forward/effect/constant_emissive/value']
        trees[f'material_instances/{green_name}.material'] = green

    files, operations = {}, []
    for path, tree in trees.items():
        data = g.serialize(tree)
        assert g.sort_key(g.parse(data)) == g.sort_key(tree), path
        files[path] = data
        operations.append({'op': 'add_file', 'path': path})
    for path in STOCK:   # the writer against the game's own files: the same trees
        assert g.sort_key(g.parse(g.serialize(g.parse(stock[path])))) == g.sort_key(g.parse(stock[path])), path

    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': "Toxic Smoke's green cloud (exe patch 180): fx/explosions/smoke_grenade_toxic, the stock smoke "
                       'grenade particle system value for value, its smoke and first puff drawn with green copies of '
                       'their materials (fx/explosions/smoke_grenade_no_light_toxic, smoke_spone_no_light_toxic: the '
                       'stock alpha-blended materials, constant_emissive (0.40, 0.52, 0.38) for the stock grey)',
        'operations': operations,
        'notes': ['made by srp-026g/scripts/toxic-smoke/make_pack.py from the stock resources.db\'s smoke grenade '
                  'particle system and materials (docs/notes/technical-skills.md)'],
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
