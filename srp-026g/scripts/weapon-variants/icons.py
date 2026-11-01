"""The skins' lobby icons and the preview pictures.

A skin's icon is its gun's live icon (the players' lobby movie, the cell the gun's config names)
with the skin's paint: the gun's model is posed to the icon (an orthographic view, its silhouette
fitted to the icon's), each icon pixel finds the texels it shows (the texture averaged down to the
pixel's footprint), and where the skin repaints them the pixel takes the skin's colour, at the
exposure of the live golden weapons' icons (GAIN) and the icon's light across the gun. Every other
pixel is the live icon's. So the skin's icon has its
gun's framing, light and outline, as the live golden weapons' icons have their gun's.
"""
import io

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

import swf
from common import MOVIE

CELL = (171, 83)
ROW_PITCH = 91
# The live golden weapons' icons are about 1.2 to 1.6 times their texture's colour (the AKM's and the
# SKS's premium copies against akmn_camo_st and sks_camo_st), whatever their gun's icon is: the
# repainted pixels take the skin's colour times this, times the icon's light across the gun.
GAIN = 1.4
SHADE = (0.6, 1.5)


def cell(atlas, row, col):
    x, y = (col - 1) * CELL[0], (row - 1) * ROW_PITCH
    return atlas[y:y + CELL[1], x:x + CELL[0]]


def rotation(yaw, pitch, roll):
    """Model space to view space: x right, y up, z toward the viewer."""
    y, p, r = np.radians([yaw, pitch, roll])
    ry = np.array([[np.cos(y), 0, np.sin(y)], [0, 1, 0], [-np.sin(y), 0, np.cos(y)]])
    rx = np.array([[1, 0, 0], [0, np.cos(p), -np.sin(p)], [0, np.sin(p), np.cos(p)]])
    rz = np.array([[np.cos(r), -np.sin(r), 0], [np.sin(r), np.cos(r), 0], [0, 0, 1]])
    return rz @ rx @ ry


def project(points, pose, box, frame=None):
    """Screen x, y and depth of `points` under `pose` (yaw, pitch, roll in degrees; dx, dy in pixels;
    a scale factor): the model's width fills the icon's (`box`, x0 y0 x1 y1) times the factor,
    centred on it and moved by dx, dy (a scope the icon draws and the model lacks makes the icon
    taller, never wider)."""
    yaw, pitch, roll, dx, dy, scale = pose
    turn = rotation(yaw, pitch, roll).T
    v = points @ turn
    framed = v if frame is None else frame @ turn
    lo, hi = framed[:, :2].min(0), framed[:, :2].max(0)
    s = (box[2] - box[0]) / max(hi[0] - lo[0], 1e-9) * scale
    cx, cy = (box[0] + box[2]) / 2 + dx, (box[1] + box[3]) / 2 + dy
    mx, my = (lo + hi) / 2
    return np.stack([cx + (v[:, 0] - mx) * s, cy - (v[:, 1] - my) * s, v[:, 2]], 1)


def silhouette(points, tris, pose, box, size):
    q = project(points, pose, box)
    image = Image.new('L', size, 0)
    draw = ImageDraw.Draw(image)
    for t in tris:
        draw.polygon([(q[i, 0], q[i, 1]) for i in t], fill=255)
    return np.asarray(image) > 0


# The pose search: yaw, pitch, roll (degrees), dx, dy (pixels), scale; each step's sizes, coarse to fine.
STEPS = [(8.0, 8.0, 8.0, 4.0, 4.0, 0.04), (4.0, 4.0, 4.0, 2.0, 2.0, 0.02), (2.0, 2.0, 2.0, 1.0, 1.0, 0.01),
         (1.0, 1.0, 1.0, 0.5, 0.5, 0.005), (0.5, 0.5, 0.5, 0.25, 0.25, 0.0025)]


def fit_pose(points, tris, alpha):
    """The orthographic pose whose silhouette best covers the icon's (intersection over union),
    searched coarse to fine from the two side views (muzzle right)."""
    size = (alpha.shape[1], alpha.shape[0])
    target = alpha > 128
    ys, xs = np.nonzero(target)
    box = (xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)
    cache = {}

    def score(pose):
        if pose not in cache:
            s = silhouette(points, tris, pose, box, size)
            cache[pose] = (s & target).sum() / max((s | target).sum(), 1)
        return cache[pose]

    best = max(((yaw, 0.0, 0.0, 0.0, 0.0, 1.0) for yaw in (-90.0, 90.0)), key=score)
    for steps in STEPS:
        improved = True
        while improved:
            improved = False
            for axis, step in enumerate(steps):
                for sign in (-1, 1):
                    pose = list(best)
                    pose[axis] = round(pose[axis] + sign * step, 4)
                    if score(tuple(pose)) > score(best):
                        best, improved = tuple(pose), True
    return best, box, score(best)


def uv_buffer(meshes, pose, box, size, frame):
    """Per pixel: which surface (index, -1 none) and the UV it shows, depth tested; the meshes framed
    as the points `frame` are (the gun's, for a skin of another shape)."""
    w, h = size
    which = np.full((h, w), -1, np.int32)
    uvs = np.zeros((h, w, 2))
    depth = np.full((h, w), -np.inf)
    allpoints = np.concatenate([m[0] for m in meshes])
    v_all = project(allpoints, pose, box, frame)
    at = 0
    for index, (points, uv, tris) in enumerate(meshes):
        q = v_all[at:at + len(points)]
        at += len(points)
        for t in tris:
            p = q[t]
            x0, y0 = np.maximum(np.floor(p[:, :2].min(0)).astype(int), 0)
            x1, y1 = np.minimum(np.ceil(p[:, :2].max(0)).astype(int), [w - 1, h - 1])
            if x0 > x1 or y0 > y1:
                continue
            den = (p[1, 1] - p[2, 1]) * (p[0, 0] - p[2, 0]) + (p[2, 0] - p[1, 0]) * (p[0, 1] - p[2, 1])
            if abs(den) < 1e-9:
                continue
            yy, xx = np.mgrid[y0:y1 + 1, x0:x1 + 1] + 0.5
            a = ((p[1, 1] - p[2, 1]) * (xx - p[2, 0]) + (p[2, 0] - p[1, 0]) * (yy - p[2, 1])) / den
            b = ((p[2, 1] - p[0, 1]) * (xx - p[2, 0]) + (p[0, 0] - p[2, 0]) * (yy - p[2, 1])) / den
            c = 1 - a - b
            z = a * p[0, 2] + b * p[1, 2] + c * p[2, 2]
            inside = (a >= 0) & (b >= 0) & (c >= 0) & (z > depth[y0:y1 + 1, x0:x1 + 1])
            if not inside.any():
                continue
            wts = np.stack([a[inside], b[inside], c[inside]], 1)
            depth[y0:y1 + 1, x0:x1 + 1][inside] = z[inside]
            which[y0:y1 + 1, x0:x1 + 1][inside] = index
            uvs[y0:y1 + 1, x0:x1 + 1][inside] = wts @ uv[t]
    return which, uvs


def footprint(which, uvs, index, size):
    """Texels of a `size` texture an icon pixel of surface `index` spans (the median over its pixels)."""
    sel = which == index
    du = np.zeros(which.shape)
    both = sel[:, 1:] & sel[:, :-1]
    d = np.abs(uvs[:, 1:] - uvs[:, :-1]).max(-1)
    du[:, 1:][both] = d[both]
    vals = du[sel & (du > 0)]
    return float(np.median(vals)) * size if len(vals) else 1.0


def filtered(texture, texels):
    """The texture averaged down so a texel is about one icon pixel (`texels` per pixel)."""
    if texels <= 1:
        return texture
    h, w = texture.shape[:2]
    image = Image.fromarray((np.clip(texture, 0, 1) * 255).astype(np.uint8))
    small = image.resize((max(1, round(w / texels)), max(1, round(h / texels))), Image.Resampling.BOX)
    return np.asarray(small, np.float32) / 255


def sample(texture, uv):
    """Bilinear, wrapping."""
    h, w = texture.shape[:2]
    x = (uv[..., 0] % 1) * w - 0.5
    y = (uv[..., 1] % 1) * h - 0.5
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    fx, fy = (x - x0)[..., None], (y - y0)[..., None]
    at = lambda yy, xx: texture[yy % h, xx % w]
    return ((at(y0, x0) * (1 - fx) + at(y0, x0 + 1) * fx) * (1 - fy)
            + (at(y0 + 1, x0) * (1 - fx) + at(y0 + 1, x0 + 1) * fx) * fy)


def smooth(a, radius):
    return ndimage.uniform_filter(a, size=(2 * radius + 1, 2 * radius + 1, 1), mode='nearest')


def icon(inputs, row, config, gun, skin, previews):
    """The skin's icon (PNG, 171x83 premultiplied RGBA, as the atlas keeps it). `gun` and `skin`: each
    [(mesh, texture: float RGB)], the live gun's surfaces and the skin's."""
    atlas = swf.atlas(inputs.read('players', MOVIE), 'LobbyMenu_WeaponIcons')
    r, c = config['ui_desc']['icon']
    live = cell(atlas, r, c).astype(np.float32)
    alpha = live[..., 3]
    meshes = [m for m, _ in gun]
    points = np.concatenate([m[0] for m in meshes])
    offsets = np.cumsum([0] + [len(m[0]) for m in meshes[:-1]])
    tris = np.concatenate([m[2] + off for m, off in zip(meshes, offsets)])
    pose, box, iou = fit_pose(points, tris, alpha)
    rgb = live[..., :3] / 255

    def render(surfaces):
        which, uvs = uv_buffer([m for m, _ in surfaces], pose, box, CELL, points)
        px = np.zeros_like(rgb)
        for index, (_, texture) in enumerate(surfaces):
            sel = which == index
            texels = footprint(which, uvs, index, texture.shape[1])
            px[sel] = sample(filtered(texture, texels), uvs[sel])
        return which, px

    which, old_px = render(gun)
    which_new, new_px = render(skin)
    shown = (which >= 0) & (which_new >= 0) & (alpha > 0)
    changed = shown & (np.abs(new_px - old_px).max(-1) > 0.02)
    # The icon's light across the gun: its brightness (smoothed over 2 pixels) over its mean on the gun.
    cover = alpha[..., None] / 255
    straight = rgb / np.maximum(cover, 1 / 255)
    weight = ((which >= 0) & (alpha > 250)).astype(np.float32)[..., None]
    bright = straight.mean(-1, keepdims=True)
    local = smooth(bright * weight, 2) / np.maximum(smooth(weight, 2), 1e-3)
    shade = np.clip(local / max(float(bright[weight[..., 0] > 0].mean()), 1e-3), *SHADE)
    out = rgb.copy()
    out[changed] = (np.clip(new_px * GAIN * shade, 0, 1) * cover)[changed]
    rgba = np.concatenate([out * 255, alpha[..., None]], -1).round().astype(np.uint8)
    picture = Image.fromarray(rgba, 'RGBA')
    if previews:
        sheet = Image.new('RGBA', (CELL[0] * 2 + 4, CELL[1]), (90, 90, 90, 255))
        sheet.alpha_composite(Image.fromarray(live.astype(np.uint8), 'RGBA'), (0, 0))
        sheet.alpha_composite(picture, (CELL[0] + 4, 0))
        sheet.resize((sheet.width * 3, sheet.height * 3), Image.Resampling.NEAREST).save(
            previews / f'{row.name}__icon.png')
    print(f'{row.name}: icon pose {pose}, silhouette IoU {iou:.3f}, {changed.sum()} pixels repainted')
    buffer = io.BytesIO()
    picture.save(buffer, format='PNG', optimize=False)
    return buffer.getvalue(), {'pose': pose, 'iou': round(float(iou), 3), 'pixels': int(changed.sum())}


def texture_preview(path, before, after, mask):
    small = lambda a: Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8)).resize((512, 512))
    sheet = Image.new('RGB', (512 * 3, 512))
    sheet.paste(small(before), (0, 0))
    sheet.paste(small(after), (512, 0))
    sheet.paste(small(np.repeat(mask[..., None], 3, 2)), (1024, 0))
    sheet.save(path)
