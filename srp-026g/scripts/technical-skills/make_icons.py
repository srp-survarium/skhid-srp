#!/usr/bin/env python3
"""The technical tree's item icons (technical-skills.md "Icons"), in 0.69d0's skill-icon style:
the item's lobby render filling the 77x77 square, a white badge in its bottom-right corner (as
0.69d0's engineering/11-13 and survival/2-5 have theirs).

    python3 -I make_icons.py --ammo <LobbyMenu_AmmoIcons.png> --equipment <LobbyMenu_EquipmentIcons.png>
                             --donor <dir> --out srp-026g/flash/lobby_menu/bitmaps

--ammo and --equipment: the live lobby_menu.swf's atlases (FFDec `-export image`). --donor: 0.69d0's
flash_movies/textures/ui_icons as dbgrab writes them (path '/' -> '__'). Needs Pillow.
"""
import argparse
import math
import os

from PIL import Image, ImageChops, ImageDraw, ImageFilter

SIZE = 77
SS = 4  # glyphs are drawn this many times larger, then scaled down
WHITE = (255, 255, 255, 255)
GREEN = (123, 209, 31, 255)

# The items' places in the atlases (the live movie's), trimmed to their alpha.
AMMO = {
    'smoke_black': (391, 453, 456, 537),  # RDG-2
    'f1': (42, 1089, 112, 1176),
    'trap': (31, 1457, 150, 1537),
}
EQUIPMENT = {'tank': (2223, 1456, 2394, 1539)}  # scav_oxygen_2, the twin cylinders

MODIFIER = 'flash_movies__textures__ui_icons__modifiers__{}.png'
SKULL = 'flash_movies__textures__ui_icons__items__boosters__booster_hero_1_2.png'
HAND = 'flash_movies__textures__ui_icons__skills__utility__14.fw.png'


def trimmed(atlas, box):
    item = atlas.crop(box)
    return item.crop(item.getchannel('A').point(lambda a: 255 if a > 8 else 0).getbbox())


def placed(item, box=70, at=(0, 0)):
    """The item scaled to fit `box`, centred in the square, moved by `at`."""
    scale = min(box / item.width, box / item.height)
    item = item.resize((max(1, round(item.width * scale)), max(1, round(item.height * scale))), Image.LANCZOS)
    out = Image.new('RGBA', (SIZE, SIZE))
    out.alpha_composite(item, ((SIZE - item.width) // 2 + at[0], (SIZE - item.height) // 2 + at[1]))
    return out


def haze(colour, puffs, alpha):
    """A soft cloud of round puffs (x, y, r) behind the item, kept inside the square."""
    mask = Image.new('L', (SIZE, SIZE))
    d = ImageDraw.Draw(mask)
    for x, y, r in puffs:
        d.ellipse((x - r, y - r, x + r, y + r), fill=alpha)
    layer = Image.new('RGBA', (SIZE, SIZE), colour[:3] + (0,))
    layer.putalpha(mask.filter(ImageFilter.GaussianBlur(4)))
    return layer


def big():
    return Image.new('RGBA', (SIZE * SS, SIZE * SS))


# The badge: centre and radius of 0.69d0's circle (engineering/11, survival/5), stroke width.
CX, CY, R, W = 59.5, 59.5, 11.5, 2.4


def circle(d):
    d.ellipse([(CX - R) * SS, (CY - R) * SS, (CX + R) * SS, (CY + R) * SS], outline=WHITE, width=round(W * SS))


def plus(g):
    d = ImageDraw.Draw(g)
    circle(d)
    a, w = 6 * SS, round(W * SS)
    d.line([(CX * SS - a, CY * SS), (CX * SS + a, CY * SS)], fill=WHITE, width=w)
    d.line([(CX * SS, CY * SS - a), (CX * SS, CY * SS + a)], fill=WHITE, width=w)


def stopwatch(g):
    d = ImageDraw.Draw(g)
    circle(d)
    w = round(W * SS)
    d.line([(CX * SS, (CY - R) * SS), (CX * SS, (CY - R - 3) * SS)], fill=WHITE, width=w)
    d.line([((CX - 2.5) * SS, (CY - R - 3.5) * SS), ((CX + 2.5) * SS, (CY - R - 3.5) * SS)], fill=WHITE, width=w)
    d.polygon([((CX - 1.5) * SS, (CY - 1.5) * SS), ((CX + 7) * SS, CY * SS), ((CX - 1.5) * SS, (CY + 1.5) * SS)],
              fill=WHITE)


def bars(g):
    d = ImageDraw.Draw(g)
    circle(d)
    for i in range(4):
        x = CX - 6.5 + i * 3.6
        d.rectangle([x * SS, (CY + 5 - 3 - i * 2.6) * SS, (x + 2.4) * SS, (CY + 5) * SS], fill=GREEN)


def back_arrow(g):
    d = ImageDraw.Draw(g)
    w = round(W * SS)
    box = [(CX - R) * SS, (CY - R) * SS, (CX + R) * SS, (CY + R) * SS]
    d.arc(box, 215, 145 + 360, fill=WHITE, width=w)
    t = math.radians(215)
    px, py = CX + R * math.cos(t), CY + R * math.sin(t)
    ux, uy = math.sin(t), -math.cos(t)  # along the arc, towards smaller angles
    rx, ry = math.cos(t), math.sin(t)
    pts = [(px + ux * 5, py + uy * 5), (px - ux * 1 + rx * 4, py - uy * 1 + ry * 4),
           (px - ux * 1 - rx * 4, py - uy * 1 - ry * 4)]
    d.polygon([(x * SS, y * SS) for x, y in pts], fill=WHITE)


def broken_bone(g):
    """A bone across the badge, broken in the middle."""
    d = ImageDraw.Draw(g)
    a, b = (49.0, 70.0), (70.0, 49.0)
    ux, uy = (b[0] - a[0]) / 29.7, (b[1] - a[1]) / 29.7
    nx, ny = -uy, ux
    d.line([(a[0] * SS, a[1] * SS), (b[0] * SS, b[1] * SS)], fill=WHITE, width=round(3.4 * SS))
    for (x, y), s in ((a, -1), (b, 1)):
        for k in (-1, 1):
            cx, cy, r = x + s * ux * 0.5 + k * nx * 2.4, y + s * uy * 0.5 + k * ny * 2.4, 2.6
            d.ellipse([(cx - r) * SS, (cy - r) * SS, (cx + r) * SS, (cy + r) * SS], fill=WHITE)
    m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
    crack = [(m[0] + nx * 4 + ux * t, m[1] + ny * 4 + uy * t) for t in (-1.2,)] + \
            [(m[0] + nx * k + ux * t, m[1] + ny * k + uy * t) for k, t in ((2, 1.0), (0, -1.0), (-2, 1.0))] + \
            [(m[0] - nx * 4 + ux * t, m[1] - ny * 4 + uy * t) for t in (-1.2,)]
    d.line([(x * SS, y * SS) for x, y in crack], fill=(0, 0, 0, 0), width=round(2.8 * SS))


def glyph(path, height):
    """A 0.69d0 white glyph (a modifier's or a booster's), in the badge's place."""
    def draw(g):
        im = Image.open(path).convert('RGBA')
        im = im.crop(im.getchannel('A').getbbox())
        scale = height * SS / max(im.width, im.height)
        im = im.resize((round(im.width * scale), round(im.height * scale)), Image.LANCZOS)
        g.alpha_composite(im, (round(CX * SS - im.width / 2), round(CY * SS - im.height / 2)))
    return draw


def badge(icon, draw):
    g = big()
    draw(g)
    g = g.resize((SIZE, SIZE), Image.LANCZOS)
    shadow = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
    shadow.putalpha(ImageChops.multiply(g.getchannel('A'), Image.new('L', (SIZE, SIZE), 190))
                    .filter(ImageFilter.GaussianBlur(1.6)))
    out = icon.copy()
    out.alpha_composite(shadow)
    out.alpha_composite(g)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--ammo', required=True)
    ap.add_argument('--equipment', required=True)
    ap.add_argument('--donor', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    ammo = Image.open(args.ammo).convert('RGBA')
    equipment = Image.open(args.equipment).convert('RGBA')
    item = {k: trimmed(ammo, b) for k, b in AMMO.items()} | {k: trimmed(equipment, b) for k, b in EQUIPMENT.items()}
    donor = lambda name: os.path.join(args.donor, name)
    modifier = lambda n, h: glyph(donor(MODIFIER.format(n)), h)

    def on(name, *layers, box=70, at=(-3, -3)):
        out = Image.new('RGBA', (SIZE, SIZE))
        for layer in layers:
            out.alpha_composite(layer)
        out.alpha_composite(placed(item[name], box, at))
        return out

    tank = lambda: on('tank', box=72, at=(-6, -2))
    icons = {
        # Dexterous Hands: 0.69d0's gloved hand (utility/14), faster
        401: badge(Image.open(donor(HAND)).convert('RGBA').resize((SIZE, SIZE), Image.LANCZOS), stopwatch),
        # oxygen tanks
        403: badge(tank(), modifier(7, 24)),          # Steady Breath: a crosshair
        413: badge(tank(), plus),                      # Tank Pocket: one pocket more
        414: badge(tank(), bars),                      # Tank Depth: every pocket bigger
        431: badge(tank(), modifier(10, 25)),         # Deep Breaths: lungs
        442: badge(tank(), modifier(11, 24)),         # Oxygen Rush: a runner against the clock
        # smoke grenades
        402: badge(on('smoke_black', haze((215, 215, 215), ((22, 22, 13), (38, 14, 12), (52, 26, 11), (26, 40, 10)), 150)), stopwatch),  # Dense Smoke
        432: badge(on('smoke_black'), glyph(donor(SKULL), 21)),                               # Dead Man's Switch
        441: badge(on('smoke_black', haze((110, 230, 60), ((22, 24, 13), (38, 14, 12), (54, 28, 12), (24, 44, 11), (46, 46, 10)), 185)), modifier(18, 24)),  # Toxic Smoke
        # grenades
        404: badge(on('f1'), modifier(0, 23)),        # Blast Drill: a shield
        443: badge(on('f1'), back_arrow),            # Scavenger: one comes back
        # traps
        423: badge(on('trap', box=74, at=(-1, -6)), back_arrow),   # Trophy Trap: a disarmed trap comes back
        433: badge(on('trap', box=74, at=(-1, -6)), stopwatch),    # Rearm: open again after 20 s
        444: badge(on('trap', box=74, at=(-1, -6)), broken_bone),  # Bear Trap: the legs break
    }
    for skill, icon in icons.items():
        icon.save(os.path.join(args.out, f'TechnicalSkill069{skill}.png'))
        print(f'TechnicalSkill069{skill}.png')


if __name__ == '__main__':
    main()
