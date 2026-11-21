#!/usr/bin/env python3
"""The guns at lobby distance with the diffuse mips as a reader of 0.26g's cooked layout finds
them: the file's last byte the sRGB flag (read_srgb_flag), the payload before it a chain of block
mips down to 4x4, so the small mips (what a streamed texture shows first, and what a gun in the
lobby is drawn with) taken from the end. On a cooked file that is the mip itself; on ImageMagick's
DDS (mips on to 1x1, no sRGB byte) every small mip is read 15 bytes off its blocks.

    python3 -I mip_preview.py --trees <name>=<tree>[::<model>]... --model <model> --level <n> --out <png>

Each tree: files at their game paths (the gun's model, materials and textures). Needs NumPy,
SciPy and Pillow.
"""
import argparse
import math
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import preview as P  # noqa: E402

M = P.M


def rgb565(c):
    c = c.astype(np.uint32)
    return np.stack([(c >> 11) & 31, (c >> 5) & 63, c & 31], -1) * np.array([255 / 31, 255 / 63, 255 / 31])


def dxt1(data, w, h):
    blocks = np.frombuffer(data, np.uint8)[:max(1, w // 4) * max(1, h // 4) * 8].reshape(-1, 8)
    c0 = blocks[:, 0].astype(np.uint16) | (blocks[:, 1].astype(np.uint16) << 8)
    c1 = blocks[:, 2].astype(np.uint16) | (blocks[:, 3].astype(np.uint16) << 8)
    a, b = rgb565(c0), rgb565(c1)
    four = c0 > c1
    pal = np.stack([a, b, np.where(four[:, None], (2 * a + b) / 3, (a + b) / 2),
                    np.where(four[:, None], (a + 2 * b) / 3, 0)], 1)
    bits = blocks[:, 4:8].astype(np.uint32)
    idx = (bits[:, 0] | bits[:, 1] << 8 | bits[:, 2] << 16 | bits[:, 3] << 24)[:, None] >> (2 * np.arange(16)) & 3
    px = pal[np.arange(len(blocks))[:, None], idx]
    bw, bh = max(1, w // 4), max(1, h // 4)
    img = px.reshape(bh, bw, 4, 4, 3).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 3)
    return np.dstack([img / 255.0, np.ones(img.shape[:2])]).astype(np.float32)


def as_read(path, level):
    data = path.read_bytes()[:-1]
    h, w = struct.unpack_from('<2I', data, 12)
    sizes = []
    while not sizes or (w >> (len(sizes) - 1)) > 4 and (h >> (len(sizes) - 1)) > 4:
        sizes.append(max(1, w >> len(sizes) >> 2) * max(1, h >> len(sizes) >> 2) * 8)
    at = len(data) - sum(sizes[level:])
    return dxt1(data[at:at + sizes[level]], max(4, w >> level), max(4, h >> level))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--trees', nargs='+', required=True)
    ap.add_argument('--model', required=True)
    ap.add_argument('--level', type=int, default=3)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    size = (640, 640)
    tiles = []
    for item in args.trees:
        name, tree = item.split('=', 1)
        tree, _, model = tree.partition('::')
        tree, model = Path(tree), model or args.model
        gun = P.Model(tree, model)
        settings = M.config(gun.dir / 'settings')
        for s, surface in zip(settings['lod_hierrarchy']['LOD0']['surfaces'], gun.surfaces):
            effect = M.config(tree / f'material_instances/{settings["material_settings"][s]["material_name"]}.material')
            surface.diffuse = as_read(tree / f'textures/{effect["material"]["g_stage"]["effect"]["texture_diffuse"]["value"]}.dds',
                                      args.level)
            surface.normal = np.full((1, 1, 4), [0.5, 0.5, 0.5, 0.5], np.float32)
        world = dict(gun.bind)
        eye = np.array([-0.9, 0.5, -0.7])
        target = np.array([0.0, 0.0, 0.2])
        img = P.draw(gun.meshes(world), P.look_at(eye, target, np.array([0.0, 1.0, 0.0])), size, 40.0)
        ImageDraw.Draw(img).text((8, 6), f'{name}: diffuse mip {args.level} as the cooked layout reads it', fill=(235, 235, 235))
        tiles.append(img)
    sheet = Image.new('RGB', (size[0] * len(tiles) + 8 * (len(tiles) - 1), size[1]), (10, 10, 10))
    for i, t in enumerate(tiles):
        sheet.paste(t, (i * (size[0] + 8), 0))
    sheet.save(args.out)
    print('wrote', args.out)


if __name__ == '__main__':
    main()
