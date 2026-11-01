#!/usr/bin/env python3
"""camo_check.py <bundle> <weapon-camo pack folder> <vostok-port> <target layers...>

Patch 44 paints a camouflage into a weapon's own surfaces where its texture's _aoac mask says
(G below 1), and a stock muzzle device is now a model of its own, which patch 44 does not bake:
a gun whose camouflage painted its built-in device shows that device plain, as every other muzzle
device is (docs/notes/muzzle-devices.md, "Camouflage"). For each gun's stock device this reads the
mask under its triangles (their UV centroids) and prints the share a camouflage used to paint.
"""
import json
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

bundle, camo, port = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
layers = sys.argv[4:]


def leaves(path):
    for db in reversed(layers):
        r = subprocess.run([port, 'dump-db-config', '--db', db, '--path', path, '--leaves'],
                           capture_output=True, text=True)
        if r.returncode == 0 and r.stdout:
            return dict(l.split('\t', 1) for l in r.stdout.splitlines() if '\t' in l)
    return None


def text(v):
    return v.split(':', 1)[1].strip('"')


def dxt_green(path):
    b = path.read_bytes()
    h, w = struct.unpack_from('<II', b, 12)
    four = b[84:88]
    size = 8 if four == b'DXT1' else 16
    blocks = np.frombuffer(b[128:128 + (w // 4) * (h // 4) * size], np.uint8).reshape(-1, size)
    colour = blocks[:, size - 8:]
    c = colour[:, :4].copy().view('<u2')
    g0, g1 = ((c[:, 0] >> 5) & 63).astype(int) * 255 // 63, ((c[:, 1] >> 5) & 63).astype(int) * 255 // 63
    pal = np.stack([g0, g1, (2 * g0 + g1) // 3, (g0 + 2 * g1) // 3], 1)
    bits = colour[:, 4:8].copy().view('<u4')[:, 0]
    sel = (bits[:, None] >> (2 * np.arange(16))) & 3
    px = np.take_along_axis(pal, sel, 1).reshape(h // 4, w // 4, 4, 4)
    return px.transpose(0, 2, 1, 3).reshape(h, w)


def static_uv_centroids(path):
    b = path.read_bytes()
    n = struct.unpack_from('<I', b, 8)[0]
    raw = np.frombuffer(b[12:12 + 28 * n], np.uint8).reshape(n, 28)
    uv = raw[:, 24:28].copy().view('<f2').astype(float)
    at = 12 + 28 * n + 8
    m = struct.unpack_from('<I', b, at)[0]
    idx = np.frombuffer(b[at + 4:at + 4 + 2 * m], '<u2').reshape(-1, 3)
    return uv[idx].mean(1)


o = bundle / 'overlay'
for settings in sorted(o.glob('models/weapons/v069d/muzzle_stock/*.model/settings')):
    root = settings.parent
    props = leaves(str(settings.relative_to(o))) or {}
    gun = root.name[:-len('.model')]
    shares = []
    for surface in root.glob('render/*/converted_model'):
        sname = surface.parent.name
        material = None
        for k, v in props.items():
            if k == f'material_settings.{sname}.material_name':
                material = text(v)
        if material is None:
            # the bundle's own settings (not in the layers): read them from the file
            r = subprocess.run([port, 'dump-config', '--file', str(settings), '--leaves'], capture_output=True, text=True)
            for line in r.stdout.splitlines():
                k, _, v = line.partition('\t')
                if k == f'material_settings.{sname}.material_name':
                    material = text(v)
        mat = leaves(f'material_instances/{material}.material')
        diffuse = text(mat['material.g_stage.effect.texture_diffuse.value'])
        mask = camo / 'files' / 'textures' / f'{diffuse}_aoac.dds'
        if not mask.exists():
            shares.append((sname, None))
            continue
        g = dxt_green(mask)
        h, w = g.shape
        uv = static_uv_centroids(surface)
        x = (np.mod(uv[:, 0], 1) * w).astype(int) % w
        y = (np.mod(uv[:, 1], 1) * h).astype(int) % h
        share = float((g[y, x] < 230).mean())
        shares.append((sname, share))
    print(gun, ' '.join(f'{s}:{"no mask" if v is None else f"{v:.1%}"}' for s, v in shares))
