#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Orthographic textured-mesh renderer used for the equipment-style weapon icons.

Adapted from resource-porter/scripts/render-weapons-069d-icons.py, the weapon
variant of its render-duty-026-icons.py / render-mask-026-icons.py workflow.
Reads the converted 0.26g geometry and its diffuse, normal, Fresnel and
roughness textures.
"""
from dataclasses import dataclass
import struct
import zlib

import numpy as np
from PIL import Image, ImageFilter


@dataclass
class Surface:
    name: str
    position: np.ndarray
    normal: np.ndarray
    tangent: np.ndarray
    bitangent: np.ndarray
    uv: np.ndarray
    triangles: np.ndarray
    diffuse: np.ndarray
    normal_map: np.ndarray
    fresnel: np.ndarray
    roughness: np.ndarray

CELL_W, CELL_H, PITCH = 171, 83, 91
SYMBOL = b'LobbyMenu_WeaponIcons'
NATIVE_REFERENCE = [(3, 3), (10, 2), (16, 2)]  # AK-12, AK-74M, A-545


def swf_tags(data):
    assert data[:3] in (b'CWS', b'FWS')
    if data[:3] == b'CWS':
        data = data[:8] + zlib.decompress(data[8:])
    at = 8 + (5 + 4 * (data[8] >> 3) + 7) // 8 + 4
    prefix, tags = data[:at], []
    while at < len(data):
        header = struct.unpack_from('<H', data, at)[0]; at += 2
        size = header & 63
        if size == 63:
            size = struct.unpack_from('<I', data, at)[0]; at += 4
        tags.append((header >> 6, data[at:at + size])); at += size
    return prefix, tags


def atlas(tags, symbol=SYMBOL):
    char_id = None
    for kind, payload in tags:
        if kind != 76:
            continue
        at = 2
        for _ in range(struct.unpack_from('<H', payload)[0]):
            ident = struct.unpack_from('<H', payload, at)[0]; at += 2
            end = payload.index(0, at)
            if payload[at:end] == symbol:
                char_id = ident
            at = end + 1
    assert char_id is not None, f'{symbol} bitmap missing'
    for index, (kind, payload) in enumerate(tags):
        if kind == 36 and struct.unpack_from('<H', payload)[0] == char_id:
            ident, fmt, w, h = struct.unpack_from('<HBHH', payload)
            assert fmt == 5
            argb = np.frombuffer(zlib.decompress(payload[7:]), np.uint8).reshape(h, w, 4).copy()
            return index, ident, argb
    raise ValueError('weapon icon bitmap is not lossless ARGB')


# --- resources -------------------------------------------------------------

def config_strings(b):
    """Every (key path, string) of a 24-byte binary config (enough to read
    settings and materials)."""
    out = {}

    def cstr(at):
        return b[at:b.index(0, at)].decode('latin-1')

    def walk(at, path):
        data, name, _, kind, count = struct.unpack_from('<QQIHH', b, at)
        if kind in (3, 4):
            for i in range(count):
                child = data + 24 * i
                key = cstr(struct.unpack_from('<Q', b, child + 8)[0]) if kind == 3 else str(i)
                walk(child, path + [key])
        elif kind == 5:
            out['.'.join(path)] = cstr(data)
    walk(0, [])
    return out


def mesh(path):
    b = path.read_bytes()
    kind, size, n = struct.unpack_from('<III', b)
    if kind != 3 or not n or (size - 4) % n:
        raise ValueError(f'{path}: invalid vertex chunk')
    stride = (size - 4) // n
    if stride not in (36, 40, 44, 48):
        raise ValueError(f'{path}: unsupported skinned vertex stride {stride}')
    raw = np.frombuffer(b, np.uint8, n * stride, 12).reshape(n, stride)
    pos = raw[:, :12].copy().view('<f4').reshape(n, 3).astype(float)
    # One to four bone influences precede packed normal/tangent/binormal.
    def basis(offset):
        words = raw[:, offset:offset+4].copy().view('<u4')[:, 0]
        return np.stack([(words >> 16) & 255, (words >> 8) & 255, words & 255], 1) / 127.5 - 1
    normal, tangent, bitangent = (basis(stride - off) for off in (20, 16, 12))
    uv = raw[:, -8:].copy().view('<f4').reshape(n, 2).astype(float)
    at = 8 + size
    kind, size, count = struct.unpack_from('<III', b, at)
    if kind != 4 or count % 3 or size != 4 + count * 2:
        raise ValueError(f'{path}: invalid triangle chunk')
    idx = np.frombuffer(b, '<u2', count, at + 12).reshape(-1, 3)
    if idx.max() >= n or not np.isfinite(pos).all() or not np.isfinite(uv).all():
        raise ValueError(f'{path}: invalid geometry')
    return pos, normal, tangent, bitangent, uv, idx


def rifle_parts(o, root):
    settings = config_strings((o / root / 'settings').read_bytes())
    parts = []
    for key, surface in sorted((k, v) for k, v in settings.items() if k.startswith('lod_hierrarchy.LOD0.surfaces.')):
        if surface.startswith('shell'):
            continue  # a pump's loose shell, which its clips keep out of sight
        material = settings[f'material_settings.{surface}.material_name']
        mat = config_strings((o / f'material_instances/{material}.material').read_bytes())
        pos, normal, tangent, bitangent, uv, idx = mesh(o / root / f'render/{surface}/converted_model')
        def texture(key):
            name = mat['material.g_stage.effect.' + key + '.value']
            if not name:
                raise ValueError(f'{material}: missing {key}')
            return np.asarray(Image.open(o / f'textures/{name}.dds').convert('RGBA'))
        parts.append(Surface(surface, pos, normal, tangent, bitangent, uv, idx,
                             texture('texture_diffuse'), texture('texture_normal'),
                             texture('texture_fresnel'), texture('texture_roughness')))

    return parts


# --- rasteriser ------------------------------------------------------------

def render(parts, view, size, margin=0.03, light=(-0.45, 0.75, 0.8), exposure=1.0, super_sample=3):
    W, H = size[0] * super_sample, size[1] * super_sample
    view = np.asarray(view, float)
    allp = np.concatenate([p.position[np.unique(p.triangles)] for p in parts]) @ view.T
    lo, hi = allp.min(0), allp.max(0)
    scale = min(W * (1 - 2 * margin) / (hi[0] - lo[0]), H * (1 - 2 * margin) / (hi[1] - lo[1]))
    cx, cy = (lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2
    rgb = np.zeros((H, W, 3)); cov = np.zeros((H, W)); zb = np.full((H, W), -1e9)
    L = np.asarray(light, float); L /= np.linalg.norm(L)
    for part in parts:
        pos, uv, idx, tex = part.position, part.uv, part.triangles, part.diffuse
        p = pos @ view.T; n = part.normal @ view.T
        tangent, bitangent = part.tangent @ view.T, part.bitangent @ view.T
        sx = (p[:, 0] - cx) * scale + W / 2; sy = H / 2 - (p[:, 1] - cy) * scale; sz = p[:, 2]
        for a, b, c in idx:
            xs, ys = sx[[a, b, c]], sy[[a, b, c]]
            x0, x1 = max(0, int(xs.min())), min(W - 1, int(np.ceil(xs.max())))
            y0, y1 = max(0, int(ys.min())), min(H - 1, int(np.ceil(ys.max())))
            if x0 > x1 or y0 > y1:
                continue
            d = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
            if abs(d) < 1e-12:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + .5, np.arange(y0, y1 + 1) + .5)
            w0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / d
            w1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / d
            w2 = 1 - w0 - w1
            m = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not m.any():
                continue
            z = w0 * sz[a] + w1 * sz[b] + w2 * sz[c]
            sub = zb[y0:y1 + 1, x0:x1 + 1]
            m &= z > sub
            if not m.any():
                continue
            sub[m] = z[m]
            u = w0 * uv[a, 0] + w1 * uv[b, 0] + w2 * uv[c, 0]
            v = w0 * uv[a, 1] + w1 * uv[b, 1] + w2 * uv[c, 1]
            def sample(texture):
                hh, ww = texture.shape[:2]
                xx = (np.mod(u[m], 1) * ww).astype(int) % ww
                yy = (np.mod(v[m], 1) * hh).astype(int) % hh
                return texture[yy, xx] / 255.0
            colour = sample(tex)[:, :3] ** 2.2
            weights = np.stack([w0[m], w1[m], w2[m]], 1)
            nn = weights @ n[[a,b,c]]
            tt = weights @ tangent[[a,b,c]]
            bb = weights @ bitangent[[a,b,c]]
            nm = sample(part.normal_map)
            xy = nm[:, [3,1]] * 2 - 1  # retail DXT5nm: X in alpha, Y in green
            nz = np.sqrt(np.maximum(0, 1-(xy*xy).sum(1)))
            nn = tt*xy[:, :1] + bb*xy[:, 1:2] + nn*nz[:, None]
            nn /= np.maximum(np.linalg.norm(nn, axis=1, keepdims=True), 1e-9)
            nn *= np.where(nn[:, 2:3] < 0, -1, 1)
            f0 = sample(part.fresnel)[:, :3]
            rough = sample(part.roughness)[:, :3].mean(1)
            half = L + [0,0,1]; half /= np.linalg.norm(half)
            key = np.maximum(nn @ L, 0)
            highlight = np.maximum(nn @ half, 0) ** (6 + 80*(1-rough)**2)
            diffuse = colour * (.45 + .7*key[:,None])
            reflection = f0 * (.035 + 2.0*highlight[:,None]) * [0.66, 0.82, 1.0]
            lit = np.maximum((diffuse + reflection)*exposure, 0) ** (1/2.2)
            rgb[y0:y1 + 1, x0:x1 + 1][m] = lit
            cov[y0:y1 + 1, x0:x1 + 1][m] = 1
    img = Image.fromarray(np.dstack([np.clip(rgb, 0, 1) * 255, cov * 255]).astype(np.uint8), 'RGBA')
    return img.resize(size, Image.LANCZOS)


def with_shadow(icon, offset=(2, 3), blur=2.5, strength=0.55):
    a = np.asarray(icon)[:, :, 3].astype(float) / 255
    shadow = Image.fromarray((a * 255 * strength).astype(np.uint8)).filter(ImageFilter.GaussianBlur(blur))
    shifted = Image.new('L', icon.size)
    shifted.paste(shadow, offset)
    shadow = np.asarray(shifted).astype(float) / 255
    rgba = np.asarray(icon).astype(float) / 255
    out_a = rgba[:, :, 3] + shadow * (1 - rgba[:, :, 3])
    out_rgb = rgba[:, :, :3] * rgba[:, :, 3:4] / np.maximum(out_a[:, :, None], 1e-9)
    return Image.fromarray((np.dstack([out_rgb, out_a]) * 255).astype(np.uint8), 'RGBA')


# Views: rows are screen x, screen y, depth (larger is nearer). Meshes face
# +z (muzzle) with +x on the weapon's left.
RIGHT = [[0, 0, 1], [0, 1, 0], [-1, 0, 0]]


def mean_luma(rgba):
    a = rgba[:, :, 3] > 200
    return (rgba[:, :, :3][a] @ [0.299, 0.587, 0.114]).mean()
