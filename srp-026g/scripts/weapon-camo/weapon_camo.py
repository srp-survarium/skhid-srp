#!/usr/bin/env python3
"""weapon_camo.py: the `_aoac` textures that let a weapon wear an armour camouflage.

usage: python3 -I weapon_camo.py <guns.tsv> <game folder> <donor folder> <out folder | ->
           [--only <texture>] [--camo <camouflage .dds>] [--previews <folder>]
           [--picker <page.html> [--camos <camouflage item folder>]] [--weapons <weapon configs>]

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
--camos         the camouflage items' configs (gameplay/items/upgrades/camouflages, as dbgrab writes
                them): the picker offers each item's camouflage (textures from the game folder)
--weapons       the weapon configs (gameplay/items/weapons): each model is posed as its idle
                animation has it (animations/ in the game folder), for what is seen and the 3D
                view; without, the bind pose, where some parts lie away from the weapon

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

    def glob(self, prefix):
        """The game paths of the files whose path starts with `prefix`."""
        d, _, start = prefix.rpartition('/')
        full = os.path.join(self.root, *d.split('/'))
        if os.path.isdir(full):
            return sorted('%s/%s' % (d, f) for f in os.listdir(full) if f.startswith(start))
        flat = prefix.replace('/', '__')
        return sorted(f.replace('__', '/') for f in self.flat if f.startswith(flat))


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


HIDDEN = 0.1      # the suggestion skips a part seen on less than this share of its surface from
                  # outside (a magazine inside the grip, a bolt inside the receiver)


def suggest(decisions, labels, seen=None):
    """The parts painted when parts.tsv has no line for the texture: the premium's large ones that
    are seen from outside (seen: {part: share seen}, visibility())."""
    used = (labels > 0).sum()
    return {i for i, (area, yes, _) in decisions.items()
            if yes and area >= MIN_PART * used and (seen is None or seen.get(i, 0) >= HIDDEN)}


def model_meshes(game, model, anim=None):
    """[(surface, xyz[n,3], uv[n,2], triangles[m,3])] of a model's render surfaces, in the pose of
    the animation `anim` (idle_pose()) when given: a weapon's mesh is stored in its bind pose, where
    some parts lie away from the weapon (the PM's magazine 8 cm beside the grip)."""
    out, bones = [], []
    for surface in game.surfaces(model):
        buf = game.read('models/%s.skinned_model/render/%s/converted_model' % (model, surface))
        uv, tris = mesh_uv(buf)
        _, size, count = struct.unpack_from('<III', buf, 0)
        raw = np.frombuffer(buf, np.uint8, count=count * ((size - 4) // count), offset=12).reshape(count, -1)
        out.append((surface, raw[:, :12].copy().view('<f4').astype(np.float64), uv, tris))
        bones.append(raw[:, 12].astype(np.int64))           # one bone a vertex (36 and 40 byte vertices)
    if anim:
        C = idle_pose(game, model, anim)
        main = np.bincount(np.concatenate(bones), minlength=len(C)).argmax()  # the weapon's body
        rel = np.linalg.inv(C[main]) @ C
        out = [(sf, np.einsum('nij,nj->ni', rel[np.minimum(b, len(C) - 1)], np.c_[xyz, np.ones(len(xyz))])[:, :3], uv, t)
               for (sf, xyz, uv, t), b in zip(out, bones)]
        # a part the pose hides is scaled to nothing (the Magnum's spare rounds): no triangles
        kept = []
        for sf, xyz, uv, t in out:
            t = t[np.linalg.norm(np.cross(xyz[t[:, 1]] - xyz[t[:, 0]], xyz[t[:, 2]] - xyz[t[:, 0]]), axis=1) > 1e-12]
            used, t = np.unique(t, return_inverse=True)
            kept.append((sf, xyz[used], uv[used], t.reshape(-1, 3)))
        out = kept
    return out


def euler(r):
    """The rotation of Euler angles r (x, y, z). The bind poses' (-pi, 0, -pi) and the idle poses'
    rotations about one axis (or by a few hundredths of a radian) are the same in any order."""
    (cx, cy, cz), (sx, sy, sz) = np.cos(r), np.sin(r)
    return (np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]]) @ np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
            @ np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]]))


def trs(t, r, s):
    m = np.eye(4)
    m[:3, :3] = euler(np.asarray(r)) * np.asarray(s)
    m[:3, 3] = t
    return m


def curve_starts(sp, name):
    """The first key's value of each curve of a .b-spline (9 a bone, see idle_pose())."""
    count = struct.unpack_from('<I', sp, 0)[0]
    at, first = 4 + 8 * 9 * count, []
    for _ in range(9 * count):
        k = struct.unpack_from('<I', sp, at)[0]
        first.append(struct.unpack_from('<f', sp, at + 8)[0])
        at += 8 * k + 4
    assert at == len(sp), '%s: curves end at %d of %d' % (name, at, len(sp))
    return first


def idle_pose(game, model, anim):
    """[bone]: the matrix that takes a vertex of that bone (render/bind_pose's order) from the bind
    pose to the pose of `anim` (an animation's path without extension: .b-spline, .bones_names).
    bind_pose: u16 bones, then each: name, translation, rotation, scale (3 f32 each), in the
    model's space. The skeleton (models/<model>.skinned_model/skeleton names it) nests the bones as
    config tables. The .b-spline: u32 bones, 9 u32 pairs per bone, then a curve per channel one
    after another (u32 k, k - 1 (time, value) f32 pairs, f32 length, u32): translation, rotation,
    scale x y z per bone, in bones_names' order and relative to the parent; the first key is the
    pose."""
    bp = game.read('models/%s.skinned_model/render/bind_pose' % model)
    bind, off = [], 2
    for _ in range(struct.unpack_from('<H', bp, 0)[0]):
        end = bp.index(b'\0', off)
        name = bp[off:end].decode()
        bind.append((name, trs(*(struct.unpack_from('<3f', bp, end + 1 + 12 * k) for k in range(3)))))
        off = end + 37
    skeleton = config_value(game.read('models/%s.skinned_model/skeleton' % model), 'skeleton')
    sk = game.read(skeleton.removeprefix('resources/') + '.skeleton')
    parents = {}

    def walk(at, parent):
        data, _, _, t, cnt = struct.unpack_from('<QQIHH', sk, at)
        for i in range(cnt if t == 3 else 0):
            c = data + 24 * i
            n = struct.unpack_from('<Q', sk, c + 8)[0]
            name = sk[n:sk.index(b'\0', n)].decode()
            if struct.unpack_from('<QQIHH', sk, c)[3] == 3:
                parents[name] = parent
                walk(c, name)
    walk(0, None)
    names = config_value(game.read(anim + '.bones_names'), 'bones_names')
    sp = game.read(anim + '.b-spline')
    first = curve_starts(sp, anim)
    local = {n: trs(*(first[9 * b + 3 * k:9 * b + 3 * k + 3] for k in range(3))) for b, n in enumerate(names)}
    binds = dict(bind)
    world = {}

    def W(name):                     # a bone the animation leaves out keeps its place to its parent
        if name not in world:
            p = parents.get(name)
            if name in local:
                world[name] = (W(p) if p else np.eye(4)) @ local[name]
            elif p in binds and name in binds:
                world[name] = W(p) @ np.linalg.inv(binds[p]) @ binds[name]
            else:
                world[name] = binds.get(name, np.eye(4))
        return world[name]
    return np.array([W(name) @ np.linalg.inv(m) for name, m in bind])


def idle_animations(folder, game):
    """{model: its idle animation (path without extension)} from the weapon configs in `folder`
    (gameplay/items/weapons, as dbgrab writes them): the config's object/model, and the 3rd view
    `idle` (else the first `idle*`) of the animation set its item animations are in."""
    import re
    out = {}
    for f in sorted(os.listdir(folder)):
        buf = open(os.path.join(folder, f), 'rb').read()
        model = config_value(buf, 'object/model') if f.endswith('.options') else None
        if not isinstance(model, str) or model in out:
            continue
        for found in sorted(set(re.findall(rb'animations/single/weapons/([\w\-./]+?)/item/stand/3rd_view/', buf))):
            stem = 'animations/single/weapons/%s/item/stand/3rd_view/idle' % found.decode()
            files = [p for p in game.glob(stem) if p.endswith('.b-spline')]
            files.sort(key=lambda p: p != stem + '.b-spline')
            if files:
                out[model] = files[0][:-len('.b-spline')]
                break
    return out


def visibility(meshes, own, labels, directions=32, res=256, density=3.0):
    """{part: share of its surface seen from outside}: the model drawn from `directions` around it
    (orthographic, every surface hiding what is behind it); each part's surfaces (those `own`
    names, model:surface) sampled by projected area, a sample seen when nothing is nearer at its
    texel. meshes: {model: model_meshes()} of every model whose surfaces use the texture."""
    h, w = labels.shape
    rng = np.random.default_rng(0)
    seen, total = np.zeros(labels.max() + 1), np.zeros(labels.max() + 1)
    k = np.arange(directions) + 0.5                 # directions spread over the sphere (Fibonacci)
    z = 1 - 2 * k / directions
    phi = np.pi * (3 - np.sqrt(5)) * k
    dirs = np.stack([np.sqrt(1 - z * z) * np.cos(phi), np.sqrt(1 - z * z) * np.sin(phi), z], 1)
    for name, model in meshes.items():
        P = np.concatenate([xyz[tris] for _, xyz, _, tris in model])              # [m,3,3]
        UV = np.concatenate([uv[tris] for _, _, uv, tris in model])               # [m,3,2]
        UV = UV - np.floor(UV.mean(1, keepdims=True))                             # as islands()
        mine = np.concatenate([np.full(len(tris), '%s:%s' % (name, s) in own) for s, _, _, tris in model])
        centre = (P.reshape(-1, 3).max(0) + P.reshape(-1, 3).min(0)) / 2
        radius = np.linalg.norm(P.reshape(-1, 3) - centre, axis=1).max()
        scale = (res - 1) / (2 * radius)
        for d in dirs:
            right = np.cross(d, [0, 0, 1] if abs(d[2]) < 0.9 else [1, 0, 0])
            right /= np.linalg.norm(right)
            up = np.cross(d, right)
            Q = np.stack([(P - centre) @ right, (P - centre) @ up, (P - centre) @ d], -1)
            Q[..., :2] = Q[..., :2] * scale + res / 2
            e1, e2 = Q[:, 1, :2] - Q[:, 0, :2], Q[:, 2, :2] - Q[:, 0, :2]
            area = np.abs(e1[:, 0] * e2[:, 1] - e1[:, 1] * e2[:, 0]) / 2
            n = np.minimum(np.ceil(area * density), 4000).astype(np.int64) + 1
            t = np.repeat(np.arange(len(P)), n)
            r1, r2 = np.sqrt(rng.random(len(t))), rng.random(len(t))
            bary = np.stack([1 - r1, r1 * (1 - r2), r1 * r2], 1)[:, :, None]
            pts = (Q[t] * bary).sum(1)
            pix = np.clip(pts[:, 0].astype(np.int64), 0, res - 1) * res + np.clip(pts[:, 1].astype(np.int64), 0, res - 1)
            near = np.full(res * res, np.inf)
            np.minimum.at(near, pix, pts[:, 2])
            vis = pts[:, 2] <= near[pix] + radius * 0.01
            m = mine[t]
            uvp = (UV[t[m]] * bary[m]).sum(1)
            part = labels[np.clip((uvp[:, 1] % 1 * h).astype(np.int64), 0, h - 1),
                          np.clip((uvp[:, 0] % 1 * w).astype(np.int64), 0, w - 1)]
            np.add.at(total, part, 1)
            np.add.at(seen, part, vis[m])
    return {i: seen[i] / total[i] for i in range(1, len(total)) if total[i]}


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


def picker_model(meshes, model, textures):
    """A model's surfaces (model_meshes()) for the picker's 3D view: the texture each uses (None: one with no _aoac,
    drawn grey) and its mesh, gzipped: u32 vertices, u32 indices, f32 xyz each, f32 uv each, u16
    indices."""
    import base64
    import gzip
    out = []
    for surface, xyz, uv, tris in meshes:
        blob = (struct.pack('<II', len(xyz), tris.size) + xyz.astype('<f4').tobytes() + uv.astype('<f4').tobytes()
                + tris.astype('<u2').tobytes())
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
    camos_dir, weapons_dir = opt('--camos'), opt('--weapons')
    for name in ('--only', '--camo', '--previews', '--picker', '--camos', '--weapons'):
        if name in args:
            i = args.index(name)
            del args[i:i + 2]
    if len(args) != 4 or (picker and not camo_path):
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
    anims = idle_animations(weapons_dir, game) if weapons_dir else {}
    meshes_of = {}

    def posed(model):                # the model's meshes as the game shows it (its idle pose)
        if model not in meshes_of:
            if weapons_dir and model not in anims:
                print('  %s: no idle animation, its bind pose' % model)
            meshes_of[model] = model_meshes(game, model, anims.get(model))
        return meshes_of[model]
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
        seen = visibility({m: posed(m) for m in dict.fromkeys(s.split(':')[0] for s in surfaces)},
                          set(surfaces), labels)
        suggested = suggest(decisions, labels, seen)
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
            dst = os.path.join(out, 'textures', *('%s_aoac' % name).split('/'))
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
        models = {m: picker_model(posed(m), m, textures) for e in entries for m in e['models']}
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
    print('%d textures written to %s' % (len(written), out))


if __name__ == '__main__':
    main()
