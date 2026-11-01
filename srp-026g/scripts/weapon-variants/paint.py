"""The skins' paint on 0.26g's textures.

0.69d0 draws its guns with metalness PBR maps: its albedo of a gun is 0.26g's diffuse of it,
lightened (bare metal by 2-3x, paint by 1.0-1.4x: measured per texture). A skin's albedo is that
albedo repainted where the skin differs. So a skin becomes a 0.26g diffuse as 0.26g's own diffuse of
the gun, with only the repainted pixels changed: there the skin's albedo, darkened back by the
factors that take this gun's 0.69d0 albedo to its 0.26g diffuse (fitted per texture and channel, for
paint and for metal by the metalness map), and shaded by what 0.26g's diffuse has that 0.69d0's
albedo lost (its baked shading, the ratio of the two). Checked on the two premium textures both
builds ship (AKM and SKS "camo_st", check_pack.py): made this way from 0.69d0's, the AKM's differs
from 0.26g's own by 0.055 a channel where repainted, 0.69d0's albedo as it is by 0.111; the SKS's
paint is 0.26g's colour already (0.015 both).

Only the 4x4 blocks with repainted pixels are encoded again (exo-gear's DXT1 codec), each mip level
from the repainted top level; every other block keeps 0.26g's bytes.
"""
import io

import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from scipy import ndimage

from common import exo_gear

# A pixel is repainted where the skin's albedo differs from the gun's by more than LOW (0..1, the
# largest channel), fully from HIGH; the mask then grows by a pixel's blur so DXT edges stay clean.
LOW, HIGH = 0.03, 0.08
# Repainted metal counts as half paint: 0.69d0's skins paint over bare metal parts but keep their
# metalness map (the AKM's "camo_st": the 0.26g texture is closest so).
PAINTED_METAL = 0.5
# The baked-shading ratio is kept within this range.
SHADE = (0.6, 1.5)
# The skin's islands grow by this many texels (of a 2048 texture: half a 4x4 block at mip 2).
ISLAND_GROW = 4


def decode_dds(data):
    """The top level of a DDS (DXT1/DXT5) as float RGB 0..1."""
    image = Image.open(io.BytesIO(data))
    image.load()
    return np.asarray(image.convert('RGB'), np.float32) / 255


def resized(image, shape):
    if image.shape[:2] == shape:
        return image
    out = Image.fromarray((np.clip(image, 0, 1) * 255).astype(np.uint8))
    return np.asarray(out.resize((shape[1], shape[0]), Image.Resampling.LANCZOS), np.float32) / 255


def blur(mask, radius):
    image = Image.fromarray((np.clip(mask, 0, 1) * 255).astype(np.uint8))
    return np.asarray(image.filter(ImageFilter.GaussianBlur(radius)), np.float32) / 255


def factors(diffuse, albedo, metal, where):
    """Per channel (paint, metal): the factors taking 0.69d0's albedo to 0.26g's diffuse (least squares
    over the pixels `where`)."""
    out = []
    for c in range(3):
        a = np.stack([albedo[..., c] * (1 - metal), albedo[..., c] * metal], -1)[where]
        k, *_ = np.linalg.lstsq(a, diffuse[..., c][where], rcond=None)
        out.append(k)
    return np.array(out)


def scale(k, metal):
    return k[None, None, :, 0] * (1 - metal[..., None]) + k[None, None, :, 1] * metal[..., None]


def islands(uv, tris, shape, grow=ISLAND_GROW):
    """The texels the triangles' UVs cover (grown by `grow` texels, the DXT and mip seams)."""
    image = Image.new('L', (shape[1], shape[0]), 0)
    draw = ImageDraw.Draw(image)
    w, h = shape[1], shape[0]
    for t in tris:
        draw.polygon([(float(uv[i, 0] % 1) * w, float(uv[i, 1] % 1) * h) for i in t], fill=255)
    covered = np.asarray(image) > 0
    return ndimage.binary_dilation(covered, iterations=grow) if grow else covered


def repaint(diffuse, gun, gun_metal, skin, skin_metal, skin_islands):
    """(the 0.26g diffuse with the skin's paint, the repainted mask 0..1) from 0.26g's diffuse of the
    gun, 0.69d0's albedo and metalness of the gun and of the skin (float RGB, any size) and the texels
    the skin's 0.69d0 mesh draws (`islands`). Outside those (the parts 0.69d0 made separate modules
    of, whose texels it left to the padding) the gun keeps 0.26g's texture."""
    shape = diffuse.shape[:2]
    gun, skin = resized(gun, shape), resized(skin, shape)
    gun_metal, skin_metal = resized(gun_metal, shape).mean(-1), resized(skin_metal, shape).mean(-1)
    k = factors(diffuse, gun, gun_metal, skin_islands)
    mask = np.clip((np.abs(skin - gun).max(-1) - LOW) / (HIGH - LOW), 0, 1) * skin_islands
    mask = np.maximum(mask, blur(mask, 1.0) * skin_islands)
    shading = np.clip(diffuse.mean(-1) / np.maximum((gun * scale(k, gun_metal)).mean(-1), 0.02), *SHADE)
    paint = skin * scale(k, skin_metal * (1 - PAINTED_METAL * mask)) * shading[..., None]
    return diffuse * (1 - mask[..., None]) + np.clip(paint, 0, 1) * mask[..., None], mask


def encode_over(dds, image, mask):
    """`dds` (0.26g's DXT1 diffuse) with the blocks `mask` touches encoded again from `image` (float RGB
    0..1 of its top level), each mip level from `image` averaged down."""
    exo = exo_gear()
    levels = exo.dds_levels(dds)
    out = bytearray(dds)
    top = image * 255
    for level, (w, h, at, size) in enumerate(levels):
        factor = 1 << level
        if factor > 1:
            if top.shape[0] % factor or top.shape[1] % factor:
                break
            picture = top.reshape(h, factor, w, factor, 3).mean((1, 3))
            changed = (mask > 0).reshape(h, factor, w, factor).any((1, 3))
        else:
            picture, changed = top, mask > 0
        bw, bh = max(1, (w + 3) // 4), max(1, (h + 3) // 4)
        padded = np.zeros((bh * 4, bw * 4, 3))
        padded[:h, :w] = picture
        blocks = padded.reshape(bh, 4, bw, 4, 3).transpose(0, 2, 1, 3, 4).reshape(bh * bw, 16, 3)
        flags = np.zeros((bh * 4, bw * 4), bool)
        flags[:h, :w] = changed
        touched = np.nonzero(flags.reshape(bh, 4, bw, 4).any((1, 3)).reshape(-1))[0]
        if len(touched):
            encoded = np.frombuffer(exo.encode(blocks[touched]), np.uint8).reshape(-1, 8)
            view = np.frombuffer(out, np.uint8, count=size, offset=at).reshape(-1, 8).copy()
            view[touched] = encoded
            out[at:at + size] = view.tobytes()
    return bytes(out)
