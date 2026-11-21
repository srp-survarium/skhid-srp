#!/usr/bin/env python3
"""Before/after previews of the STK2 AK-74s' scope fits (scope_fits.py): per optic, the live
AK-74M's fit on the left, the new gun's on the right; the gun with its optic from the left, and the
view through a collimator from the variant's eye (an optical scope: its line from behind the eye).

    python3 -I scope_preview.py --live <tree> --scopes <pack folder> --out <png> \
        --scope <shop scope name>... [--gun <name>]

<live>: the live files (the AK-74M's variants with their models, materials and textures) with the
weapons-stk2 pack's files over them; <scopes>: the weapons-stk2-scopes pack. Same camera and
exposure on both sides. Needs NumPy, SciPy and Pillow.
"""
import argparse
import math
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import preview as P  # noqa: E402
import scopes_lib as lib  # noqa: E402

M = P.M


class Static:
    """A static surface (28-byte vertices: position, three packed vectors, half UV)."""

    def __init__(self, path, diffuse, normal):
        b = path.read_bytes()
        _, size, n = struct.unpack_from('<3I', b)
        raw = np.frombuffer(b, np.uint8, n * 28, 12).reshape(n, 28)
        self.pos = raw[:, :12].copy().view('<f4').reshape(n, 3).astype(np.float64)
        self.nrm, self.tan, self.bit = lib.basis(raw)
        self.uv = raw[:, 24:28].copy().view('<f2').reshape(n, 2).astype(np.float64)
        _, _, count = struct.unpack_from('<3I', b, 8 + size)
        self.tri = np.frombuffer(b, '<u2', count, 20 + size).reshape(-1, 3).astype(np.int64)
        self.diffuse, self.normal = diffuse, normal

    def posed(self, m):
        h = lambda v, w: (np.c_[v, np.full(len(v), w)] @ m)[:, :3]
        return h(self.pos, 1.0), h(self.nrm, 0.0), h(self.tan, 0.0), h(self.bit, 0.0)


def find(roots, path):
    return next(r / path for r in roots if (r / path).exists())


def static_model(roots, model):
    d = find(roots, f'models/{model}.model')
    settings = M.config(d / 'settings')
    out = []
    for s in settings['lod_hierrarchy']['LOD0']['surfaces']:
        effect = M.config(find(roots, f'material_instances/{settings["material_settings"][s]["material_name"]}.material'))
        stage = effect['material'].get('g_stage') or effect['material'].get('forward')
        tex = lambda key: (np.asarray(Image.open(find(roots, f'textures/{stage["effect"][key]["value"]}.dds')).convert('RGBA'),
                                      np.float32) / 255.0 if stage['effect'].get(key, {}).get('value') else
                           np.full((1, 1, 4), [0.5, 0.5, 0.5, 0.5], np.float32))
        out.append(Static(d / f'render/{s}/converted_model', tex('texture_diffuse'), tex('texture_normal')))
    return out


def render_pair(roots, live, cfg, size):
    mods = {tuple(m['path']): m['value'] for x in cfg['modifications'] for m in x['modifiers']}
    gun_dir = find(roots, f'models/{mods[("object", "model")]}.skinned_model')
    gun = P.Model(roots[0], mods[('object', 'model')])
    bind = gun.bind
    world = dict(bind)
    gun_meshes = gun.meshes(world)
    scope = M.locator(gun_dir, 'scope_point')
    scope_frame = P.mat(P.HALF, np.zeros(3)) @ M.matrix(scope.position, scope.rotation) @ bind[gun.names[scope.bone]]
    idle = [(s, *s.posed(scope_frame)) for s in static_model(roots, cfg['data']['idle_model'])]
    aimed = [(s, *s.posed(scope_frame)) for s in static_model(roots, cfg['data']['aimed_model'])]
    # the side: from the gun's left, level, on the receiver
    target = np.array([0.0, 0.06, 0.12])
    side = P.look_at(target + np.array([-1.1, 0.12, 0.0]), target, np.array([0.0, 1.0, 0.0]))
    a = P.draw(gun_meshes + idle, side, size, 22.0)
    aim = M.locator(gun_dir, 'aim')
    eye = P.mat(P.HALF, np.zeros(3)) @ M.matrix(aim.position, aim.rotation) @ bind[gun.names[aim.bone]]
    fov = 2 * math.degrees(math.atan(math.tan(math.radians(P.FOV / 2)) * cfg['data'].get('fov_factor', 1.0)))
    if cfg['data'].get('hide_weapon_on_aim', False):
        # An optical scope draws its own view (forward shaders this renderer has not): the eye
        # pulled 25 cm back from its place instead, looking along its line, the optic in front.
        moved = eye.copy()
        moved[3, :3] -= eye[2, :3] * 0.25
        b = P.draw(gun_meshes + idle, moved, size, 30.0)
        what = 'its eye\'s line, from 25 cm behind'
    else:
        b = P.draw(gun_meshes + aimed, eye, size, fov, near=0.01)
        what = 'aimed, from the eye'
    ImageDraw.Draw(b).line([(size[0] / 2 - 6, size[1] / 2), (size[0] / 2 + 6, size[1] / 2)], fill=(0, 255, 0))
    ImageDraw.Draw(b).line([(size[0] / 2, size[1] / 2 - 6), (size[0] / 2, size[1] / 2 + 6)], fill=(0, 255, 0))
    return a, b, what


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--live', type=Path, required=True)
    ap.add_argument('--scopes', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--scope', action='append', required=True)
    ap.add_argument('--gun', default='ak74_tactical')
    args = ap.parse_args()
    roots = [args.live, args.scopes / 'files']
    size = (560, 315)
    rows = []
    for scope in args.scope:
        short = scope.removeprefix('shop_scope_')
        ours = next(p for p in (args.live / 'gameplay/items/scopes').glob('*.options')
                    if M.config(p).get('srp_scope_fit', {}).get('scope') == scope
                    and 'ak_74m' in M.config(p)['srp_scope_fit']['weapons'])
        theirs = args.scopes / f'files/gameplay/items/scopes/fitk_{short}_{args.gun}.options'
        tiles = []
        for label, path in (('AK-74M (live)', ours), (args.gun, theirs)):
            a, b, aimed = render_pair(roots, args.live, M.config(path), size)
            for img, what in ((a, 'side'), (b, aimed)):
                ImageDraw.Draw(img).text((8, 6), f'{label}  {short}  {what}', fill=(235, 235, 235))
                tiles.append(img)
        rows.append(tiles)
    sheet = Image.new('RGB', (size[0] * 4 + 24, (size[1] + 8) * len(rows)), (10, 10, 10))
    for i, tiles in enumerate(rows):
        for j, t in enumerate((tiles[0], tiles[2], tiles[1], tiles[3])):
            sheet.paste(t, (j * (size[0] + 8), i * (size[1] + 8)))
    sheet.save(args.out)
    print('wrote', args.out)


if __name__ == '__main__':
    main()
