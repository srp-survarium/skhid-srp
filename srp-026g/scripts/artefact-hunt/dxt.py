"""dxt.py: a DXT1 texture as a DXT5 one whose alpha is its colour (make_pack.py).

0.46c2's particle materials with `use_ttransparency_dxt1` take their transparency from a DXT1
texture's colour (grey masks: the channels differ by at most 16 of 255); 0.26g's forward material
reads it from the alpha of a DXT5 texture (its own fx/artifacts/rays, star_trail, stroke_double), so
a DXT1 texture there is opaque everywhere and every particle a solid sheet (the owner's test,
2026-11-21). Each block of each mip level keeps its colour (a three-colour block put in the
four-colour form DXT5's colour block always has) and gets an alpha block of its grey, made linear
as the game reads the sRGB colour it stood for (alpha is never converted). Plain Python.
"""
import struct

HEADER = 128


def rgb565(c):
    return ((c >> 11) & 31) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31


def linear(v):
    s = v / 255
    return round(255 * (s / 12.92 if s <= 0.04045 else ((s + 0.055) / 1.055) ** 2.4))


def colour_block(c0, c1, indices):
    """The block's 16 colours and its colour block in four-colour form."""
    a, b = rgb565(c0), rgb565(c1)
    if c0 > c1:
        palette = [a, b, tuple((2 * x + y) // 3 for x, y in zip(a, b)), tuple((x + 2 * y) // 3 for x, y in zip(a, b))]
        return [palette[(indices >> (2 * i)) & 3] for i in range(16)], struct.pack('<HHI', c0, c1, indices)
    palette = [a, b, tuple((x + y) // 2 for x, y in zip(a, b)), (0, 0, 0)]
    pixels = [palette[(indices >> (2 * i)) & 3] for i in range(16)]
    if c0 == c1:
        return pixels, struct.pack('<HHI', c0, c1, 0)
    # four-colour form: the endpoints swapped (c1 > c0), each pixel its nearest of the four
    hi, lo = rgb565(c1), rgb565(c0)
    four = [hi, lo, tuple((2 * x + y) // 3 for x, y in zip(hi, lo)), tuple((x + 2 * y) // 3 for x, y in zip(hi, lo))]
    out = 0
    for i, px in enumerate(pixels):
        best = min(range(4), key=lambda k: sum((p - q) ** 2 for p, q in zip(px, four[k])))
        out |= best << (2 * i)
    return pixels, struct.pack('<HHI', c1, c0, out)


def alpha_block(values):
    hi, lo = max(values), min(values)
    if hi == lo:
        return struct.pack('<BB', hi, lo) + bytes(6)
    palette = [hi, lo] + [((7 - k) * hi + k * lo) // 7 for k in range(1, 7)]
    bits = 0
    for i, v in enumerate(values):
        bits |= min(range(8), key=lambda k: abs(v - palette[k])) << (3 * i)
    return struct.pack('<BB', hi, lo) + bits.to_bytes(6, 'little')


def dxt1_to_dxt5(data):
    if data[:4] != b'DDS ' or data[84:88] != b'DXT1':
        raise ValueError('not a DXT1 DDS')
    height, width = struct.unpack_from('<II', data, 12)
    mips = max(1, struct.unpack_from('<I', data, 28)[0])
    out, at = bytearray(), HEADER
    for level in range(mips):
        w, h = max(1, width >> level), max(1, height >> level)
        for _ in range(max(1, (w + 3) // 4) * max(1, (h + 3) // 4)):
            c0, c1, indices = struct.unpack_from('<HHI', data, at)
            at += 8
            pixels, colour = colour_block(c0, c1, indices)
            out += alpha_block([linear(px[1]) for px in pixels]) + colour
    if len(data) - at > 1:
        raise ValueError(f'{len(data) - at} bytes after the mip levels')
    header = bytearray(data[:HEADER])
    header[84:88] = b'DXT5'
    struct.pack_into('<I', header, 20, max(1, (width + 3) // 4) * max(1, (height + 3) // 4) * 16)
    # 0.46c2's DDS files end with one byte past their mip levels (01): kept as it is
    return bytes(header) + bytes(out) + data[at:]
