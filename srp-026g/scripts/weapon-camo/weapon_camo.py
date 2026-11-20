#!/usr/bin/env python3
"""weapon_camo.py: the `_aoac` textures that let a weapon wear an armour camouflage.

usage: python3 -I weapon_camo.py <guns.tsv> <game folder> <donor folder> <out folder | ->
           [--v069 <0.69d0 folder>] [--only <texture>] [--camo <camouflage .dds>] [--previews <folder>]
           [--picker <page.html> [--camos <camouflage item folder>]] [--pack <id>@<version>]
           [--bare-masks]

<game folder>   the players' game files at their game paths or flattened as dbgrab writes them
                (models__weapons__uzi.skinned_model__settings): the stock resources.db with every
                offered patch over it, for the models (their meshes) and the textures in guns.tsv
<donor folder>  the same for a later build's files that guns.tsv names with `28:` (0.28a2's
                resources.db: premium weapons whose camouflaged textures our data does not have)
--v069          the same for 0.69d0's, named with `69:` (references.py writes those columns: its
                painted textures and metalness maps of the same UVs)
<out folder>    gets textures/<texture>_aoac.dds and .options for every row and every premium
                texture of the row (the same mask: the same UVs); must not exist yet (-: none)
--previews      also writes <texture>.png there: the plain texture, a painted version, the mask, and
                the texture as the game's bake would make it with --camo (default: the painted version)
--picker        also writes a page to choose the painted parts by hand (needs --camo): the texture
                and the weapon in 3D side by side; click a part on either to paint it or not; the
                page gives the lines of parts.tsv
--pack          the out folder is a content pack (release/content.tsv's kind `pack`): files/<path> and
                pack.json, an add_file for each _aoac the game folder does not have, a replace_file
                (its sha256) for one it has; the name is the folder's under $SKHID_PACKS_DIR
                (<id>-<version>) and the manifest's row (<id>@<version>, the sha256 of pack.json)
--bare-masks    a texture with nothing painted still gets an _aoac, one that paints nothing (G 1, B 0.5,
                R 1: the bake gives the plain texture back), so every surface of a camouflaged weapon
                finds its mask and none fails to load (the weapon packs' sights: weapons-031)
--camos         the camouflage items' configs (gameplay/items/upgrades/camouflages, as dbgrab writes
                them): the picker offers each item's camouflage (textures from the game folder)

Which parts are painted: one rule for every weapon (choose(), README.md), part by part: a part is what
a UV island's texels cover on the model. parts.tsv beside guns.tsv holds the hand picks over it.

The engine bakes a camouflage into a surface's textures (camouflage_decal_texture_type_baker_cook,
bake_decal_composition.ps) only when `<diffuse>_aoac.dds` loads beside the diffuse:
    c = lerp(diffuse, camo(uv * scale), 1 - aoac.G)       G: 1 bare, 0 painted
    c = overlay(c, aoac.B)                                B: 0.5 changes nothing
    c = c * aoac.R                                        R: 1 changes nothing
On a painted part B carries the plain texture's fine shading (scratches, edges) and R its broad
shading, so the camouflage keeps the part's wear and shape; elsewhere G is 1, B 0.5 and R 1: the
bake leaves the texture as it is.
"""
import collections
import hashlib
import json
import os
import struct
import sys

import numpy as np
from PIL import Image, ImageFilter

DXT1_OPTIONS = None        # the stock _aoac's .options, read from the game folder
OPTIONS_TEMPLATE = 'textures/character/human/black_market_01/black_market_01_torso_aoac.options'


class Files:
    """A folder of game files at their game paths or flattened by dbgrab ('/' as '__')."""

    def __init__(self, root):
        self.root = root
        self.flat = set(os.listdir(root)) if os.path.isdir(root) else set()
        self.spans = {}

    def path(self, game_path):
        p = os.path.join(self.root, *game_path.split('/'))
        if os.path.exists(p):
            return p
        f = game_path.replace('/', '__')
        return os.path.join(self.root, f) if f in self.flat else None

    def read(self, game_path):
        p = self.path(game_path)
        if p is None:
            raise FileNotFoundError(game_path)
        return open(p, 'rb').read()

    def surfaces(self, model):
        """The render surfaces of models/<model>.skinned_model (the ones with a converted_model)."""
        d = os.path.join(self.root, 'models', *(model + '.skinned_model/render').split('/'))
        if os.path.isdir(d):
            return sorted(s for s in os.listdir(d) if os.path.exists(os.path.join(d, s, 'converted_model')))
        pre = ('models/%s.skinned_model/render/' % model).replace('/', '__')
        return sorted(f[len(pre):-len('__converted_model')] for f in self.flat
                      if f.startswith(pre) and f.endswith('__converted_model'))

    def mesh(self, model, surface):
        return mesh(self.read('models/%s.skinned_model/render/%s/converted_model' % (model, surface)))

    def span(self, model):
        """A weapon model from butt to muzzle: (z0, z1) of all its surfaces."""
        if model not in self.spans:
            z = np.concatenate([self.mesh(model, s)[2][:, 2] for s in self.surfaces(model)])
            self.spans[model] = (float(z.min()), float(z.max()))
        return self.spans[model]


def config_value(buf, path):
    """A value of a Vostok binary config (24-byte values, names by offset)."""
    off = 0
    for part in path.split('/'):
        data, _, _, t, cnt = struct.unpack_from('<QQIHH', buf, off)
        if t not in (3, 4):
            return None
        for i in range(cnt):
            c = data + 24 * i
            name_off = struct.unpack_from('<Q', buf, c + 8)[0]
            name = buf[name_off:buf.index(b'\0', name_off)].decode() if t == 3 else str(i)
            if name == part:
                off = c
                break
        else:
            return None
    data, _, _, t, cnt = struct.unpack_from('<QQIHH', buf, off)
    if t == 5:
        return buf[data:data + cnt - 1].decode()
    if t == 2:
        return struct.unpack('<f', struct.pack('<I', data & 0xffffffff))[0]
    if t in (0, 1):
        return data & 0xffffffff
    return [config_value(buf, path + '/' + str(i)) for i in range(cnt)] if t == 4 else None


def mesh(buf):
    """(uv[n,2], triangles[m,3], xyz[n,3]) of a converted_model: a vertex chunk (3) of `count` vertices,
    each starting with its position (three floats) and ending with its UV (two floats), then the 16-bit
    index chunk (4)."""
    kind, size, count = struct.unpack_from('<III', buf, 0)
    assert kind == 3 and (size - 4) % count == 0, 'no vertex chunk'
    stride = (size - 4) // count
    raw = np.frombuffer(buf, dtype=np.uint8, count=count * stride, offset=12).reshape(count, stride)
    uv = raw[:, stride - 8:].copy().view('<f4').reshape(count, 2).astype(np.float64)
    xyz = raw[:, :12].copy().view('<f4').reshape(count, 3).astype(np.float64)
    at = 12 + count * stride
    kind, _, indices = struct.unpack_from('<III', buf, at)
    assert kind == 4 and indices % 3 == 0, 'no index chunk'
    tris = np.frombuffer(buf, dtype='<u2', count=indices, offset=at + 12).reshape(-1, 3).astype(np.int64)
    return uv, tris, xyz


def mesh_uv(buf):
    return mesh(buf)[:2]


def island_triangles(meshes):
    """(island number of each triangle, its UV corners [t,3,2], whether it tiles past 0..1) for the
    triangles of `meshes` ([(uv, triangles, ...)]) in their order: two triangles are in one island when
    they share an edge in UV (parts packed side by side that only touch at a corner stay apart)."""
    parent = []

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    edges = {}
    for uv, tris, *_ in meshes:
        keys = [tuple(k) for k in np.round(uv * 4096).astype(np.int64)]
        for t in tris:
            n = len(parent)
            parent.append(n)
            ks = [keys[i] for i in t]
            for a, b in ((0, 1), (1, 2), (2, 0)):
                edge = (min(ks[a], ks[b]), max(ks[a], ks[b]))
                other = edges.setdefault(edge, n)
                ra, rb = find(other), find(n)
                if ra != rb:
                    parent[rb] = ra
    if not parent:
        return np.zeros(0, np.int64), np.zeros((0, 3, 2)), np.zeros(0, bool)
    corners = np.concatenate([uv[tris] for uv, tris, *_ in meshes])
    # a part laid out whole UVs away (the TOZ-66's, u -6..11) samples the texture where its
    # fraction falls: move each triangle by the whole part of its centre
    corners = corners - np.floor(corners.mean(axis=1))[:, None, :]
    outside = np.maximum(0, np.abs(corners - 0.5) - 0.5).max(axis=(1, 2)) > 1e-3
    numbers = {}
    for i in np.argsort(outside, kind='stable'):       # the islands inside 0..1 numbered first
        numbers.setdefault(find(int(i)), len(numbers) + 1)
    return np.array([numbers[find(i)] for i in range(len(parent))]), corners, outside


def islands(meshes, size, triangles=None):
    """A raster [h,w] of UV island numbers (0 = no island) for the triangles of `meshes`
    (island_triangles(), given or made)."""
    h, w = size
    labels = np.zeros((h, w), dtype=np.int32)
    ids, all_corners, outside = triangles if triangles is not None else island_triangles(meshes)
    # triangles inside 0..1 first; one that tiles past it (a barrel's repeated strip) only fills
    # texels no other island has, so it never takes a part's texels
    for k in np.argsort(outside, kind='stable'):
        corners, n = all_corners[k], ids[k]
        x = corners[:, 0] * w - 0.5
        y = corners[:, 1] * h - 0.5
        x0, x1 = int(np.floor(x.min())), int(np.ceil(x.max()))
        y0, y1 = int(np.floor(y.min())), int(np.ceil(y.max()))
        x0, y0 = max(x0, -w * 4), max(y0, -h * 4)
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1), np.arange(y0, y1 + 1))
        (ax, bx, cx), (ay, by, cy) = x, y
        den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
        if abs(den) < 1e-12:
            continue
        l1 = ((by - cy) * (gx - cx) + (cx - bx) * (gy - cy)) / den
        l2 = ((cy - ay) * (gx - cx) + (ax - cx) * (gy - cy)) / den
        inside = (l1 >= -0.02) & (l2 >= -0.02) & (1 - l1 - l2 >= -0.02)
        px, py = gx[inside] % w, gy[inside] % h            # UVs past 0..1 wrap, as the sampler does
        if outside[k]:
            free = labels[py, px] == 0
            px, py = px[free], py[free]
        labels[py, px] = n
    return labels


def load_rgb(buf):
    import io
    return np.asarray(Image.open(io.BytesIO(buf)).convert('RGB'), dtype=np.float32) / 255


def resize(img, size):
    h, w = size
    if img.shape[:2] == (h, w):
        return img
    a = Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8))
    return np.asarray(a.resize((w, h), Image.LANCZOS), dtype=np.float32) / 255


def blur(a, r):
    if r <= 0:
        return a
    img = Image.fromarray(np.clip(a * 255, 0, 255).astype(np.uint8))
    return np.asarray(img.filter(ImageFilter.GaussianBlur(r)), dtype=np.float32) / 255


def lum(c):
    return c @ np.array([0.299, 0.587, 0.114], dtype=np.float32)


def grow(labels, steps):
    """Each island's label spread `steps` texels into the bare texels around it (the bake samples
    across the edge at lower mips)."""
    out = labels.copy()
    for _ in range(steps):
        empty = out == 0
        for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            moved = np.roll(np.roll(out, dy, 0), dx, 1)
            take = empty & (moved > 0)
            out[take] = moved[take]
            empty = out == 0
    return out


DIFFER = 0.10      # a texel's colour differs from the plain texture's (smoothed, 0..1 per channel)

# A part of a weapon: what one UV island's texels cover on it. `area` m2; `length` its extent along
# the weapon (z: the bore, the muzzle at +z), m; `tube`: the radius of the tube about that axis it is
# (m), None when it is none; `along`: where its middle is on the weapon, 0 the butt, 1 the muzzle;
# `texels` in the raster.
Part = collections.namedtuple('Part', 'area length tube along texels')
ROUND = 0.10        # a tube: its corners within this share of the radius from a circle about its axis ...
FACET = 0.15        # ... and its facets' middles within this one (an octagon's are 0.08, a box's 0.2 and more)
RING = 75.0         # degrees: a whole tube leaves no wider gap around its axis; a part that does (a
SHELL = 8.0         # slide, a receiver cover, open below) is a tube only this many diameters long (a
                    # barrel unwrapped in halves)


def tube(triangles):
    """The radius of the tube triangles ([t,3,3], z along the bore) are the wall of, or None. About the
    least-squares axis of their corners: each corner as far from it as the others at its place along
    the bore (a barrel tapers), each facet's middle nearly so (a box's corners are on a circle too, its
    sides are not), and corners all around it."""
    points = np.unique(triangles.reshape(-1, 3), axis=0)
    xy = np.unique(points[:, :2], axis=0)
    if len(xy) < 6:
        return None
    a = np.column_stack([xy, np.ones(len(xy))])
    (cx, cy, _), *_ = np.linalg.lstsq(a, (xy ** 2).sum(axis=1), rcond=None)
    centre = np.array([cx, cy]) / 2
    far = np.hypot(*(points[:, :2] - centre).T)
    radius = float(np.median(far))
    if radius <= 0 or radius > np.ptp(xy, axis=0).max():
        return None
    wall = far > 0.5 * radius                              # not an end cap's middle
    place = np.round(points[:, 2] / 0.002).astype(np.int64)
    order = np.lexsort((far, place))
    starts = np.flatnonzero(np.r_[True, place[order][1:] != place[order][:-1]])
    ends = np.r_[starts[1:], len(order)]
    local = np.empty(len(points))
    for s, e in zip(starts, ends):                         # the corners at one place along the bore
        local[order[s:e]] = np.median(far[order[s:e]][far[order[s:e]] > 0.5 * radius]) if wall[order[s:e]].any() else radius
    if np.percentile(np.abs(far - local)[wall] / radius, 75) > ROUND:
        return None
    ends_a, ends_b = triangles, np.roll(triangles, 1, axis=1)
    across = np.abs(ends_a[..., 2] - ends_b[..., 2]) < 0.25 * np.hypot(*(ends_a[..., :2] - ends_b[..., :2]).transpose(2, 0, 1))
    if across.any():                                       # the facets' edges around the bore
        da, db = (np.hypot(*(e[..., :2] - centre).transpose(2, 0, 1)) for e in (ends_a, ends_b))
        middle = np.hypot(*((ends_a[..., :2] + ends_b[..., :2]) / 2 - centre).transpose(2, 0, 1))
        rim = across & (da > 0.5 * radius) & (db > 0.5 * radius)
        if rim.any() and np.percentile(1 - middle[rim] / ((da + db) / 2)[rim], 75) > FACET:
            return None
    angle = np.sort(np.degrees(np.arctan2(*(points[wall][:, 1::-1] - centre[::-1]).T)))
    gap = float(np.diff(np.r_[angle, angle[0] + 360]).max())
    if gap > RING and np.ptp(points[:, 2]) < SHELL * 2 * radius:
        return None
    return radius


def parts_of(meshes, labels, triangles, spans):
    """{island: Part} of the islands' raster. A triangle belongs to the island that has the texel under
    its centre: halves mirrored over one another in UV are one part (they take the same paint).
    Measured on the mesh that has most of the part (two guns sharing a texture each have it); `spans`:
    each mesh's weapon from butt to muzzle, (z0, z1)."""
    h, w = labels.shape
    _, corners, _ = triangles
    centre = corners.mean(axis=1)
    owner = labels[(centre[:, 1] * h).astype(int) % h, (centre[:, 0] * w).astype(int) % w]
    texels = np.bincount(labels.ravel(), minlength=int(labels.max()) + 1)
    out, at = {}, 0
    for (uv, tris, xyz), (z0, z1) in zip(meshes, spans):
        mine = owner[at:at + len(tris)]
        at += len(tris)
        p = xyz[tris]
        area = 0.5 * np.linalg.norm(np.cross(p[:, 1] - p[:, 0], p[:, 2] - p[:, 0]), axis=1)
        for i in np.unique(mine[mine > 0]):
            sel = mine == i
            total = float(area[sel].sum())
            if i in out and out[i].area >= total:
                continue
            lo, hi = p[sel][:, :, 2].min(), p[sel][:, :, 2].max()
            along = ((lo + hi) / 2 - z0) / max(z1 - z0, 1e-6)
            out[int(i)] = Part(total, float(hi - lo), tube(p[sel]), float(along), int(texels[i]))
    return out


def repainted(plain, painted):
    """[h,w] bool: the texels a painted version of a texture (a premium, a 0.69d0 skin) painted over."""
    smooth = lambda img: np.stack([blur(img[..., k], 2) for k in range(3)], 2)
    return np.abs(smooth(painted) - smooth(plain)).max(axis=2) > DIFFER


def shares(labels, raster):
    """{island: the mean of `raster` over its texels}."""
    n = int(labels.max()) + 1
    count = np.bincount(labels.ravel(), minlength=n)
    total = np.bincount(labels.ravel(), weights=raster.ravel().astype(np.float64), minlength=n)
    return {i: total[i] / count[i] for i in range(1, n) if count[i]}


# The rule (docs/notes/weapon-camo.md, "Where the paint goes"): a weapon is painted on its body, all
# of its outside, and not on what stays bare metal on a painted gun.
TINY = 3e-4         # m2: under 3 cm2 a part is a pin, a screw, a trigger, a sight's blade
METAL = 0.10        # 0.69d0's metalness map: a part under this is polymer, wood or bakelite
FROM = 0.3          # a metal tube not at the butt (a wire stock's struts) is
ROD = 0.014         # m: a rod under this radius: barrel, gas tube, piston, cleaning rod, magazine tube;
SLIM = 0.020        # m: a slim tube under this one, when it is ...
LONG = 2.0          # ... this many times longer than wide: a heavy barrel, a suppressor
BODY = 10e-4        # m2: any other metal part of 10 cm2 is a piece of the body (receiver, cover, slide)
REPAINTED = 0.5     # a painted version (a premium, a 0.69d0 skin) paints a part when it repaints this much


def choose(parts, metal, repaint):
    """{island: why} of the parts that take the camouflage. `metal` {island: metalness} of the texture's
    0.69d0 metalness map ({} without one: every part counts as metal), `repaint` {island: the largest
    share of it a painted version of the weapon repaints} ({} without one). What is not metal is painted;
    of the metal a rod never, a slim tube and a small part only where a painted version paints it, the
    rest (the body) always."""
    chosen = {}
    for i, part in parts.items():
        if part.area < TINY or not part.texels:
            continue
        tube = part.tube is not None and part.along >= FROM
        if metal and metal.get(i, 1.0) < METAL:
            chosen[i] = 'not metal'
        elif tube and part.tube < ROD:
            continue
        elif part.area >= BODY and not (tube and part.tube < SLIM and part.length >= LONG * 2 * part.tube):
            chosen[i] = 'body'
        elif repaint.get(i, 0.0) >= REPAINTED:
            chosen[i] = 'repainted'
    return chosen


def aoac(base, mask, labels, detail=4.0, ao=0.7):
    """The aoac texture [h,w,3] 0..1 for the plain texture and its paint mask. A painted part is shaded
    against its own bright level (the 80th percentile of its texels' luminance), not the weapon's:
    paint covers a dark part as it covers a light one (until 2026-11-20 one level for the whole
    texture, and the camouflage came out at half its brightness on a black receiver beside tan
    furniture)."""
    part = grow(labels * mask, 6)
    owner = part > 0
    soft = blur(owner.astype(np.float32), 1.0)
    L = lum(base)
    high = L - blur(L, 4)
    B = 0.5 + np.clip(detail * high, -0.5, 0.5) * soft
    broad = blur(L, 12)
    ids = (labels * mask).ravel()
    order = np.lexsort((L.ravel(), ids))
    count = np.bincount(ids, minlength=int(part.max()) + 1)
    bright = L.ravel()[order][np.maximum(np.cumsum(count) - 1 - (0.2 * count).astype(np.int64), 0)]
    ref = np.maximum(np.where(count > 0, bright, 1.0)[part], 1e-3)
    R = 1 - ao * np.clip(1 - broad / ref, 0, 1) * soft
    G = 1 - soft
    return np.stack([R, G, B], 2)


def bake(base, camo, a, scale):
    """What bake_decal_composition makes of the diffuse (the camouflage tiled `scale` times)."""
    h, w = base.shape[:2]
    ch, cw = camo.shape[:2]
    v, u = np.mgrid[0:h, 0:w].astype(np.float32)
    tile = camo[((v / h * scale) * ch).astype(int) % ch, ((u / w * scale) * cw).astype(int) % cw]
    paint = (1 - a[..., 1:2])
    c = base + (tile - base) * paint
    B = a[..., 2:3]
    c = np.where(c < 0.5, 2 * c * B, 1 - 2 * (1 - c) * (1 - B))
    return np.clip(c * a[..., 0:1], 0, 1)


# DXT1 (BC1) with mips, as the stock _aoac files: 4-colour blocks, endpoints the block's range
def bc1_blocks(img):
    h, w = img.shape[:2]
    px = (np.clip(img, 0, 1) * 255).reshape(h // 4, 4, w // 4, 4, 3).transpose(0, 2, 1, 3, 4).reshape(-1, 16, 3)
    lo, hi = px.min(axis=1), px.max(axis=1)

    def to565(c):
        c = np.clip(np.round(c), 0, 255).astype(np.int32)
        return ((c[:, 0] >> 3) << 11) | ((c[:, 1] >> 2) << 5) | (c[:, 2] >> 3)

    def from565(v):
        r, g, b = (v >> 11) & 31, (v >> 5) & 63, v & 31
        return np.stack([(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)], 1).astype(np.float32)

    c0, c1 = to565(hi), to565(lo)
    swap = c0 < c1
    c0, c1 = np.where(swap, c1, c0), np.where(swap, c0, c1)
    same = c0 == c1
    e0, e1 = from565(c0), from565(c1)
    palette = np.stack([e0, e1, (2 * e0 + e1) / 3, (e0 + 2 * e1) / 3], 1)          # [n,4,3]
    dist = ((px[:, :, None, :] - palette[:, None, :, :]) ** 2).sum(-1)              # [n,16,4]
    idx = dist.argmin(-1).astype(np.uint32)
    idx[same] = 0
    bits = (idx << (2 * np.arange(16, dtype=np.uint32))).sum(1).astype(np.uint32)
    out = np.zeros((len(px), 2), dtype=np.uint32)
    out[:, 0] = c0.astype(np.uint32) | (c1.astype(np.uint32) << 16)
    out[:, 1] = bits
    return out.astype('<u4').tobytes()


def dds_dxt1(img):
    h, w = img.shape[:2]
    levels, body = [], b''
    level = img
    while True:
        body += bc1_blocks(level)
        levels.append(level.shape[:2])
        if min(level.shape[:2]) <= 4:
            break
        level = level.reshape(level.shape[0] // 2, 2, level.shape[1] // 2, 2, 3).mean(axis=(1, 3))
    # as the stock textures have it: flags 0xA100F, the "linear size" the bytes after the header,
    # which end with one byte after the last mip
    body += b'\0'
    header = struct.pack('<4sIIIIIII44sIIIIIIIIIIIII', b'DDS ', 124, 0xA100F, h, w, len(body), 0,
                         len(levels), bytes(44), 32, 4, 0x31545844, 0, 0, 0, 0, 0, 0x401008, 0, 0, 0, 0)
    return header + body


def island_points(labels):
    """{island: (u, v)}: a texel of each island, the one nearest its centre, as parts.tsv names it."""
    h, w = labels.shape
    ys, xs = np.nonzero(labels)
    ids = labels[ys, xs]
    if not len(ids):
        return {}
    order = np.argsort(ids, kind='stable')
    ys, xs, ids = ys[order], xs[order], ids[order]
    starts = np.flatnonzero(np.r_[True, ids[1:] != ids[:-1]])
    out = {}
    for s, e in zip(starts, np.r_[starts[1:], len(ids)]):
        cy, cx = ys[s:e].mean(), xs[s:e].mean()
        k = s + int(np.argmin((ys[s:e] - cy) ** 2 + (xs[s:e] - cx) ** 2))
        out[int(ids[s])] = ((xs[k] + 0.5) / w, (ys[k] + 0.5) / h)
    return out


def read_parts(path):
    """parts.tsv: {texture: [(paint, u, v)]}: the hand picks over the rule, a UV point on each part:
    `+u,v` paints it, `-u,v` leaves it bare. A texture with no points stays plain."""
    parts = {}
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if not line or line.startswith('#'):
            continue
        texture, _, points = line.partition('\t')
        picks = []
        for p in points.split():
            if p[0] not in '+-':
                sys.exit('%s: %s: a pick is +u,v (paint the part) or -u,v (leave it bare)' % (path, texture))
            u, v = (float(c) for c in p[1:].split(','))
            picks.append((p[0] == '+', u, v))
        parts[texture] = picks
    return parts


def picked(chosen, picks, labels, texture):
    """The rule's parts with the texture's hand picks applied."""
    if not picks:
        return set()
    h, w = labels.shape
    chosen = set(chosen)
    for paint, u, v in picks:
        i = int(labels[min(int(v * h), h - 1), min(int(u * w), w - 1)])
        if not i:
            sys.exit('%s: parts.tsv point %.4f,%.4f is on no part (the UVs changed? pick again)' % (texture, u, v))
        (chosen.add if paint else chosen.discard)(i)
    return chosen


PICKER = 1024      # the picker page's texels a side


def data_url(img, fmt, **kw):
    import base64
    import io
    b = io.BytesIO()
    img.save(b, fmt, **kw)
    return 'data:image/%s;base64,%s' % (fmt.lower(), base64.b64encode(b.getvalue()).decode())


def picker_entry(texture, base, donor, labels, points, suggested, chosen, from_file, models):
    """The picker page's data for one texture: its images at PICKER texels, the parts' numbers as
    a PNG (r + 256 g), the bake's shading (aoac R in r, B in g, as aoac() makes them on a painted
    part), a point on each part, the suggested parts, the parts painted now and its models."""
    small = lambda img, n=PICKER: Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8)).resize((n, n), Image.LANCZOS)
    L = lum(base)
    broad = blur(L, 12)
    used = broad[labels > 0]
    ref = max(np.percentile(used, 90) if used.size else 1.0, 1e-3)
    shade = np.stack([1 - 0.7 * np.clip(1 - broad / ref, 0, 1), 0.5 + np.clip(4.0 * (L - blur(L, 4)), -0.5, 0.5),
                      np.zeros_like(L)], 2)
    h, w = labels.shape
    ids = labels[(np.arange(PICKER) * h // PICKER)[:, None], (np.arange(PICKER) * w // PICKER)[None, :]].astype(np.uint32)
    rgb = np.stack([ids & 255, ids >> 8, np.zeros_like(ids)], 2).astype(np.uint8)
    return {'texture': texture, 'base': data_url(small(base), 'JPEG', quality=85),
            'donor': data_url(small(donor), 'JPEG', quality=85) if donor is not base else None, 'labels': data_url(Image.fromarray(rgb), 'PNG'),
            'shade': data_url(small(shade, PICKER // 2), 'JPEG', quality=85), 'models': models,
            'points': {str(i): '%.4f,%.4f' % p for i, p in points.items()},
            'suggested': sorted(suggested), 'chosen': sorted(chosen), 'from_file': from_file}


def picker_model(game, model, textures):
    """A model's surfaces for the picker's 3D view: the texture each uses (None: one with no _aoac,
    drawn grey) and its mesh, gzipped: u32 vertices, u32 indices, f32 xyz each, f32 uv each, u16
    indices."""
    import base64
    import gzip
    out = []
    for surface in game.surfaces(model):
        uv, tris, xyz = game.mesh(model, surface)
        blob = struct.pack('<II', len(uv), tris.size) + xyz.astype('<f4').tobytes() + uv.astype('<f4').tobytes() \
            + tris.astype('<u2').tobytes()
        out.append({'surface': surface, 'texture': textures.get('%s:%s' % (model, surface)),
                    'mesh': base64.b64encode(gzip.compress(blob, 9, mtime=0)).decode()})
    return out


def picker_camos(folder, game):
    """[(item, camouflage texture)] of the camouflage items' configs in `folder`."""
    out = []
    for f in sorted(os.listdir(folder)):
        buf = open(os.path.join(folder, f), 'rb').read()
        for i in range(8):
            value = config_value(buf, 'modifications/0/modifiers/%d/value' % i)
            if value is None:
                break
            if config_value(buf, 'modifications/0/modifiers/%d/path' % i) == ['data'] and value[0] == 'camouflage':
                out.append((f.replace('__', '/').rsplit('/', 1)[-1].rsplit('.', 1)[0], value[1]))
    return out


def write_picker(path, entries, camos, models):
    """camos: [(name, image)], the first shown first."""
    import json
    tile = lambda img: data_url(Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8)).resize((512, 512), Image.LANCZOS),
                                'JPEG', quality=85)
    data = {'camos': [{'name': n, 'image': tile(img)} for n, img in camos], 'guns': entries, 'models': models}
    page = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'picker.html'), encoding='utf-8').read()
    assert '/*DATA*/null' in page
    open(path, 'w', encoding='utf-8').write(page.replace('/*DATA*/null', json.dumps(data)))


Row = collections.namedtuple('Row', 'texture painted surfaces premiums metal')


def read_rows(path):
    """The table's rows. `painted`: [(painted version, the plain texture it was painted over)]."""
    rows = []
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if not line or line.startswith('#'):
            continue
        cols = line.split('\t') + [''] * 6
        texture, surfaces = cols[0], cols[2].split(',')
        over = cols[4] or texture
        painted = [tuple((d + '@' + over).split('@')[:2]) for d in cols[1].split(',') if d]
        rows.append(Row(texture, painted, surfaces, [p for p in cols[3].split(',') if p], cols[5] or None))
    return rows


def main():
    args = [a for a in sys.argv[1:]]
    opt = lambda name: args[args.index(name) + 1] if name in args else None
    only, camo_path, previews, picker = opt('--only'), opt('--camo'), opt('--previews'), opt('--picker')
    camos_dir, pack, v069 = opt('--camos'), opt('--pack'), opt('--v069')
    bare_masks = '--bare-masks' in args
    if bare_masks:
        args.remove('--bare-masks')
    for name in ('--only', '--camo', '--previews', '--picker', '--camos', '--pack', '--v069'):
        if name in args:
            i = args.index(name)
            del args[i:i + 2]
    if len(args) != 4 or (picker and not camo_path) or (pack and (args[3] == '-' or only or '@' not in pack)):
        sys.exit(__doc__)
    table, game, donor, out = args
    sources = {'28': Files(donor), '69': Files(v069) if v069 else None}
    game = Files(game)
    if out != '-' and os.path.exists(out):
        sys.exit('%s: already there' % out)
    parts_path = os.path.join(os.path.dirname(os.path.abspath(table)), 'parts.tsv')
    parts = read_parts(parts_path) if os.path.exists(parts_path) else {}
    options = game.read(OPTIONS_TEMPLATE)
    camo = load_rgb(open(camo_path, 'rb').read()) if camo_path else None
    written, entries = [], []

    def texture_of(name, size=None):
        files = game
        if name[2:3] == ':':
            files, name = sources[name[:2]], name[3:]
            if files is None:
                sys.exit('%s: the table names 0.69d0 textures: --v069 <folder>' % table)
        img = load_rgb(files.read('textures/%s.dds' % name))
        return resize(img, size) if size else img

    for row in read_rows(table):
        texture = row.texture
        if only and texture != only:
            continue
        base = texture_of(texture)
        size = base.shape[:2]
        meshes, spans = [], []
        for s in row.surfaces:
            model, surface = s.split(':')
            meshes.append(game.mesh(model, surface))
            spans.append(game.span(model))
        triangles = island_triangles(meshes)
        labels = islands(meshes, size, triangles)
        weapon = parts_of(meshes, labels, triangles, spans)
        metal = shares(labels, texture_of(row.metal, size)[..., 0]) if row.metal else {}
        repaint = {}
        for painted, plain in row.painted:
            for i, share in shares(labels, repainted(texture_of(plain, size), texture_of(painted, size))).items():
                repaint[i] = max(repaint.get(i, 0.0), share)
        first = texture_of(row.painted[0][0], size) if row.painted else base   # what the previews and the picker show
        rule = choose(weapon, metal, repaint)
        chosen = picked(rule, parts[texture], labels, texture) if texture in parts else set(rule)
        mask = np.isin(labels, sorted(chosen))
        a = aoac(base, mask, labels)
        why = collections.Counter(rule.values())
        print('%-58s %3d parts, %3d painted (%s%s), %5.1f%% of its surface'
              % (texture, len(weapon), len(chosen), ', '.join('%d %s' % (n, w) for w, n in sorted(why.items())) or 'none',
                 '; parts.tsv: %d picks' % len(parts[texture]) if texture in parts else '',
                 100 * sum(weapon[i].area for i in chosen) / max(1e-9, sum(p.area for p in weapon.values()))))
        premiums = row.premiums
        suggested = set(rule)
        if picker:
            models = list(dict.fromkeys(s.split(':')[0] for s in surfaces))
            entries.append(picker_entry(texture, base, first, labels, island_points(labels), suggested, chosen,
                                        texture in parts, models))
        if not mask.any() and not bare_masks:
            print('  nothing painted: no _aoac')
            continue
        if not mask.any():
            print('  nothing painted: a bare _aoac (--bare-masks)')
        blob = dds_dxt1(a)
        for name in [texture] + premiums if out != '-' else []:
            dst = os.path.join(out, *(['files'] if pack else []), 'textures', *('%s_aoac' % name).split('/'))
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            open(dst + '.dds', 'wb').write(blob)
            open(dst + '.options', 'wb').write(options)
            written.append(name + '_aoac')
        if previews:
            os.makedirs(previews, exist_ok=True)
            c = camo if camo is not None else first
            shown = bake(base, c, a, 4.0) if camo is not None else first
            sm = lambda img: Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8)).resize((512, 512), Image.LANCZOS)
            row = Image.new('RGB', (2560, 512))
            for k, img in enumerate([base, first, np.repeat(mask[..., None], 3, 2).astype(np.float32), a,
                                     bake(base, c, a, 4.0)]):
                row.paste(sm(img), (512 * k, 0))
            row.save(os.path.join(previews, texture.replace('/', '__') + '.png'))
    if picker:
        textures = {s: row.texture for row in read_rows(table) for s in row.surfaces}
        models = {m: picker_model(game, m, textures) for e in entries for m in e['models']}
        camos = [(os.path.basename(camo_path).rsplit('.', 1)[0].rsplit('__', 1)[-1], camo)]
        if camos_dir:                  # the items instead, --camo's first
            items = []
            for name, texture in picker_camos(camos_dir, game):
                try:
                    items.append((name, texture, load_rgb(game.read('textures/%s.dds' % texture))))
                except FileNotFoundError:
                    print('  camouflage %s: no textures/%s.dds in the game folder' % (name, texture))
            same = lambda t: os.path.basename(camo_path).startswith(('textures/%s.dds' % t).replace('/', '__')) \
                or camo_path.replace('\\', '/').endswith('textures/%s.dds' % t)
            items.sort(key=lambda it: not same(it[1]))
            camos = [(name, img) for name, _, img in items] or camos
        write_picker(picker, entries, camos, models)
        print('picker: %s' % picker)
    if pack:
        write_pack(out, pack, written, game)
    print('%d textures written to %s' % (len(written), out))


def write_pack(out, pack, written, game):
    """pack.json of the pack in `out`: each _aoac's .dds and .options, added (or replacing the game's own,
    pinned by its sha256), in path order."""
    ident, version = pack.split('@', 1)
    operations = []
    for path in sorted({'textures/%s%s' % (name, ext) for name in written for ext in ('.dds', '.options')}):
        try:
            operations.append({'op': 'replace_file', 'path': path,
                               'base_sha256': hashlib.sha256(game.read(path)).hexdigest()})
        except FileNotFoundError:
            operations.append({'op': 'add_file', 'path': path})
    manifest = {'format_version': 1, 'id': ident, 'version': version, 'target_build': '0.26g',
                'description': 'weapon camouflage masks: the _aoac textures of srp-026g/scripts/weapon-camo '
                               '(guns.tsv, parts.tsv), so a weapon wears an armour camouflage (client patch 44)',
                'operations': operations}
    open(os.path.join(out, 'pack.json'), 'w').write(json.dumps(manifest, indent=2) + '\n')
    print('pack %s: %d operations (%d replace an own file); pack.json sha256 %s' % (
        pack, len(operations), sum(o['op'] == 'replace_file' for o in operations),
        hashlib.sha256(open(os.path.join(out, 'pack.json'), 'rb').read()).hexdigest()))


if __name__ == '__main__':
    main()
