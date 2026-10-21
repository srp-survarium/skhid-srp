#!/usr/bin/env python3
"""weapon_camo.py: the `_aoac` textures that let a weapon wear an armour camouflage.

usage: python3 -I weapon_camo.py <guns.tsv> <game folder> <donor folder> <out folder | ->
           [--only <texture>] [--camo <camouflage .dds>] [--previews <folder>]
           [--picker <page.html> [--camos <camouflage item folder>]] [--pack <id>@<version>]

<game folder>   the players' game files at their game paths or flattened as dbgrab writes them
                (models__weapons__uzi.skinned_model__settings): the stock resources.db with every
                offered patch over it, for the models (their meshes' UVs) and the textures in guns.tsv
<donor folder>  the same for a later build's files that guns.tsv names with `28:` (0.28a2's
                resources.db: premium weapons whose camouflaged textures our data does not have)
<out folder>    gets textures/<texture>_aoac.dds and .options for every row and every premium
                texture of the row (the same mask: the same UVs); must not exist yet (-: none)
--previews      also writes <texture>.png there: the plain texture, a premium one, the mask, and the
                texture as the game's bake would make it with --camo (default: the first donor)
--picker        also writes a page to choose the painted parts by hand (needs --camo): the texture
                and the weapon in 3D side by side; click a part on either to paint it or not; the
                page gives the lines of parts.tsv
--pack          the out folder is a content pack (release/content.tsv's kind `pack`): files/<path> and
                pack.json, an add_file for each _aoac the game folder does not have, a replace_file
                (its sha256) for one it has; the name is the folder's under $SKHID_PACKS_DIR
                (<id>-<version>) and the manifest's row (<id>@<version>, the sha256 of pack.json)
--camos         the camouflage items' configs (gameplay/items/upgrades/camouflages, as dbgrab writes
                them): the picker offers each item's camouflage (textures from the game folder)

Which parts are painted: parts.tsv beside guns.tsv names them (a UV point on each part) for the
textures it has a line for; for the others, the large parts the premium painted (suggest()).

The engine bakes a camouflage into a surface's textures (camouflage_decal_texture_type_baker_cook,
bake_decal_composition.ps) only when `<diffuse>_aoac.dds` loads beside the diffuse:
    c = lerp(diffuse, camo(uv * scale), 1 - aoac.G)       G: 1 bare, 0 painted
    c = overlay(c, aoac.B)                                B: 0.5 changes nothing
    c = c * aoac.R                                        R: 1 changes nothing
What the premium weapons' own textures say was painted (most premiums are repainted as a whole,
so this alone paints nearly the whole weapon):
- two premium textures of one weapon (two camouflages over the same base): the texels the two
  leave the same are bare. A part of the model (a UV island) is painted when the two differ on
  most of it and each differs from the plain texture there (a part one of them only tints, the
  Uzi's grips in uzi_camo_st, is no paint);
- one premium texture: a UV island is painted when the premium differs from the plain texture on
  most of it (the premium was painted again as a whole, so texel by texel says too little).
On a painted part B carries the plain texture's fine shading (scratches, edges) and R its broad
shading, so the camouflage keeps the part's wear and shape; elsewhere G is 1, B 0.5 and R 1: the
bake leaves the texture as it is.
"""
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


def mesh_uv(buf):
    """(uv[n,2], triangles[m,3]) of a converted_model: a vertex chunk (3) of `count` vertices whose
    last 8 bytes are the UV as two floats, then the 16-bit index chunk (4)."""
    kind, size, count = struct.unpack_from('<III', buf, 0)
    assert kind == 3 and (size - 4) % count == 0, 'no vertex chunk'
    stride = (size - 4) // count
    raw = np.frombuffer(buf, dtype=np.uint8, count=count * stride, offset=12).reshape(count, stride)
    uv = raw[:, stride - 8:].copy().view('<f4').reshape(count, 2).astype(np.float64)
    at = 12 + count * stride
    kind, _, indices = struct.unpack_from('<III', buf, at)
    assert kind == 4 and indices % 3 == 0, 'no index chunk'
    tris = np.frombuffer(buf, dtype='<u2', count=indices, offset=at + 12).reshape(-1, 3).astype(np.int64)
    return uv, tris


def islands(meshes, size):
    """A raster [h,w] of UV island numbers (0 = no island) for the triangles of `meshes`, two
    triangles in one island when they share an edge in UV (parts packed side by side that only
    touch at a corner stay apart)."""
    h, w = size
    labels = np.zeros((h, w), dtype=np.int32)
    parent = []

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    tri_keys, edges = [], {}
    for uv, tris in meshes:
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
            tri_keys.append((uv[t], n))
    # a part laid out whole UVs away (the TOZ-66's, u -6..11) samples the texture where its
    # fraction falls: move each triangle by the whole part of its centre
    tri_keys = [(corners - np.floor(corners.mean(0)), find(n)) for corners, n in tri_keys]
    ids = {}
    # triangles inside 0..1 first; one that tiles past it (a barrel's repeated strip) only fills
    # texels no other island has, so it never takes a part's texels
    outside = lambda corners: float(np.maximum(0, np.abs(corners - 0.5) - 0.5).max())
    tri_keys.sort(key=lambda tk: outside(tk[0]) > 1e-3)
    for corners, root in tri_keys:
        n = ids.setdefault(root, len(ids) + 1)
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
        if outside(corners) > 1e-3:
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
DIFFER_AB = 6 / 255
ONE_DONOR = 0.2   # a part with one donor is painted when this much of it differs (dark camo blobs
                  # over a dark part do not differ: the Rem 700 stock is 0.44, bare parts under 0.06)


def paint_mask(base, donors, labels):
    """(mask[h,w] 0..1, per-island decisions) from the plain texture and its premium donors."""
    n = labels.max()
    smooth = lambda img: np.stack([blur(img[..., k], 2) for k in range(3)], 2)
    sb = smooth(base)
    differs = [np.abs(smooth(d) - sb).max(axis=2) > DIFFER for d in donors]
    decisions = {}
    if not donors:                     # no premium: every part may be painted (the suggestion and
        for i in range(1, n + 1):      # parts.tsv choose)
            area = (labels == i).sum()
            if area:
                decisions[i] = (area, True, ())
        return labels > 0, decisions
    if len(donors) >= 2:
        a, b = donors[0], donors[1]
        h, w = labels.shape
        d = np.abs(a - b).max(axis=2)
        blocks = d[:h // 4 * 4, :w // 4 * 4].reshape(h // 4, 4, w // 4, 4).max(axis=(1, 3)) > DIFFER_AB
        texel = np.zeros((h, w), dtype=bool)
        texel[:h // 4 * 4, :w // 4 * 4] = blocks.repeat(4, 0).repeat(4, 1)
        for i in range(1, n + 1):
            at = labels == i
            area = at.sum()
            if not area:
                continue
            ab, da, db = texel[at].mean(), differs[0][at].mean(), differs[1][at].mean()
            decisions[i] = (area, ab > 0.5 and min(da, db) > 0.3, (ab, da, db))
        painted = np.zeros(labels.shape, dtype=bool)
        for i, (_, yes, _) in decisions.items():
            if yes:
                painted |= labels == i
        # the two donors' own boundary inside a painted island; a painted island counts whole
        # where its texels differ in blocks (a hole of a few blocks is a stripe both painted alike)
        m = Image.fromarray((painted & texel).astype(np.uint8) * 255)
        m = m.filter(ImageFilter.MaxFilter(9)).filter(ImageFilter.MinFilter(9))
        mask = (np.asarray(m) > 127) & painted
    else:
        for i in range(1, n + 1):
            at = labels == i
            area = at.sum()
            if not area:
                continue
            dp = differs[0][at].mean()
            decisions[i] = (area, dp > ONE_DONOR, (dp,))
        mask = np.zeros(labels.shape, dtype=bool)
        for i, (_, yes, _) in decisions.items():
            if yes:
                mask |= labels == i
    return mask, decisions


def aoac(base, mask, labels, detail=4.0, ao=0.7):
    """The aoac texture [h,w,3] 0..1 for the plain texture and its paint mask."""
    owner = grow(labels * mask, 6) > 0
    soft = blur(owner.astype(np.float32), 1.0)
    L = lum(base)
    high = L - blur(L, 4)
    B = 0.5 + np.clip(detail * high, -0.5, 0.5) * soft
    broad = blur(L, 12)
    painted_l = broad[owner]
    ref = np.percentile(painted_l, 90) if painted_l.size else 1.0
    R = 1 - ao * np.clip(1 - broad / max(ref, 1e-3), 0, 1) * soft
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


MIN_PART = 0.02   # the suggestion: a part the premium painted, of at least this share of the
                  # texture's parts (the receiver, handguard, stock, magazine; not pins, triggers)


def suggest(decisions, labels):
    """The parts painted when parts.tsv has no line for the texture: the premium's large ones."""
    used = (labels > 0).sum()
    return {i for i, (area, yes, _) in decisions.items() if yes and area >= MIN_PART * used}


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
    """parts.tsv: {texture: [(u, v)]}, a point on each painted part (none: the texture stays plain)."""
    parts = {}
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if not line or line.startswith('#'):
            continue
        texture, _, points = line.partition('\t')
        parts[texture] = [tuple(float(c) for c in p.split(',')) for p in points.split()]
    return parts


def parts_islands(points, labels, texture):
    h, w = labels.shape
    chosen = set()
    for u, v in points:
        i = int(labels[min(int(v * h), h - 1), min(int(u * w), w - 1)])
        if not i:
            sys.exit('%s: parts.tsv point %.4f,%.4f is on no part (the UVs changed? pick again)' % (texture, u, v))
        chosen.add(i)
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
        buf = game.read('models/%s.skinned_model/render/%s/converted_model' % (model, surface))
        uv, tris = mesh_uv(buf)
        _, size, count = struct.unpack_from('<III', buf, 0)
        xyz = np.frombuffer(buf, np.uint8, count=count * ((size - 4) // count), offset=12).reshape(count, -1)[:, :12]
        blob = struct.pack('<II', count, tris.size) + xyz.tobytes() + uv.astype('<f4').tobytes() + tris.astype('<u2').tobytes()
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


def read_rows(path):
    rows = []
    for line in open(path, encoding='utf-8'):
        line = line.rstrip('\n')
        if not line or line.startswith('#'):
            continue
        cols = line.split('\t')
        texture, donors, surfaces = cols[0], [d for d in cols[1].split(',') if d], cols[2].split(',')
        premiums = [p for p in cols[3].split(',') if p] if len(cols) > 3 else []
        compare = cols[4] if len(cols) > 4 and cols[4] else None
        rows.append((texture, donors, surfaces, premiums, compare))
    return rows


def main():
    args = [a for a in sys.argv[1:]]
    opt = lambda name: args[args.index(name) + 1] if name in args else None
    only, camo_path, previews, picker = opt('--only'), opt('--camo'), opt('--previews'), opt('--picker')
    camos_dir, pack = opt('--camos'), opt('--pack')
    for name in ('--only', '--camo', '--previews', '--picker', '--camos', '--pack'):
        if name in args:
            i = args.index(name)
            del args[i:i + 2]
    if len(args) != 4 or (picker and not camo_path) or (pack and (args[3] == '-' or only or '@' not in pack)):
        sys.exit(__doc__)
    table, game, donor, out = args
    game, donor = Files(game), Files(donor)
    if out != '-' and os.path.exists(out):
        sys.exit('%s: already there' % out)
    parts_path = os.path.join(os.path.dirname(os.path.abspath(table)), 'parts.tsv')
    parts = read_parts(parts_path) if os.path.exists(parts_path) else {}
    options = game.read(OPTIONS_TEMPLATE)
    camo = load_rgb(open(camo_path, 'rb').read()) if camo_path else None
    written, entries = [], []
    def texture_of(name, size=None):
        files, name = (donor, name[3:]) if name.startswith('28:') else (game, name)
        img = load_rgb(files.read('textures/%s.dds' % name))
        return resize(img, size) if size else img

    for texture, donors, surfaces, premiums, compare in read_rows(table):
        if only and texture != only:
            continue
        base = texture_of(texture)
        size = base.shape[:2]
        imgs = [texture_of(d, size) for d in donors]
        first = imgs[0] if imgs else base          # the premium the previews and the picker show
        plain = texture_of(compare, size) if compare else base
        meshes = []
        for s in surfaces:
            model, surface = s.split(':')
            meshes.append(mesh_uv(game.read('models/%s.skinned_model/render/%s/converted_model' % (model, surface))))
        labels = islands(meshes, size)
        _, decisions = paint_mask(plain, imgs, labels)
        suggested = suggest(decisions, labels)
        chosen = parts_islands(parts[texture], labels, texture) if texture in parts else suggested
        mask = np.isin(labels, sorted(chosen))
        a = aoac(base, mask, labels)
        print('%-44s %d donors, %3d parts, premium painted %3d, %3d painted (%s), %5.1f%% of the parts'
              % (texture, len(imgs), len(decisions), sum(1 for _, y, _ in decisions.values() if y), len(chosen),
                 'parts.tsv' if texture in parts else 'suggested', 100 * mask.sum() / max(1, (labels > 0).sum())))
        if picker:
            models = list(dict.fromkeys(s.split(':')[0] for s in surfaces))
            entries.append(picker_entry(texture, base, first, labels, island_points(labels), suggested, chosen,
                                        texture in parts, models))
        if not mask.any():
            print('  nothing painted: no _aoac')
            continue
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
        textures = {s: t for t, _, surfaces, _, _ in read_rows(table) for s in surfaces}
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
