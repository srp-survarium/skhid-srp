#!/usr/bin/env python3
"""Bind 0.69's original range pictures to the 0.26 movie sources.

Usage: ui.py EXTRACTED_DIRECTORY
The directory contains donor-minimap.png and donor-loading.png.
The minimap's authored frame is centred at (-38, 4), with half extents
(100, 90) metres. Pad its vertical extent to 100 for the square 0.26 frame;
do not stretch the map's world coordinates to a square.
"""
import hashlib
from pathlib import Path
import sys

from PIL import Image

FLASH = Path(__file__).resolve().parents[2] / 'flash'
SOURCES = {
    'minimap': '047754d7aa2718e37286cbd83b89f371c8fefdea7c59bba2e12d9a6016387761',
    'loading': '3379dbf44f4899a4228f43db46664ac6e6be09d8b7a439255a17eccd2fc4f02a',
}


def main():
    root = Path(sys.argv[1])
    for name, expected in SOURCES.items():
        path = root / f'donor-{name}.png'
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise SystemExit(f'{path}: not the reviewed 0.69 source')
    minimap = Image.open(root / 'donor-minimap.png').convert('RGBA')
    if minimap.size != (1024, 1024):
        raise SystemExit('unexpected donor minimap dimensions')
    # Output pixel -> source pixel, preserving the authored centre and X/Z
    # distances. Transparent padding adds 10 m at each end of the Z frame.
    minimap.transform(
        (512, 512), Image.Transform.AFFINE,
        (2, 0, 0, 0, 2 / 0.9, 512 * (1 - 1 / 0.9)),
        resample=Image.Resampling.BICUBIC, fillcolor=(0, 0, 0, 0),
    ).save(FLASH / 'hud/bitmaps/minimap_shooting_range.png')
    loading = root / 'donor-loading.png'
    if Image.open(loading).size != (1920, 798):
        raise SystemExit('unexpected donor loading-picture dimensions')
    # The donor is palette-indexed; the source movie compiler accepts RGB(A).
    Image.open(loading).convert('RGB').save(FLASH / 'lobby_menu/bitmaps/Level_shooting_range.png')


if __name__ == '__main__':
    main()
