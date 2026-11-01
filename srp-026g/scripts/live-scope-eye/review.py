#!/usr/bin/env python3
"""review.py: each scope variant of make_pack.py's (variants.tsv), aimed, before and after: its aimed
model seen from the weapon model's aim locator with the locator's turn, the screen's centre marked
(where the bullets go), the reticle's offset from it in pixels at 1280x720 (BASE_FOV, from the game:
the AK-74M's TA01 10 px above at x4 for a 0.004 rad turn). An approximation, not the game's renderer.
Draws with the rifle scopes' render_icon.py (scripts/rifle-scopes, PR #516).

usage: python3 -I review.py --live <folder> --pack <make_pack.py's folder> --rifle-scopes <scripts/rifle-scopes> --out <png>
"""
import argparse
import importlib.util
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
BASE_FOV = 60.0
SIZE = (320, 180)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--pack', required=True, type=Path)
    parser.add_argument('--rifle-scopes', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    sys.path.insert(0, str(args.rifle_scopes))
    import render_icon as ri  # noqa: E402
    spec = importlib.util.spec_from_file_location('mp', HERE / 'make_pack.py')
    mp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mp)
    lib = mp.lib
    roots = [args.pack / 'files', args.live]
    rows = [l.split('\t') for l in (HERE / 'variants.tsv').read_text().splitlines() if l and not l.startswith('#')]
    out = []
    for name, weapon, model, new in rows:
        d = lib.leaves(args.live / f'gameplay/items/scopes/{name}.options')
        fov = BASE_FOV * d['/data/fov_factor']
        props = lib.leaves(lib.model_dir(args.live, model) / 'render/export_properties')
        _, sp, sp_rot = lib.locator(props, 'scope_point')
        _, aim, rot = lib.locator(props, 'aim')
        sp, aim, rot = np.array(sp), np.array(aim), np.array(rot or [0.0, 0.0, 0.0])
        t = (sp_rot or [0, 0, 0])[1]
        turn = np.array([[math.cos(t), 0, math.sin(t)], [0, 1, 0], [-math.sin(t), 0, math.cos(t)]])
        a, b = rot[0], -rot[1]
        view = np.array([[1, 0, 0], [0, math.cos(a), -math.sin(a)], [0, math.sin(a), math.cos(a)]]) @ \
            np.array([[math.cos(b), 0, math.sin(b)], [0, 1, 0], [-math.sin(b), 0, math.cos(b)]])
        f = (SIZE[1] / 2) / math.tan(math.radians(fov) / 2)
        views = []
        for label, aimed in (('before', d['/data/aimed_model']), ('after', new if new != '-' else None)):
            if aimed is None:
                views.append((label, None, None))
                continue
            adir = next(x for x in (lib.model_dir(r, aimed) for r in roots) if x)
            items = []
            for p, uv, ix, tex in ri.parts(adir, roots, skip=('scope_lense',)):
                q = (turn @ (p * np.array([-1.0, 1.0, -1.0])).T).T + sp - aim
                items.append(((view @ q.T).T, uv, ix, tex))
            names = [s for s in lib.surfaces(adir) if not s.startswith('scope_lense')]
            for i, s in enumerate(names):
                if 'reddot' in s:
                    items[i] = (items[i][0], items[i][1], items[i][2], np.array([[[255.0, 30.0, 30.0]]]))
                elif 'aimgrid' in s or 'background' in s:
                    items[i] = (items[i][0], items[i][1], items[i][2], np.zeros((1, 1, 3)))
            img = ri.perspective(items, SIZE, (0.0, 0.0, 0.0), fov, near=0.01, background=(150, 170, 190))
            c = mp.lib.bbox_centre(adir, mp.REDDOT if mp.REDDOT in lib.surfaces(adir) else mp.GRID)
            q = view @ (turn @ (c * np.array([-1.0, 1.0, -1.0])) + sp - aim)
            dx, dy = f * q[0] / -q[2], f * q[1] / -q[2]
            px = (dx * 720 / SIZE[1], dy * 720 / SIZE[1])
            dr = ImageDraw.Draw(img)
            cx, cy = SIZE[0] / 2, SIZE[1] / 2
            dr.line((cx - 12, cy, cx + 12, cy), fill=(0, 255, 0))
            dr.line((cx, cy - 12, cx, cy + 12), fill=(0, 255, 0))
            views.append((label, img, px))
        out.append((f'{name} on {weapon} (x{1 / d["/data/fov_factor"]:.2g}, aim turn {rot[0]:+.4f}, {rot[1]:+.4f})', views))
        print(out[-1][0], ' '.join(f'{l}: {p[0]:+.1f} right,{p[1]:+.1f} up px' if p else f'{l}: -' for l, _, p in views))
    W, H = 2 * SIZE[0] + 30, len(out) * (SIZE[1] + 34) + 10
    sheet = Image.new('RGB', (W, H), (30, 32, 36))
    dr = ImageDraw.Draw(sheet)
    y = 10
    for title, views in out:
        dr.text((10, y), title, fill=(230, 230, 230))
        for k, (label, img, px) in enumerate(views):
            x = 10 + k * (SIZE[0] + 10)
            if img is None:
                dr.text((x, y + 60), f'{label}: left as it is', fill=(200, 200, 120))
                continue
            sheet.paste(img, (x, y + 30))
            dr.text((x, y + 16), f'{label}: reticle {px[0]:+.1f} px right, {px[1]:+.1f} px up (1280x720)', fill=(200, 220, 200))
        y += SIZE[1] + 34
    sheet.save(args.out)
    print(args.out)


if __name__ == '__main__':
    main()
