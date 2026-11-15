#!/usr/bin/env python3
"""make_battery_icon.py: the hud movie's kill feed icon of a battery strike (exe patch 143,
../../docs/notes/battery-hit.md "Kill feed").

usage: python3 -I make_battery_icon.py --model <converted_model> --out <folder>

--model  0.26g's battery mesh, models/weapons/electrobox.skinned_model/render/electrobox_s/converted_model
         (`vostok-port extract`, or scripts/battery-hit/extract.sh's folder; sha256 checked)
--out    srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)

CombatLogBatteryIcon.png is drawn as the weapon icons are (frames of the hud's sprite 698): a white
silhouette, its details cut out of it, a black shadow at 40 % one pixel right and two down. The
silhouette is the battery's own mesh seen from the front and 38 degrees above: the box, its two
handles, and on its top the two open cell wells either side of the middle block (cut out; the hud's
battery rows draw it so, a box with two cells), the top's front edge a cut line. After it, the impact
burst of the gun-stock icon (48: flash/lobby_menu/bitmaps/KillFeedWeapon48.png, the hud's frame,
sha256 checked), as the feed draws a melee hit.

Needs Pillow and numpy.
"""
import argparse
import hashlib
import math
from pathlib import Path
import struct
import sys

import numpy as np
from PIL import Image, ImageFilter

HERE = Path(__file__).resolve().parent
MODEL_SHA256 = '5e581cc9a4c589b27fa5f3977b642c4f1630c1f1f424cf38405477d1082d2cab'
STOCK_ICON = HERE.parents[1] / 'flash' / 'lobby_menu' / 'bitmaps' / 'KillFeedWeapon48.png'
STOCK_ICON_SHA256 = '9017a7b646896927439d620184616b9397d969a7bfc1d7a3bf035f780adb1ac3'

PITCH = -38                      # degrees: looking down on the top
HEIGHT = 20                      # the battery's rows
SS = 8                           # drawn at 8x, then reduced
GROW = 9                         # the silhouette thickened 1/2 px a side (the handles' tubes are thin)
SEAM = 7                         # the top's front edge: a cut 7/8 px wide
# The mesh's box (metres): inside its walls |x| < 0.257 and -0.137 < z < 0.145; its rim 0.235
# high, the cells below 0.226; the middle block's top between 0.205 and 0.216.
INSIDE_X, INSIDE_Z, TOP_Y, BLOCK_Y = 0.257, (-0.137, 0.145), 0.226, (0.205, 0.216)


def mesh(data):
    """Positions and triangles of a converted_model (a skinned mesh: vertex block 3, index block 4)."""
    kind, size, count = struct.unpack_from('<III', data)
    stride = (size - 4) // count
    assert kind == 3 and stride in (36, 40, 44, 48)
    vertices = np.array([struct.unpack_from('<3f', data, 12 + i * stride) for i in range(count)])
    kind, _, icount = struct.unpack_from('<III', data, 8 + size)
    assert kind == 4
    return vertices, np.array(struct.unpack_from(f'<{icount}H', data, 20 + size)).reshape(-1, 3)


def raster(vertices, triangles, rows):
    """Orthographic, from the front and PITCH above, `rows` high: per pixel the nearest triangle
    (-1: none) and the point's position on the model."""
    p = math.radians(PITCH)
    view = np.array([[1, 0, 0], [0, math.cos(p), -math.sin(p)], [0, math.sin(p), math.cos(p)]])
    q = vertices @ view.T
    lo, hi = q[:, :2].min(0), q[:, :2].max(0)
    scale = (rows - 1) / (hi[1] - lo[1])
    x, y, z = (q[:, 0] - lo[0]) * scale, (hi[1] - q[:, 1]) * scale, q[:, 2]
    height, width = rows, int(math.ceil((hi[0] - lo[0]) * scale)) + 1
    depth = np.full((height, width), np.inf)
    nearest = np.full((height, width), -1)
    for t, corners in enumerate(triangles):
        xs, ys, zs = x[corners], y[corners], z[corners]
        det = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
        if abs(det) < 1e-12:
            continue
        x0, x1 = max(int(xs.min()), 0), min(int(math.ceil(xs.max())), width - 1)
        y0, y1 = max(int(ys.min()), 0), min(int(math.ceil(ys.max())), height - 1)
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + .5, np.arange(y0, y1 + 1) + .5)
        l0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / det
        l1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / det
        l2 = 1 - l0 - l1
        d = l0 * zs[0] + l1 * zs[1] + l2 * zs[2]
        near = (l0 >= 0) & (l1 >= 0) & (l2 >= 0) & (d < depth[y0:y1 + 1, x0:x1 + 1])
        depth[y0:y1 + 1, x0:x1 + 1][near] = d[near]
        nearest[y0:y1 + 1, x0:x1 + 1][near] = t
    gx, gy = np.meshgrid(np.arange(width) + .5, np.arange(height) + .5)
    seen = np.stack([gx / scale + lo[0], hi[1] - gy / scale, np.where(nearest >= 0, depth, 0)], -1)
    return nearest, seen @ view


def dilated(mask, size):
    return np.asarray(Image.fromarray(mask.astype(np.uint8) * 255).filter(ImageFilter.MaxFilter(size))) > 0


def battery(vertices, triangles):
    """The battery's coverage, HEIGHT rows: the silhouette less the cell wells and the top's edge."""
    nearest, at = raster(vertices, triangles, HEIGHT * SS)
    a, b, c = (vertices[triangles[:, i]] for i in range(3))
    normals = np.cross(b - a, c - a)
    normals /= np.maximum(np.linalg.norm(normals, axis=1, keepdims=True), 1e-12)
    normal = np.abs(normals[np.maximum(nearest, 0)])
    covered = nearest >= 0
    x, y, z = at[..., 0], at[..., 1], at[..., 2]
    block = (normal[..., 1] > 0.9) & (y > BLOCK_Y[0]) & (y < BLOCK_Y[1])
    wells = covered & (np.abs(x) < INSIDE_X) & (z > INSIDE_Z[0]) & (z < INSIDE_Z[1]) & (y < TOP_Y) & ~block
    top = covered & (normal[..., 1] > 0.7) & (y > TOP_Y)
    front = covered & (normal[..., 2] > 0.7) & (z < INSIDE_Z[0])
    edge = (top & np.roll(front, -1, 0)) | (front & np.roll(top, -1, 0))
    return reduce(dilated(covered, GROW) & ~wells & ~dilated(edge, SEAM))


def reduce(white):
    """The 8x drawing in pixels, placed on the pixel grid where its edges come out sharpest."""
    best = None
    for oy in range(SS):
        for ox in range(SS):
            h, w = white.shape[0] + oy, white.shape[1] + ox
            canvas = np.zeros(((h + SS - 1) // SS * SS, (w + SS - 1) // SS * SS))
            canvas[oy:h, ox:w] = white
            cover = canvas.reshape(canvas.shape[0] // SS, SS, canvas.shape[1] // SS, SS).mean((1, 3))
            blur = (cover * (1 - cover)).sum()
            if best is None or blur < best[0] - 1e-9:
                best = (blur, cover)
    cover = np.clip((best[1] - 0.5) * 1.5 + 0.5, 0, 1)
    rows, cols = np.nonzero(cover)
    return cover[rows.min():rows.max() + 1, cols.min():cols.max() + 1]


def burst(stock):
    """The gun-stock icon's impact burst: its white (no shadow), right of the stock."""
    pixels = np.asarray(stock.convert('RGBA')).astype(float) / 255
    white = pixels[..., 3] * pixels[..., 0]
    keep = np.zeros(white.shape, bool)
    keep[:, 15:] = True
    keep[12:, 13:] = True
    white = np.where(keep, white, 0)
    rows, cols = np.nonzero(white > 0.02)
    return white[rows.min():rows.max() + 1, cols.min():cols.max() + 1]


def beside(left, right, overlap=3):
    """`right` after `left`, overlapping it by `overlap` px, both centred on one row."""
    height, width = max(left.shape[0], right.shape[0]), left.shape[1] + right.shape[1] - overlap
    out = np.zeros((height, width))
    top = (height - left.shape[0]) // 2
    out[top:top + left.shape[0], :left.shape[1]] = left
    top, x = (height - right.shape[0]) // 2, left.shape[1] - overlap
    out[top:top + right.shape[0], x:] = np.maximum(out[top:top + right.shape[0], x:], right)
    return out


def styled(white):
    """The weapon icons' look: white, over a black shadow at 40 % one pixel right and two down."""
    height, width = white.shape
    front = np.zeros((height + 2, width + 1))
    front[:height, :width] = white
    shadow = np.zeros_like(front)
    shadow[2:, 1:] = white * 0.4
    alpha = front + shadow * (1 - front)
    grey = np.divide(front, alpha, out=np.zeros_like(alpha), where=alpha > 0)
    rgba = np.stack([grey, grey, grey, alpha], -1)
    return Image.fromarray(np.round(rgba * 255).astype(np.uint8), 'RGBA')


def checked(path, digest, what):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        sys.exit(f'{path}: not {what} (sha256 {hashlib.sha256(data).hexdigest()})')
    return data


def battery_icon(model, stock):
    return styled(beside(battery(*mesh(model)), burst(stock)))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    model = checked(args.model, MODEL_SHA256, "0.26g's battery mesh (electrobox_s converted_model)")
    checked(STOCK_ICON, STOCK_ICON_SHA256, "the hud's gun-stock icon")
    args.out.mkdir(parents=True, exist_ok=True)
    battery_icon(model, Image.open(STOCK_ICON)).save(args.out / 'CombatLogBatteryIcon.png', optimize=True)


if __name__ == '__main__':
    main()
