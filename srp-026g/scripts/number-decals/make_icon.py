#!/usr/bin/env python3
"""make_icon.py: the number decals' one lobby icon, a "№" in the stencil's white
(docs/notes/number-decals.md; the owner, 2026-11-26: one shared icon, the number in the name).

usage: python3 -I make_icon.py --font <FreeMonoBold.ttf> --out icon.png
       (needs numpy, scipy and Pillow; the font is GNU FreeFont's FreeMono Bold, 20120503)

The owner's art has digits only, so the sign is FreeMono Bold's (a typewriter face, as the
digits are), drawn large, its edge roughened as the stencil's are (a smooth noise moves the
outline by a few pixels; seeded, so the icon is the same every time), then fitted in the decal
atlas's 82x82 cell with 4 pixels round it. The result is committed (icon.png): make_pack.py
reads it, not the font.
"""
import argparse
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont
from scipy import ndimage

CELL = 82
PAD = 4
DRAW = 1024          # the sign drawn this tall before it is roughened and scaled down
ROUGH = 4.0          # how far, in pixels of the large drawing, the noise moves the edge
SEED = 1000


def sign(font_path):
    font = ImageFont.truetype(str(font_path), DRAW)
    canvas = Image.new('L', (DRAW * 2, DRAW * 2), 0)
    ImageDraw.Draw(canvas).text((DRAW // 4, DRAW // 4), '№', font=font, fill=255)
    alpha = np.asarray(canvas, dtype=np.float64) / 255
    # Signed distance to the outline, moved by smooth noise: a ragged, inked edge.
    inside = alpha >= 0.5
    distance = ndimage.distance_transform_edt(inside) - ndimage.distance_transform_edt(~inside)
    noise = ndimage.gaussian_filter(np.random.default_rng(SEED).standard_normal(alpha.shape), 10)
    noise *= ROUGH / noise.std()
    rough = np.clip(distance + noise + 0.5, 0, 1)
    image = Image.fromarray(np.round(rough * 255).astype(np.uint8), 'L')
    return image.crop(image.getbbox())


def icon(font_path):
    mask = sign(font_path)
    box = CELL - 2 * PAD
    scale = box / max(mask.size)
    w, h = (max(1, round(d * scale)) for d in mask.size)
    mask = mask.resize((w, h), Image.Resampling.LANCZOS)
    out = Image.new('RGBA', (CELL, CELL), (255, 255, 255, 0))
    white = Image.new('RGBA', (w, h), (255, 255, 255, 255))
    white.putalpha(mask)
    out.paste(white, ((CELL - w) // 2, (CELL - h) // 2))
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--font', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    icon(args.font).save(args.out, format='PNG')


if __name__ == '__main__':
    main()
