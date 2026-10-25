#!/usr/bin/env python3
"""Review sheets for the bands in bands.tsv, worst first, in batches; renders on the CPU.

usage: review.py bands.tsv [--batch N] [--size 20] [--only BASE_MODEL ...] [--jobs N]

Per torso model: rows rest, rifle aim, pistol, shotgun, run, crouch, death; columns blue
front / left / back, red front / right / back, and a per-texel debug (band green by coverage,
eligible cloth outside the band grey-green, rejected texels magenta, arm pieces left out blue);
plus a 4x nearest-neighbour crop of a band edge (and of the band meeting a left-out piece) from
a high-resolution render. Per batch: an index sheet with each item's status.
Output: $BANDS_WORK/review/.
"""
import argparse
import os
import sys
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from PIL import Image, ImageDraw
import bandsenv as E
import bandlib as BL
import model
import paint as PT
import render as RD
import texband as TB
from measure import read_table
from pose import posed_dir

POSES = ['rest', 'rifle_aim', 'pistol', 'shotgun', 'run', 'crouch', 'death']
COLS = [('blue', 'front'), ('blue', 'side'), ('blue', 'back'), ('red', 'front'), ('red', 'right'), ('red', 'back'),
        ('debug', 'side')]
CELL = 200
GREEN, GREYGREEN, MAGENTA, BLUE = (40, 200, 60), (105, 140, 105), (235, 30, 225), (60, 110, 255)
RANK = {'problem': 0, 'adjusted': 1, 'draft': 2, 'good': 3}


def textures(models, window, base):
    """Painted diffuses (blue, red) of the material, a per-texel debug texture, and the left-out
    pieces' face colours for `base`."""
    rs, g, info = TB.material_band(models, window)
    r = next(x for x in rs if x['base'] == base)
    tex = PT.painted(r['diffuse'], g)
    dif = BL.texture(r['diffuse'])
    dbg = dif[..., :3] * 0.45
    dbg[r['elig_tex']] = GREYGREEN
    m = g > 0
    dbg[m] = dbg[m] * (1 - g[m, None]) + np.array(GREEN, float) * g[m, None]
    dbg[r['hole_tex']] = MAGENTA
    tex['debug'] = np.concatenate([dbg, dif[..., 3:4]], 2)
    frgb = np.full((len(r['eligible']), 3), np.nan)
    frgb[r['excluded']] = BLUE
    return rs, tex, frgb


def shot(bases, diffuse, rgba, pose, view, size=CELL, ss=2, face_rgb=None, frame=None, label=''):
    RD.override(diffuse, rgba)
    try:
        im, fr = RD.render(bases, view, 'neutral', size=(size, size), ss=ss,
                           geometry=posed_dir(pose) if pose != 'rest' else None, frame=frame, face_rgb=face_rgb)
    finally:
        RD.override(diffuse, None)
    if label:
        ImageDraw.Draw(im).text((3, 3), label, fill=(255, 255, 255))
    return im


def grid(rows, title, path, cell=CELL):
    W = max(len(r) for r in rows) * cell
    img = Image.new('RGB', (W, len(rows) * cell + 18), (255, 255, 255))
    for i, row in enumerate(rows):
        for j, im in enumerate(row):
            img.paste(im, (j * cell, 18 + i * cell))
    ImageDraw.Draw(img).text((4, 3), title, fill=(0, 0, 0))
    img.save(path)
    return path


def closeups(r, tex, frgb, out, label, pose='rest', view='front'):
    """4x nearest crops: a straight band edge facing the camera, and the band meeting a piece
    left out (if any)."""
    s = dict((n, x) for n, x, _, _ in model.model(r['base'], posed_dir(pose) if pose != 'rest' else None))[r['surface']]
    pos, tri = s['pos'], s['tri']
    right, up, fwd, _ = RD.VIEWS[view]
    paths = []
    for k in [0, 1]:
        sel = r['eligible'] & (r['side_of'] == k)
        if not sel.any():
            continue
        P = pos[np.unique(tri[sel])]
        X, Y = P @ right, P @ up
        cx, cy = (X.min() + X.max()) / 2, (Y.min() + Y.max()) / 2
        half = max(X.max() - X.min(), Y.max() - Y.min()) / 2 * 1.3
        frame = (cx - half, cx + half, cy - half, cy + half)
        size = 800
        a, d, L, _, _ = BL.arm_frame(r['bp'], TB.SIDES[k])
        pts = []
        best = None
        for fi in np.nonzero(sel)[0]:
            t = tri[fi]
            h = (r['s']['pos'][t] - a) @ d / L - r['window'][1]
            n = np.cross(pos[t[1]] - pos[t[0]], pos[t[2]] - pos[t[0]])
            facing = (n / (np.linalg.norm(n) + 1e-12)) @ fwd
            if h.max() >= 0 >= h.min() and facing > 0.5 and (best is None or pos[t].mean(0) @ fwd > best[0]):
                best = (pos[t].mean(0) @ fwd, pos[t].mean(0))
        if best is not None:
            pts.append(('edge', best[1]))
        ex = np.nonzero(~np.isnan(frgb[:, 0]))[0]
        bandc = pos[tri[sel]].mean(1)
        near = []
        for fi in ex:
            c = pos[tri[fi]].mean(0)
            n = np.cross(pos[tri[fi][1]] - pos[tri[fi][0]], pos[tri[fi][2]] - pos[tri[fi][0]])
            if (n / (np.linalg.norm(n) + 1e-12)) @ fwd > 0.3:
                dist = np.min(np.linalg.norm(bandc - c, axis=1))
                if dist < 0.03:
                    near.append((dist, c))
        if near:
            pts.append(('piece', min(near, key=lambda x: x[0])[1]))
        if not pts:
            continue
        painted = shot(r['base'], r['diffuse'], tex['red'], pose, view, size=size, frame=frame)
        debug = shot(r['base'], r['diffuse'], tex['debug'], pose, view, size=size, frame=frame,
                     face_rgb={(r['base'], r['surface']): frgb})
        for name, p in pts:
            x = int((p @ right - frame[0]) / (frame[1] - frame[0]) * size)
            y = int((frame[3] - p @ up) / (frame[3] - frame[2]) * size)
            box = (max(x - 50, 0), max(y - 50, 0), min(x + 50, size), min(y + 50, size))
            wh = ((box[2] - box[0]) * 4, (box[3] - box[1]) * 4)
            img = Image.new('RGB', (wh[0] * 2 + 8, wh[1] + 18), (255, 255, 255))
            img.paste(painted.crop(box).resize(wh, Image.NEAREST), (0, 18))
            img.paste(debug.crop(box).resize(wh, Image.NEAREST), (wh[0] + 8, 18))
            ImageDraw.Draw(img).text((4, 3), '%s %s arm, %s %s, band %s: 4x nearest of an %d px render (painted | debug)' % (
                label, TB.SIDES[k], pose, view, 'edge' if name == 'edge' else 'meeting a left-out piece', size), fill=(0, 0, 0))
            path = '%s/%s_crop_%s_%s.png' % (out, label, TB.SIDES[k].lower(), name)
            img.save(path)
            paths.append(path)
        break
    return paths


def item_task(args):
    row, models, out = args
    base = row['base_model']
    label = base.replace('character/human/', '').replace('/', '_')
    window = (float(row['window_from']), float(row['window_to']))
    rs, tex, frgb = textures(models, window, base)
    r = next(x for x in rs if x['base'] == base)
    rows = []
    for pose in POSES:
        rows.append([shot(base, r['diffuse'], tex[c], pose, v, label='%s %s %s' % (pose, c, v),
                          face_rgb={(base, r['surface']): frgb} if c == 'debug' else None) for c, v in COLS])
    title = '%s  window %.2f-%.2f  status %s  %s' % (base, window[0], window[1], row['status'], row.get('notes', ''))
    sheet = grid(rows, title[:220], '%s/%s.png' % (out, label))
    crops = closeups(r, tex, frgb, out, label)
    thumb = [shot(base, r['diffuse'], tex['red'], 'rest', 'front', size=180),
             shot(base, r['diffuse'], tex['blue'], 'rifle_aim', 'side', size=180),
             shot(base, r['diffuse'], tex['red'], 'run', 'right', size=180)]
    BL._PREP.clear()
    return base, sheet, crops, thumb


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table')
    ap.add_argument('--batch', type=int, default=1)
    ap.add_argument('--size', type=int, default=20)
    ap.add_argument('--only', action='append', default=[])
    ap.add_argument('--jobs', type=int, default=8)
    a = ap.parse_args()
    _, _, rows = read_table(a.table)
    by_mat = {}
    for r in rows:
        by_mat.setdefault(r['material'], []).append((r['base_model'], r['surface']))
    order = sorted(rows, key=lambda r: (RANK.get(r['status'], 2), float(r['visible_cloth_pct'] or 0), r['base_model']))
    if a.only:
        batch = [r for r in order if r['base_model'] in a.only]
        name = 'only'
    else:
        batch = order[(a.batch - 1) * a.size:a.batch * a.size]
        name = 'batch%d' % a.batch
    out = E.work('review', name, 'x')[:-2]
    with Pool(a.jobs) as p:
        res = p.map(item_task, [(r, by_mat[r['material']], out) for r in batch])
    # Index: each item's three thumbnails and its status.
    cols = 2
    tw = 180 * 3 + 10
    th = 180 + 36
    idx = Image.new('RGB', (cols * tw, ((len(res) + cols - 1) // cols) * th + 18), (255, 255, 255))
    status = {r['base_model']: r for r in batch}
    for i, (base, sheet, crops, thumb) in enumerate(res):
        x, y = (i % cols) * tw, 18 + (i // cols) * th
        for j, im in enumerate(thumb):
            idx.paste(im, (x + j * 180, y + 32))
        r = status[base]
        ImageDraw.Draw(idx).text((x + 2, y + 2), '%d. %s  %s-%s  %s' % (i + 1 + (a.batch - 1) * a.size, base.split('/')[-1],
                                                                       r['window_from'], r['window_to'], r['status']), fill=(0, 0, 0))
        ImageDraw.Draw(idx).text((x + 2, y + 16), (r.get('notes') or '')[:90], fill=(120, 0, 0))
    ImageDraw.Draw(idx).text((4, 3), '%s: rest front red | rifle aim left blue | run right red' % name, fill=(0, 0, 0))
    idx.save(os.path.join(out, 'index.png'))
    print(os.path.join(out, 'index.png'))
    for base, sheet, crops, _ in res:
        print(status[base]['status'], base, sheet, *crops)


if __name__ == '__main__':
    main()
