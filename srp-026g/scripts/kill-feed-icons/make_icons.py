#!/usr/bin/env python3
"""make_icons.py: the hud movie's two kill feed icons (2026-10-25; ../../docs/notes/kill-feed-icons.md).

usage: python3 -I make_icons.py --hud <hud.swf> --penetration <icon_penetration.png> --out <folder>

--hud          the hud movie the tree builds over (`tools/actionscript/flash_tree.py original
               srp-026g/flash/hud resources.db.patch_22 <hud.swf>`)
--penetration  0.69d0's kill log icon `flash_movies/textures/ui_icons/log/icon_penetration.png`
               (`vostok-port extract`; its archive is the build's resources.43.db)
--out          srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)

CombatLogWallIcon.png is 0.69d0's penetration icon as it is (the retail kill log's mark of a kill
through a wall; the file's sha256 is checked). CombatLogPainIcon.png is the Anesthetic's own picture
(painkiller.options ui_desc.icon [10, 1]: the hud's quick slot atlas `hudItems`, 171x83 cells in
columns, cell 12 by QuickSlots.hudIcons) made a white kill feed icon as the hud's own cause icons are:
cropped to the ampoule, 23 px high (their height), white shaded by the picture's own light (75 % to
100 %), a dark shadow one pixel down and right.

Needs Pillow.
"""
import argparse
import hashlib
from pathlib import Path
import struct
import sys
import zlib

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'tools' / 'actionscript'))
import swf  # noqa: E402

PENETRATION_SHA256 = '97be544bec2a5fa7baa20e9368c346249e3364212467bff939707677805ff1f8'
CELL_W, CELL_H, PAINKILLER_CELL = 171, 83, 12
HEIGHT = 23


def hud_items(movie):
    """The hud's `hudItems` bitmap, RGBA."""
    _, tags = swf.split(movie)
    cid = next(c for c, name in swf.symbol_classes(tags) if name == 'hudItems')
    for kind, raw in tags:
        if kind == swf.DEFINE_BITS_LOSSLESS2 and struct.unpack_from('<H', swf.payload(raw))[0] == cid:
            body = swf.payload(raw)
            _, form, width, height = struct.unpack_from('<HBHH', body)
            if form != 5:
                raise ValueError('hudItems is not a 32-bit bitmap')
            argb = zlib.decompress(body[7:])
            rgba = bytearray()
            for i in range(0, len(argb), 4):
                a, r, g, b = argb[i:i + 4]
                rgba += bytes((r * 255 // a, g * 255 // a, b * 255 // a, a) if a else (0, 0, 0, 0))
            return Image.frombytes('RGBA', (width, height), bytes(rgba))
    raise ValueError('hudItems has no DefineBitsLossless2')


def pain_icon(atlas):
    cell = atlas.crop((0, CELL_H * PAINKILLER_CELL, CELL_W, CELL_H * (PAINKILLER_CELL + 1)))
    cell = cell.crop(cell.getchannel('A').point(lambda a: 255 if a > 24 else 0).getbbox())
    width = max(1, round(cell.width * HEIGHT / cell.height))
    small = cell.convert('RGBa').resize((width, HEIGHT), Image.LANCZOS).convert('RGBA')
    alpha = small.getchannel('A')
    light = small.convert('L')
    lo, hi = light.getextrema()
    shade = light.point(lambda v: round(255 * (0.75 + 0.25 * (v - lo) / max(1, hi - lo))))
    white = Image.merge('RGBA', (shade, shade, shade, alpha))
    out = Image.new('RGBA', (width + 1, HEIGHT + 1), (0, 0, 0, 0))
    shadow = Image.merge('RGBA', [Image.new('L', alpha.size, 40)] * 3 + [alpha.point(lambda a: a * 3 // 5)])
    out.alpha_composite(shadow, (1, 1))
    out.alpha_composite(white, (0, 0))
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--hud', type=Path, required=True)
    parser.add_argument('--penetration', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    wall = args.penetration.read_bytes()
    if hashlib.sha256(wall).hexdigest() != PENETRATION_SHA256:
        sys.exit(f'{args.penetration}: not 0.69d0\'s icon_penetration.png (sha256 {hashlib.sha256(wall).hexdigest()})')
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / 'CombatLogWallIcon.png').write_bytes(wall)
    pain_icon(hud_items(args.hud.read_bytes())).save(args.out / 'CombatLogPainIcon.png', optimize=True)
    print(f'Wrote {args.out}/CombatLogWallIcon.png and CombatLogPainIcon.png')


if __name__ == '__main__':
    main()
