"""pbr_026.py: 0.69d0's metalness-workflow textures as 0.26g draws them (resource-porter's 0.69d0
weapon port does the same: src/weapons_026/donor_069d/convert.rs). A texture is told by its name:
`<stem>` the albedo, `<stem>_smap` the smoothness, `<stem>_fmap` the metalness, `<stem>_nmap` the
normal. Each pixel is mapped, then encoded DXT1 again with the source's mips (ImageMagick), the
source's header and its last byte (0.26g reads the texture as sRGB when the .dds's last byte is 1:
the diffuse's; the gloss's and Fresnel's 0):
- the diffuse (sRGB values): `0.76 * albedo * (1 - 0.5 * metalness)` (DIFFUSE; bare metal goes
  darker: a metalness workflow paints it bright in the albedo);
- the gloss (0.26g's `_smap`, from the smoothness): `0.36 * smoothness` (GLOSS);
- the Fresnel F0 (`_fmap`, from the metalness): `0.08 + 0.09 * sqrt(metalness)` (FRESNEL);
- the normal map as it is (0.26g's layout, x in alpha and y in green: the AK-12's, the PPSh's, the
  CompM4's and the ACOG's are the same in both builds).
Fitted on the ten texture sets both builds ship with the same layout (the A-545, AK-12, AK-74M,
AK-74N, AKMN, ASh-12, PPSh, ACOG, Cobra and CompM4): the gloss and Fresnel per pixel, the diffuse to
each set's mean, contrast and saturation (the PPSh: stock 34.5, 20.9, 7.0; made 32.2, 16.4, 5.4).
Before (the rifle scopes @4, resource-porter's convert.rs):
- the curves were applied to the DXT colour endpoints, and a block whose two endpoints the curve
  made equal was left as it was: raw metalness as Fresnel in up to 63 % of a map's blocks (the
  AK-74M's; the A-545 38 %, the aperture sight 40 %), so the guns came out 2 to 3 times as
  metallic as the stock ones;
- the Fresnel was 0 where the metalness is 0 (0.26g gives every surface some: median 21/255);
- the diffuse, `0.514 * (albedo * (1 - 0.8 * metalness)) ^ 0.75`, was right on average but flat:
  half the stock contrast and saturation.
The textures before @4 were taken as they are, and metal drew as a black mirror of its
surroundings (seen in the game, 2026-11-01: a scope body on the SCAR-H).
"""
import io
import struct
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

DIFFUSE = (0.76, 0.5)  # diffuse = k * albedo * (1 - m * metalness)
GLOSS = 0.36
FRESNEL = (0.08, 0.09, 0.5)  # F0 = a + b * metalness ^ c


class Dds:
    """A DXT1 or DXT5 .dds of the game's: its header, size, mips and the byte after its mips."""

    def __init__(self, data):
        if data[:4] != b'DDS ':
            raise ValueError('not a DDS')
        self.data = bytes(data)
        self.height, self.width = struct.unpack_from('<II', data, 12)
        self.mips = max(1, struct.unpack_from('<I', data, 28)[0])
        block = {b'DXT1': 8, b'DXT5': 16}.get(bytes(data[84:88]))
        if block is None:
            raise ValueError(f'a {bytes(data[84:88])!r} DDS (DXT1 or DXT5 only)')
        end = 128 + sum(-(-max(1, self.width >> m) // 4) * -(-max(1, self.height >> m) // 4) * block
                        for m in range(self.mips))
        if len(data) != end + 1 or data[end] not in (0, 1):
            raise ValueError(f'{len(data)} bytes: not its mips ({end - 128} bytes) and an sRGB byte')
        self.srgb = data[end]

    def pixels(self):
        """The top mip as an RGB array, values 0..255."""
        return np.asarray(Image.open(io.BytesIO(self.data)).convert('RGB'), dtype=np.float64)


def encoded(pixels, like):
    """`pixels` (RGB, 0..255) as DXT1 with the mips, header and sRGB byte of `like` (a Dds)."""
    p = np.clip(np.round(pixels), 0, 255).astype(np.uint8)
    if p.shape[:2] != (like.height, like.width):
        raise ValueError('pixels of another size')
    with tempfile.TemporaryDirectory() as t:
        src, dst = Path(t) / 'd.png', Path(t) / 'd.dds'
        Image.fromarray(p, 'RGB').save(src)
        # names relative to the folder: ImageMagick 7.1 refused some absolute paths here
        subprocess.run(['magick', src.name, '-define', 'dds:compression=dxt1', '-define', f'dds:mipmaps={like.mips - 1}',
                        dst.name], check=True, capture_output=True, cwd=t)
        made = dst.read_bytes()
    if made[84:88] != b'DXT1' or struct.unpack_from('<II', made, 12) != (like.height, like.width) \
            or max(1, struct.unpack_from('<I', made, 28)[0]) != like.mips:
        raise ValueError('the encoder made another size or mip count')
    out = bytearray(like.data[:128]) + made[128:] + bytes([like.srgb])
    out[84:88] = b'DXT1'
    Dds(out)
    return bytes(out)


def diffuse_dds(albedo, metalness):
    """The 0.26g diffuse of an albedo and metalness pair: DXT1, the albedo's size, mips and sRGB byte."""
    a, mm = Dds(albedo), Dds(metalness)
    rgb = a.pixels() / 255.0
    metal = mm.pixels()[:, :, 0]
    if metal.shape != rgb.shape[:2]:
        metal = np.asarray(Image.fromarray(metal.astype(np.uint8), 'L').resize((a.width, a.height), Image.Resampling.NEAREST),
                           dtype=np.float64)
    k, m = DIFFUSE
    return encoded(255.0 * k * rgb * (1.0 - m * metal[:, :, None] / 255.0), a)


def gloss_dds(smoothness):
    s = Dds(smoothness)
    return encoded(GLOSS * s.pixels(), s)


def fresnel_dds(metalness):
    m = Dds(metalness)
    a, b, c = FRESNEL
    return encoded(255.0 * (a + b * np.power(m.pixels() / 255.0, c)), m)


def convert(files):
    """Every donor texture of `files` (game path -> bytes, textures/...dds) made as 0.26g draws it."""
    out = dict(files)
    for path, data in files.items():
        if not (path.startswith('textures/') and path.endswith('.dds')):
            continue
        stem = path[:-len('.dds')]
        if stem.endswith('_smap'):
            out[path] = gloss_dds(data)
        elif stem.endswith('_fmap'):
            out[path] = fresnel_dds(data)
        elif stem.endswith('_nmap'):
            continue
        elif stem + '_fmap.dds' in files:
            out[path] = diffuse_dds(data, files[stem + '_fmap.dds'])
    return out
