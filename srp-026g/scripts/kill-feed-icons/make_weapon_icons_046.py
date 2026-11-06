#!/usr/bin/env python3
"""make_weapon_icons_046.py: the kill feed's icons of the weapons ported from 0.46c2 (pack weapons-046;
../../docs/notes/weapons-046.md, ../../docs/notes/kill-feed-icons.md).

usage: python3 -I make_weapon_icons_046.py --log-046c2 <folder> --log-069d0 <folder> --cells <folder>
                                           --out <hud folder>

--log-046c2  a folder with 0.46c2's kill log icons `flash_movies/textures/ui_icons/log/icon_<n>.png`
--log-069d0  the same of 0.69d0 (its icon_96.png: 0.46c2 gives the Glock 17 the APS's icon)
--cells      the pack's lobby cells (its icons/ folder, LobbyMenu_WeaponIcons-r<row>c<col>.png): the
             Colt M1911's icon 99 is drawn from its cell, as no build has one (0.46c2 gives it the PM's
             16, 0.69d0 the CZ 75's 82)
--out        srp-026g/flash/hud: writes bitmaps/CombatLogWeapon<icon>.png and src/CombatLogWeapon<icon>.as

A weapon's kill feed icon is its config's `ui_desc/combat_log_icon`; past the hud clip's 53 frames the
hud shows the bitmap class CombatLogWeapon<icon> (CombatLogItem.moreWeapon). 0.46c2 numbers its
weapons' icons on from 0.31a2's and loads icon n from icon_<n + 1>.png; 0.69d0 keeps those numbers.
Each icon is the build's own image, its pixels as they are (RGBA; the donors' PNGs are colour-mapped).
The sources' sha256 are checked.

Needs Pillow.
"""
import argparse
import hashlib
from pathlib import Path
import sys

from PIL import Image

# icon: (build, the sha256 of its icon_<icon + 1>.png, the weapons)
ICONS = {
    75: ('046c2', '50c0d44d754ba5c5dff849cc8ef192e78c98ac8eca6333cc316dc2676ba1ff94', 'svu_a'),
    77: ('046c2', 'e3ac9e0ac19c421f9e69687fbde899bb22fbb7ce4f3ebe28fd46e3770c8db7b5', 'mp5a1'),
    78: ('046c2', '74298a72dc5d3519f12363ec804f1e86fdff745645443c3b173395d63c72bcd6', 'jackhammer'),
    79: ('046c2', '344b13ad8157408c3034535d0e4738bc803f98c688e62db0cd9d359dbe9860ce', 'fort_401'),
    80: ('046c2', '07846d44c867abdeb8555b7652af69048cac2e7eaac7c6cecdd0205627a2f88e', 'l85a2'),
    82: ('046c2', 'ad7ad4790926ff2e831ee96afc9ac92b96380b2e60cf6b5d6874384e729e9dab', 'cz_75'),
    83: ('046c2', 'eadeed3cdad0847fe042390020958cd15664bbdedfd876d7579346819f7bced4', 'mp443'),
    84: ('046c2', '0cea844948bb69d8f2559429d0a495a1788465ca0abc6ea4d862427397a59aac', 'beretta93r'),
    95: ('069d0', '6fb32640e9d4a3c96b61316095a208f50fb5adf5da26ca76a487d89615c5f872', 'glock_17'),
    99: ('cell', 'a5a3346e14f9394d1dc627e9af72345b4e61ceffc10c257de49291fb3902b9b0', 'colt_m1911'),
}

# The cell an icon is drawn from (build 'cell').
CELLS = {99: 'LobbyMenu_WeaponIcons-r10c06.png'}
# A kill feed icon's size (the hud's 0.46c2 pistols').
ICON_SIZE = (21, 15)


def from_cell(cell):
    """A kill feed icon from a lobby cell: the gun cropped to its pixels, fitted into ICON_SIZE and
    lightened to the near-white the donors' icons are (grey = 175 + 1.2 x luminance)."""
    cell = cell.convert('RGBA')
    gun = cell.crop(cell.getchannel('A').point(lambda a: 255 if a > 40 else 0).getbbox())
    w, h = ICON_SIZE
    scale = min(w / gun.width, h / gun.height)
    size = (max(1, round(gun.width * scale)), max(1, round(gun.height * scale)))
    gun = gun.resize(size, Image.LANCZOS)
    icon = Image.new('RGBA', ICON_SIZE, (0, 0, 0, 0))
    icon.paste(gun, ((w - size[0]) // 2, (h - size[1]) // 2))
    px = icon.load()
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            v = min(255, int(175 + 1.2 * (r * 299 + g * 587 + b * 114) // 1000))
            px[x, y] = (v, v, v, a)
    return icon

CLASS = '''package
{{
   import flash.display.BitmapData;

   // The kill feed's weapon icon {icon} (ui_desc/combat_log_icon), the {weapons}: {source} (bitmaps/, which the build adds).
   public dynamic class CombatLogWeapon{icon} extends BitmapData
   {{

      public function CombatLogWeapon{icon}(w:int = {width}, h:int = {height})
      {{
         super(w,h);
      }}
   }}
}}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--log-046c2', type=Path, required=True)
    parser.add_argument('--log-069d0', type=Path, required=True)
    parser.add_argument('--cells', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--print-sums', action='store_true', help='print the sources\' sha256 and stop')
    args = parser.parse_args()
    folders = {'046c2': args.log_046c2, '069d0': args.log_069d0, 'cell': args.cells}
    for icon, (build, digest, weapons) in ICONS.items():
        name = CELLS[icon] if build == 'cell' else f'icon_{icon + 1}.png'
        path = folders[build] / name
        data = path.read_bytes()
        got = hashlib.sha256(data).hexdigest()
        if args.print_sums:
            print(f'{icon}\t{build}\t{got}\t{weapons}')
            continue
        if got != digest:
            sys.exit(f"{path}: not {build}'s {name} (sha256 {got})")
        if build == 'cell':
            image = from_cell(Image.open(path))
            source = f"drawn from its lobby cell {name} (pack weapons-046)"
        else:
            image = Image.open(path).convert('RGBA')
            source = f"{build}'s {name}"
        image.save(args.out / 'bitmaps' / f'CombatLogWeapon{icon}.png', optimize=True)
        (args.out / 'src' / f'CombatLogWeapon{icon}.as').write_text(CLASS.format(
            icon=icon, weapons=weapons, source=source, width=image.width, height=image.height))
    if not args.print_sums:
        print(f'Wrote {len(ICONS)} icons to {args.out}/bitmaps and their classes to {args.out}/src')


if __name__ == '__main__':
    main()
