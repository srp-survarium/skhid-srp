#!/usr/bin/env python3
"""make_pack.py: the content pack of the corrected Flag of Serbia decal (2026-10-29;
../../docs/notes/serbia-flag.md).

usage: python3 -I make_pack.py --stock <folder> --out <packs folder>/serbia-flag-<version>

--stock  the stock resources.db's files at their game paths (no Skhid patch carries them;
         `srp-game-data get --db <resources.db> --out <folder> <path>`): TEXTURE.
--out    the pack's folder (a new one): pack.json, files/ and icons/, for `vostok-port pack` and
         the manifest's `pack` row (release/content.tsv).

The stock decal `char_decal_4_34` ("Flag Of Serbia", texture character/human/decal/4_34, lobby icon
[4, 34] of LobbyMenu_DecalsIcons) is blue over white over red: the flag of Yugoslavia (and of Serbia
and Montenegro), not Serbia's, which is red over blue over white.

- The texture (DXT5, 512x512, 8 mips) is recoloured in its blocks by one affine colour map that
  takes the art's blue to its red, the white to the blue, the red to the white and the brown-grey
  grime around the paint to itself. A block's colours are its two endpoints and fixed blends of
  them, and an affine map commutes with blending, so mapping the two endpoints (RGB565) maps every
  texel of every mip, and worn paint (a blend of paint and grime) becomes the new paint with the
  same wear. The alpha blocks and the file's layout stay byte for byte.
- The icon is that texture at the atlas's 82x82 (each pixel the area average of the texels it
  covers, in premultiplied alpha): the stock icon is the stock texture at that size, its flag in
  the same place. It replaces the stock icon in its cell (`replaces_sha256`: the porter checks that
  the cell holds the stock icon), so the item's config stays as it is. The stock icon's own wear is
  not kept: its red stripe is worn through to transparency, which on a white stripe reads as grey.
"""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

PACK_ID = 'serbia-flag'
VERSION = '1'
TEXTURE = 'textures/character/human/decal/4_34.dds'
TEXTURE_SHA256 = 'd28572bc9504b3489dc316891d03ffcd1ffcc6a5b0edf440ac7d27ed770ca90a'
MOVIE = 'flash_movies/lobby_menu.swf'
SYMBOL = 'LobbyMenu_DecalsIcons'
CELL = 82                       # the decal atlas's square cells
ICON = (4, 34)                  # ui_desc/icon of char_decal_4_34: [row, col], 1-based
ICON_SHA256 = 'ada4fcf32ca6df220036c78d17b4423f794dafbd26661ec05b92c10fc219f95d'   # the stock cell's ARGB
ICON_SOURCE = 'icons/LobbyMenu_DecalsIcons/r04c34.png'

# The texture's colours (its block endpoints): the stripes and the grime the map keeps.
COLOURS = {'blue': (0, 56, 131), 'white': (255, 255, 255), 'red': (222, 36, 24)}
GRIME = (90, 80, 74)
SERBIA = {'blue': 'red', 'white': 'blue', 'red': 'white'}   # stock stripe -> its new colour


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def solve(rows, rhs):
    """x of rows x = rhs (exact, Gauss-Jordan over fractions)."""
    n = len(rows)
    m = [[Fraction(v) for v in row] + [Fraction(b)] for row, b in zip(rows, rhs)]
    for i in range(n):
        pivot = next(k for k in range(i, n) if m[k][i] != 0)
        m[i], m[pivot] = m[pivot], m[i]
        m[i] = [v / m[i][i] for v in m[i]]
        for k in range(n):
            if k != i and m[k][i] != 0:
                m[k] = [a - m[k][i] * b for a, b in zip(m[k], m[i])]
    return [row[n] for row in m]


def colour_map():
    """The affine map taking each stripe's colour to its new stripe's and GRIME to itself."""
    src = [COLOURS[s] for s in SERBIA] + [GRIME]
    dst = [COLOURS[SERBIA[s]] for s in SERBIA] + [GRIME]
    rows = [list(c) + [1] for c in src]
    coeffs = [solve(rows, [d[k] for d in dst]) for k in range(3)]

    def apply(rgb):
        return tuple(min(255, max(0, round(sum(c * v for c, v in zip(row, list(rgb) + [1])))))
                     for row in coeffs)
    for s, d in zip(src, dst):
        assert apply(s) == d
    return apply


def rgb565(v):
    r, g, b = v >> 11 & 31, v >> 5 & 63, v & 31
    return (r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2)


def to565(rgb):
    r, g, b = rgb
    return round(r * 31 / 255) << 11 | round(g * 63 / 255) << 5 | round(b * 31 / 255)


def dxt5_header(dds):
    assert dds[:4] == b'DDS ' and dds[84:88] == b'DXT5', 'not a DXT5 DDS'
    height, width = struct.unpack_from('<II', dds, 12)
    return width, height, struct.unpack_from('<I', dds, 28)[0]


def recolour_dxt5(dds, apply):
    """The DDS with every colour block's endpoints mapped; everything else as it is."""
    width, height, mips = dxt5_header(dds)
    out = bytearray(dds)
    at, w, h = 128, width, height
    for _ in range(mips):
        for _ in range(max(1, (w + 3) // 4) * max(1, (h + 3) // 4)):
            c0, c1 = struct.unpack_from('<HH', dds, at + 8)
            struct.pack_into('<HH', out, at + 8, to565(apply(rgb565(c0))), to565(apply(rgb565(c1))))
            at += 16
        w, h = max(1, w // 2), max(1, h // 2)
    assert at <= len(dds)   # the stock file has one byte past its last mip; it stays
    return bytes(out)


def decode_dxt5(dds):
    """(width, height, RGBA rows) of the DDS's first mip."""
    width, height, _ = dxt5_header(dds)
    rgba = [[None] * width for _ in range(height)]
    at = 128
    for by in range(0, height, 4):
        for bx in range(0, width, 4):
            a0, a1 = dds[at], dds[at + 1]
            alphas = [a0, a1] + ([((7 - k) * a0 + k * a1) // 7 for k in range(1, 7)] if a0 > a1 else
                                 [((5 - k) * a0 + k * a1) // 5 for k in range(1, 5)] + [0, 255])
            abits = int.from_bytes(dds[at + 2:at + 8], 'little')
            c0, c1 = (rgb565(v) for v in struct.unpack_from('<HH', dds, at + 8))
            colours = [c0, c1, tuple((2 * p + q) // 3 for p, q in zip(c0, c1)),
                       tuple((p + 2 * q) // 3 for p, q in zip(c0, c1))]
            cbits = struct.unpack_from('<I', dds, at + 12)[0]
            for k in range(16):
                rgba[by + k // 4][bx + k % 4] = colours[cbits >> 2 * k & 3] + (alphas[abits >> 3 * k & 7],)
            at += 16
    return width, height, rgba


def icon_argb(dds):
    """The texture's first mip at CELL x CELL, premultiplied ARGB (area average)."""
    width, height, rgba = decode_dxt5(dds)
    assert width == height
    scale = Fraction(width, CELL)
    spans = []   # per output pixel: the texels it covers and their weights
    for o in range(CELL):
        lo, hi = o * scale, (o + 1) * scale
        spans.append([(t, min(hi, t + 1) - max(lo, t)) for t in range(int(lo), min(width, -(-hi // 1)))])
    area = scale * scale
    out = bytearray()
    for oy in range(CELL):
        for ox in range(CELL):
            acc = [Fraction(0)] * 4
            for y, wy in spans[oy]:
                for x, wx in spans[ox]:
                    r, g, b, a = rgba[y][x]
                    w = wy * wx
                    acc[0] += w * a
                    for k, v in enumerate((r, g, b), 1):
                        acc[k] += w * a * v / 255
            out += bytes(min(255, round(v / area)) for v in acc)
    return bytes(out)


def png(argb):
    """An RGBA PNG of a cell's premultiplied pixels as they are (read with premultiplied: true)."""
    raw = b''.join(b'\0' + b''.join(argb[o + 1:o + 4] + argb[o:o + 1]
                                    for o in range(line * CELL * 4, (line + 1) * CELL * 4, 4))
                   for line in range(CELL))

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', CELL, CELL, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stock', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: exists; the pack goes in a new folder')

    texture = (args.stock / TEXTURE).read_bytes()
    if sha256(texture) != TEXTURE_SHA256:
        sys.exit(f'--stock: {TEXTURE} is not the stock texture (sha256 {TEXTURE_SHA256})')
    serbia = recolour_dxt5(texture, colour_map())
    files = {'files/' + TEXTURE: serbia, ICON_SOURCE: png(icon_argb(serbia))}
    pack = {
        'format_version': 1,
        'id': PACK_ID,
        'version': VERSION,
        'target_build': '0.26g',
        'description': 'the Flag of Serbia decal (char_decal_4_34) red over blue over white, its texture '
                       'recoloured and its lobby icon drawn from it (stock: blue over white over red, the '
                       'flag of Yugoslavia)',
        'operations': [
            {'op': 'replace_file', 'path': TEXTURE, 'base_sha256': TEXTURE_SHA256},
            {'op': 'atlas_icons_add', 'swf': MOVIE, 'symbol': SYMBOL,
             'icons': [{'row': ICON[0], 'col': ICON[1], 'source': ICON_SOURCE, 'premultiplied': True,
                        'replaces_sha256': ICON_SHA256}]},
        ],
        'notes': ['made by srp-026g/scripts/serbia-flag/make_pack.py from the stock texture'],
    }
    for path, data in files.items():
        target = args.out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: the texture and the icon, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


if __name__ == '__main__':
    main()
