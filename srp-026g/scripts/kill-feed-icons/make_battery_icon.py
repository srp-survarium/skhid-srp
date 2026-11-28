#!/usr/bin/env python3
"""make_battery_icon.py: the hud movie's kill feed icon of a battery strike (exe patch 143,
../../docs/notes/battery-hit.md "Kill feed").

usage: python3 -I make_battery_icon.py --model <converted_model> [--out <folder>] [--svg <file>]

--model  0.26g's battery mesh, models/weapons/electrobox.skinned_model/render/electrobox_s/converted_model
         (`vostok-port extract`, or scripts/battery-hit/extract.sh's folder; sha256 checked)
--out    srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)
--svg    the same battery without the burst as an SVG, the website map's battery marker (the Maps
         tab's build, scripts/maps-tab/build.py, calls `marker` itself)

CombatLogBatteryIcon.png is drawn as the weapon icons are (frames of the hud's sprite 698): a white
silhouette, its details cut out of it, a black shadow at 40 % one pixel right and two down. The
silhouette is the battery's own mesh in three-quarter view (turned 135 degrees, 30 above, 22 px
high): the box, its faces (top, long side, end) parted by cut lines, the handles ringed by one, and
in its top's rim the openings of the two cell wells either side of the middle block (cut out; the
hud's battery rows draw it so, a box with two cells). After it, the impact burst of the gun-stock
icon (48: flash/lobby_menu/bitmaps/KillFeedWeapon48.png, the hud's frame, sha256 checked), as the
feed draws a melee hit.

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

YAW, PITCH = 135, -30            # degrees: a long side and a handle's end, the top seen from above
HEIGHT = 22                      # the battery's rows
SS = 8                           # the bitmap drawn at 8x, then reduced
SVG_SS = 16                      # the marker traced at 16x
SVG_TOLERANCE = 1.0              # its outlines kept within 1/16 px
# At `ss` times the size: the silhouette thickened (ss + 1) / 2ss px a side (the handles' tubes are
# thin), the cut between two faces (ss - 1) / ss px wide.
# The mesh's box (metres): its rim's top 0.235 high, open in x -0.226 to 0.228, z -0.115 to 0.122,
# the middle block in x -0.046 to 0.085 between the two cell wells; its long sides past z -0.137
# and 0.145; the handles past |x| 0.27.
RIM_Y, OPEN_X, OPEN_Z, BLOCK_X = 0.235, (-0.226, 0.228), (-0.115, 0.122), (-0.046, 0.085)
TOP_Y, SIDE_Z, HANDLE_X = 0.226, (-0.137, 0.145), 0.27


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
    """Orthographic, turned YAW and PITCH, `rows` high: per pixel the nearest triangle (-1: none)
    and the point's position on the model; and the direction the view looks along."""
    y, p = math.radians(YAW), math.radians(PITCH)
    view = (np.array([[1, 0, 0], [0, math.cos(p), -math.sin(p)], [0, math.sin(p), math.cos(p)]])
            @ np.array([[math.cos(y), 0, math.sin(y)], [0, 1, 0], [-math.sin(y), 0, math.cos(y)]]))
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
    return nearest, seen @ view, view[2]


def dilated(mask, size):
    return np.asarray(Image.fromarray(mask.astype(np.uint8) * 255).filter(ImageFilter.MaxFilter(size))) > 0


def drawing(vertices, triangles, ss):
    """The battery at `ss` times HEIGHT rows: its silhouette and its white (the silhouette, each face
    of the box (top, long side, end) parted from the next by a cut, the cell wells' openings cut out,
    the handles ringed by a cut)."""
    grow, seam = ss + 1, ss - 1
    nearest, at, look = raster(vertices, triangles, HEIGHT * ss)
    covered = nearest >= 0
    handle = covered & (np.abs(at[..., 0]) > HANDLE_X)
    # the wells: where the line of sight crosses the rim's plane inside an opening
    rim = at + ((RIM_Y - at[..., 1]) / look[1])[..., None] * look
    x, z = rim[..., 0], rim[..., 2]
    wells = (covered & ~handle & (x > OPEN_X[0]) & (x < OPEN_X[1]) & (z > OPEN_Z[0]) & (z < OPEN_Z[1])
             & ((x < BLOCK_X[0]) | (x > BLOCK_X[1])))
    # each point of the box by the face it lies on, by its place (a knob or a plate is its face's)
    y, z = at[..., 1], at[..., 2]
    face = np.select([~covered | handle | wells, y > TOP_Y, z < SIDE_Z[0], z > SIDE_Z[1]], [0, 1, 2, 3], 4)
    edge = np.zeros_like(covered)
    for axis in (0, 1):
        other = np.roll(face, -1, axis)
        edge |= (face != other) & (face > 0) & (other > 0)
    handle = dilated(handle, grow)
    ring = dilated(handle, 2 * seam - 1) & ~handle
    shape = dilated(covered, grow)
    return shape | handle, (shape & ~wells & ~dilated(edge, seam) & ~ring) | handle


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


def loops(mask):
    """The mask's outlines: closed loops of pixel corners (x, y), each a run of the pixels' edges
    with the mask on one side; where two regions touch at a corner, they stay apart."""
    m = np.pad(mask, 1)
    ys, xs = np.nonzero(mask)
    out = {}
    # clockwise round each pixel inside (y down), an edge where its neighbour is out
    for (dy, dx), (a, b) in (((-1, 0), ((0, 0), (1, 0))), ((0, 1), ((1, 0), (1, 1))),
                             ((1, 0), ((1, 1), (0, 1))), ((0, -1), ((0, 1), (0, 0)))):
        open_ = ~m[ys + 1 + dy, xs + 1 + dx]
        for x, y in zip(xs[open_], ys[open_]):
            out.setdefault((x + a[0], y + a[1]), []).append((x + b[0], y + b[1]))
    result = []
    while out:
        start = at = next(iter(out))
        loop, heading = [], None
        while True:
            ends = out[at]
            if heading is not None and len(ends) > 1:
                # turn right first: regions meeting at a corner stay apart
                right = (-heading[1], heading[0])
                ends.sort(key=lambda e: (e[0] - at[0], e[1] - at[1]) != right)
            end = ends.pop(0)
            if not ends:
                del out[at]
            loop.append(at)
            heading, at = (end[0] - at[0], end[1] - at[1]), end
            if at == start:
                break
        result.append(loop)
    return result


def simplified(points, tolerance):
    """A closed loop with its points within `tolerance` of the loop kept (Douglas-Peucker)."""
    pts = np.array(points, float)
    far = int(np.argmax(((pts - pts[0]) ** 2).sum(1)))
    keep = {0, far}
    stack = [(0, far), (far, len(pts))]
    while stack:
        i, j = stack.pop()
        a, b = pts[i], pts[j % len(pts)]
        if j - i < 2:
            continue
        seg = pts[i + 1:j]
        d = b - a
        length = math.hypot(*d)
        dist = (np.abs(d[0] * (seg[:, 1] - a[1]) - d[1] * (seg[:, 0] - a[0])) / length if length
                else np.hypot(*(seg - a).T))
        k = int(np.argmax(dist))
        if dist[k] > tolerance:
            keep.add(i + 1 + k)
            stack += [(i, i + 1 + k), (i + 1 + k, j)]
    return pts[sorted(keep)]


def path(mask, ss):
    """The mask as SVG path data, in pixels of HEIGHT rows."""
    d = []
    for loop in loops(mask):
        pts = simplified(loop, SVG_TOLERANCE) / ss
        d.append('M' + ' '.join(f'{x:.2f} {y:.2f}'.replace('.00', '') for x, y in pts) + 'Z')
    return ''.join(d)


def battery(vertices, triangles):
    """The bitmap's battery: its white, HEIGHT rows."""
    return reduce(drawing(vertices, triangles, SS)[1])


def marker(model):
    """The battery as an SVG (the website's map marker): the bitmap's drawing, no burst, as vector
    outlines traced at SVG_SS times: the white over its silhouette in the black of the page's
    outlines, so the cuts read dark as the feed's do."""
    silhouette, white = drawing(*mesh(model), SVG_SS)
    rows, cols = np.nonzero(silhouette)
    silhouette = silhouette[rows.min():rows.max() + 1, cols.min():cols.max() + 1]
    white = white[rows.min():rows.max() + 1, cols.min():cols.max() + 1]
    height, width = (n / SVG_SS for n in silhouette.shape)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:g}" height="{height:g}" '
            f'viewBox="0 0 {width:g} {height:g}">\n'
            '<!-- Built by srp-026g/scripts/kill-feed-icons/make_battery_icon.py from the game\'s battery mesh. -->\n'
            f'<path fill="#000" fill-opacity="0.85" fill-rule="evenodd" d="{path(silhouette, SVG_SS)}"/>\n'
            f'<path fill="#fff" fill-rule="evenodd" d="{path(white, SVG_SS)}"/>\n</svg>\n')


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


def beside(left, right):
    """`right` after `left`, both centred on one row."""
    height, width = max(left.shape[0], right.shape[0]), left.shape[1] + right.shape[1]
    out = np.zeros((height, width))
    top = (height - left.shape[0]) // 2
    out[top:top + left.shape[0], :left.shape[1]] = left
    top, x = (height - right.shape[0]) // 2, left.shape[1]
    out[top:top + right.shape[0], x:] = right
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


def battery_model(path):
    return checked(path, MODEL_SHA256, "0.26g's battery mesh (electrobox_s converted_model)")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--svg', type=Path)
    args = parser.parse_args()
    if not (args.out or args.svg):
        parser.error('--out, --svg or both')
    model = battery_model(args.model)
    if args.out:
        checked(STOCK_ICON, STOCK_ICON_SHA256, "the hud's gun-stock icon")
        args.out.mkdir(parents=True, exist_ok=True)
        battery_icon(model, Image.open(STOCK_ICON)).save(args.out / 'CombatLogBatteryIcon.png', optimize=True)
    if args.svg:
        args.svg.write_text(marker(model))


if __name__ == '__main__':
    main()
