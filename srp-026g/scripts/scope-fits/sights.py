"""sights.py: a release-41 gun's iron sights under an optic, in a copy of its model (make_pack.py, as
sights.tsv says; srp-026g/docs/notes/weapon_scopes.md, section 7).

- fold: the port baked 0.69d0's sight module up (its `model`) into a surface of the gun; it becomes
  the module 0.69d0 shows under an optic (its `model_param1`, folded: the same parts, the flaps
  turned down). The up module's triangles are found in the surface (every one, exactly, at one
  translation) and taken out; the folded module's are added at that translation, bound to the
  bone the up one was, its texture coordinates checked to be the up one's.
- cut: the mesh islands of a surface wholly inside a box (the locators' frame) taken out: a fixed
  sight that is part of a bigger surface (a whole surface: make_pack.leave_out).

A skinned surface (`converted_model`, 0.26g): chunk 3 the vertices (count, then each: float3
position, four bone bytes, a float weight a bone but the last, then the normal, tangent and
binormal each packed as bytes z, y, x, w, then float2 UV), chunk 4 the 16-bit indices (count,
then the indices). A 0.69d0 static module's vertex: float3 position, the three vectors each one
float whose mantissa holds the bytes x, y, z, half2 UV (28 bytes); chunks 110 and 114 after.
"""
import struct
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'rifle-scopes'))
import scopes_lib as lib  # noqa: E402

HALF_TURN = np.array([-1.0, 1.0, -1.0])
NEAR_UM = 1.0  # a baked module's vertices are its own to 0.05 micrometres
IN_ORDER = 0.003  # metres: an older build's module of the same mesh, moved a little (the VHS-2's front post: 2.3 mm)


def chunks(data):
    out, at = [], 0
    while at < len(data):
        cid, size = struct.unpack_from('<2I', data, at)
        out.append((cid, data[at + 8:at + 8 + size]))
        at += 8 + size
    if at != len(data):
        sys.exit('a converted_model runs past its last chunk')
    return out


def chunk(cs, cid, what):
    found = [body for i, body in cs if i == cid]
    if len(found) != 1:
        sys.exit(f'{what}: {len(found)} chunks {cid}, not 1')
    return found[0]


class Skinned:
    """A skinned surface of 0.26g: its vertices as bytes, its triangles."""

    def __init__(self, path):
        self.path = Path(path)
        cs = chunks(self.path.read_bytes())
        if [i for i, _ in cs] != [3, 4]:
            sys.exit(f'{path}: chunks {[i for i, _ in cs]}, not a 0.26g skinned surface')
        vb, ib = chunk(cs, 3, path), chunk(cs, 4, path)
        n = struct.unpack_from('<I', vb)[0]
        self.stride = (len(vb) - 4) // n
        if len(vb) != 4 + n * self.stride or self.stride < 36 or self.stride % 4:
            sys.exit(f'{path}: {len(vb) - 4} vertex bytes for {n} vertices')
        self.raw = np.frombuffer(vb, dtype=np.uint8, count=n * self.stride, offset=4).reshape(n, self.stride).copy()
        m = struct.unpack_from('<I', ib)[0]
        self.tris = np.frombuffer(ib, dtype='<u2', count=m, offset=4).reshape(-1, 3).astype(np.int64)
        self.basis_at = 16 + 4 * ((self.stride - 36) // 4)  # past the bones and all weights but the last
        self.first = self.positions()

    def positions(self):
        return self.raw[:, 0:12].copy().view('<f4').reshape(-1, 3).astype(np.float64)

    def uv(self):
        return self.raw[:, self.basis_at + 12:self.basis_at + 20].copy().view('<f4').reshape(-1, 2)

    def keep(self, keep_tris):
        """Only the triangles `keep_tris` (a mask), and the vertices they use, in their order."""
        tris = self.tris[keep_tris]
        used = np.zeros(len(self.raw), bool)
        used[tris.ravel()] = True
        remap = np.cumsum(used) - 1
        self.raw, self.tris = self.raw[used], remap[tris]

    def add(self, raw, tris):
        self.tris = np.concatenate([self.tris, tris + len(self.raw)])
        self.raw = np.concatenate([self.raw, raw])

    def write(self):
        n, m = len(self.raw), self.tris.size
        if n > 0xffff:
            sys.exit(f'{self.path}: {n} vertices, past 16-bit indices')
        data = struct.pack('<3I', 3, 4 + n * self.stride, n) + self.raw.tobytes()
        data += struct.pack('<3I', 4, 4 + 2 * m, m) + self.tris.astype('<u2').tobytes()
        self.path.write_bytes(data)
        # the surface's export_properties: its counts, and its bounding box (in the frame it is
        # written in, the model's or the locators') grown if the surface now reaches past it
        props = self.path.parent / 'export_properties'
        buf = bytearray(props.read_bytes())
        old = lib.leaves(props)
        for key, value in (('vertices', n), ('indices', m), ('primitives', m // 3)):
            lib.ce.set_number(buf, f'source/{key}', old[f'/source/{key}'], value)
        box = np.array(old['/bounding_box/min'] + old['/bounding_box/max'])
        frame = min((1.0, HALF_TURN), key=lambda f: np.abs(bounds(self.first * f) - box).max())
        if np.abs(bounds(self.first * frame) - box).max() > 0.0015:  # a port writes it to a millimetre
            sys.exit(f'{props}: its bounding box is not its surface\'s')
        grown = bounds(np.concatenate([self.first, self.positions()]) * frame)
        if (grown[:3] < box[:3] - 1e-4).any() or (grown[3:] > box[3:] + 1e-4).any():
            for key, value in (('min', np.minimum(box[:3], grown[:3])), ('max', np.maximum(box[3:], grown[3:]))):
                at = lib.ce.rec(buf, lib.ce.find(buf, f'bounding_box/{key}'))[0]
                struct.pack_into('<3f', buf, at, *value)
        props.write_bytes(bytes(buf))


def bounds(p):
    """min and max of points, six numbers."""
    return np.concatenate([p.min(axis=0), p.max(axis=0)])


class Static:
    """A 0.69d0 static module surface: positions, the three packed vectors' bytes (x, y, z), UVs,
    triangles."""

    def __init__(self, path):
        cs = chunks(Path(path).read_bytes())
        if any(i not in (3, 4, 110, 114) for i, _ in cs):
            sys.exit(f'{path}: chunks {[i for i, _ in cs]}, not a 0.69d0 static surface')
        vb, ib = chunk(cs, 3, path), chunk(cs, 4, path)
        n = struct.unpack_from('<I', vb)[0]
        if len(vb) != 4 + 28 * n:
            sys.exit(f'{path}: not 28-byte vertices')
        raw = np.frombuffer(vb, dtype=np.uint8, count=28 * n, offset=4).reshape(n, 28)
        self.pos = raw[:, 0:12].copy().view('<f4').reshape(n, 3).astype(np.float64)
        floats = raw[:, 12:24].copy().view('<f4').reshape(n, 3).astype(np.float64)
        if (floats < 0).any() or (floats >= 1).any():
            sys.exit(f'{path}: a basis float outside [0, 1)')
        packed = np.round(floats * 16777216.0).astype(np.int64)
        self.basis = np.stack([(packed >> 16) & 0xff, (packed >> 8) & 0xff, packed & 0xff], axis=-1)  # n, 3, (x, y, z)
        self.uvh = raw[:, 24:28].copy().view('<u2').reshape(n, 2)  # half floats, as bits
        m = struct.unpack_from('<I', ib)[0]
        self.tris = np.frombuffer(ib, dtype='<u2', count=m, offset=4).reshape(-1, 3).astype(np.int64)


def places(p):
    """Each vertex's place: an id shared by the vertices within NEAR_UM of each other (a seam's,
    written twice; a port may have moved one of them a few hundredths of a micrometre)."""
    tol = NEAR_UM * 1e-6
    parent = np.arange(len(p))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    order = np.argsort(p[:, 0], kind='stable')
    xs = p[order, 0]
    for k, i in enumerate(order):
        j = k + 1
        while j < len(order) and xs[j] - xs[k] < tol:
            if np.abs(p[order[j]] - p[i]).max() < tol:
                a, b = find(i), find(order[j])
                if a != b:
                    parent[max(a, b)] = min(a, b)
            j += 1
    return np.array([find(i) for i in range(len(p))])


def snapped(gun_p, gun_places, q):
    """The place of the gun's vertex at each of `q` (within NEAR_UM), -1 where none is."""
    out = np.full(len(q), -1)
    for i0 in range(0, len(q), 256):
        d = np.abs(q[i0:i0 + 256, None, :] - gun_p[None, :, :]).max(axis=2)
        j = d.argmin(axis=1)
        ok = d[np.arange(len(j)), j] < NEAR_UM * 1e-6
        out[i0:i0 + 256] = np.where(ok, gun_places[j], -1)
    return out


def tri_keys(ids, tris):
    """Each triangle as its three vertices' places, turned to its least turn (a triangle has one key
    whatever vertex it starts at)."""
    k = ids[tris]
    return [min((a, b, c), (b, c, a), (c, a, b)) for a, b, c in k.tolist()]


def components(n, tris):
    parent = np.arange(n)

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    for a, b, c in tris:
        for x, y in ((a, b), (b, c)):
            x, y = find(x), find(y)
            if x != y:
                parent[max(x, y)] = min(x, y)
    return np.array([find(i) for i in range(n)])


def shape(p):
    return np.sort(np.round(p - p.mean(axis=0), 5), axis=0)


def translation(gun_p, gun_tris, mod_p, mod_tris):
    """The translations that put the module's largest piece on a piece of the gun's surface."""
    mc = components(len(mod_p), mod_tris)
    labels, counts = np.unique(mc, return_counts=True)
    piece = mod_p[mc == labels[counts.argmax()]]
    want = shape(piece)
    gc = components(len(gun_p), gun_tris)
    out = []
    for g in np.unique(gc):
        q = gun_p[gc == g]
        if len(q) == len(piece) and np.abs(shape(q) - want).max() < 2e-5:
            out.append(q.mean(axis=0) - piece.mean(axis=0))
    return out


def encode(mod, uv, alphas):
    """The module's vectors as 0.26g words (bytes z, y, x, w): the x, y, z bytes 0.69d0 packs; w the
    normal's 127, the tangent's and binormal's as `alphas` says (0xff, or the fraction of u and v
    (`uv`) times 1024 that 0.26g's own models carry)."""
    n = len(mod.pos)
    out = np.zeros((n, 12), np.uint8)
    uv = uv.astype(np.float64)
    frac = [np.clip(np.floor(255.5 * (uv[:, k] * 1024.0 - np.floor(uv[:, k] * 1024.0))), 0, 255) for k in (0, 1)]
    for k in range(3):
        out[:, 4 * k + 0] = mod.basis[:, k, 2]
        out[:, 4 * k + 1] = mod.basis[:, k, 1]
        out[:, 4 * k + 2] = mod.basis[:, k, 0]
        out[:, 4 * k + 3] = 127 if k == 0 else (0xff if alphas == 'ff' else frac[k - 1])
    return out


def found_exactly(gun, gp, up):
    """The up module in the surface, every triangle at one translation (micrometres): (translation,
    the surface's triangles that are its, each module vertex's surface vertex) or None."""
    gplaces = places(gp)
    gun_keys = tri_keys(gplaces, gun.tris)
    found = []
    for t in translation(gp, gun.tris, up.pos, up.tris):
        ids = snapped(gp, gplaces, up.pos + t)
        if (ids < 0).any():
            continue
        keys = set(tri_keys(ids, up.tris))
        hit = np.array([k in keys for k in gun_keys])
        if hit.sum() == len(up.tris) and len(keys) == len(up.tris):
            found.append((t, hit, ids))
    if len(found) > 1:
        sys.exit(f'{gun.path}: the up sight found {len(found)} times in it')
    if not found:
        return None
    t, hit, ids = found[0]
    half = gun.uv().astype(np.float16).view('<u2')
    by_key = {(gplaces[i], tuple(half[i])): i for i in np.unique(gun.tris[hit])}
    pairs = [by_key.get((ids[i], tuple(up.uvh[i])), -1) for i in range(len(up.pos))]
    if min(pairs) < 0:
        sys.exit(f'{gun.path}: an up sight vertex with other texture coordinates than 0.69d0\'s')
    return t, hit, np.array(pairs)


def found_in_order(gun, gp, up):
    """The up module in the surface as the port wrote it from an older build's module of the same
    mesh: its vertices in their order at one place in the surface, its triangles those, each vertex
    within IN_ORDER of 0.69d0's at one translation. (translation, triangles, vertices, the largest
    distance) or None."""
    n = len(up.pos)
    want = set(map(tuple, up.tris.tolist()))
    best = None
    for o in range(len(gp) - n + 1):
        d = gp[o:o + n] - up.pos
        t = np.median(d, axis=0)
        off = np.abs(d - t).max()
        if off < IN_ORDER and (best is None or off < best[1]):
            best = (o, off, t)
    if best is None:
        return None
    o, off, t = best
    inside = (gun.tris >= o).all(axis=1) & (gun.tris < o + n).all(axis=1)
    hit = inside & np.array([tuple(r) in want for r in (gun.tris - o).tolist()])
    if hit.sum() != len(up.tris):
        return None
    half = gun.uv()[o:o + n].astype(np.float16).view('<u2').astype(np.int64)
    if np.abs(half - up.uvh.astype(np.int64)).max() > 1:
        sys.exit(f'{gun.path}: the up sight in its order with other texture coordinates than 0.69d0\'s')
    return t, hit, np.arange(o, o + n), off


def fold(surface_dir, up_path, folded_path):
    """The up module of `up_path` in the surface made the folded one of `folded_path`. Returns what
    was done: how the up one was found (exactly; or in order, its largest distance from 0.69d0's),
    translation, triangles out and in, the largest difference of a vector byte between the port's
    up sight and 0.69d0's as ours are written."""
    gun = Skinned(surface_dir / 'converted_model')
    up, down = Static(up_path), Static(folded_path)
    if len(up.pos) != len(down.pos) or not np.array_equal(np.sort(up.uvh.view('<u4').ravel()), np.sort(down.uvh.view('<u4').ravel())):
        sys.exit(f'{folded_path}: not {up_path} folded (other vertices or texture coordinates)')
    gp = gun.positions()
    exact = found_exactly(gun, gp, up)
    if exact:
        (t, hit, gi), how = exact, 'exactly'
    else:
        ordered = found_in_order(gun, gp, up)
        if ordered is None:
            sys.exit(f'{surface_dir}: {up_path.parent.parent.name} is not in it')
        t, hit, gi, off = ordered
        how = f'in order, {off * 1000:.2f} mm off'
    out_v = np.unique(gun.tris[hit])
    if np.isin(out_v, gun.tris[~hit]).any():
        sys.exit(f'{surface_dir}: the up sight shares vertices with the rest of the surface')
    bones = gun.raw[out_v][:, 12:gun.basis_at]
    if (bones != bones[0]).any():
        sys.exit(f'{surface_dir}: the up sight is bound to more than one bone')
    # the folded one: the up one's texture coordinates as the port has them (a port from an older
    # build may carry them finer than 0.69d0's half floats), its vectors 0.69d0's, w as the port
    # writes it (0xff, or the UV fractions), checked on the up one: within a step of the port's
    guv = gun.uv()
    finer = {tuple(h): guv[g] for h, g in zip(up.uvh, gi)}
    port = gun.raw[gi][:, gun.basis_at:gun.basis_at + 12]
    alphas = 'ff' if (port[:, [7, 11]] == 0xff).all() else 'uv'
    up_uv = np.array([finer[tuple(h)] for h in up.uvh])
    step = int(np.abs(encode(up, up_uv, alphas).astype(int) - port.astype(int)).max())
    if step > 2:
        sys.exit(f'{surface_dir}: the up sight\'s vectors as 0.69d0 packs them differ from the port\'s by {step}')
    gun.keep(~hit)
    raw = np.zeros((len(down.pos), gun.stride), np.uint8)
    raw[:, 0:12] = (down.pos + t).astype('<f4').view(np.uint8).reshape(-1, 12)
    raw[:, 12:gun.basis_at] = bones[0]
    down_uv = np.array([finer[tuple(h)] for h in down.uvh], dtype='<f4')
    raw[:, gun.basis_at:gun.basis_at + 12] = encode(down, down_uv, alphas)
    raw[:, gun.basis_at + 12:gun.basis_at + 20] = down_uv.view(np.uint8).reshape(-1, 8)
    gun.add(raw, down.tris)
    gun.write()
    return {'how': how, 't': t, 'out': int(hit.sum()), 'in': len(down.tris), 'step': step, 'w': alphas}


def cut(surface_dir, box):
    """The mesh islands of the surface wholly inside `box` (x0, y0, z0, x1, y1, z1, the locators'
    frame: a skinned weapon's vertices turned half about Y) taken out: a sight that is islands of its
    own in a bigger surface. Returns the islands and triangles taken."""
    gun = Skinned(surface_dir / 'converted_model')
    p = gun.positions() * HALF_TURN
    welded = places(p)
    labels = components(welded.max() + 1, welded[gun.tris])[welded]  # islands: vertices at one place joined
    inside = ((p >= np.array(box[:3])) & (p <= np.array(box[3:]))).all(axis=1)
    taken = [c for c in np.unique(labels[inside]) if inside[labels == c].all()]
    if not taken:
        sys.exit(f'{surface_dir}: no island inside {box}')
    out = np.isin(labels, taken)[gun.tris].all(axis=1)
    gun.keep(~out)
    gun.write()
    return {'islands': len(taken), 'triangles': int(out.sum())}


def lower(surface_dir, box, height):
    """The surface's vertices inside `box` (as cut's) above `height` lowered to it: the part of a
    bigger island standing above that height cut flat (the Fort-401's front sight's ears, the top of
    its front block; their walls fold to nothing). Returns the vertices lowered."""
    gun = Skinned(surface_dir / 'converted_model')
    p = gun.positions() * HALF_TURN
    moved = ((p >= np.array(box[:3])) & (p <= np.array(box[3:]))).all(axis=1) & (p[:, 1] > height)
    if not moved.any():
        sys.exit(f'{surface_dir}: nothing inside {box} above {height}')
    p[moved, 1] = height
    gun.raw[:, 0:12] = (p * HALF_TURN).astype('<f4').view(np.uint8).reshape(-1, 12)
    gun.write()
    return {'vertices': int(moved.sum())}
