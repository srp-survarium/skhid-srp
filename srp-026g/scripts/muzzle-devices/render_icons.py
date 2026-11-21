#!/usr/bin/env python3
"""render_icons.py: the muzzle devices' lobby icons, drawn from their models.

usage: python3 render_icons.py --modules <the weapon-modules pack's folder> --inputs <prepare.sh's folder> --out <new folder>

Needs NumPy and Pillow. For each device of devices.tsv: its model of the modules pack (LOD0, its
diffuse, normal, Fresnel and roughness maps), drawn by ../weapon-icons/renderer.py (the weapons'
own icon renderer) in a three-quarter view from the right, muzzle right, in a 171x83 cell with the
weapons' soft shadow; exposed so its mean brightness is that of the stock scopes' icons
(LobbyMenu_AmmoIcons [1..3, 4] of the players' lobby movie). Writes <item>.png (straight alpha) for
make_pack.py --icons, and sheet.png (every icon with its dict id, to look at).
"""
import argparse
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'weapon-icons'))
import devices as dev  # noqa: E402
import renderer as R  # noqa: E402

ATLAS = b'LobbyMenu_AmmoIcons'
STOCK_SCOPES = [(1, 4), (2, 4), (3, 4)]
YAW, PITCH = 32.0, 14.0
LIGHT = np.array([-0.45, 0.75, 0.8]) / np.linalg.norm([-0.45, 0.75, 0.8])   # the renderer's key light, in view space
AMBIENT, KEY = 0.45, 0.7                                                     # its diffuse terms


def view(yaw, pitch):
    """renderer.RIGHT (screen x the muzzle's way, the camera on the gun's right) turned by yaw about
    the up axis (the muzzle towards the camera) and tipped by pitch (seen a little from above)."""
    y, p = np.radians(yaw), np.radians(pitch)
    right = np.array(R.RIGHT, float)
    turn = np.array([[np.cos(y), 0, -np.sin(y)], [0, 1, 0], [np.sin(y), 0, np.cos(y)]])
    tip = np.array([[1, 0, 0], [0, np.cos(p), -np.sin(p)], [0, np.sin(p), np.cos(p)]])
    return tip @ turn @ right


def static_mesh(path):
    """A 0.26g static mesh (28-byte vertices): positions, the packed basis, UVs (half floats), triangles."""
    b = path.read_bytes()
    kind, size, n = struct.unpack_from('<III', b)
    if kind != 3 or size != 4 + 28 * n:
        raise ValueError(f'{path}: not a static vertex chunk')
    raw = np.frombuffer(b, np.uint8, n * 28, 12).reshape(n, 28)
    pos = raw[:, :12].copy().view('<f4').reshape(n, 3).astype(float)

    def basis(at):
        words = raw[:, at:at + 4].copy().view('<u4')[:, 0]
        return np.stack([(words >> 16) & 255, (words >> 8) & 255, words & 255], 1) / 127.5 - 1
    uv = raw[:, 24:28].copy().view('<f2').reshape(n, 2).astype(float)
    at = 8 + size
    kind, size, count = struct.unpack_from('<III', b, at)
    if kind != 4 or count % 3 or size != 4 + 2 * count:
        raise ValueError(f'{path}: not a triangle chunk')
    idx = np.frombuffer(b, '<u2', count, at + 12).reshape(-1, 3)
    return pos, basis(12), basis(16), basis(20), uv, idx


def parts(pack, model):
    root = pack / 'files/models' / f'{model}.model'
    settings = R.config_strings((root / 'settings').read_bytes())
    out = []
    for key, surface in sorted((k, v) for k, v in settings.items() if k.startswith('lod_hierrarchy.LOD0.surfaces.')):
        material = settings[f'material_settings.{surface}.material_name']
        mat = R.config_strings((pack / f'files/material_instances/{material}.material').read_bytes())

        def texture(key):
            name = mat['material.g_stage.effect.' + key + '.value']
            return np.asarray(Image.open(pack / f'files/textures/{name}.dds').convert('RGBA'))
        pos, normal, tangent, bitangent, uv, idx = static_mesh(root / f'render/{surface}/converted_model')
        out.append(R.Surface(surface, pos, normal, tangent, bitangent, uv, idx, texture('texture_diffuse'),
                             texture('texture_normal'), texture('texture_fresnel'), texture('texture_roughness')))
    return out


def in_shadow(surfaces, camera):
    """For each surface, each triangle's key light that the model itself keeps off it (0 where none).

    The renderer has no shadows, so the inside of a cone (the AKS-74U's bell) took the highlight of
    a face turned to the light. A triangle is in shadow when the ray from its centre to the light
    meets another triangle of the model; what it misses is the key light its normal would take."""
    camera = np.asarray(camera, float)
    every = np.concatenate([s.position[s.triangles] @ camera.T for s in surfaces])
    a, e1, e2 = every[:, 0], every[:, 1] - every[:, 0], every[:, 2] - every[:, 0]
    h = np.cross(LIGHT, e2)
    det = (e1 * h).sum(1)
    inv = np.where(np.abs(det) > 1e-12, 1 / np.where(det == 0, 1, det), 0)
    out = []
    for s in surfaces:
        tri = s.position[s.triangles] @ camera.T
        to = tri.mean(1)[:, None, :] - a[None]          # Moller-Trumbore against every triangle at once
        u = (to * h[None]).sum(2) * inv
        q = np.cross(to, e1[None])
        v = (q * LIGHT).sum(2) * inv
        t = (q * e2[None]).sum(2) * inv
        blocked = ((inv != 0) & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-6)).any(1)
        normal = (s.normal[s.triangles] @ camera.T).mean(1)
        normal *= np.where(normal[:, 2:3] < 0, -1, 1)   # turned to the camera, as the renderer does
        out.append(np.where(blocked, np.maximum(normal @ LIGHT, 0), 0.0))
    return out


def shadowed(surfaces, camera):
    """The surfaces with their shadowed triangles split off: no highlight, and the key light they miss
    taken out of the diffuse texture (in tenths)."""
    out = []
    for s, missed in zip(surfaces, in_shadow(surfaces, camera)):
        out.append(R.Surface(**{**s.__dict__, 'triangles': s.triangles[missed == 0]}))
        for tenth in range(10):
            pick = (missed > tenth / 10) & (missed <= (tenth + 1) / 10)
            if pick.any():
                dim = (AMBIENT / (AMBIENT + KEY * (tenth + 0.5) / 10)) ** (1 / 2.2)   # the texture is gamma 2.2
                diffuse = s.diffuse.astype(float) * [dim, dim, dim, 1]
                out.append(R.Surface(**{**s.__dict__, 'triangles': s.triangles[pick], 'diffuse': diffuse.astype(np.uint8),
                                        'fresnel': np.zeros_like(s.fresnel)}))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--modules', type=Path, required=True)
    ap.add_argument('--inputs', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    args.out.mkdir(parents=True)
    _, tags = R.swf_tags((args.inputs / 'live/flash_movies/lobby_menu.swf').read_bytes())
    _, _, argb = R.atlas(tags, ATLAS)
    rgba = argb[:, :, [1, 2, 3, 0]]
    W, H, P = R.CELL_W, R.CELL_H, R.PITCH
    target = np.mean([R.mean_luma(rgba[(r - 1) * P:(r - 1) * P + H, (c - 1) * W:c * W]) for r, c in STOCK_SCOPES])
    found = dev.read_table()
    size = (W - 24, H - 16)
    camera = view(YAW, PITCH)
    icons = []
    for device in found:
        surfaces = shadowed(parts(args.modules, device['model']), camera)
        exposure = 1.0
        for _ in range(4):      # the finished icon's brightness is not linear in the exposure
            icon = R.render(surfaces, camera, size, light=LIGHT, exposure=exposure)
            exposure *= target / max(R.mean_luma(np.asarray(icon)), 1e-6)
        canvas = Image.new('RGBA', (W, H))
        canvas.paste(icon, ((W - icon.size[0]) // 2, (H - 4 - icon.size[1]) // 2))
        canvas = R.with_shadow(canvas)
        canvas.save(args.out / f'{device["item"]}.png')
        icons.append((device, canvas))
        print(f'{device["item"]}: brightness {R.mean_luma(np.asarray(canvas)):.1f} (the stock scopes\' {target:.1f})')
    columns = 6
    sheet = Image.new('RGBA', (columns * W, -(-len(icons) // columns) * (H + 14)), (34, 36, 40, 255))
    draw = ImageDraw.Draw(sheet)
    for i, (device, icon) in enumerate(icons):
        x, y = i % columns * W, i // columns * (H + 14)
        sheet.alpha_composite(icon, (x, y))
        draw.text((x + 4, y + H), f'{device["dict_id"]} {device["family"]}', fill=(200, 200, 200, 255))
    sheet.save(args.out / 'sheet.png')


if __name__ == '__main__':
    main()
