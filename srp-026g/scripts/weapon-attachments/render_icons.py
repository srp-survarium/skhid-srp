#!/usr/bin/env python3
"""render_icons.py: each device's shop icon, rendered from its 0.46c2 model (0.46c2 drew the
attachments' icons from the models at run time, `ui_desc.icon_renderer`: it has no pictures of
them to port).

usage: python3 -I render_icons.py --donor <folder> --out <folder>

--donor  the files prepare_donor.sh makes (the models, their materials and textures).
--out    a new folder: <device name>.png for each device of devices.tsv, 171x83 RGBA (the lobby's
         consumables atlas cell, LobbyMenu_AmmoIcons), for make_pack.py's atlas_icons_add.

The picture: the model's meshes (render/<surface>/converted_model: u32 3, the vertex bytes, the
count, 28-byte vertices {float3 position, three packed normals, half2 uv}; u32 4, the index bytes,
the count, u16 indices), each textured with its material's texture_diffuse (the .dds as 0.46c2 has
it), turned three-quarters (the muzzle to the right, seen from above and in front, as the atlas's
scopes are), orthographic, lit by a key light from the upper left and a fill, its face normals;
four times the cell's size, scaled down. Needs Pillow and numpy.
"""
import argparse
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

CELL = (171, 83)
SCALE = 4
MARGIN = 0.2


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def mesh(data):
    kind, size, count = struct.unpack_from('<3I', data, 0)
    assert kind == 3 and size == 4 + 28 * count, 'not a static mesh of 28-byte vertices'
    raw = np.frombuffer(data, dtype=np.uint8, count=28 * count, offset=12).reshape(count, 28)
    positions = raw[:, 0:12].copy().view('<f4').reshape(count, 3).astype(np.float64)
    uv = raw[:, 24:28].copy().view('<f2').reshape(count, 2).astype(np.float64)
    at = 12 + 28 * count
    kind, size, n = struct.unpack_from('<3I', data, at)
    assert kind == 4 and size == 4 + 2 * n, 'no index block after the vertices'
    indices = np.frombuffer(data, dtype='<u2', count=n, offset=at + 12).reshape(-1, 3)
    return positions, uv, indices


def leaf(buf, path):
    return ce.value_of(buf, ce.find(buf, path))


def diffuse(donor, material):
    """The material's diffuse texture as an RGB array (grey without one in the donor)."""
    buf = bytearray((donor / material).read_bytes())
    name = leaf(buf, 'material/g_stage/effect/texture_diffuse/value')
    path = donor / 'textures' / (name + '.dds')
    if not path.exists():
        return np.full((4, 4, 3), 140, dtype=np.float64)
    return np.asarray(Image.open(path).convert('RGB'), dtype=np.float64)


def surfaces(donor, model):
    """(mesh file, material file) of each of the model's surfaces."""
    buf = bytearray((donor / model / 'settings').read_bytes())
    table = ce.find(buf, 'material_settings')
    out = []
    for c in ce.children(buf, table):
        surface = ce.name_at(buf, ce.rec(buf, c)[1]).decode()
        name = leaf(buf, f'material_settings/{surface}/material_name')
        out.append((donor / model / 'render' / surface / 'converted_model',
                    f'material_instances/{name}.material'))
    return out


def rotation():
    """Model to view: the barrel (-Z) to the screen's right, turned 25 degrees towards the
    viewer, seen 20 degrees from above."""
    def ry(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])

    def rx(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    return rx(np.radians(20)) @ ry(np.radians(-90 + 25))


def render(parts):
    """parts: [(positions, uv, indices, texture)]. An RGBA image of CELL."""
    turn = rotation()
    views = [(p @ turn.T, uv, ix, tex) for p, uv, ix, tex in parts]
    allp = np.concatenate([v[0] for v in views])
    lo, hi = allp[:, :2].min(axis=0), allp[:, :2].max(axis=0)
    w, h = CELL[0] * SCALE, CELL[1] * SCALE
    fit = min(w * (1 - 2 * MARGIN) / (hi[0] - lo[0]), h * (1 - 2 * MARGIN) / (hi[1] - lo[1]))
    centre = (lo + hi) / 2
    color = np.zeros((h, w, 3))
    alpha = np.zeros((h, w))
    depth = np.full((h, w), -np.inf)
    key = np.array([-0.45, 0.7, 0.55]); key /= np.linalg.norm(key)
    fill = np.array([0.6, -0.2, 0.75]); fill /= np.linalg.norm(fill)
    for p, uv, ix, tex in views:
        sx = (p[:, 0] - centre[0]) * fit + w / 2
        sy = h / 2 - (p[:, 1] - centre[1]) * fit
        th, tw = tex.shape[:2]
        for a, b, c in ix:
            normal = np.cross(p[b] - p[a], p[c] - p[a])
            length = np.linalg.norm(normal)
            if length == 0:
                continue
            normal /= length
            if normal[2] < 0:
                normal = -normal
            light = 0.5 + 0.95 * max(0.0, normal @ key) + 0.3 * max(0.0, normal @ fill)
            half = key + np.array([0, 0, 1.0]); half /= np.linalg.norm(half)
            shine = 0.45 * max(0.0, normal @ half) ** 20
            xs, ys = sx[[a, b, c]], sy[[a, b, c]]
            x0, x1 = int(max(0, np.floor(xs.min()))), int(min(w - 1, np.ceil(xs.max())))
            y0, y1 = int(max(0, np.floor(ys.min()))), int(min(h - 1, np.ceil(ys.max())))
            if x1 < x0 or y1 < y0:
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
            z = l0 * p[a, 2] + l1 * p[b, 2] + l2 * p[c, 2]
            region = depth[y0:y1 + 1, x0:x1 + 1]
            nearer = inside & (z > region)
            if not nearer.any():
                continue
            u = l0 * uv[a, 0] + l1 * uv[b, 0] + l2 * uv[c, 0]
            v = l0 * uv[a, 1] + l1 * uv[b, 1] + l2 * uv[c, 1]
            tx = (np.mod(u, 1.0) * (tw - 1)).astype(int)
            ty = (np.mod(v, 1.0) * (th - 1)).astype(int)
            texel = tex[ty, tx]
            shade = np.clip(texel * light + 255 * shine, 0, 255)
            region[nearer] = z[nearer]
            color[y0:y1 + 1, x0:x1 + 1][nearer] = shade[nearer]
            alpha[y0:y1 + 1, x0:x1 + 1][nearer] = 255
    image = np.dstack([color, alpha]).astype(np.uint8)
    big = Image.fromarray(image, 'RGBA')
    # premultiplied while scaling down, as the atlas keeps its pixels
    pm = np.asarray(big, dtype=np.float64)
    pm[:, :, :3] *= pm[:, :, 3:4] / 255
    small = Image.fromarray(pm.astype(np.uint8), 'RGBA').resize(CELL, Image.Resampling.LANCZOS)
    out = np.asarray(small, dtype=np.float64)
    a = out[:, :, 3:4]
    out[:, :, :3] = np.where(a > 0, out[:, :, :3] * 255 / np.maximum(a, 1), 0)
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), 'RGBA')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    args.out.mkdir(parents=True)
    for row in rows('devices.tsv'):
        name, model = row[1], row[3]
        parts = []
        for mesh_file, material in surfaces(args.donor, model):
            positions, uv, indices = mesh(mesh_file.read_bytes())
            parts.append((positions, uv, indices, diffuse(args.donor, material)))
        render(parts).save(args.out / f'{name}.png')
        print(f'{name}: {args.out / (name + ".png")}')


if __name__ == '__main__':
    main()
