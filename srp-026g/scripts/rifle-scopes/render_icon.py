"""render_icon.py: a shop icon rendered from a static model (0.69d0 drew its optics' icons from the
models at run time, `ui_desc.icon_renderer`: it has no pictures of them to port), and the review
sheet's views (review.py).

A model's meshes (render/<surface>/converted_model: 28-byte vertices {float3 position, packed
normals, half2 uv}, or a skinned model's 36-byte ones, then u16 triangles), each textured with its
material's texture_diffuse, found in the given roots; orthographic or perspective; lit by a key
light from the upper left and a fill, its face normals; four times the size, scaled down. Needs
Pillow and numpy.
"""
import struct
from pathlib import Path

import numpy as np
from PIL import Image

import scopes_lib as lib

CELL = (171, 83)
SCALE = 4
MARGIN = 0.12


def mesh(data):
    kind, size, count = struct.unpack_from('<3I', data, 0)
    stride = (size - 4) // count
    raw = np.frombuffer(data, dtype=np.uint8, count=stride * count, offset=12).reshape(count, stride)
    p = raw[:, 0:12].copy().view('<f4').reshape(count, 3).astype(np.float64)
    uv = raw[:, 24:28].copy().view('<f2').reshape(count, 2).astype(np.float64)
    at = 12 + stride * count
    kind, size, n = struct.unpack_from('<3I', data, at)
    ix = np.frombuffer(data, dtype='<u2', count=n, offset=at + 12).reshape(-1, 3)
    return p, uv, ix


def find(roots, path):
    for root in roots:
        p = Path(root) / path
        if p.exists():
            return p
    return None


def texture(roots, material):
    """The material's diffuse texture as an RGB array (grey without one)."""
    m = find(roots, f'material_instances/{material}.material')
    if m:
        name = lib.leaves(m).get('/material/g_stage/effect/texture_diffuse/value')
        if not name:
            name = lib.leaves(m).get('/material/forward/effect/texture_diffuse/value')
        t = find(roots, f'textures/{name}.dds') if name else None
        if t:
            return np.asarray(Image.open(t).convert('RGB'), dtype=np.float64)
    return np.full((4, 4, 3), 120.0)


def parts(model, roots, offset=(0, 0, 0), flip=False, skip=('scope_lense',)):
    """(positions, uv, triangles, texture) of each surface of a model directory: moved by
    `offset` and, with `flip`, turned 180 degrees about Y first (a scope model as the weapon draws
    it at scope_point)."""
    settings = lib.leaves(model / 'settings')
    out = []
    for s in lib.surfaces(model):
        if any(s.startswith(k) for k in skip):
            continue
        p, uv, ix = mesh((model / 'render' / s / 'converted_model').read_bytes())
        if flip:
            p = p * np.array([-1.0, 1.0, -1.0])
        out.append((p + np.array(offset), uv, ix, texture(roots, settings.get(f'/material_settings/{s}/material_name', ''))))
    return out


def turn(yaw, pitch):
    def ry(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])

    def rx(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    return rx(np.radians(pitch)) @ ry(np.radians(yaw))


def raster(views, size, project, background=None):
    """views: [(view-space points, uv, triangles, texture)]; project: points -> (x, y, depth), depth
    larger nearer."""
    w, h = size
    color = np.zeros((h, w, 3)) if background is None else np.full((h, w, 3), background, dtype=np.float64)
    alpha = np.zeros((h, w))
    depth = np.full((h, w), -np.inf)
    key = np.array([-0.45, 0.7, 0.55]); key /= np.linalg.norm(key)
    fill = np.array([0.6, -0.2, 0.75]); fill /= np.linalg.norm(fill)
    half = key + np.array([0, 0, 1.0]); half /= np.linalg.norm(half)
    for p, uv, ix, tex in views:
        sx, sy, z = project(p)
        th, tw = tex.shape[:2]
        for a, b, c in ix:
            if not (np.isfinite(sx[[a, b, c]]).all() and np.isfinite(sy[[a, b, c]]).all()):
                continue
            normal = np.cross(p[b] - p[a], p[c] - p[a])
            length = np.linalg.norm(normal)
            if length == 0:
                continue
            normal /= length
            if normal[2] < 0:
                normal = -normal
            light = 0.5 + 0.95 * max(0.0, normal @ key) + 0.3 * max(0.0, normal @ fill)
            shine = 0.45 * max(0.0, normal @ half) ** 20
            xs, ys = sx[[a, b, c]], sy[[a, b, c]]
            x0, x1 = int(max(0, np.floor(xs.min()))), int(min(w - 1, np.ceil(xs.max())))
            y0, y1 = int(max(0, np.floor(ys.min()))), int(min(h - 1, np.ceil(ys.max())))
            if x1 < x0 or y1 < y0 or (x1 - x0) * (y1 - y0) > w * h:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            d = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
            if abs(d) < 1e-12:
                continue
            l0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / d
            l1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / d
            l2 = 1 - l0 - l1
            inside = (l0 >= -1e-9) & (l1 >= -1e-9) & (l2 >= -1e-9)
            if not inside.any():
                continue
            zz = l0 * z[a] + l1 * z[b] + l2 * z[c]
            region = depth[y0:y1 + 1, x0:x1 + 1]
            nearer = inside & (zz > region)
            if not nearer.any():
                continue
            u = l0 * uv[a, 0] + l1 * uv[b, 0] + l2 * uv[c, 0]
            v = l0 * uv[a, 1] + l1 * uv[b, 1] + l2 * uv[c, 1]
            u, v = np.nan_to_num(u), np.nan_to_num(v)
            texel = tex[(np.mod(v, 1.0) * (th - 1)).astype(int), (np.mod(u, 1.0) * (tw - 1)).astype(int)]
            region[nearer] = zz[nearer]
            color[y0:y1 + 1, x0:x1 + 1][nearer] = np.clip(texel * light + 255 * shine, 0, 255)[nearer]
            alpha[y0:y1 + 1, x0:x1 + 1][nearer] = 255
    return color, alpha


def scaled(color, alpha, size):
    big = np.dstack([color * alpha[:, :, None] / 255, alpha]).astype(np.uint8)
    small = np.asarray(Image.fromarray(big, 'RGBA').resize(size, Image.Resampling.LANCZOS), dtype=np.float64)
    a = small[:, :, 3:4]
    small[:, :, :3] = np.where(a > 0, small[:, :, :3] * 255 / np.maximum(a, 1), 0)
    return Image.fromarray(np.clip(small, 0, 255).astype(np.uint8), 'RGBA')


def orthographic(items, size, yaw, pitch, margin=MARGIN, background=None):
    """items: parts(); seen from the side (yaw, pitch degrees), fitted into `size`."""
    r = turn(yaw, pitch)
    views = [(p @ r.T, uv, ix, tex) for p, uv, ix, tex in items]
    allp = np.concatenate([v[0] for v in views])
    lo, hi = allp[:, :2].min(axis=0), allp[:, :2].max(axis=0)
    w, h = size[0] * SCALE, size[1] * SCALE
    fit = min(w * (1 - 2 * margin) / (hi[0] - lo[0]), h * (1 - 2 * margin) / (hi[1] - lo[1]))
    centre = (lo + hi) / 2

    def project(p):
        return (p[:, 0] - centre[0]) * fit + w / 2, h / 2 - (p[:, 1] - centre[1]) * fit, p[:, 2]
    color, alpha = raster(views, (w, h), project, background)
    return scaled(color, alpha, size) if background is None else Image.fromarray(
        np.asarray(Image.fromarray(color.astype(np.uint8)).resize(size, Image.Resampling.LANCZOS)))


def perspective(items, size, eye, fov_deg, near=0.02, background=(70, 80, 95)):
    """items in weapon space, seen from `eye` along -Z (the weapon's muzzle), vertical field of view
    `fov_deg`."""
    w, h = size[0] * SCALE, size[1] * SCALE
    f = (h / 2) / np.tan(np.radians(fov_deg) / 2)
    views = [(p - np.array(eye), uv, ix, tex) for p, uv, ix, tex in items]

    def project(p):
        dz = -p[:, 2]
        ok = dz > near
        with np.errstate(divide='ignore', invalid='ignore'):
            x = np.where(ok, w / 2 + f * p[:, 0] / dz, np.nan)
            y = np.where(ok, h / 2 - f * p[:, 1] / dz, np.nan)
        return x, y, -dz
    color, alpha = raster(views, (w, h), project, background)
    return Image.fromarray(color.astype(np.uint8)).resize(size, Image.Resampling.LANCZOS)


def icon(model, roots):
    """The model's shop icon: three-quarters from above, the eyepiece to the left."""
    return orthographic(parts(model, roots), CELL, yaw=-90 + 25, pitch=20)
