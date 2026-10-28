#!/usr/bin/env python3
"""make_weapon_icons.py: the kill feed's weapon icons past the hud movie's 53 frames (2026-10-26;
../../docs/notes/kill-feed-icons.md "The ported weapons' icons").

usage: python3 -I make_weapon_icons.py --hud-028d0 <hud.swf> --log-031a2 <folder> --out <folder>

--hud-028d0  0.28d0's `flash_movies/hud.swf` (`vostok-port extract` from that build's resources.db)
--log-031a2  a folder with 0.31a2's kill log icons `flash_movies/textures/ui_icons/log/icon_65.png`
             to `icon_69.png` (that build's resources.00.db)
--out        srp-026g/flash/hud/bitmaps (flash_tree.py adds each <class>.png to the movie)

A weapon's kill feed icon is its config's `ui_desc/combat_log_icon`, frame (icon + 1) of the hud's
weapon icon clip. 0.26g's clip has 53 frames (icons 0 to 52); the builds the weapons were ported from
number theirs on: 0.27d0 to 0.31a2 give each ported weapon the same icon. CombatLogWeapon<icon>.png is:
  53 to 63  the bitmap of that frame of 0.28d0's clip (sprite 583, frames 54 to 64), as it is (the
            art of the stock frames, which 0.28d0's clip keeps bit for bit);
  64 to 68  0.31a2's icon_<icon + 1>.png, as it is (0.31a2 loads its kill log icons from textures;
            0.28d0's clip ends at 63).
The sources' sha256 are checked.

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

HUD_028D0 = '291d30067e6b29f74bfe95559868e9b4be2f16239219fc34c45e4b5a04f0a43f'
# icon: the bitmap its frame of 0.28d0's clip shows (DefineBitsLossless2 character id), the weapon
FRAMES_028D0 = {
    53: (561, 'saiga_12'), 54: (563, 'soc_vepr'), 55: (565, 'pkp_pecheneg'), 56: (567, 'vepr_12'),
    57: (569, 'aek_919k'), 58: (571, 'kedr'), 59: (573, 'neostead'), 60: (575, 'fn_fnc'),
    61: (577, 'mp7'), 62: (579, 'mp7_silencer'), 63: (581, 'hk416'),
}
# icon: the sha256 of 0.31a2's icon_<icon + 1>.png, the weapon
LOG_031A2 = {
    64: ('9ffc24bfa74ab0e9ddc0e0b23651fd873f35ec724a96e591a0ad72ddbfd729f5', 'fn_p90'),
    65: ('353f40cf9d35edabdac01de3bca3f754fc43e0627ac510de86181de22c2d972a', 'kriss_vector'),
    66: ('23d1abe7f94511d54df863e0a0a4414865c3572a76ce49a439289c3b65c020b5', 'msbs_carbine'),
    67: ('fbdb781e611c70b2791c0727156b4d604fd40077290fd7a23542bde81d25483b', 'valmet'),
    68: ('ba4a9f248969bd2df0fde653214f73b6479e80e3ebc31ca7001bfeb149dc7f4e', 'vz58'),
}


def lossless2(body):
    """A DefineBitsLossless2 body (8-bit colour-mapped RGBA or 32-bit ARGB, premultiplied) as an
    RGBA image, un-premultiplied so that flash_tree.py's premultiplication gives back the same bytes."""
    _, kind, width, height = struct.unpack_from('<HBHH', body)
    if kind == 3:
        colours = body[7] + 1
        data = zlib.decompress(body[8:])
        table = [data[i * 4:i * 4 + 4] for i in range(colours)]          # R, G, B, A
        stride = (width + 3) // 4 * 4
        pixels = [table[data[colours * 4 + y * stride + x]] for y in range(height) for x in range(width)]
    elif kind == 5:
        data = zlib.decompress(body[7:])
        pixels = [data[i + 1:i + 4] + data[i:i + 1] for i in range(0, width * height * 4, 4)]
    else:
        raise ValueError(f'bitmap format {kind}, not 8-bit colour-mapped or 32-bit ARGB')
    rgba = bytearray()
    for r, g, b, a in pixels:
        rgba += bytes((c * 255 + a - 1) // a if a else 0 for c in (r, g, b)) + bytes((a,))
    return Image.frombytes('RGBA', (width, height), bytes(rgba))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--hud-028d0', type=Path, required=True)
    parser.add_argument('--log-031a2', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    data = args.hud_028d0.read_bytes()
    if hashlib.sha256(data).hexdigest() != HUD_028D0:
        sys.exit(f"{args.hud_028d0}: not 0.28d0's hud.swf (sha256 {hashlib.sha256(data).hexdigest()})")
    bitmaps = {}
    for kind, raw in swf.split(data)[1]:
        if kind == swf.DEFINE_BITS_LOSSLESS2:
            body = swf.payload(raw)
            bitmaps[struct.unpack_from('<H', body)[0]] = body
    icons = {icon: lossless2(bitmaps[cid]) for icon, (cid, _) in FRAMES_028D0.items()}
    logs = {}
    for icon, (digest, _) in LOG_031A2.items():
        path = args.log_031a2 / f'icon_{icon + 1}.png'
        logs[icon] = path.read_bytes()
        if hashlib.sha256(logs[icon]).hexdigest() != digest:
            sys.exit(f"{path}: not 0.31a2's icon_{icon + 1}.png (sha256 {hashlib.sha256(logs[icon]).hexdigest()})")
    args.out.mkdir(parents=True, exist_ok=True)
    for icon, image in icons.items():
        image.save(args.out / f'CombatLogWeapon{icon}.png', optimize=True)
    for icon, png in logs.items():
        (args.out / f'CombatLogWeapon{icon}.png').write_bytes(png)
    print(f'Wrote {args.out}/CombatLogWeapon53.png to CombatLogWeapon68.png')


if __name__ == '__main__':
    main()
