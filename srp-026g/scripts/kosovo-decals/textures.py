"""textures.py: the two Kosovo decals' textures, made from the official artwork in 0.69d's own styles
(../../docs/notes/kosovo-decals.md).

- The flag: an embroidered patch as 0.69d's flags 4_42 to 4_75. Albania's patch (4_69) gives the frame,
  the border, the field's weave, its relief and gloss; its eagle is covered by the weave around it.
  The threads take the colours 0.69d gives the same threads: blue as Chile's canton (4_62), white as
  Chile's star, gold as Vietnam's star (4_60), moved by the official colours' difference. The stars
  and the map are raised as Vietnam's star, a dark seam around them as on Chile's.
- The coat of arms: a worn painted emblem as 0.69d's coats of arms 5_1 to 5_42. Bosnia's shield (5_42)
  gives the wear: where its paint is gone, its scratches and relief, its gloss per paint.

Every input is an array of the donor's top mip; every output an array of the texture's top mip.
"""
import numpy as np
from scipy import ndimage as nd

SIZE = 512
# The official colours (CMYK 100/80/0/0, 0/20/60/20 and white of the 2008 files), as the
# government's PDF renders them in sRGB.
BLUE = np.array([37, 74, 165], dtype=np.float64)
GOLD = np.array([207, 165, 80], dtype=np.float64)
WHITE = np.array([255, 255, 255], dtype=np.float64)
# The pages' other fills: the caption's black text, the page frame's grey line (not the design).
CAPTION = np.array([35, 31, 32], dtype=np.float64)
FRAME = np.array([233, 233, 233], dtype=np.float64)
# The donors' official colours: Chile's blue, Vietnam's yellow.
CHILE_BLUE = np.array([0, 57, 166], dtype=np.float64)
VIETNAM_YELLOW = np.array([255, 205, 0], dtype=np.float64)
# The donors' threads as their textures show them (a 9-pixel mean), to find each region.
CHILE_BLUE_THREAD = (32, 50, 99)
CHILE_WHITE_THREAD = (142, 131, 131)
VIETNAM_GOLD_THREAD = (122, 105, 29)
RED_THREAD = (120, 29, 29)
RIM = (13, 12, 13)


def blur(a, sigma):
    return nd.gaussian_filter(a, sigma, mode='nearest')


def blur_channels(img, sigma):
    return nd.gaussian_filter(img, (sigma, sigma, 0), mode='nearest')


def spread(img, mask, sigma):
    """The broad shading of an image's masked pixels, carried over the unmasked ones (a normalised
    blur)."""
    w = blur(mask.astype(np.float64), sigma)[..., None]
    return blur_channels(img * mask[..., None], sigma) / np.maximum(w, 1e-6)


def region(rgb, colour, others, size=9, erode=3):
    """The pixels of a patch's flat region of `colour`: each pixel's neighbourhood mean is nearest to
    it of `others`, the region's edge left out."""
    mean = nd.uniform_filter(rgb, (size, size, 1), mode='nearest')
    dist = [np.linalg.norm(mean - np.asarray(c, dtype=np.float64), axis=2) for c in [colour] + others]
    mask = np.argmin(np.stack(dist), axis=0) == 0
    # (scipy erodes until nothing changes at 0 iterations)
    return nd.binary_erosion(mask, iterations=erode) if erode else mask


def palette(rgb, mask, steps=256):
    """A thread's colours by brightness: entry q is the mean colour of the region's pixels at the
    q-th quantile of brightness."""
    px = rgb[mask]
    order = np.argsort(px.sum(axis=1), kind='stable')
    px = px[order]
    edges = np.linspace(0, len(px), steps + 1).astype(int)
    return np.stack([px[a:max(b, a + 1)].mean(axis=0) for a, b in zip(edges[:-1], edges[1:])])


def linear(srgb):
    c = np.asarray(srgb, dtype=np.float64) / 255
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def srgb(lin):
    c = np.clip(lin, 0, 1)
    return 255 * np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def shifted(pal, donor_official, official):
    """A palette dyed another colour. In linear light each of the donor's entries is its shading of
    the donor's official colour plus a grey haze, shade x official + haze: the channel the official
    colour lacks gives the haze, its strongest channel the shading. The new entry is the same
    shading of the new official colour plus the same haze."""
    lin, off = linear(pal), linear(donor_official)
    lo, hi = int(np.argmin(off)), int(np.argmax(off))
    shade = np.clip((lin[:, hi:hi + 1] - lin[:, lo:lo + 1]) / max(off[hi] - off[lo], 1e-6), 0, None)
    haze = lin[:, lo:lo + 1] - shade * off[lo]
    return srgb(shade * linear(official) + haze)


def quantiles(values, mask):
    """Each value's rank among the masked values, 0..1."""
    flat = values[mask]
    order = np.sort(flat, kind='stable')
    return np.clip(np.searchsorted(order, values, side='right') / len(order), 0, 1)


def paint(pal, q):
    i = np.clip(np.round(q * (len(pal) - 1)).astype(int), 0, len(pal) - 1)
    return pal[i]


def normal_xy(nmap):
    """A 0.26g normal map's x (alpha) and y (green), -1..1."""
    return (nmap[..., 3] - 128) / 127, (nmap[..., 1] - 128) / 127


def bevel(mask, sigma):
    """The slope of a raised shape: the gradient of its soft height."""
    h = blur(mask.astype(np.float64), sigma)
    gy, gx = np.gradient(h)
    return gx, gy


def fit_bevel(nx, ny, mask, sigma):
    """How the donor's normal map writes a raised shape's slope: the factor on x and on y (their signs
    are the map's convention), fitted where the shape's edge slopes."""
    gx, gy = bevel(mask, sigma)
    edge = (np.abs(gx) + np.abs(gy)) > 0.02
    kx = (nx[edge] * gx[edge]).sum() / (gx[edge] ** 2).sum()
    ky = (ny[edge] * gy[edge]).sum() / (gy[edge] ** 2).sum()
    return kx, ky


def best_shift(lum, clean, dx_range, dy_range):
    """The shift (dx, dy) that maps the weave onto itself best over the clean pixels."""
    best = None
    ys, xs = np.nonzero(clean)
    for dy in dy_range:
        for dx in dx_range:
            sy, sx = ys + dy, xs + dx
            ok = (sy >= 0) & (sy < lum.shape[0]) & (sx >= 0) & (sx < lum.shape[1])
            ok[ok] &= clean[sy[ok], sx[ok]]
            if ok.sum() < 2000:
                continue
            err = np.mean((lum[ys[ok], xs[ok]] - lum[sy[ok], sx[ok]]) ** 2)
            if best is None or err < best[0]:
                best = (err, dx, dy)
    return best[1], best[2]


def flag(albania, albania_n, albania_s, chile, vietnam, vietnam_n, design):
    """The embroidered Flag of Kosovo.

    albania, albania_n, albania_s: 4_69's diffuse (RGBA), normal map (RGBA), specular (RGB);
    chile, vietnam: 4_62's and 4_60's diffuse (RGBA); vietnam_n: 4_60's normal map;
    design: the official flag's stars and map as two coverage arrays (0..1) at SIZE x SIZE over the
    patch's field (`flag_field`), from the official file."""
    rgb, alpha = albania[..., :3], albania[..., 3]
    inside = alpha > 128
    depth = nd.distance_transform_edt(inside)
    redness = rgb[..., 0] / ((rgb[..., 1] + rgb[..., 2]) / 2 + 4)
    thread = inside & (redness > 1.8) & (depth > 4)

    # The eagle: the dark thread inside the field, with its seam.
    dark = inside & (depth > 24) & ~thread
    dark = nd.binary_opening(dark, iterations=1)
    labels, n = nd.label(nd.binary_closing(dark, iterations=3))
    sizes = nd.sum(np.ones_like(labels), labels, range(1, n + 1))
    eagle = np.isin(labels, 1 + np.nonzero(sizes > 400)[0])
    # (its relief reaches past its thread: 12 pixels; the cover fades in over the outer 5)
    eagle = nd.binary_dilation(nd.binary_fill_holes(eagle), iterations=12)
    fade = np.clip(nd.distance_transform_edt(eagle) / 5, 0, 1)
    field = thread & ~eagle

    # Cover the eagle with the weave beside it: its left half from the left, its right half from the
    # right, each by the shift that repeats the weave best.
    lum = rgb[..., 0]
    clean = field & (depth > 16)
    ys, xs = np.nonzero(eagle)
    cx = (xs.min() + xs.max()) // 2
    halfwidth = (xs.max() - xs.min()) // 2 + 8
    left = best_shift(lum, clean, range(-halfwidth - 24, -halfwidth + 1), range(-6, 7))
    right = best_shift(lum, clean, range(halfwidth, halfwidth + 25), range(-6, 7))
    # Only the weave's detail is copied; its broad shading (the patch's bulge) is the field's around
    # the eagle, spread over it.
    src = {}
    for key, img in (('rgb', rgb), ('n', albania_n), ('s', albania_s)):
        broad = spread(img, field, 40)
        detail = img - blur_channels(img, 12)
        cover = img.copy()
        for (dx, dy), side in ((left, xs <= cx), (right, xs > cx)):
            ty, tx = ys[side], xs[side]
            cover[ty, tx] = broad[ty, tx] + detail[ty + dy, tx + dx]
        src[key] = img * (1 - fade[..., None]) + cover * fade[..., None]
    rgb, nmap, smap = src['rgb'], src['n'], src['s']
    thread = thread | eagle

    # The emblems' weave: the field's, turned a quarter (embroidery fills a shape across the field's
    # stitches), normal map x and y swapped with it.
    turned_rgb = np.swapaxes(rgb, 0, 1)
    turned_n = np.swapaxes(nmap, 0, 1)[..., [0, 3, 2, 1]]
    turned_s = np.swapaxes(smap, 0, 1)
    turned_thread = np.swapaxes(thread & (depth > 20), 0, 1)

    stars, land = design
    emblem = np.maximum(stars, land)
    hard = emblem > 0.5

    # Weave sources for the emblems: the turned field, shifted so the emblems sit on clean weave.
    ey, ex = np.nonzero(hard)
    ty0, tx0 = np.nonzero(turned_thread)
    oy = ty0.min() + (ty0.max() - ty0.min() - (ey.max() - ey.min())) // 2 - ey.min()
    ox = tx0.min() + (tx0.max() - tx0.min() - (ex.max() - ex.min())) // 2 - ex.min()
    def turned(img):
        out = np.zeros_like(img)
        yy, xx = np.mgrid[0:SIZE, 0:SIZE]
        sy = np.clip(yy + oy, 0, img.shape[0] - 1)
        sx = np.clip(xx + ox, 0, img.shape[1] - 1)
        return img[sy, sx]
    e_rgb, e_n, e_s = turned(turned_rgb), turned(turned_n), turned(turned_s)

    # Colours: each thread's palette from its donor, moved to the official colour.
    chile_rgb = chile[..., :3]
    blue_pal = shifted(palette(chile_rgb, region(chile_rgb, CHILE_BLUE_THREAD, [CHILE_WHITE_THREAD, RED_THREAD, RIM])),
                       CHILE_BLUE, BLUE)
    white_pal = palette(chile_rgb, region(chile_rgb, CHILE_WHITE_THREAD, [CHILE_BLUE_THREAD, RED_THREAD, RIM]))
    viet_rgb = vietnam[..., :3]
    yellow = region(viet_rgb, VIETNAM_GOLD_THREAD, [RED_THREAD, RIM], erode=2)
    gold_pal = shifted(palette(viet_rgb, yellow), VIETNAM_YELLOW, GOLD)

    field_q = quantiles(rgb[..., 0], thread)
    emblem_q = quantiles(e_rgb[..., 0], thread)
    out = rgb.copy()
    out[thread] = paint(blue_pal, field_q)[thread]
    star_rgb = paint(white_pal, emblem_q)
    land_rgb = paint(gold_pal, emblem_q)

    # The seam: the field darkens just outside each emblem, as around Chile's star.
    outside = nd.distance_transform_edt(~hard)
    seam = np.clip(1 - (outside - 0.5) / 2.5, 0, 1) * (outside > 0)
    out = out * (1 - 0.7 * seam[..., None])
    inner = nd.distance_transform_edt(hard)
    edge_shade = 1 - 0.35 * np.clip(1 - (inner - 0.5) / 2.0, 0, 1)
    emb_rgb = (star_rgb * stars[..., None] + land_rgb * land[..., None]) / np.maximum(emblem, 1e-6)[..., None]
    emb_rgb = emb_rgb * edge_shade[..., None]
    out = out * (1 - emblem[..., None]) + emb_rgb * emblem[..., None]

    # Relief: the field's weave outside, the turned weave inside, the emblems raised as Vietnam's star.
    vx, vy = normal_xy(vietnam_n)
    kx, ky = fit_bevel(vx, vy, yellow_shape(viet_rgb), 1.5)
    fx, fy = normal_xy(nmap)
    gx_, gy_ = normal_xy(e_n)
    bx, by = bevel(hard, 1.5)
    nx = fx * (1 - emblem) + gx_ * emblem + kx * bx
    ny = fy * (1 - emblem) + gy_ * emblem + ky * by
    n_out = nmap.copy()
    n_out[..., 3] = np.clip(128 + 127 * nx, 0, 255)
    n_out[..., 1] = np.clip(128 + 127 * ny, 0, 255)

    s_out = smap * (1 - emblem[..., None]) + e_s * emblem[..., None]
    s_out = s_out * (1 - 0.7 * seam[..., None])

    diffuse = np.concatenate([out, alpha[..., None]], axis=2)
    return diffuse, n_out, s_out


def yellow_shape(viet_rgb):
    """Vietnam's star, whole (its seam and edge in)."""
    mask = region(viet_rgb, VIETNAM_GOLD_THREAD, [RED_THREAD, RIM], size=5, erode=0)
    labels, n = nd.label(mask)
    sizes = nd.sum(np.ones_like(labels), labels, range(1, n + 1))
    return nd.binary_fill_holes(labels == 1 + int(np.argmax(sizes)))


def flag_field(albania):
    """The patch's field: the box inside the frame the flag's design spans, (x0, y0, x1, y1)."""
    alpha = albania[..., 3] > 128
    ys, xs = np.nonzero(alpha)
    return xs.min() + 8, ys.min() + 8, xs.max() - 7, ys.max() - 7


# Bosnia's shield (5_42): its paints as its texture has them, and their official colours.
BOSNIA_BLUE = np.array([0, 0, 156], dtype=np.float64)
BOSNIA_YELLOW = np.array([252, 201, 0], dtype=np.float64)
BOSNIA_BLUE_OFFICIAL = np.array([0, 35, 149], dtype=np.float64)
BOSNIA_YELLOW_OFFICIAL = np.array([254, 203, 0], dtype=np.float64)
RIM_WIDTH = 7           # the shield's worn rim, inward from its outline
# Kosovo's shield is worn less than Bosnia's (the owner, 2026-11-19: "a bit too much tear", "remove that
# white part at the bottom", "and grey one too", "wear at top is fine"): its dirt, scrapes to the base,
# their dull gloss and relief at half strength, only its chips of more than CHIP_MIN pixels, and none of
# them in the shield's lower part: they fade out from WORN_TO to CLEAN_FROM of its height (Bosnia's big
# scrape and the chips at its point). The rim stays all round.
WEAR = 0.5
CHIP_MIN = 40
WORN_TO, CLEAN_FROM = 0.45, 0.58


def bosnia_parts(rgb, alpha):
    """Bosnia's shield split: its shape, the clean blue and yellow paint, the stars, and where its own
    design's edges are (the stars and the diagonal, with their relief), which says nothing of the wear."""
    shape = nd.binary_fill_holes(alpha > 128)
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    blue = shape & (b > 110) & (r < 90) & (g < 90)
    yellow = shape & (r > 150) & (g > 110) & (b < 110)
    white = shape & (r > 190) & (g > 190) & (b > 190)
    # The diagonal: the line between the clean blue and the clean yellow.
    touch = nd.binary_dilation(blue, iterations=2) & nd.binary_dilation(yellow, iterations=2)
    ty, tx = np.nonzero(touch)
    slope, icept = np.polyfit(tx, ty, 1)
    yy, xx = np.mgrid[0:rgb.shape[0], 0:rgb.shape[1]]
    off = (yy - (slope * xx + icept)) / np.hypot(slope, 1)     # > 0 below the line: the blue side
    # The stars: solid white along the diagonal on the blue side (the wear's scrapes are thin).
    solid = nd.binary_opening(white, iterations=3)
    labels, n = nd.label(solid)
    stars = np.zeros_like(shape)
    for i in range(1, n + 1):
        part = labels == i
        if 150 < part.sum() < 4000 and 0 < off[part].mean() < 70:
            stars |= part
    stars = nd.binary_fill_holes(nd.binary_dilation(stars, iterations=3))
    edges = nd.binary_dilation(stars, iterations=5) | (np.abs(off) < 6) & shape
    side = np.where(off > 0, 1, 2)                               # 1 blue, 2 yellow
    return shape, side, stars, edges


def fill_unknown(img, known):
    """Pixels not known take the spread of the known ones around them: the nearest scale that
    reaches them."""
    out = (img if img.ndim == 3 else img[..., None]).copy()
    done = known.copy()
    for sigma in (3, 6, 12, 24, 48):
        w = blur(known.astype(np.float64), sigma)
        near = spread(out, known, sigma)
        take = ~done & (w > 0.02)
        out[take] = near[take]
        done |= take
    return out if img.ndim == 3 else out[..., 0]


def by_depth(values, depth, mask, width):
    """The median of `values` at each whole depth 1..width from a shape's outline."""
    return np.array([np.median(values[mask & (np.round(depth) == d)]) for d in range(1, width + 1)])


def outline_rows(shape):
    """Each column's top and bottom row of a shape (-1 where the column is empty)."""
    any_ = shape.any(axis=0)
    top = np.where(any_, shape.argmax(axis=0), -1)
    bottom = np.where(any_, shape.shape[0] - 1 - shape[::-1].argmax(axis=0), -1)
    return top, bottom


def shield_warp(kosovo, bosnia):
    """For each pixel of Kosovo's shield, the point of Bosnia's it takes its wear from: columns matched
    across the two shapes' boxes, and within a column the same share of the way from top to bottom,
    so outline lands on outline."""
    ky, kx = np.nonzero(kosovo)
    by, bx = np.nonzero(bosnia)
    ktop, kbot = outline_rows(kosovo)
    btop, bbot = outline_rows(bosnia)
    yy, xx = np.mgrid[0:SIZE, 0:SIZE].astype(np.float64)
    sx = bx.min() + (xx - kx.min()) * (bx.max() - bx.min()) / max(kx.max() - kx.min(), 1)
    col = np.clip(np.round(sx).astype(int), bx.min(), bx.max())
    kc = np.clip(xx.astype(int), kx.min(), kx.max())
    t = (yy - ktop[kc]) / np.maximum(kbot[kc] - ktop[kc], 1)
    sy = btop[col] + t * (bbot[col] - btop[col])
    return sy, sx


def sample(img, sy, sx):
    if img.ndim == 2:
        return nd.map_coordinates(img, [sy, sx], order=1, mode='nearest')
    return np.stack([sample(img[..., c], sy, sx) for c in range(img.shape[2])], axis=2)


def coat_of_arms(bosnia, bosnia_n, bosnia_s, design):
    """The worn Coat of arms of Kosovo.

    bosnia, bosnia_n, bosnia_s: 5_42's diffuse (RGBA), normal map (RGBA), specular (RGB);
    design: the official coat of arms' coverages (0..1) at SIZE x SIZE: 'shape', 'gold' (the border),
    'stars', 'land' (the map); the rest of the shape is blue."""
    rgb, alpha = bosnia[..., :3], bosnia[..., 3]
    shape_b, side, stars_b, edges = bosnia_parts(rgb, alpha)
    depth_b = nd.distance_transform_edt(shape_b)
    rim_b = shape_b & (depth_b <= RIM_WIDTH)
    known = shape_b & ~edges & ~stars_b & ~rim_b

    # The wear in each pixel: pixel = paint x u + base x w, by least squares (u: the paint's dirt,
    # w: how far it is worn to the base).
    base = np.median(rgb[shape_b & (rgb.min(axis=2) > 215) & ~stars_b], axis=0)
    paint_b = np.where((side == 1)[..., None], BOSNIA_BLUE, BOSNIA_YELLOW)
    a11 = (paint_b ** 2).sum(axis=2)
    a12 = (paint_b * base).sum(axis=2)
    a22 = (base ** 2).sum()
    r1 = (paint_b * rgb).sum(axis=2)
    r2 = (rgb * base).sum(axis=2)
    det = a11 * a22 - a12 ** 2
    u = np.clip((r1 * a22 - r2 * a12) / det, 0, 1.2)
    w = np.clip((r2 * a11 - r1 * a12) / det, 0, 1)
    u, w = fill_unknown(u, known), fill_unknown(w, known)

    # Its relief without its design's edges, and its gloss over each paint's own.
    nx, ny = normal_xy(bosnia_n)
    kx, ky = fit_bevel(nx, ny, stars_b, 1.5)
    rx, ry = fit_bevel(nx * rim_b, ny * rim_b, shape_b, 2.0)
    unchipped = rim_b & (alpha > 250)
    rim_w = by_depth(w, depth_b, unchipped, RIM_WIDTH)
    rim_s = by_depth(bosnia_s[..., 0], depth_b, unchipped, RIM_WIDTH)
    nx, ny = fill_unknown(nx, known), fill_unknown(ny, known)
    s = bosnia_s[..., 0]
    gloss_b = {1: np.median(s[known & (side == 1) & (w < 0.1) & ~rim_b]),
               2: np.median(s[known & (side == 2) & (w < 0.1) & ~rim_b])}
    m = fill_unknown(s / np.where(side == 1, gloss_b[1], gloss_b[2]), known)
    chips = np.where(shape_b, alpha / 255, 1.0)

    # Onto Kosovo's shield.
    shape_k = design['shape'] > 0.5
    sy, sx = shield_warp(shape_k, shape_b)
    ys_k = np.nonzero(shape_k)[0]
    height = (np.arange(SIZE, dtype=np.float64)[:, None] - ys_k.min()) / (ys_k.max() - ys_k.min())
    keep = WEAR * np.clip((CLEAN_FROM - height) / (CLEAN_FROM - WORN_TO), 0, 1)
    u, w, m = sample(u, sy, sx), sample(w, sy, sx), sample(m, sy, sx)
    u, w, m = 1 + keep * (u - 1), keep * w, 1 + keep * (m - 1)
    nx, ny = sample(nx, sy, sx), sample(ny, sy, sx)
    nx, ny = nx * (keep / WEAR) ** 0.5, ny * (keep / WEAR) ** 0.5
    chips = sample(chips, sy, sx)
    holes, n = nd.label(chips < 0.5)
    sizes = nd.sum(np.ones_like(holes), holes, range(1, n + 1))
    lower = np.zeros(n, dtype=bool)
    for i, sl in enumerate(nd.find_objects(holes)):
        lower[i] = height[sl[0].stop - 1, 0] > WORN_TO
    drop = np.isin(holes, 1 + np.nonzero((sizes < CHIP_MIN) | lower)[0])
    chips = np.where(nd.binary_dilation(drop, iterations=1), 1.0, chips)
    # The rim: Kosovo's own outline, worn and glossed by depth as Bosnia's, sloped as its.
    depth_k = nd.distance_transform_edt(shape_k)
    rim_k = shape_k & (depth_k <= RIM_WIDTH)
    level = np.clip(np.round(depth_k).astype(int), 1, RIM_WIDTH) - 1
    w = np.where(rim_k, np.maximum(w, rim_w[level]), w)
    gx_, gy_ = bevel(shape_k, 2.0)
    nx, ny = nx + rx * gx_, ny + ry * gy_

    gold, stars, land = design['gold'], design['stars'], design['land']
    blue = np.clip(design['shape'] - gold - stars - land, 0, 1)
    blue_paint = shifted(BOSNIA_BLUE[None], BOSNIA_BLUE_OFFICIAL, BLUE)[0]
    gold_paint = shifted(BOSNIA_YELLOW[None], BOSNIA_YELLOW_OFFICIAL, GOLD)[0]
    cover = np.maximum(design['shape'], 1e-6)[..., None]
    paint_k = (blue[..., None] * blue_paint + (gold + land)[..., None] * gold_paint
               + stars[..., None] * base) / cover
    out = paint_k * u[..., None] + base * w[..., None]
    out_alpha = 255 * np.clip(design['shape'], 0, 1) * chips

    # Relief: the wear's, and the border, stars and map raised as Bosnia's stars.
    raised = np.clip(gold + stars + land, 0, 1) > 0.5
    bx_, by_ = bevel(raised, 1.5)
    n_out = bosnia_n.copy()
    n_out[..., 3] = np.clip(128 + 127 * (nx + kx * bx_), 0, 255)
    n_out[..., 1] = np.clip(128 + 127 * (ny + ky * by_), 0, 255)

    gloss_k = (blue * gloss_b[1] + (gold + land) * gloss_b[2] + stars * gloss_b[2]) / cover[..., 0]
    s_out = np.where(rim_k, rim_s[level], gloss_k * m)
    s_out = np.repeat(np.clip(s_out, 0, 255)[..., None], 3, axis=2)

    diffuse = np.concatenate([np.clip(out, 0, 255), out_alpha[..., None]], axis=2)
    return diffuse, n_out, s_out


def coverage(mask, box, size=SIZE):
    """A high-resolution mask boxed down into (x0, y0, x1, y1) of a SIZE x SIZE canvas."""
    from PIL import Image
    x0, y0, x1, y1 = box
    small = Image.fromarray((mask * 255).astype(np.uint8)).resize((x1 - x0, y1 - y0), Image.Resampling.BOX)
    canvas = np.zeros((size, size))
    canvas[y0:y1, x0:x1] = np.asarray(small, dtype=np.float64) / 255
    return canvas


def nearest(page, colours):
    """Each pixel's index of its nearest colour (the official files' flat fills)."""
    return np.argmin(np.stack([np.linalg.norm(page - np.asarray(c, dtype=np.float64), axis=2)
                               for c in colours]), axis=0)


def design_flag(page, field):
    """The official flag's stars and map over the patch's field: scaled by the field's height,
    centred (the patch is wider than 2:3; the field's blue takes the rest)."""
    kind = nearest(page, (BLUE, GOLD, WHITE, CAPTION, FRAME))
    ys, xs = np.nonzero(kind == 0)
    crop = kind[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    x0, y0, x1, y1 = field
    h = y1 - y0
    w = int(round(crop.shape[1] * h / crop.shape[0]))
    cx = (x0 + x1) // 2 - w // 2
    box = (cx, y0, cx + w, y1)
    return coverage(crop == 2, box), coverage(crop == 1, box)


def design_shield(page, bosnia_alpha):
    """The official coat of arms over Bosnia's shield: as tall as it, centred on it."""
    kind = nearest(page, (BLUE, GOLD, WHITE, CAPTION, FRAME))
    white = kind == 2
    # Outside: the page's white and its frame line, from the page's edge in.
    labels, _ = nd.label(white | (kind == 4))
    border = np.unique(np.concatenate([labels[0], labels[-1], labels[:, 0], labels[:, -1]]))
    outside = np.isin(labels, border[border > 0])
    ys, xs = np.nonzero(~outside)
    sl = (slice(ys.min(), ys.max() + 1), slice(xs.min(), xs.max() + 1))
    shape, kind, white = ~outside[sl], kind[sl], white[sl]
    # The gold: the border band and the map, told apart by touching the outline's ring.
    gold = (kind == 1) & shape
    glabels, gn = nd.label(gold)
    sizes = nd.sum(np.ones_like(glabels), glabels, range(1, gn + 1))
    by, bx = np.nonzero(bosnia_alpha > 128)
    h = by.max() + 1 - by.min()
    w = int(round(shape.shape[1] * h / shape.shape[0]))
    cx = (bx.min() + bx.max() + 1) // 2 - w // 2
    box = (cx, by.min(), cx + w, by.max() + 1)
    ring = nd.binary_dilation(~shape, iterations=max(3, shape.shape[0] // 150)) & shape
    band_ids = np.unique(glabels[ring & gold])
    band = np.isin(glabels, band_ids[band_ids > 0])
    return {'shape': coverage(shape, box), 'gold': coverage(band, box),
            'stars': coverage(white & shape, box), 'land': coverage(gold & ~band, box)}
