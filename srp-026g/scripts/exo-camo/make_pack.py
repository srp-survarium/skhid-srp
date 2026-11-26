#!/usr/bin/env python3
"""make_pack.py: the content pack that lets the Duty and Freedom exoskeleton bodies take a camouflage
(the owner, 2026-11-26: "maybe we can add camo to the exoskeleton too", "needs to be tasteful";
../../docs/notes/exo-suits.md, Camouflage).

usage: python3 -I make_pack.py --live <folder> --out <packs folder>/exo-camo-<version>
           [--level minimal|moderate|fuller]

--live      the released files at their game paths (`srp-game-data get --db <stock> --patch <each
            listed patch>... --out <folder> --paths <a file of NEEDS' paths>`), each checked by its
            sha256 (LIVE): the bodies' surfaces (their UVs), the two suits' torso and legs diffuse
            maps, the Freedom legs' model, the stock mask's options.
--out       the pack's folder (a new one): pack.json and files/, for `vostok-port pack` and the
            manifest's `pack` row (release/content.tsv).
--level     how much of the suit a camouflage paints (LEVELS; the owner's pick, minimal, is the
            default: the pack is built with it).

A stock vest or pair of trousers takes a camouflage by its config's `parameters/camo_available` and
by `<diffuse>_aoac.dds` beside each surface's diffuse: the game bakes the camouflage into a surface
only when that mask loads (camouflage_decal_texture_type_baker_cook; weapon-camo.md, "The engine"):
    c = lerp(diffuse, camo(uv * 4), 1 - aoac.G)       G: 1 bare, 0 painted
    c = overlay(c, aoac.B) * aoac.R                   B 0.5 and R 1 change nothing
The STK2 bodies (packs duty-exo and freedom-exo) have neither. The pack sets the flag on both bodies
and adds a mask for the two textures a body's own surfaces draw, `torso_diffuse` (its torso_s: the
sleeves, the chest and back panel, the shoulder pads, the waist band, the pouches) and `legs_diffuse`
(the legs' legs_s: the trousers, the knee pads), per suit (their own colours, one UV layout). The
frame and the back unit (`attach_diffuse`), the chest sign, the gloves, boots and helmet (other items,
as stock ones) get none and stay as they are.

What a mask paints: the UV islands of the body's surfaces that LEVELS names, and in them the cloth
and plate texels only: not the black straps, webbing and holes (darker than 0.55 of the part around
them) and not the faction's red or orange stripes (saturated). On a painted texel B carries the
texture's fine shading and R its broad shading (the weapon masks' rule, weapon_camo.aoac), so the
camouflage keeps the suit's wear and folds.

The Freedom body's legs get a model file name of their own, `ported/freedom_026/freedom_exo_legs_s1`
(the legs' six files again, byte for byte): the game names an item's baked textures after its
models' file names without their folders, and both suits' legs are `duty_03_legs_s1` (exo-decals did
the same for the torso).
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

import numpy as np
from PIL import Image, ImageFilter

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'weapon-camo'))
import weapon_camo  # noqa: E402  (dds_dxt1: the stock masks' DXT1 with mips)

PACK_ID = 'exo-camo'
VERSION = '1'
SUITS = ('duty', 'freedom')
CONFIGS = 'gameplay/items/armour/torso'
# Each texture a body's own surface draws and the surface whose UVs lay it out (one mesh, both suits).
SURFACES = {
    'torso': 'ported/duty_026/duty_03_torso_s1.skinned_model/render/torso_s/converted_model',
    'legs': 'ported/duty_026/duty_03_legs_s1.skinned_model/render/legs_s/converted_model',
}
# The UV islands each level paints (their numbers: islands(), the triangles connected through shared
# vertices, numbered by np.unique of each one's root vertex; exo-suits.md, Camouflage). torso: 29, 30 the
# sleeves, 24 the chest and back panel, 7 and 9 the shoulder pads, 26, 25, 22 the waist band, 19 the
# chest disc, 11 to 14 the pouches. legs: 0 the trousers, 2 and 4 the knee pads, 1 and 3 their backs.
# Never: the straps (torso 1, 4, 21, 23), the PDA (10), the small fittings, the gloves' island.
LEVELS = {
    'minimal': {'torso': (29, 30), 'legs': (0,)},
    'moderate': {'torso': (29, 30, 24, 7, 9), 'legs': (0, 2, 4)},
    'fuller': {'torso': (29, 30, 24, 7, 9, 26, 25, 22, 19, 11, 12, 13, 14), 'legs': (0, 1, 2, 3, 4)},
}
DEFAULT_LEVEL = 'minimal'  # the owner's pick, 2026-11-26
OPTIONS = 'textures/character/human/black_market_01/black_market_01_torso_aoac.options'
LEGS_OLD = 'ported/freedom_026/duty_03_legs_s1'
LEGS_NEW = 'ported/freedom_026/freedom_exo_legs_s1'
TABLES = ('data', 'data_full', 'data_short', 'data_long')
# The released files read, by sha256 (the live game data of patch 73; read() refuses others).
LIVE = {
    'models/ported/duty_026/duty_03_legs_s1.skinned_model/render/legs_s/converted_model': '2aa237d776d2ce625777fcacf2f0fb774e697e50e32066fc4af20eb3ada0c1a8',
    'models/ported/duty_026/duty_03_torso_s1.skinned_model/render/torso_s/converted_model': '1b466d0c2d2071b5242bcdab61e7310c0bc84a770ff55e9f1c1dae4ab5a404d1',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/render/bind_pose': '72d6130cddfe7dfa66f46a5c9f709d4c0d068b57d2f53a9fb74757ed197c6e93',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/render/export_properties': '6922840be31cab7a87413c409493ffb368e7d8cc45ee922a4ed1efd45de1470e',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/render/legs_s/converted_model': '2aa237d776d2ce625777fcacf2f0fb774e697e50e32066fc4af20eb3ada0c1a8',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/render/legs_s/export_properties': 'ef66e7e0d6cf485f370c41414a8300aa6f2ebf4fa4df859e31f97cbfb8057ec3',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/settings': '83dfb88d2232d0b49f5dcf98e6b8256aee81dd595de14f5d53cfea1bbfdc88e2',
    'models/ported/freedom_026/duty_03_legs_s1.skinned_model/skeleton': '2a952e1632dd4209fc310032c295ede5682f5e46488014d7837fcadf23acdd0c',
    'textures/character/human/black_market_01/black_market_01_torso_aoac.options': '4a360f03fe2638a7b41aa4f0af1d590600ae74504fb7d99b65a7e744e9004d2d',
    'textures/ported/duty_026/legs_diffuse.dds': '2fb04c04d71a52114e1743cdd65bdec449f95724a89da707780f7b7c05c8fdfc',
    'textures/ported/duty_026/torso_diffuse.dds': 'ad1cd4e83ee9d167be95c6a7074ee0ca17ed1a108455c21e4165c647625e7df2',
    'textures/ported/freedom_026/legs_diffuse.dds': '701049dee471756692f5f1efdf51a6ab0ca9503cb9a6340bf894f6002159730f',
    'textures/ported/freedom_026/torso_diffuse.dds': 'f7152530945e7e54bacc1987d768717ecb63e831de19bd6f89bfdd21e6870845',
}


def surface(raw):
    """(positions[n,3], uv[n,2], normals[n,3], triangles[m,3]) of a converted_model."""
    uv, tris = weapon_camo.mesh_uv(raw)
    _, size, count = struct.unpack_from('<III', raw, 0)
    stride = (size - 4) // count
    v = np.frombuffer(raw, np.uint8, count * stride, 12).reshape(count, stride)
    p = v[:, :12].copy().view('<f4').astype(float)
    n = v[:, 28:31][:, ::-1].astype(float) / 127.5 - 1
    return p, uv, n, tris


def islands(tris, vertices):
    """Each triangle's UV island: triangles that share a vertex (a UV seam splits vertices)."""
    parent = np.arange(vertices)

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    for a, b, c in tris:
        ra = find(a)
        parent[find(b)] = ra
        parent[find(c)] = ra
    roots = np.array([find(t[0]) for t in tris])
    _, ids = np.unique(roots, return_inverse=True)
    return ids


def raster(uv, tris, ids, size):
    """Each texel's island (-1: none) at `size` x `size`."""
    out = np.full((size, size), -1, int)
    q_all = uv * size
    for t, tri in enumerate(tris):
        q = q_all[tri]
        x0, y0 = np.maximum(np.floor(q.min(0)).astype(int), 0)
        x1, y1 = np.minimum(np.ceil(q.max(0)).astype(int), size - 1)
        if x0 > x1 or y0 > y1:
            continue
        den = (q[1, 1] - q[2, 1]) * (q[0, 0] - q[2, 0]) + (q[2, 0] - q[1, 0]) * (q[0, 1] - q[2, 1])
        if abs(den) < 1e-12:
            continue
        yy, xx = np.mgrid[y0:y1 + 1, x0:x1 + 1] + 0.5
        a = ((q[1, 1] - q[2, 1]) * (xx - q[2, 0]) + (q[2, 0] - q[1, 0]) * (yy - q[2, 1])) / den
        b = ((q[2, 1] - q[0, 1]) * (xx - q[2, 0]) + (q[0, 0] - q[2, 0]) * (yy - q[2, 1])) / den
        inside = (a >= -0.01) & (b >= -0.01) & (1 - a - b >= -0.01)
        out[y0:y1 + 1, x0:x1 + 1][inside] = ids[t]
    return out


def blur(a, r):
    img = Image.fromarray(np.clip(a * 255, 0, 255).astype(np.uint8))
    return np.asarray(img.filter(ImageFilter.GaussianBlur(r)), float) / 255


def lum(c):
    return c @ np.array([0.299, 0.587, 0.114])


def mask(base, grid, chosen_ids):
    """The _aoac [h,w,3] of a diffuse `base` [h,w,3] 0..1 painting `chosen_ids`' cloth and plates."""
    k = base.shape[0] / 1024                       # the radii were set at 1024 texels
    chosen = np.isin(grid, chosen_ids)
    L = blur(lum(base), 1.5 * k)
    around = blur(np.where(chosen, lum(base), 0), 24 * k) / np.maximum(blur(chosen.astype(float), 24 * k), 1e-3)
    mx, mn = base.max(2), base.min(2)
    sat = blur((mx - mn) / np.maximum(mx, 1e-3), 1.5 * k)
    keep = (L > 0.55 * around) & (L > 0.04) & (sat < 0.33)
    keep = blur(keep.astype(float), 2.5 * k) > 0.55
    paint = chosen & keep
    soft = blur(paint.astype(float), 1.5 * k)
    full = lum(base)
    B = 0.5 + np.clip(4.0 * (full - blur(full, 4 * k)), -0.5, 0.5) * soft
    broad = blur(full, 12 * k)
    ref = np.percentile(broad[paint], 90) if paint.any() else 1.0
    R = 1 - 0.7 * np.clip(1 - broad / max(ref, 1e-3), 0, 1) * soft
    return np.stack([R, 1 - soft, B], 2), float(paint.mean())


def read(live, path):
    data = (live / path).read_bytes()
    if hashlib.sha256(data).hexdigest() != LIVE.get(path):
        sys.exit(f'{path} is not a released file this pack reads (LIVE)')
    return data


def rgb(raw):
    import io
    return np.asarray(Image.open(io.BytesIO(raw)).convert('RGB'), float) / 255


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--live', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--level', choices=sorted(LEVELS), default=DEFAULT_LEVEL)
    a = p.parse_args()
    if a.out.exists():
        sys.exit(f'{a.out} exists')
    files, operations = {}, []
    layout = {}
    for part, path in SURFACES.items():
        _, uv, _, tris = surface(read(a.live, f'models/{path}'))
        layout[part] = (uv, tris, islands(tris, len(uv)))
    options = read(a.live, OPTIONS)
    for suit in SUITS:
        for part, (uv, tris, ids) in layout.items():
            name = f'textures/ported/{suit}_026/{part}_diffuse'
            base = rgb(read(a.live, f'{name}.dds'))
            grid = raster(uv, tris, ids, base.shape[0])
            aoac, share = mask(base, grid, LEVELS[a.level][part])
            files[f'{name}_aoac.dds'] = weapon_camo.dds_dxt1(aoac)
            files[f'{name}_aoac.options'] = options
            print(f'{name}_aoac: {share:.1%} of the texture painted ({a.level})')
    old = a.live / f'models/{LEGS_OLD}.skinned_model'
    for source in sorted(q for q in old.rglob('*') if q.is_file()):
        rel = source.relative_to(old).as_posix()
        files[f'models/{LEGS_NEW}.skinned_model/{rel}'] = read(a.live, f'models/{LEGS_OLD}.skinned_model/{rel}')
    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    camo = {'parameters.camo_available': True}
    legs = {f'{table}.additional_models.2.base_model': LEGS_NEW for table in TABLES}
    operations += [
        {'op': 'config_set', 'path': f'{CONFIGS}/duty_exo_026.options', 'values': camo},
        {'op': 'config_set', 'path': f'{CONFIGS}/freedom_exo_026.options', 'values': {**camo, **legs}},
    ]
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': 'the Duty and Freedom exoskeleton bodies take a camouflage (parameters/camo_available and '
                       f'the _aoac masks of their torso and legs maps, {a.level} coverage: the cloth and plates, not '
                       f'the frame, straps or stripes); the Freedom legs under a file name of their own ({LEGS_NEW})',
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
