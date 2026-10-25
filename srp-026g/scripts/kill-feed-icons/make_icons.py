#!/usr/bin/env python3
"""make_icons.py: the hud movie's three kill feed icons (2026-10-25; ../../docs/notes/kill-feed-icons.md).

usage: python3 -I make_icons.py --penetration <icon_penetration.png> --ricochet <icon_ricochet.png> --out <folder>

--penetration  0.69d0's kill log icon `flash_movies/textures/ui_icons/log/icon_penetration.png`
--ricochet     0.69d0's kill log icon `flash_movies/textures/ui_icons/log/icon_ricochet.png`
               (`vostok-port extract`; their archive is the 0.69d0 build's resources.43.db)
--out          srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)

CombatLogWallIcon.png is 0.69d0's penetration icon and CombatLogRicochetIcon.png its ricochet icon,
each as it is (the retail kill log's marks of a kill through a wall and by a ricochet; the files'
sha256 are checked). CombatLogPainIcon.png is a lightning bolt for pain shock, drawn here (the
owner's pick, 2026-10-25: neither 0.26g nor retail has a pain or shock icon): white, 23 px high
(the hud's cause icons' height), a dark shadow one pixel down and right, as the hud's own cause
icons are.

Needs Pillow.
"""
import argparse
import hashlib
from pathlib import Path
import sys

from PIL import Image, ImageDraw

RETAIL = (
    ('penetration', 'CombatLogWallIcon', '97be544bec2a5fa7baa20e9368c346249e3364212467bff939707677805ff1f8'),
    ('ricochet', 'CombatLogRicochetIcon', '1abe15b91d360f6bb8c273b1280e26e6b41b5a38ccf34d0313e42dda07251411'),
)
HEIGHT = 23
SCALE = 8                        # drawn at 8x, then reduced


def shadowed(icon):
    """The combat log's look: a dark shadow (40, 40, 40 at 60 %) one pixel down and right."""
    alpha = icon.getchannel('A')
    out = Image.new('RGBA', (icon.width + 1, icon.height + 1), (0, 0, 0, 0))
    shadow = Image.merge('RGBA', [Image.new('L', alpha.size, 40)] * 3 + [alpha.point(lambda a: a * 3 // 5)])
    out.alpha_composite(shadow, (1, 1))
    out.alpha_composite(icon, (0, 0))
    return out


def pain_icon():
    """A lightning bolt, white."""
    h = HEIGHT * SCALE
    big = Image.new('RGBA', (h * 2, h), (0, 0, 0, 0))
    cx, cy = h * 0.6, h / 2
    bolt = [(cx + h * .1, 0), (cx - h * .22, cy + h * .06), (cx - h * .01, cy + h * .06), (cx - h * .14, h),
            (cx + h * .24, cy - h * .1), (cx + h * .03, cy - h * .1), (cx + h * .22, 0)]
    ImageDraw.Draw(big).polygon(bolt, fill=(255, 255, 255, 255))
    big = big.crop(big.getchannel('A').point(lambda a: 255 if a > 16 else 0).getbbox())
    width = max(1, round(big.width * HEIGHT / big.height))
    return shadowed(big.convert('RGBa').resize((width, HEIGHT), Image.LANCZOS).convert('RGBA'))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for option, _, _ in RETAIL:
        parser.add_argument(f'--{option}', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    copies = {}
    for option, name, digest in RETAIL:
        path = getattr(args, option)
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            sys.exit(f"{path}: not 0.69d0's icon_{option}.png (sha256 {hashlib.sha256(data).hexdigest()})")
        copies[name] = data
    args.out.mkdir(parents=True, exist_ok=True)
    for name, data in copies.items():
        (args.out / f'{name}.png').write_bytes(data)
    pain_icon().save(args.out / 'CombatLogPainIcon.png', optimize=True)
    print(f'Wrote {args.out}/: ' + ', '.join(f'{n}.png' for n in [*copies, 'CombatLogPainIcon']))


if __name__ == '__main__':
    main()
