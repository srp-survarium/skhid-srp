#!/usr/bin/env python3
"""review.py: a review sheet of the pack: each new scope on a few of its weapons, the outside view
(the weapon and the variant's model at scope_point, turned 180 degrees about Y as the weapon draws a
scope) and the aimed view (from the weapon model's `aim` locator along the barrel, at the scope's
zoom: a collimator with its weapon, a scope's aimed model alone, as hide_weapon_on_aim has it).

usage: python3 -I review.py --pack <make_pack.py's folder> --live <folder> --donor <folder> --out <png>

An approximation for review: the weapon untextured grey, the optics with their materials as
render_icon.py shades them; the red dot and reticle dots red, the reticle grid and the eyepiece's
background black, the lenses left out; a vertical field of view of BASE_FOV degrees times the
scope's fov_factor over the weapon's aim_zoom_factor; a collimator's view clipped NEAR_COLLIMATOR
metres in front of the eye (the weapon's own rear sight is in front of the camera there), its red
dot marked with a red ring where it projects.
"""
import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render_icon as ri  # noqa: E402
import scopes_lib as lib  # noqa: E402

BASE_FOV = 45.0
NEAR_COLLIMATOR = 0.08
PER_SCOPE = 3
PREFERRED = ('m4a1', 'ak_74m', 'hk416', 'a_545', 'fn_fnc', 'ak_12', 'akmn', 'rpk', 'vepr', 'socom16', 'msbs_carbine')
VIEW = (420, 200)
AIM = (320, 200)


def coloured(model, roots, offset, flip, skip=('scope_lense',)):
    """The model's Surfaces with its materials, the dots red and the grid and eyepiece background
    black (flat colours: the game draws them in its forward pass)."""
    def flat(material):
        if 'glowing_dot' in material or 'reddot' in material:
            return ri.Material(albedo=(1.0, 0.02, 0.02), gloss=0.0, f0=0.0)
        if 'aimgrid' in material or 'aim_background' in material:
            return ri.Material(albedo=(0.0, 0.0, 0.0), gloss=0.0, f0=0.0)
        return None
    return ri.parts(model, roots, offset=offset, flip=flip, skip=skip, materials=flat)


def weapon_parts(model_dir):
    """The weapon's meshes, flat grey."""
    grey = ri.Material(albedo=(0.3, 0.3, 0.3), gloss=0.2, f0=0.05)
    return ri.parts(model_dir, [], materials=lambda name: grey)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--pack', required=True, type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    files = args.pack / 'files'
    roots = [files, args.donor, args.live]
    report = {}
    for scope, weapon, dict_id, change in lib.rows('variants.tsv'):
        report.setdefault(scope, []).append((weapon, change))
    weapons = {}
    d = lib.leaves(args.live / 'gameplay/db_static_dictionaries')
    i = 0
    while f'/items_dict[{i}]/name' in d:
        cfg = d[f'/items_dict[{i}]/cfg_name']
        if cfg.startswith('gameplay/items/weapons/wpn_'):
            w = lib.leaves(args.live / cfg)
            weapons[d[f'/items_dict[{i}]/name']] = (w.get('/object/model'), w.get('/parameters/aim_zoom_factor', 1.0))
        i += 1
    rows_out = []
    for scope in lib.scopes():
        fitted = report.get(scope['name'], [])
        chosen = [x for p in PREFERRED for x in fitted if x[0] == p][:PER_SCOPE] or fitted[:PER_SCOPE]
        for weapon, change in chosen:
            base, zoom = weapons[weapon]
            model = change if change != '-' else base
            wdir = lib.model_dir(files, model) or lib.model_dir(args.live, model)
            props = lib.leaves(wdir / 'render/export_properties')
            sp = np.array(lib.locator(props, 'scope_point')[1])
            eye = np.array(lib.locator(props, 'aim')[1])
            name = f'fitn_{scope["short"]}_{weapon}'
            idle = files / 'models/weapons/attach/fit4' / f'{name}.model'
            aimed = files / 'models/weapons/attach/fit4' / f'{name}_aim.model'
            gun = weapon_parts(wdir)
            outside = ri.render(gun + coloured(idle, roots, sp, True), VIEW, -90 + 20, 12,
                                frame=(VIEW[0] * 0.04, VIEW[1] * 0.04, VIEW[0] * 0.96, VIEW[1] * 0.96),
                                background=(70, 80, 95))
            optic = coloured(aimed, roots, sp, True, skip=('scope_lense',))
            items = optic + (gun if scope['kind'] == 'C' else [])
            fov = BASE_FOV * scope['fov'] / float(zoom or 1.0)
            near = NEAR_COLLIMATOR if scope['kind'] == 'C' else 0.01
            view = ri.perspective(items, AIM, eye, fov, near=near, background=(150, 170, 190))
            if scope['kind'] == 'C':
                # the red dot is a few pixels in the game's view (an emissive disk 2.2 mm across, 0.6 m
                # on): marked where it projects
                c = lib.bbox_centre(aimed, lib.DOT) * np.array([-1.0, 1.0, -1.0]) + sp - eye
                f = (AIM[1] / 2) / np.tan(np.radians(fov) / 2)
                x, y = AIM[0] / 2 + f * c[0] / -c[2], AIM[1] / 2 - f * c[1] / -c[2]
                ImageDraw.Draw(view).ellipse((x - 4, y - 4, x + 4, y + 4), outline=(255, 30, 30), width=2)
            rows_out.append((f'{scope["name"]} on {weapon} (x{1 / scope["fov"]:.2f})', outside, view))
            print(rows_out[-1][0])
    w = VIEW[0] + AIM[0] + 30
    h = len(rows_out) * (VIEW[1] + 28) + 10
    sheet = Image.new('RGB', (w, h), (30, 32, 36))
    draw = ImageDraw.Draw(sheet)
    y = 10
    for title, outside, view in rows_out:
        draw.text((10, y), title, fill=(230, 230, 230))
        sheet.paste(outside.convert('RGB'), (10, y + 16))
        sheet.paste(view.convert('RGB'), (20 + VIEW[0], y + 16))
        y += VIEW[1] + 28
    sheet.save(args.out)
    print(args.out)


if __name__ == '__main__':
    main()
