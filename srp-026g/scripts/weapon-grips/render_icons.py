#!/usr/bin/env python3
"""render_icons.py: the grips' lobby icons, drawn from their models as the muzzle devices' are.

usage: python3 render_icons.py --donor <donor.sh's folder> --modules <the weapon-modules pack's folder>
                               --inputs <../muzzle-devices/prepare.sh's folder> --out <new folder>

Needs NumPy and Pillow. For each grip of grips.py: its model of the modules pack drawn by
../muzzle-devices/render_icons.py (icon_of: the same view, brightness and shadow as the devices'),
<module>.png for make_pack.py --icons, and sheet.png (every icon with its dict id, to look at).
0.69d0's own pictures of the grips are not taken: they are drawn in its blue-lit style.
"""
import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'muzzle-devices'))
import grips as lib  # noqa: E402
import render_icons as devices  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--donor', type=Path, required=True)
    ap.add_argument('--modules', type=Path, required=True)
    ap.add_argument('--inputs', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    args.out.mkdir(parents=True)
    target = devices.stock_brightness(args.inputs)
    grips = lib.grips(args.donor)
    W, H = devices.R.CELL_W, devices.R.CELL_H
    sheet = Image.new('RGBA', (5 * W, -(-len(grips) // 5) * (H + 14)), (34, 36, 40, 255))
    draw = ImageDraw.Draw(sheet)
    for i, grip in enumerate(grips):
        icon = devices.icon_of(args.modules, grip.model, target)
        icon.save(args.out / f'{grip.module}.png')
        x, y = i % 5 * W, i // 5 * (H + 14)
        sheet.alpha_composite(icon, (x, y))
        draw.text((x + 4, y + H), f'{grip.dict_id} {grip.module}', fill=(200, 200, 200, 255))
        print(f'{grip.module}: brightness {devices.R.mean_luma(np.asarray(icon)):.1f} (the stock scopes\' {target:.1f})')
    sheet.save(args.out / 'sheet.png')


if __name__ == '__main__':
    main()
