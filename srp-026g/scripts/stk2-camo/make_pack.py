#!/usr/bin/env python3
"""make_pack.py: the content pack that lets the STK2 outfits' jackets and trousers take a camouflage
(the owner, 2026-11-26: "let's also add camo to other STK2" armour, after the exoskeletons;
../../docs/notes/stk2-outfits.md, Camouflage).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/stk2-camo-<version>

--live  the released files at their game paths (`srp-game-data get --db <stock> --patch <each listed
        patch>... --out <folder> --paths <a file of LIVE's paths>`), each checked by its sha256 (LIVE):
        the jackets' and trousers' surfaces of every length (their UVs), the six diffuse maps, the stock
        mask's options.
--out   the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the manifest's
        `pack` row (release/content.tsv).

The bandit's, the loner's and the Novice's jackets and trousers (packs stk2-bandit, stk2-loner,
stk2-novice) took none: their configs had `parameters/camo_available` false, and the game bakes a
camouflage into a surface only when `<diffuse>_aoac.dds` loads beside its diffuse, which their maps
lacked. The pack sets the flag on the seven configs and adds a mask for each set's `torso_diffuse`
and `legs_diffuse` (the jacket's and the trousers' own surfaces, `torso` and `legs`, of every length;
the Novice's two jackets share one map). Their model file names are their own already (stk2-decals).

What a mask paints (as exo-camo's, ../exo-camo/make_pack.py), each map by its entry in SETS:
- `points`: the UV islands of the surfaces under them, and in them the cloth texels only, not the
  straps and the much darker or brighter details (darker than 0.55 or brighter than 1.45 of the cloth
  within `local` texels at 1024, 24 unless given: seams, stripes, zips, the belt);
- `whole`: the islands under them whole, at any shade (a garment of one cloth, whose shaded folds,
  rims and insides would fall to the darkness test: the loner's hood, scarf and sleeves, the Novice's
  collar);
- `cloth`: in the islands under its points, the shirt's light cloth (blurred luminance above
  `min_lum`, saturation below `max_sat`, in pieces of `min_texels` or more): the loner's shirt where
  his vest's islands hold it (shoulders, sides, hem);
- `polygons`: painted whole (the loner's knitted sweater between the vest's halves);
- not the badges: the dark patches (blurred luminance under 0.30, 800 texels or more, and 4 texels
  round) of the painted texels inside each `badges` zone (the loner's shoulder badges);
- not the `bare` boxes (the straps across the loner's sweater).
B and R keep the cloth's shading. Bare as well: the loner's vest, bags and holster, the bandit's
T-shirt (on his jacket's own surface, a garment of its own) and forearms (another surface), the
Novice's turtleneck under his collar, the gloves, boots, hood and backpack (other items, as stock ones).
@2 (2026-11-27, the owner: the Novice's jacket "leaves a square at the back", the loner's shirt
unpainted where it is "the same textile", the scarf "not done fully"): @1 left bare a box on the
Novice's back print (taken for a chest logo), a box on the loner's scarf (taken for an emblem) and
boxes round the whole of his shoulder badges; it painted no cloth inside other islands, and the
darkness test dropped shaded folds of the hood, scarf and sleeves. The bandit's and the trousers'
masks are @1's, byte for byte.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('exo_camo', HERE.parent / 'exo-camo' / 'make_pack.py')
exo = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(exo)

PACK_ID = 'stk2-camo'
VERSION = '2'
OPTIONS = exo.OPTIONS
# Each set: its configs (camo_available), and for each map the models whose surface of that name
# draws it (every length) and what its mask paints (the module's docstring; (u, v) from 0 to 1).
SETS = {
    'bandit': {
        'configs': ('torso/stk2_bandit_026_torso', 'legs/stk2_bandit_026_legs'),
        'torso': dict(models=('stk2_bandit_026_torso', 'stk2_bandit_026_torso_s2', 'stk2_bandit_026_torso_s3'),
                      points=((0.50, 0.30), (0.22, 0.65), (0.55, 0.65)), bare=()),
        'legs': dict(models=('stk2_bandit_026_legs', 'stk2_bandit_026_legs_s2', 'stk2_bandit_026_legs_s3'),
                     points=((0.49, 0.33),), bare=()),
    },
    'loner': {
        'configs': ('torso/stk2_loner_026_torso', 'legs/stk2_loner_026_legs'),
        'torso': dict(models=('stk2_loner_026_torso', 'stk2_loner_026_torso_s2', 'stk2_loner_026_torso_s3'),
                      points=(),
                      # the sleeves, the hood, the scarf and the scarf's inside at the throat
                      whole=((0.195, 0.235), (0.77, 0.24), (0.48, 0.42), (0.51, 0.64), (0.52, 0.58)),
                      # the vest's front and back islands
                      cloth=dict(points=((0.15, 0.75), (0.80, 0.70)), min_lum=0.32, max_sat=0.25, min_texels=400),
                      polygons=(((0.186, 0.632), (0.257, 0.632), (0.247, 0.915), (0.198, 0.915)),),
                      badges=((0.14, 0.01, 0.29, 0.13), (0.69, 0.01, 0.85, 0.13)),
                      bare=((0.18, 0.735, 0.26, 0.768), (0.18, 0.853, 0.26, 0.885)), local=8),
        'legs': dict(models=('stk2_loner_026_legs', 'stk2_loner_026_legs_s2', 'stk2_loner_026_legs_s3'),
                     points=((0.26, 0.59), (0.76, 0.31)), bare=()),
    },
    'novice': {
        'configs': ('torso/stk2_novice_026_torso', 'torso/stk2_novice_026_torso_v2', 'legs/stk2_novice_026_legs'),
        'torso': dict(models=('stk2_novice_026_torso', 'stk2_novice_026_torso_s2', 'stk2_novice_026_torso_s3',
                              'stk2_novice_026_torso_v2', 'stk2_novice_026_torso_v2_s2', 'stk2_novice_026_torso_v2_s3'),
                      points=((0.38, 0.215), (0.38, 0.46), (0.25, 0.80), (0.54, 0.80)),
                      # the collar's outside and inside
                      whole=((0.10, 0.562), (0.06, 0.80)), bare=(), local=8),
        'legs': dict(models=('stk2_novice_026_legs', 'stk2_novice_026_legs_s2', 'stk2_novice_026_legs_s3'),
                     points=((0.50, 0.68),), bare=()),
    },
}
# The released files read, by sha256 (the live game data of patch 73; read() refuses others).
LIVE = {
    'models/ported/stk2_bandit_026/stk2_bandit_026_legs.skinned_model/render/legs/converted_model': '7047bb652bc6709195e45e1be385c02f04cad8fc98f50f62902a688a4eca3d8c',
    'models/ported/stk2_bandit_026/stk2_bandit_026_legs_s2.skinned_model/render/legs/converted_model': 'e39cc6b3e4071cbb4464b50ca2525e277b1b95e9ea9471e2fce3954db23c8171',
    'models/ported/stk2_bandit_026/stk2_bandit_026_legs_s3.skinned_model/render/legs/converted_model': 'efc8d22265c5261b00b177a49a88dfcc832b905f9c754002b0ec0f1cd5a5b3cd',
    'models/ported/stk2_bandit_026/stk2_bandit_026_torso.skinned_model/render/torso/converted_model': '7ed75cb93b90018abe15d954dd49f4f0fee41dbb30f9f4f4ca457460a4a793d2',
    'models/ported/stk2_bandit_026/stk2_bandit_026_torso_s2.skinned_model/render/torso/converted_model': '2d0965b1f787c8e11b4b9e6f73f931c13673bbd2163b43e7d18821721ce70351',
    'models/ported/stk2_bandit_026/stk2_bandit_026_torso_s3.skinned_model/render/torso/converted_model': '2ba4df87962be32859507e20342d1e1bfb5f69e435030b7d5136556cb08fd51d',
    'models/ported/stk2_loner_026/stk2_loner_026_legs.skinned_model/render/legs/converted_model': '2968a074459acf1a3a72c50b1cb74fe67866f4f1de9f5b40286395098a6558c6',
    'models/ported/stk2_loner_026/stk2_loner_026_legs_s2.skinned_model/render/legs/converted_model': 'f31ee724a3fa689d06f3c3a51340f1dcba833599482503f4eccb0231c600ba96',
    'models/ported/stk2_loner_026/stk2_loner_026_legs_s3.skinned_model/render/legs/converted_model': 'acfbdc0c57e32fa7be47abeb7376c64fc84ce5849863d9010a1bc4fa075522a7',
    'models/ported/stk2_loner_026/stk2_loner_026_torso.skinned_model/render/torso/converted_model': '0962c786caf5e3e53103223c3c59187c03e29aaaa97a7ac93ac63b1f26e67002',
    'models/ported/stk2_loner_026/stk2_loner_026_torso_s2.skinned_model/render/torso/converted_model': 'f5399f7abe2dd4d8429d52f89a199ff161dd174d75e1eab6eeb674c362ee833f',
    'models/ported/stk2_loner_026/stk2_loner_026_torso_s3.skinned_model/render/torso/converted_model': '1a687f1120261ea8d1a8f22809246681466b427084e591994951d1889aafc41a',
    'models/ported/stk2_novice_026/stk2_novice_026_legs.skinned_model/render/legs/converted_model': '30651b9bb3e981e4bc9f255149e72e113ad322eb824640bf8d0c7b5ea5b64df8',
    'models/ported/stk2_novice_026/stk2_novice_026_legs_s2.skinned_model/render/legs/converted_model': 'a6e711219aac2fd767eb2d357a317d4b167c7124335b3f9cc1edbecef3fa51ea',
    'models/ported/stk2_novice_026/stk2_novice_026_legs_s3.skinned_model/render/legs/converted_model': '9acaf33ebf98ffa03407e27d65446c372865d52646c668a2a0c328b3e8b099dd',
    'models/ported/stk2_novice_026/stk2_novice_026_torso.skinned_model/render/torso/converted_model': '8e3226621f7a7e69431828471a6ef94e159ac0bec8a7926be91dff514a1a6490',
    'models/ported/stk2_novice_026/stk2_novice_026_torso_s2.skinned_model/render/torso/converted_model': 'b0720b94170bbc9259a2ab41bd6bcbe40480cc753c18ee50e682e58ec38790ca',
    'models/ported/stk2_novice_026/stk2_novice_026_torso_s3.skinned_model/render/torso/converted_model': 'b6867e9b70026564d5db7e8b20f98e00cca5a4a431c070a3d346228d4f328f9f',
    'models/ported/stk2_novice_026/stk2_novice_026_torso_v2.skinned_model/render/torso/converted_model': 'a478f48f4987cdc2e990cd81380a6c6805357d0cab306cab18ccd5d2928ccce1',
    'models/ported/stk2_novice_026/stk2_novice_026_torso_v2_s2.skinned_model/render/torso/converted_model': 'e39cddd4b43495597631e9dff93d1d9beaa6cd142fb59a9fe3df8878f826354c',
    'models/ported/stk2_novice_026/stk2_novice_026_torso_v2_s3.skinned_model/render/torso/converted_model': '5700c743b441bbcd7912ee020aaccba78727e2a00e18eb29bc09da4bef0e2186',
    'textures/character/human/black_market_01/black_market_01_torso_aoac.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
    'textures/ported/stk2_bandit_026/legs_diffuse.dds': '07e9e84c3d2e9bbdc69157a179d0bbaa745d96384c754066c43c7b0a094196db',
    'textures/ported/stk2_bandit_026/torso_diffuse.dds': 'd721062c4e262b9ebdfaad877cc14fc976d54297a9b9c55db9e7ce6f9a926b19',
    'textures/ported/stk2_loner_026/legs_diffuse.dds': 'e254f5b7ee754bf0569060b07cfdcbda858f28d08f372828aa9087622cf6f8d0',
    'textures/ported/stk2_loner_026/torso_diffuse.dds': '2df57c669871f77d5339ffe5f6ddaa12681ec194fa8ef9bb9244996414a21c46',
    'textures/ported/stk2_novice_026/legs_diffuse.dds': '367ae136c3ed4397e76aed6f4451e03b672fc2f290d3fe783aa92c21d667d87e',
    'textures/ported/stk2_novice_026/torso_diffuse.dds': '52c0daea343a9f50c086f52fd0cd1890208599fd05f8a10f86927a8eeef14541',
}


def read(live, path):
    data = (live / path).read_bytes()
    if hashlib.sha256(data).hexdigest() != LIVE.get(path):
        sys.exit(f'{path} is not a released file this pack reads (LIVE)')
    return data


def islands(live, suit, part, spec, size, points):
    """The texels of the islands under `points`, over every model that draws the map."""
    out = np.zeros((size, size), bool)
    for model in spec['models']:
        raw = read(live, f'models/ported/stk2_{suit}_026/{model}.skinned_model/render/{part}/converted_model')
        _, uv, _, tris = exo.surface(raw)
        grid = exo.raster(uv, tris, exo.islands(tris, len(uv)), size)
        picked = {grid[min(int(v * size), size - 1), min(int(u * size), size - 1)] for u, v in points} - {-1}
        out |= np.isin(grid, sorted(picked))
    return out


def pieces(texels, min_texels):
    """The connected pieces of `texels` of min_texels or more."""
    labels, count = ndimage.label(texels)
    sizes = ndimage.sum(texels, labels, range(1, count + 1))
    return np.isin(labels, [i + 1 for i, n in enumerate(sizes) if n >= min_texels])


def box(size, u0, v0, u1, v1):
    out = np.zeros((size, size), bool)
    out[int(v0 * size):int(v1 * size), int(u0 * size):int(u1 * size)] = True
    return out


def chosen(live, suit, part, spec, base):
    """(region, whole): the texels whose cloth is painted, and those painted at any shade."""
    size = base.shape[0]
    k = size / 1024
    region = islands(live, suit, part, spec, size, spec['points'])
    whole = islands(live, suit, part, spec, size, spec.get('whole', ()))
    if 'cloth' in spec:
        c = spec['cloth']
        L = exo.blur(exo.lum(base), 4 * k)
        hi, lo = base.max(2), base.min(2)
        sat = exo.blur((hi - lo) / np.maximum(hi, 1e-3), 4 * k)
        light = islands(live, suit, part, spec, size, c['points']) & (L > c['min_lum']) & (sat < c['max_sat'])
        region |= pieces(light, c['min_texels'] * k * k)
    for polygon in spec.get('polygons', ()):
        drawn = Image.new('L', (size, size), 0)
        ImageDraw.Draw(drawn).polygon([(u * size, v * size) for u, v in polygon], fill=255)
        whole |= np.asarray(drawn) > 0
    bare = np.zeros((size, size), bool)
    for zone in spec.get('badges', ()):
        dark = (region | whole) & box(size, *zone) & (exo.blur(exo.lum(base), 4 * k) < 0.30)
        bare |= ndimage.binary_dilation(pieces(dark, 800 * k * k), iterations=max(1, round(4 * k)))
    for b in spec['bare']:
        bare |= box(size, *b)
    return (region | whole) & ~bare, whole & ~bare


def mask(base, region, whole, local=24):
    """The _aoac of `base` painting `region`'s cloth and `whole` at any shade (exo.mask's rule, with an
    upper bound; the cloth compared within `local` texels at 1024)."""
    k = base.shape[0] / 1024
    full = exo.lum(base)
    L = exo.blur(full, 1.5 * k)
    around = exo.blur(np.where(region, full, 0), local * k) / np.maximum(exo.blur(region.astype(float), local * k), 1e-3)
    keep = (L > 0.55 * around) & (L < 1.45 * around + 0.04) & (L > 0.04)
    keep = exo.blur(keep.astype(float), 2.5 * k) > 0.55
    paint = (region & keep) | whole
    soft = exo.blur(paint.astype(float), 1.5 * k)
    B = 0.5 + np.clip(4.0 * (full - exo.blur(full, 4 * k)), -0.5, 0.5) * soft
    broad = exo.blur(full, 12 * k)
    ref = np.percentile(broad[paint], 90) if paint.any() else 1.0
    R = 1 - 0.7 * np.clip(1 - broad / max(ref, 1e-3), 0, 1) * soft
    return np.stack([R, 1 - soft, B], 2), float(paint.mean())


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--live', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out} exists')
    files = {}
    options = read(a.live, OPTIONS)
    for suit, spec in SETS.items():
        for part in ('torso', 'legs'):
            name = f'textures/ported/stk2_{suit}_026/{part}_diffuse'
            base = exo.rgb(read(a.live, f'{name}.dds'))
            region, whole = chosen(a.live, suit, part, spec[part], base)
            aoac, share = mask(base, region, whole, spec[part].get('local', 24))
            files[f'{name}_aoac.dds'] = exo.weapon_camo.dds_dxt1(aoac)
            files[f'{name}_aoac.options'] = options
            print(f'{name}_aoac: {share:.1%} of the texture painted')
    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations += [{'op': 'config_set', 'path': f'gameplay/items/armour/{config}.options',
                    'values': {'parameters.camo_available': True}}
                   for spec in SETS.values() for config in spec['configs']]
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        # The configs it edits are the outfit packs' own files (each adds them whole).
        'requires': [f'stk2-{suit}' for suit in SETS],
        'description': 'the STK2 bandit\'s, loner\'s and Novice\'s jackets and trousers take a camouflage '
                       '(parameters/camo_available and the _aoac masks of their torso and legs maps: the cloth, '
                       'not the vests, bags, straps, buckles or badges)',
        'operations': operations,
    }
    for path, data in files.items():
        target = a.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (a.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    print(f'{a.out}: {len(files)} files, {len(operations)} operations; pack.json '
          f'{hashlib.sha256((a.out / "pack.json").read_bytes()).hexdigest()}')


if __name__ == '__main__':
    main()
