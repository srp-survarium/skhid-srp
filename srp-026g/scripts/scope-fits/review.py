#!/usr/bin/env python3
"""review.py: a review sheet of the scope fits pack: on each gun of guns.tsv (or those named) a
collimator and a scope it takes, outside (the gun and the variant's model at scope_point, turned
180 degrees about Y as the weapon draws a scope) and aimed (from the variant's weapon model's `aim`
locator along the barrel, at the scope's zoom: a collimator with the gun, a scope's aimed model
alone), as ../rifle-scopes/review.py draws the rifle scopes' (an approximation, not the game's
renderer).

usage: python3 -I review.py --pack <make_pack.py's folder> --live <folder> --donor <folder> \\
           --guns <folder> [--guns ...] --out <png> [--only <gun,gun>] [--per-gun <n>]
"""
import argparse
import importlib.util
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import render_icon as ri  # noqa: E402
import scopes_lib as lib  # noqa: E402

spec = importlib.util.spec_from_file_location('rifle_scopes_review', HERE.parent / 'rifle-scopes' / 'review.py')
rv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rv)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--pack', required=True, type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--guns', required=True, type=Path, action='append')
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--only', default='')
    parser.add_argument('--per-gun', type=int, default=2)
    args = parser.parse_args()
    files = args.pack / 'files'
    roots = [files] + args.guns + [args.donor, args.live]
    only = set(filter(None, args.only.split(',')))
    by_gun = {}
    for scope, gun, dict_id, change, t in rows(HERE / 'variants.tsv'):
        by_gun.setdefault(gun, []).append((scope, change))
    out_rows = []
    for gun, scopes in by_gun.items():
        if only and gun not in only:
            continue
        cfg = next(Path(r) / f'gameplay/items/weapons/wpn_{gun}.options' for r in roots
                   if (Path(r) / f'gameplay/items/weapons/wpn_{gun}.options').exists())
        w = lib.leaves(cfg)
        zoom = float(w.get('/parameters/aim_zoom_factor', 1.0) or 1.0)
        kinds = {}
        for scope, change in scopes:
            d = lib.leaves(files / f'gameplay/items/scopes/fitn_{scope.removeprefix("shop_scope_")}_{gun}.options')
            kind = 'C' if d.get('/data/hide_weapon_on_aim') is False else 'S'
            kinds.setdefault(kind, []).append((scope, change, d))
        chosen = (kinds.get('C', [])[:max(1, args.per_gun // 2)] + kinds.get('S', [])[:max(1, args.per_gun // 2)])[:args.per_gun]
        if not kinds.get('S'):
            chosen = kinds.get('C', [])[:args.per_gun]
        for scope, change, d in chosen:
            model = change if change != '-' else w['/object/model']
            wdir = next(m for m in (lib.model_dir(r, model) for r in roots) if m)
            props = lib.leaves(wdir / 'render/export_properties')
            sp = np.array(lib.locator(props, 'scope_point')[1])
            eye = np.array(lib.locator(props, 'aim')[1])
            idle = lib.model_dir(files, d['/data/idle_model'])
            aimed = lib.model_dir(files, d['/data/aimed_model'])
            # a skinned model's vertices are in model space, its locators in its bones' (a half turn about
            # Y): drawn in the locators' frame, as the optic at scope_point is
            grey = ri.Material(albedo=(0.3, 0.3, 0.3), gloss=0.2, f0=0.05)
            gun_parts = ri.parts(wdir, [], flip=True, materials=lambda name: grey)
            frame = (rv.VIEW[0] * 0.04, rv.VIEW[1] * 0.04, rv.VIEW[0] * 0.96, rv.VIEW[1] * 0.96)
            outside = ri.render(gun_parts + rv.coloured(idle, roots, sp, True), rv.VIEW, -90 + 20, 12, frame=frame,
                                background=(70, 80, 95))
            side = ri.render(gun_parts + rv.coloured(idle, roots, sp, True), rv.VIEW, -90, 0, frame=frame,
                             background=(70, 80, 95))
            optic = rv.coloured(aimed, roots, sp, True, skip=('scope_lense',))
            kind = 'C' if d.get('/data/hide_weapon_on_aim') is False else 'S'
            items = optic + (gun_parts if kind == 'C' else [])
            fov = rv.BASE_FOV * d['/data/fov_factor'] / zoom
            near = rv.NEAR_COLLIMATOR if kind == 'C' else 0.01
            view = ri.perspective(items, rv.AIM, eye, fov, near=near, background=(150, 170, 190))
            if kind == 'C' and lib.DOT in lib.surfaces(aimed):
                c = lib.bbox_centre(aimed, lib.DOT) * np.array([-1.0, 1.0, -1.0]) + sp - eye
                f = (rv.AIM[1] / 2) / np.tan(np.radians(fov) / 2)
                x, y = rv.AIM[0] / 2 + f * c[0] / -c[2], rv.AIM[1] / 2 - f * c[1] / -c[2]
                ImageDraw.Draw(view).ellipse((x - 4, y - 4, x + 4, y + 4), outline=(255, 30, 30), width=2)
            out_rows.append((f'{scope} on {gun} (x{1 / d["/data/fov_factor"]:.2f})', outside, side, view))
            print(out_rows[-1][0])
    w = 2 * rv.VIEW[0] + rv.AIM[0] + 40
    h = len(out_rows) * (rv.VIEW[1] + 28) + 10
    sheet = Image.new('RGB', (w, h), (30, 32, 36))
    draw = ImageDraw.Draw(sheet)
    y = 10
    for title, outside, side, view in out_rows:
        draw.text((10, y), title, fill=(230, 230, 230))
        sheet.paste(outside.convert('RGB'), (10, y + 16))
        sheet.paste(side.convert('RGB'), (20 + rv.VIEW[0], y + 16))
        sheet.paste(view.convert('RGB'), (30 + 2 * rv.VIEW[0], y + 16))
        y += rv.VIEW[1] + 28
    sheet.save(args.out)
    print(args.out)


def rows(path):
    return [line.split('\t') for line in Path(path).read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#')]


if __name__ == '__main__':
    main()
