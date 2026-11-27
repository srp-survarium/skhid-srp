#!/usr/bin/env python3
"""The medical tree's drug icons (medical-skills.md "Icons"), the technical tree's way
(scripts/technical-skills/make_icons.py, whose functions this uses): the item's lobby render filling
the 77x77 square, a white badge in its bottom-right corner. Each painkiller skill shows the
painkiller, each antitoxin skill the antitoxin, its badge telling them apart; Additional Medicine
shows medkits and a field dressing.

    python3 -I make_icons.py --ammo <LobbyMenu_AmmoIcons.png> --donor <dir> --out srp-026g/flash/lobby_menu/bitmaps

--ammo: the live lobby_menu.swf's atlas (FFDec `-export image`). --donor: 0.69d0's
flash_movies/textures/ui_icons as dbgrab writes them (path '/' -> '__'). Needs Pillow.
"""
import argparse
import importlib.util
import os

from PIL import Image, ImageFilter

spec = importlib.util.spec_from_file_location(
    'technical_icons', os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'technical-skills', 'make_icons.py'))
tech = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tech)
SIZE = tech.SIZE

# The items' places in the atlas (the live movie's; their configs' ui_desc/icon [row, column]),
# trimmed to their alpha.
AMMO = {
    'painkiller': (0, 823, 171, 898),     # gameplay/items/drugs/painkiller.options [10, 1], the ampoule
    'antitoxin': (0, 1374, 171, 1437),    # antitoxin.options [16, 1], the syringe
    'medikit': (0, 1276, 171, 1354),      # the Medikit [15, 1], orange
    'blue': (342, 1367, 513, 1445),       # military_medkit [16, 3]
    'bandage': (171, 97, 342, 168),       # bandages [2, 2], the field dressing
}


def shadowed(item, angle):
    """An item turned by `angle`, over its own soft shadow (to part the items of a pile)."""
    if angle:
        item = item.rotate(angle, Image.BICUBIC, expand=True)
    pad = Image.new('RGBA', (item.width + 8, item.height + 8))
    pad.alpha_composite(item, (4, 4))
    shadow = Image.new('RGBA', pad.size, (0, 0, 0, 0))
    shadow.putalpha(pad.getchannel('A').point(lambda a: a * 170 // 255).filter(ImageFilter.GaussianBlur(1.6)))
    shadow.alpha_composite(pad)
    return shadow


def pile(*layers):
    """Items (image, width, centre x, centre y, angle), back to front."""
    out = Image.new('RGBA', (SIZE, SIZE))
    for item, width, x, y, angle in layers:
        scale = width / item.width
        item = shadowed(item.resize((round(item.width * scale), round(item.height * scale)), Image.LANCZOS), angle)
        out.alpha_composite(item, (round(x - item.width / 2), round(y - item.height / 2)))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--ammo', required=True)
    ap.add_argument('--donor', required=True)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    ammo = Image.open(args.ammo).convert('RGBA')
    item = {k: tech.trimmed(ammo, b) for k, b in AMMO.items()}
    modifier = lambda n, h: tech.glyph(os.path.join(args.donor, tech.MODIFIER.format(n)), h)  # noqa: E731

    painkiller = lambda: tech.placed(item['painkiller'], 70, (-3, -3))  # noqa: E731
    # The syringe is long and thin: along the diagonal, clear of the badge.
    syringe = item['antitoxin'].rotate(32, Image.BICUBIC, expand=True)
    syringe = syringe.crop(syringe.getchannel('A').getbbox())
    antitoxin = lambda: tech.placed(syringe, 82, (-5, -5))  # noqa: E731
    icons = {
        # painkiller
        301: tech.badge(painkiller(), tech.plus),            # Potent Painkiller: more protection, longer
        303: tech.badge(painkiller(), modifier(11, 24)),     # Numb Legs: a runner, sprint stamina
        323: tech.badge(painkiller(), modifier(0, 23)),      # Unflinching: a shield, hits' stopping power
        # antitoxin
        314: tech.badge(antitoxin(), modifier(7, 24)),       # Sharp Eye: a crosshair, aim time
        324: tech.badge(antitoxin(), modifier(10, 25)),      # Second Wind: lungs, stamina
        333: tech.badge(antitoxin(), tech.stopwatch),        # Concentration: reload speed
        # Additional Medicine: two medkits fanned, a field dressing in front (the owner's pick, 2026-11-22)
        322: pile((item['blue'], 54, 26, 30, 18), (item['medikit'], 54, 50, 30, -18), (item['bandage'], 46, 38, 54, 0)),
    }
    for skill, icon in icons.items():
        icon.save(os.path.join(args.out, f'MedicalSkill069{skill}.png'))
        print(f'MedicalSkill069{skill}.png')


if __name__ == '__main__':
    main()
