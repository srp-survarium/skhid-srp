#!/usr/bin/env python3
"""Render actual equipment pairs, selecting garments with the accessory's model_type.

--pairs is JSON: [[garment_config, accessory_config], ...]. Trees are extracted
resources in overlay order. Requires numpy/Pillow and the porter's renderer.
A --pose command is the pose-stk2-outfits-026 example; --db and --clip then
pose all meshes once before rendering. Uniform colours make gaps and clipping
visible independently of the textures. This is an offline geometry preview.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw

from check_boot_classes import Trees, bcw


def locate(trees, path):
    for tree in reversed(trees):
        file = tree / path
        if file.is_file():
            return file
    raise ValueError(f'missing resource {path}')


def surfaces(trees, data):
    result = []
    for part in [data, *data.get('additional_models', [])]:
        root = f"models/{part['base_model']}.skinned_model/render"
        result.append((locate(trees, f"{root}/{part['part_name']}/converted_model"),
                       locate(trees, f'{root}/bind_pose')))
    return result


def mesh(path, colour):
    raw = path.read_bytes()
    count = struct.unpack_from('<I', raw, 8)[0]
    stride = (struct.unpack_from('<I', raw, 4)[0] - 4) // count
    if stride not in (32, 36, 40, 44, 48):
        raise ValueError(f'{path}: unsupported stride {stride}')
    vertices = np.frombuffer(raw, dtype=np.uint8, count=count * stride, offset=12).reshape(count, stride)
    positions = vertices.copy().view('<f4').reshape(count, stride // 4)[:, :3].astype(float)
    indices = struct.unpack_from('<I', raw, 20 + count * stride)[0]
    faces = np.frombuffer(raw, dtype='<u2', offset=24 + count * stride, count=indices).reshape(-1, 3)
    triangles = positions[faces]
    face_normals = np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0])
    normals = np.zeros_like(positions)
    for corner in range(3):
        np.add.at(normals, faces[:, corner], face_normals)
    normals /= np.maximum(np.linalg.norm(normals, axis=1)[:, None], 1e-12)
    texture = np.array([[colour]], dtype=float)
    return positions, np.zeros((count, 2)), normals, faces, texture, np.unique(faces)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--tree', type=Path, action='append', required=True)
    p.add_argument('--pairs', type=Path, required=True)
    p.add_argument('--renderer', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--pose', type=Path)
    p.add_argument('--db', type=Path)
    p.add_argument('--clip')
    args = p.parse_args()
    if args.pose and not (args.db and args.clip):
        p.error('--pose requires --db and --clip')
    spec = importlib.util.spec_from_file_location('renderer', args.renderer)
    renderer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(renderer)
    view = Trees(args.tree)
    pairs, unique = [], {}
    for garment, accessory in json.loads(args.pairs.read_text()):
        ad = bcw.loads(view.read(accessory))['data']
        gc = bcw.loads(view.read(garment))
        gd = gc[ad['model_type']]
        left, right = surfaces(args.tree, gd), surfaces(args.tree, ad)
        for entry in left + right:
            unique.setdefault(entry, len(unique))
        pairs.append((garment, accessory, left, right))
    posed = args.out.with_suffix('.posed')
    if args.pose:
        subprocess.run([str(args.pose), '--db', str(args.db), '--clip', args.clip,
                        '--out', str(posed), *[f'{a}={b}' for a, b in unique]], check=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    size = (280, 400)
    sheet = Image.new('RGB', (size[0] * 3, (size[1] + 36) * len(pairs)), (45, 48, 52))
    draw = ImageDraw.Draw(sheet)
    for row, (garment, accessory, left, right) in enumerate(pairs):
        parts = []
        for items, colour in [(left, (.65, .59, .44)), (right, (.23, .50, .64))]:
            for entry in items:
                file = posed / str(unique[entry]) / 'converted_model' if args.pose else entry[0]
                parts.append(mesh(file, colour))
        y = row * (size[1] + 36)
        draw.text((8, y + 4), f'{Path(garment).stem} + {Path(accessory).stem}', fill='white')
        for col, yaw in enumerate([0, 90, 180]):
            picture = renderer.render(parts, yaw, 0, size=size)
            sheet.paste(picture, (col * size[0], y + 30), picture)
    sheet.save(args.out)
    print(f'{len(pairs)} pairs, 3 views each: {args.out}')


if __name__ == '__main__':
    main()
