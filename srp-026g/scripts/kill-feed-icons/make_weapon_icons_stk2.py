#!/usr/bin/env python3
"""make_weapon_icons_stk2.py: the kill feed's icons of the two STK2 AK-74s (pack weapons-stk2;
../../docs/notes/weapons-stk2.md, ../../docs/notes/kill-feed-icons.md).

usage: python3 -I make_weapon_icons_stk2.py --cells <folder> --out <hud folder>

--cells  resource-porter's ports/icons/stk2 (the guns' lobby icons, ../weapons-stk2/icons.py)
--out    srp-026g/flash/hud: writes bitmaps/CombatLogWeapon<icon>.png and src/CombatLogWeapon<icon>.as

No build has a kill log icon of these guns: each is drawn from its lobby cell as the Colt M1911's
99 (make_weapon_icons_046.py's from_cell), fitted into the 0.69d0 rifles' 50 x 21. Then rerun
make_lobby_weapon_icons.py for the lobby's tooltips. The sources' sha256 are checked.

Needs Pillow.
"""
import argparse
import hashlib
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_weapon_icons_046 as m046  # noqa: E402

# icon: (its cell image, its sha256, the weapon)
ICONS = {
    100: ('ak74_tactical.png', 'f8a071b59d177efe5dbb70c868e874e0c786fd5d0b071b9c26a19e94dbfb8bd0', 'ak74_tactical'),
    101: ('ak74_tactical_carbine.png', '486f2561678e6ecb73993c8601cffbd2e8296700eafa0449cdb0675c041cb821', 'ak74_tactical_carbine'),
}
m046.ICON_SIZE = (50, 21)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--cells', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--print-sums', action='store_true', help='print the sources\' sha256 and stop')
    args = parser.parse_args()
    for icon, (name, digest, weapon) in ICONS.items():
        data = (args.cells / name).read_bytes()
        got = hashlib.sha256(data).hexdigest()
        if args.print_sums:
            print(f'{icon}\t{got}\t{weapon}')
            continue
        if got != digest:
            sys.exit(f'{args.cells / name}: not the cell this was made from (sha256 {got})')
        image = m046.from_cell(Image.open(args.cells / name))
        image.save(args.out / 'bitmaps' / f'CombatLogWeapon{icon}.png', optimize=True)
        (args.out / 'src' / f'CombatLogWeapon{icon}.as').write_text(m046.CLASS.format(
            icon=icon, weapons=weapon, source=f'drawn from its lobby cell {name} (pack weapons-stk2)',
            width=image.width, height=image.height))
    if not args.print_sums:
        print(f'Wrote {len(ICONS)} icons to {args.out}/bitmaps and their classes to {args.out}/src')


if __name__ == '__main__':
    main()
