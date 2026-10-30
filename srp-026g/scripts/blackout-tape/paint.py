"""The team colour painted into a torso's diffuse over a band coverage map, composed the way
the game's camouflage bake composes (bake_decal_composition): lerp to a fabric tinted to the
team's worn colour by the coverage, overlay the diffuse's fine shading, multiply its broad
shading; so folds, seams and dirt stay. Colours: look C's worn navy (team 0) and brick (team 1).
"""
import numpy as np
import bandlib as BL

WORN = {'blue': (38, 64, 122), 'red': (146, 50, 36)}
FABRIC = 'character/human/camo/scavenger/fabric_grey'
TILES = 6


def _box(a, k, axis):
    if k < 1:
        return a
    pad = [(0, 0), (0, 0)]
    pad[axis] = (k + 1, k)
    c = np.cumsum(np.pad(a, pad, mode='edge'), axis=axis)
    hi = np.take(c, np.arange(2 * k + 1, c.shape[axis]), axis=axis)
    lo = np.take(c, np.arange(0, c.shape[axis] - 2 * k - 1), axis=axis)
    return (hi - lo) / (2 * k + 1)


def blur(a, r):
    """About a Gaussian of sigma r: three box passes each way."""
    k = max(int(round(np.sqrt(4 * r * r + 1) - 1) // 2), 0) if r >= 0.5 else 0
    a = a.astype(np.float64)
    for _ in range(3):
        a = _box(_box(a, k, 0), k, 1)
    return a


def lum(c):
    return c[..., 0] * 0.2126 + c[..., 1] * 0.7152 + c[..., 2] * 0.0722


def band_aoac(base, cov):
    """The camouflage bake's aoac for the band: R broad shading, G 1 - coverage, B fine shading."""
    L = lum(base)
    high = L - blur(L, 4)
    B = 0.5 + np.clip(4.0 * high, -0.5, 0.5) * cov
    broad = blur(L, 12)
    sel = broad[cov > 0.5]
    ref = np.percentile(sel, 90) if sel.size else 1.0
    R = 1 - 0.7 * np.clip(1 - broad / max(ref, 1e-3), 0, 1) * cov
    return np.stack([R, 1 - cov, B], 2)


def bake(base, tile, a):
    h, w = base.shape[:2]
    th, tw = tile.shape[:2]
    v, u = np.mgrid[0:h, 0:w].astype(np.float32)
    t = tile[((v / h * TILES) * th).astype(int) % th, ((u / w * TILES) * tw).astype(int) % tw]
    c = base + (t - base) * (1 - a[..., 1:2])
    B = a[..., 2:3]
    c = np.where(c < 0.5, 2 * c * B, 1 - 2 * (1 - c) * (1 - B))
    return np.clip(c * a[..., 0:1], 0, 1)


_FABRIC = {}


def fabric(colour):
    if colour not in _FABRIC:
        f = BL.texture(FABRIC)[..., :3] / 255
        L = lum(f)
        _FABRIC[colour] = (L / L.mean())[..., None] * (np.array(colour) / 255.0)[None, None, :]
    return _FABRIC[colour]


def painted(diffuse, cov):
    """{colour: RGBA 0..255} of `diffuse` with the band painted over coverage `cov`."""
    dif = BL.texture(diffuse) / 255
    a = band_aoac(dif[..., :3], cov)
    return {c: np.concatenate([bake(dif[..., :3], fabric(rgb), a), dif[..., 3:4]], 2) * 255 for c, rgb in WORN.items()}
