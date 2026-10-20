#!/usr/bin/env python3
"""weapon_camo.py: the `_aoac` textures that let a weapon wear an armour camouflage.

usage: python3 -I weapon_camo.py <guns.tsv> <game folder> <donor folder> <out folder>
           [--only <texture>] [--camo <camouflage .dds>] [--previews <folder>]

<game folder>   the players' game files at their game paths or flattened as dbgrab writes them
                (models__weapons__uzi.skinned_model__settings): the stock resources.db with every
                offered patch over it, for the models (their meshes' UVs) and the textures in guns.tsv
<donor folder>  the same for a later build's files that guns.tsv names with `28:` (0.28a2's
                resources.db: premium weapons whose camouflaged textures our data does not have)
<out folder>    gets textures/<texture>_aoac.dds and .options for every row and every premium
                texture of the row (the same mask: the same UVs); must not exist yet
--previews      also writes <texture>.png there: the plain texture, a premium one, the mask, and the
                texture as the game's bake would make it with --camo (default: the first donor)

The engine bakes a camouflage into a surface's textures (camouflage_decal_texture_type_baker_cook,
bake_decal_composition.ps) only when `<diffuse>_aoac.dds` loads beside the diffuse:
    c = lerp(diffuse, camo(uv * scale), 1 - aoac.G)       G: 1 bare, 0 painted
    c = overlay(c, aoac.B)                                B: 0.5 changes nothing
    c = c * aoac.R                                        R: 1 changes nothing
The mask comes from the premium weapons' own textures, the places their painters painted:
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
    tri_keys = [(corners, find(n)) for corners, n in tri_keys]
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
    only, camo_path, previews = opt('--only'), opt('--camo'), opt('--previews')
    for name in ('--only', '--camo', '--previews'):
        if name in args:
            i = args.index(name)
            del args[i:i + 2]
    if len(args) != 4:
        sys.exit(__doc__)
    table, game, donor, out = args
    game, donor = Files(game), Files(donor)
    if os.path.exists(out):
        sys.exit('%s: already there' % out)
    options = game.read(OPTIONS_TEMPLATE)
    camo = load_rgb(open(camo_path, 'rb').read()) if camo_path else None
    written = []
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
        plain = texture_of(compare, size) if compare else base
        meshes = []
        for s in surfaces:
            model, surface = s.split(':')
            meshes.append(mesh_uv(game.read('models/%s.skinned_model/render/%s/converted_model' % (model, surface))))
        labels = islands(meshes, size)
        mask, decisions = paint_mask(plain, imgs, labels)
        a = aoac(base, mask, labels)
        painted = 100 * mask.mean()
        print('%-44s %d donors, %3d islands, %3d painted, %5.1f%% of the texture'
              % (texture, len(imgs), len(decisions), sum(1 for _, y, _ in decisions.values() if y), painted))
        if not mask.any():
            print('  nothing painted: no _aoac')
            continue
        blob = dds_dxt1(a)
        for name in [texture] + premiums:
            dst = os.path.join(out, 'textures', *('%s_aoac' % name).split('/'))
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            open(dst + '.dds', 'wb').write(blob)
            open(dst + '.options', 'wb').write(options)
            written.append(name + '_aoac')
        if previews:
            os.makedirs(previews, exist_ok=True)
            c = camo if camo is not None else imgs[0]
            shown = bake(base, c, a, 4.0) if camo is not None else imgs[0]
            sm = lambda img: Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8)).resize((512, 512), Image.LANCZOS)
            row = Image.new('RGB', (2560, 512))
            for k, img in enumerate([base, imgs[0], np.repeat(mask[..., None], 3, 2).astype(np.float32), a,
                                     bake(base, c, a, 4.0)]):
                row.paste(sm(img), (512 * k, 0))
            row.save(os.path.join(previews, texture.replace('/', '__') + '.png'))
    print('%d textures written to %s' % (len(written), out))


if __name__ == '__main__':
    main()
