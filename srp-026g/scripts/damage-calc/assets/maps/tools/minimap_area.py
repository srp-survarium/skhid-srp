"""The play area a native HUD minimap bitmap shows, as a boolean mask of a
W x W frame (the bitmap covers the same square frame as the render).

Three kinds of bitmap, told apart by their alpha (kind_of):
  "alpha" (the 0.26g maps): the play area is opaque and the rest clear, so
    the area is alpha > 0.15.
  "alpha:<t>" (London): the bitmap fades out in a soft vignette around a more
    opaque play area; t is the valley of its alpha histogram (0.4).
  "line" (the 0.11e ports): opaque everywhere with a blurred edge, the play
    area outlined by a drawn light-grey line; the area is what that line
    encloses (the line's gaps closed by 2 px).
"""
import numpy as np
from PIL import Image
from scipy import ndimage


def kind_of(bitmap):
    """The bitmap's kind, from its alpha: a fifth or more fully opaque is a
    painting with a drawn outline ("line"); two fifths or more faint (alpha
    above 0 up to 0.15) is a vignette, cut at its histogram's valley between
    0.16 and 0.84; little of either is the 0.26g kind ("alpha"). Bitmaps
    near those bounds raise ValueError: give them a kind by hand (the
    map's `area` in build_all.py's NATIVE or PATCHED) rather than guess."""
    a = np.asarray(Image.open(bitmap).convert('RGBA'))[..., 3].astype(int)
    opaque, faint = (a == 255).mean(), ((a > 0) & (a <= 38)).mean()
    if opaque >= 0.2 and faint < 0.3:
        return 'line'
    if faint >= 0.4 and opaque < 0.05:
        hist = np.bincount(a.ravel(), minlength=256) / a.size
        smooth = np.convolve(hist, np.ones(15) / 15, mode='same')
        valley = 40 + int(np.argmin(smooth[40:216]))
        return f'alpha:{valley / 255:.4g}'
    if opaque < 0.05 and faint < 0.3:
        return 'alpha'
    raise ValueError(f'{bitmap}: alpha fully opaque {opaque:.2f}, faint {faint:.2f}: no clear kind of play area')


def area_mask(bitmap, W, kind='alpha'):
    im = Image.open(bitmap).convert('RGBA')
    if kind.startswith('alpha'):
        threshold = float(kind.split(':', 1)[1]) if ':' in kind else 0.15
        alpha = np.asarray(im.resize((W, W), Image.BILINEAR), np.float32)[..., 3] / 255
        return ndimage.binary_fill_holes(alpha > threshold)
    if kind != 'line':
        raise ValueError(f'unknown play-area kind {kind!r}')
    x = np.asarray(im).astype(int)
    rgb = x[..., :3]
    line = (rgb.min(-1) > 150) & (rgb.max(-1) - rgb.min(-1) < 40) & (x[..., 3] > 200)
    line = ndimage.binary_dilation(line, iterations=2)
    seed = np.zeros_like(line)
    seed[0, :] = seed[-1, :] = seed[:, 0] = seed[:, -1] = True
    outside = ndimage.binary_propagation(seed & ~line, mask=~line)
    inside = ndimage.binary_opening(~outside, iterations=6)
    lab, n = ndimage.label(inside)
    inside = lab == 1 + int(np.argmax(ndimage.sum(inside, lab, range(1, n + 1))))
    return np.asarray(Image.fromarray(inside).resize((W, W), Image.NEAREST))
