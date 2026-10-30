"""The duct tape's look: a photo-scanned tape set (tape_source.json, files from $TAPE_SOURCE) wound
round a limb several times, drawn into one atlas the tape shells of every item share.

The atlas (ATLAS_W x ATLAS_H) has four cells, a limb's wrap in each: rows arm (top) and thigh
(bottom), columns the left and the right side. A cell's u is the angle round the limb's bone (a
little more than one turn, MARGIN of a turn before and after, so a shell triangle across the
start of the turn still lands in its own cell), its v the distance along the bone from the wrap's
centre, -HALF_MM to +HALF_MM. In a cell, the tape is one strip wound TURNS times: each turn a
little higher or lower than the last (DRIFT_MM) and tilted its own way (TILT_MM), so the edges of
the turns underneath show as layered edges; its free end is the scan's torn end, its first end
the scan's other torn end. The strip is the scan's pieces end to end (the weave, wrinkles and
folds of the photo), tinted by multiplying the grey scan with the team colour.

Maps (all in the atlas's u, v): per colour an RGBA diffuse (alpha the tape, for the alpha test),
one tangent-space normal map (from the layers' height and the scan's own relief) and one
roughness map.
"""
import json
import os
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE_SPEC = os.path.join(HERE, 'tape_source.json')

ATLAS_W, ATLAS_H = 2048, 512
MARGIN = 0.12                 # of a turn, each side of a cell's turn
HALF_MM = 48.0                # a cell's half height along the bone
LIMBS = {
    # name: (atlas row, nominal radius mm for the tape's length round it)
    'arm': (0, 58.0),
    'thigh': (1, 82.0),
}
SIDES = ['Left', 'Right']     # atlas columns

# The wrap (the owner's picks to confirm): three turns and a bit, the top turn ending in a torn
# flap; the turns walk DRIFT_MM along the limb and lean TILT_MM, each its own way.
TURNS = 2.72
DRIFT_MM = 8.5
TILT_MM = 5.5
THICK_MM = 0.35               # one layer of tape
TEAM = {
    # sRGB of the tape's mean: a strong blue and a strong red, a little worn.
    'blue': (34, 78, 170),
    'red': (182, 34, 30),
}
SCALE = 0.5                   # the scan is read at half its size (3.7 px/mm at 2K)
END_MM = 30.0                 # the torn ends' length taken from the scan


def _load(path, mode):
    im = Image.open(path).convert(mode)
    w, h = im.size
    return np.asarray(im.resize((int(w * SCALE), int(h * SCALE)), Image.LANCZOS)).astype(np.float32) / 255.0


class Roll:
    """The scan's pieces end to end: arrays [length rows, width cols] of luminance, alpha, height
    and roughness; `ppm` pixels per mm; row 0 is just before the free (torn) end."""

    def __init__(self, folder=None):
        spec = json.load(open(SOURCE_SPEC))
        folder = folder or os.environ.get('TAPE_SOURCE')
        if not folder:
            raise SystemExit('TAPE_SOURCE is not set: the folder of %s' % spec['source'])
        f = spec['files']
        col = _load(os.path.join(folder, f['color']), 'L')
        op = _load(os.path.join(folder, f['opacity']), 'L')
        hi = _load(os.path.join(folder, f['height']), 'L')
        ro = _load(os.path.join(folder, f['roughness']), 'L')
        self.source = spec['source']
        pieces = []
        width = None
        for p in spec['pieces']:
            x0, x1, y0, y1 = (int(p[k] * SCALE) for k in ('x0', 'x1', 'y0', 'y1'))
            a = op[y0:y1, x0:x1]
            # The tape's own side edges: where the piece is opaque in most rows.
            cols = np.nonzero((a > 0.5).mean(0) > 0.5)[0]
            c0, c1 = cols.min(), cols.max() + 1
            m = int(round((c1 - c0) * 0.04))
            c0, c1 = max(c0 - m, 0), min(c1 + m, x1 - x0)
            sl = (slice(y0, y1), slice(x0 + c0, x0 + c1))
            arrs = [col[sl], op[sl], hi[sl], ro[sl]]
            if p.get('reverse'):
                arrs = [x[::-1] for x in arrs]
            if width is None:
                width = arrs[0].shape[1]
            arrs = [np.asarray(Image.fromarray(x).resize((width, x.shape[0]), Image.BILINEAR)) for x in arrs]
            pieces.append(arrs)
        self.margin = 0.04 / 1.08          # of the width, outside each edge
        self.ppm = width * (1 - 2 * self.margin) / spec['width_mm']
        joint = int(spec['joint'] * SCALE)
        out = [x.copy() for x in pieces[0]]
        self.starts = [0]
        for arrs in pieces[1:]:
            n = out[0].shape[0]
            # The next piece starts where it is opaque across; cross-faded over `joint` rows.
            first = int(np.argmax((arrs[1] > 0.5).mean(1) > 0.9))
            arrs = [x[first:] for x in arrs]
            last = n - 1 - int(np.argmax(((out[1] > 0.5).mean(1) > 0.9)[::-1]))
            out = [x[:last + 1] for x in out]
            t = np.linspace(0, 1, joint)[:, None]
            blend = [o[-joint:] * (1 - t) + a[:joint] * t for o, a in zip(out, arrs)]
            blend[1] = np.maximum(out[1][-joint:], arrs[1][:joint])
            self.starts.append(out[0].shape[0] - joint)
            out = [np.concatenate([o[:-joint], b, a[joint:]]) for o, b, a in zip(out, blend, arrs)]
        self.lum, self.alpha, self.height, self.rough = out
        # The free end: the first row the tape is opaque across in half its width.
        self.free_end = int(np.argmax((self.alpha > 0.5).mean(1) > 0.5))
        self.lum_mean = float(self.lum[self.alpha > 0.5].mean())
        self.length_mm = (self.lum.shape[0] - self.free_end) / self.ppm

    def sample(self, d_mm, v, tail_mm=None):
        """(lum, alpha, height, rough) at d_mm from the free end along the tape and v across it
        (0, 1 its edges). Past the roll's end it runs back and forth over its pieces after the
        first (no seam). The alpha is the scan's at the two torn ends (the free end; with tail_mm,
        the tape's whole length, the last END_MM come from the roll's far torn end) and a slightly
        ragged straight edge along the sides, so the pieces' joints never show in it."""
        rows = self.lum.shape[0]
        r = self.free_end + d_mm * self.ppm
        loop0 = self.starts[1] if len(self.starts) > 1 else self.free_end
        last = self._far_end() - 2
        span = last - loop0
        m = np.mod(r - loop0, 2 * span)
        r = np.where(r > last, loop0 + np.where(m < span, m, 2 * span - m), r)
        tail = np.zeros(np.shape(r), bool)
        if tail_mm is not None:
            from_end = (tail_mm - d_mm) * self.ppm
            tail = from_end < END_MM * self.ppm
            r = np.where(tail, self._far_end() - from_end, r)
        cols = self.lum.shape[1]
        c = (self.margin + np.clip(v, -0.2, 1.2) * (1 - 2 * self.margin)) * (cols - 1)
        c = np.clip(c, 0, cols - 1.001)
        r = np.clip(r, 0, rows - 1.001)
        r0, c0 = np.floor(r).astype(int), np.floor(c).astype(int)
        fr, fc = r - r0, c - c0

        def bil(a):
            return ((a[r0, c0] * (1 - fc) + a[r0, c0 + 1] * fc) * (1 - fr)
                    + (a[r0 + 1, c0] * (1 - fc) + a[r0 + 1, c0 + 1] * fc) * fr)
        # The sides: straight, a fraction of a millimetre ragged (fibres), anti-aliased.
        w = float(json.load(open(SOURCE_SPEC))['width_mm'])
        rag = (np.sin(d_mm * 2.9 + 1.3) * 0.5 + np.sin(d_mm * 7.3) * 0.3 + np.sin(d_mm * 17.0 + 2.0) * 0.2) * 0.35 / w
        edge = np.minimum(v - rag, 1 - v + rag) * w * self.ppm
        side = np.clip(edge + 0.5, 0, 1)
        ends = (d_mm < END_MM) | tail
        al = side * np.where(ends, np.clip((bil(self.alpha) - 0.5) * 4 + 0.5, 0, 1), 1.0)
        al = np.where(d_mm < -1, 0.0, al)
        return bil(self.lum), al, bil(self.height), bil(self.rough)

    def _far_end(self):
        """The roll's last row the tape is opaque across in half its width (its far torn end)."""
        if not hasattr(self, '_far'):
            self._far = self.lum.shape[0] - 1 - int(np.argmax(((self.alpha > 0.5).mean(1) > 0.5)[::-1]))
        return self._far


def cell_uv(limb, side, theta, s_mm):
    """Atlas (u, v) of a point of a limb: theta its angle round the bone (radians, one turn from
    the cell's start; may run MARGIN past either end), s_mm its distance along the bone from the
    wrap's centre."""
    row, _ = LIMBS[limb]
    col = SIDES.index(side)
    t = theta / (2 * np.pi)
    u = (col + (t + MARGIN) / (1 + 2 * MARGIN)) * 0.5
    v = (row + (s_mm / HALF_MM + 1) * 0.5) * 0.5
    return u, v


def _noise(shape, scale, seed):
    """Smooth value noise 0..1 of feature size `scale` pixels, periodic along axis 1."""
    rng = np.random.default_rng(seed)
    h, w = shape
    gh, gw = max(int(h / scale) + 2, 2), max(int(round(w / scale)), 2)
    g = rng.random((gh, gw))
    y = np.linspace(0, gh - 1.001, h)[:, None] * np.ones((1, w))
    x = np.linspace(0, gw, w, endpoint=False)[None, :] * np.ones((h, 1))
    y0, x0 = np.floor(y).astype(int), np.floor(x).astype(int)
    fy, fx = y - y0, x - x0
    fy, fx = fy * fy * (3 - 2 * fy), fx * fx * (3 - 2 * fx)
    x1 = (x0 + 1) % gw
    return (g[y0, x0] * (1 - fx) + g[y0, x1] * fx) * (1 - fy) + (g[y0 + 1, x0] * (1 - fx) + g[y0 + 1, x1] * fx) * fy


def creases(rng, length_mm, width_mm, per_m=70):
    """Small creases where the tape bunched: short ridges, most near its edges, mostly across it
    (d0, v0 mm, direction, half length, height)."""
    n = max(int(length_mm / 1000 * per_m), 4)
    d0 = rng.uniform(0, length_mm, n)
    edge = rng.random(n) < 0.75
    v0 = np.where(edge, np.where(rng.random(n) < 0.5, rng.beta(1.2, 6, n), 1 - rng.beta(1.2, 6, n)), rng.random(n)) * width_mm
    ang = np.pi / 2 + rng.normal(0, 0.45, n)
    return [(d0[i], v0[i], np.array([np.cos(ang[i]), np.sin(ang[i])]), rng.uniform(2.5, 8.0), rng.uniform(0.12, 0.3))
            for i in range(n)]


def crease_height(cs, d, vmm):
    out = np.zeros_like(d)
    for d0, v0, dirn, half, hgt in cs:
        px, py = d - d0, vmm - v0
        t = np.clip(px * dirn[0] + py * dirn[1], -half, half)
        qx, qy = px - t * dirn[0], py - t * dirn[1]
        dist2 = qx * qx + qy * qy
        fade = 1 - (t / half) ** 2
        out += hgt * np.exp(-dist2 / (2 * 0.45 ** 2)) * fade
    return out


FLAP_MM = 24.0                # the free end's flap, a little lifted and veering off


def wrap_cell(roll, limb, side, seed):
    """One cell of the atlas: (lum, alpha, height mm, rough), [H, W] each, in the cell's pixels."""
    W, H = ATLAS_W // 2, ATLAS_H // 2
    _, radius = LIMBS[limb]
    rng = np.random.default_rng(seed)
    u = (np.arange(W) + 0.5) / W
    v = (np.arange(H) + 0.5) / H
    theta = (u * (1 + 2 * MARGIN) - MARGIN) * 2 * np.pi
    s = (v * 2 - 1) * HALF_MM
    TH, S = np.meshgrid(theta, s)
    start = rng.uniform(0, 2 * np.pi)          # where the first turn begins, round the limb
    phase = rng.uniform(0, 2 * np.pi, 8)
    drift_dir = rng.choice([-1, 1])
    flap_dir = rng.choice([-1, 1])
    end = TURNS * 2 * np.pi
    total_mm = end * radius
    width = roll_width(roll)
    cs = creases(rng, total_mm, width)
    px_mm = (2 * HALF_MM) / H                  # mm per pixel along the limb
    lum = np.zeros((H, W), np.float32)
    alpha = np.zeros((H, W), np.float32)
    height = np.zeros((H, W), np.float32)
    rough = np.full((H, W), 0.5, np.float32)
    layers = int(np.ceil(TURNS)) + 1
    for k in range(layers):
        phi = np.mod(TH - start, 2 * np.pi) + 2 * np.pi * (k - 1)
        ok = (phi >= 0) & (phi <= end)
        if not ok.any():
            continue
        turn = phi / (2 * np.pi)
        d = (end - phi) * radius
        flap = np.clip(1 - d / FLAP_MM, 0, 1)
        centre = (drift_dir * DRIFT_MM * (turn - TURNS / 2)
                  + TILT_MM * np.sin(phi * 1.13 + phase[0])
                  + 1.2 * np.sin(phi * 2.7 + phase[1])
                  + flap_dir * 6.0 * flap ** 2)
        vt = (S - centre) / width + 0.5
        l, a, h, r = roll.sample(np.where(ok, d, 0), vt, tail_mm=total_mm)
        a = np.where(ok, a, 0).astype(np.float32)
        ch = crease_height(cs, d, vt * width)
        h = (h - 0.5) * 0.45 + ch + flap * 0.7
        # The layer's edge casts a thin shadow on what is under it (wider under the lifted flap),
        # and catches a little light along its own rim.
        rim = np.clip(_blur(a, 1.2 / px_mm) - a, 0, 1)
        lift = _blur(a * flap, 2.5 / px_mm)
        lum = lum * (1 - np.clip(rim * 1.0 + lift * 0.6, 0, 0.55) * (1 - a))
        inner = np.clip(a - _blur(a, 0.8 / px_mm), 0, 1)
        l = l * (1 + 0.35 * inner)
        # Creases shaded by their slope (light from above the limb).
        l = l * (1 + np.clip(np.gradient(ch, axis=0) / px_mm * -2.2, -0.35, 0.35))
        top = a > 0.02
        lum = np.where(top, lum * (1 - a) + l * a, lum)
        rough = np.where(top, rough * (1 - a) + r * a, rough)
        height = np.where(top, height * (1 - a) + (k * THICK_MM + h + THICK_MM) * a, height)
        alpha = np.maximum(alpha, a)
    return lum, alpha, height, rough


def roll_width(roll):
    return float(json.load(open(SOURCE_SPEC))['width_mm'])


def _blur(a, r):
    from paint import blur
    return blur(a, r)


def atlas(roll, seed=7):
    """The atlas maps: ({colour: RGBA uint8}, normal RGBA uint8 (X in alpha, Y in green, the
    game's convention), roughness L uint8)."""
    W, H = ATLAS_W // 2, ATLAS_H // 2
    lum = np.zeros((ATLAS_H, ATLAS_W), np.float32)
    alpha = np.zeros_like(lum)
    height = np.zeros_like(lum)
    rough = np.zeros_like(lum)
    for limb, (row, radius) in LIMBS.items():
        for col, side in enumerate(SIDES):
            l, a, h, r = wrap_cell(roll, limb, side, seed + 10 * row + col)
            if side == 'Left':
                # The left arm's u runs the other way round the limb (mirror): flip so its
                # tape reads as the right's mirror image.
                pass
            sl = (slice(row * H, (row + 1) * H), slice(col * W, (col + 1) * W))
            lum[sl], alpha[sl], height[sl], rough[sl] = l, a, h, r
    # Ambient occlusion from the layers' relief: hollows (the step under a turn's edge) darker.
    cav = np.clip((_blur(height, 2.5) - height) * 2.4, 0, 0.5)
    occ = 1 - cav
    # Wear: a little dust and fading, large and soft.
    wear = _noise(lum.shape, 60, 3) * 0.6 + _noise(lum.shape, 13, 4) * 0.4
    detail = np.clip(lum / max(roll.lum_mean, 1e-3), 0.5, 1.5) ** 2.2 * occ
    maps = {}
    for name, rgb in TEAM.items():
        c = (np.array(rgb, np.float32) / 255.0) ** 2.2
        lin = detail[..., None] * c[None, None, :]
        dust = np.array([0.42, 0.40, 0.37], np.float32) ** 2.2 * 0.55
        k = (0.10 + 0.14 * wear)[..., None]
        lin = lin * (1 - k) + dust * detail[..., None] * k
        srgb = np.clip(lin, 0, 1) ** (1 / 2.2)
        maps[name] = np.dstack([srgb, alpha]).clip(0, 1)
    # Tangent space normals of the relief (mm): u is ATLAS_W / 2 per (1 + 2 MARGIN) turn round a
    # limb of the cell's radius, v ATLAS_H / 2 per 2 HALF_MM.
    mm_u = np.zeros((1, ATLAS_W), np.float32)
    nrm_h = _blur(height, 0.7)
    gu = np.zeros_like(height)
    gv = np.gradient(nrm_h, axis=0) / ((2 * HALF_MM) / H)
    for limb, (row, radius) in LIMBS.items():
        per = (1 + 2 * MARGIN) * 2 * np.pi * radius / W
        rows = slice(row * H, (row + 1) * H)
        gu[rows] = np.gradient(nrm_h[rows], axis=1) / per
    del mm_u
    n = np.dstack([-gu, -gv, np.ones_like(gu)])
    n /= np.linalg.norm(n, axis=2, keepdims=True)
    # The game's maps keep X = n.x and Y = -n.y against a binormal along +v (measured on the stock
    # torso and legs maps: X follows -dL/du, Y follows +dL/dv).
    X = n[..., 0] * 0.5 + 0.5
    Y = -n[..., 1] * 0.5 + 0.5
    normal = np.dstack([np.zeros_like(X), Y, np.zeros_like(X), X])
    # Off the tape: flat.
    flat = np.array([0, 0.5, 0, 0.5], np.float32)
    normal = np.where(alpha[..., None] > 0.02, normal, flat)
    rough_map = np.where(alpha > 0.02, np.clip(rough * 0.85, 0, 1), 0.6)
    return ({k: (v * 255).round().astype(np.uint8) for k, v in maps.items()},
            (normal * 255).round().astype(np.uint8), (rough_map * 255).round().astype(np.uint8),
            dict(alpha=alpha, height=height))


_ALPHA = None


def atlas_alpha():
    """The atlas's alpha (the tape's footprint in every cell), made once per process."""
    global _ALPHA
    if _ALPHA is None:
        _ALPHA = atlas(Roll())[3]['alpha']
    return _ALPHA
