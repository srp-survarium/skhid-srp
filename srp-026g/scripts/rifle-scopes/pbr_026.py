"""pbr_026.py: 0.69d0's metalness-workflow textures as 0.26g draws them, as resource-porter's 0.69d0
weapon port converts them (src/weapons_026/donor_069d/convert.rs, fitted on the seven assault-rifle
texture sets both builds ship):
- the diffuse: `0.514 * (albedo * (1 - 0.8 * metalness)) ^ 0.75` per pixel (bare metal goes dark: a
  metalness workflow paints it bright in the albedo), re-encoded DXT1 with its mips (ImageMagick);
- the gloss (0.26g's `_smap`, from the smoothness): 0.36 x, in the DXT colour endpoints;
- the Fresnel (`_fmap`, from the metalness): 0.162 x m^0.388, in the endpoints;
- the normal map as it is.
Taken as they are (the rifle scopes @1..@3), metal drew as a black mirror of its surroundings, light and
dark streaks (seen in the game, 2026-11-01: a scope body on the SCAR-H). A texture is told by its name:
`<stem>` the albedo, `<stem>_smap` the smoothness, `<stem>_fmap` the metalness, `<stem>_nmap` the normal.
"""
import io
import struct
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

GLOSS = [(0, 0), (255, 91)]
FRESNEL = [(0, 0), (4, 8), (16, 14), (32, 18), (64, 24), (96, 28), (128, 32), (160, 34), (192, 37),
           (224, 39), (255, 41)]


def curve(knots, v):
    for (x0, y0), (x1, y1) in zip(knots, knots[1:]):
        if v <= x1:
            t = (v - x0) / max(1, x1 - x0)
            return int(min(255, max(0, round(y0 + t * (y1 - y0)))))
    return knots[-1][1]


def remap_565(c, knots):
    r, g, b = (c >> 11) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31
    r, g, b = (curve(knots, v) for v in (r, g, b))
    pack = lambda v, m: (v * m + 127) // 255  # noqa: E731
    return (pack(r, 31) << 11) | (pack(g, 63) << 5) | pack(b, 31)


def remap_dds(data, knots):
    """`knots` on every colour endpoint of a DXT1/DXT5 DDS, every mip (DXT1 blocks keep their mode)."""
    assert data[:4] == b'DDS ', 'not a DDS'
    h, w = struct.unpack_from('<II', data, 12)
    mips = max(1, struct.unpack_from('<I', data, 28)[0])
    block = {b'DXT1': 8, b'DXT5': 16}[bytes(data[84:88])]
    out = bytearray(data)
    table = {}
    at = 128
    for mip in range(mips):
        n = -(-max(1, w >> mip) // 4) * -(-max(1, h >> mip) // 4)
        for _ in range(n):
            c = at + block - 8
            c0, c1 = struct.unpack_from('<HH', out, c)
            n0 = table.setdefault(c0, remap_565(c0, knots))
            n1 = table.setdefault(c1, remap_565(c1, knots))
            if block == 16 or ((c0 > c1) == (n0 > n1) and (c0 == c1) == (n0 == n1)):
                struct.pack_into('<HH', out, c, n0, n1)
            at += block
    return bytes(out)


def diffuse_dds(albedo, metalness):
    """The 0.26g diffuse of an albedo and metalness pair: DXT1, the albedo's size and mips."""
    a = np.asarray(Image.open(io.BytesIO(albedo)).convert('RGB'), dtype=np.float64) / 255.0
    m = Image.open(io.BytesIO(metalness)).convert('L')
    if m.size != (a.shape[1], a.shape[0]):
        m = m.resize((a.shape[1], a.shape[0]), Image.Resampling.NEAREST)
    m = np.asarray(m, dtype=np.float64)[:, :, None] / 255.0
    d = np.clip(np.round(255.0 * 0.514 * np.power(a * (1.0 - 0.8 * m), 0.75)), 0, 255).astype(np.uint8)
    mips = max(1, struct.unpack_from('<I', albedo, 28)[0])
    with tempfile.TemporaryDirectory() as t:
        src, dst = Path(t) / 'd.png', Path(t) / 'd.dds'
        Image.fromarray(d, 'RGB').save(src)
        # names relative to the folder: ImageMagick 7.1 refused some absolute paths here
        subprocess.run(['magick', src.name, '-define', 'dds:compression=dxt1', '-define', f'dds:mipmaps={mips - 1}',
                        dst.name], check=True, capture_output=True, cwd=t)
        made = dst.read_bytes()
    assert made[84:88] == b'DXT1' and struct.unpack_from('<II', made, 12) == struct.unpack_from('<II', albedo, 12)
    # the albedo's header (its flags, sRGB as the game reads it), the encoder's blocks
    out = bytearray(albedo[:128]) + made[128:]
    struct.pack_into('<I', out, 28, max(1, struct.unpack_from('<I', made, 28)[0]))
    out[84:88] = b'DXT1'
    return bytes(out)


def convert(files):
    """Every donor texture of `files` (game path -> bytes, textures/...dds) made as 0.26g draws it."""
    out = dict(files)
    for path, data in files.items():
        if not (path.startswith('textures/') and path.endswith('.dds')):
            continue
        stem = path[:-len('.dds')]
        if stem.endswith('_smap'):
            out[path] = remap_dds(data, GLOSS)
        elif stem.endswith('_fmap'):
            out[path] = remap_dds(data, FRESNEL)
        elif stem.endswith('_nmap'):
            continue
        elif stem + '_fmap.dds' in files:
            out[path] = diffuse_dds(data, files[stem + '_fmap.dds'])
    return out
