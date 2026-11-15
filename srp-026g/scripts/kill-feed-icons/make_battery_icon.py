#!/usr/bin/env python3
"""make_battery_icon.py: the hud movie's kill feed icon of a battery strike (exe patch 143,
../../docs/notes/battery-hit.md "Kill feed").

usage: python3 make_battery_icon.py --out <folder>

--out  srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)

CombatLogBatteryIcon.png is the battery of the hud's own battery rows (`CombatLogItem`'s found, lost
and bring frames: the bitmaps of sprites 702, 705 and 708, a 12 x 9 px box with two dark cells beside
the arrow or tick), drawn at twice its size in the weapon's place (24 x 18 px, the weapon icons are
15 to 26 px high): white, a dark shadow one pixel down and right, as make_icons.py's.

Needs Pillow.
"""
import argparse
from pathlib import Path

from PIL import Image, ImageDraw

from make_icons import shadowed

# The hud's battery, one character a pixel: the box and its two cells.
BATTERY = (
    '############',
    '############',
    '##  ####  ##',
    '##  ####  ##',
    '##  ####  ##',
    '##  ####  ##',
    '##  ####  ##',
    '############',
    '############',
)
SCALE = 2


def battery_icon():
    icon = Image.new('RGBA', (len(BATTERY[0]) * SCALE, len(BATTERY) * SCALE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(icon)
    for y, row in enumerate(BATTERY):
        for x, pixel in enumerate(row):
            if pixel == '#':
                draw.rectangle((x * SCALE, y * SCALE, x * SCALE + SCALE - 1, y * SCALE + SCALE - 1),
                               fill=(255, 255, 255, 255))
    return shadowed(icon)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    battery_icon().save(args.out / 'CombatLogBatteryIcon.png', optimize=True)


if __name__ == '__main__':
    main()
