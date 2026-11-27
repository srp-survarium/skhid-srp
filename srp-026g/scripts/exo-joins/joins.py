"""Geometry of the exoskeleton joins (docs/notes/exo-joins.md): the meshes' vertex layout, the
rays the checks cast, the neck piece under the exo helmets and the exo legs' hem grown into boots.

A skinned surface's `converted_model` is a vertex chunk (3: size, count, then count vertices of
`stride` bytes) and an index chunk (4: size, count, then u16 triangles). A vertex: position
(3 f32), four bone indices (u8), stride-36 / 4 stored weights (f32; the last one is 1 minus
their sum), three packed unit vectors (normal, tangent, binormal: bytes Z, Y, X, aux, each
(b / 127.5 - 1)), the UV (2 f32). 36 to 48 bytes: one to four influences.
"""
from dataclasses import dataclass, field
import math
import struct

import numpy as np

# The native bones the joins name (render bind order, the same in every 69-bone character bind).
NECK, HEAD, JAW, LEFT_EYE, RIGHT_EYE = 21, 44, 46, 45, 47


@dataclass
class Mesh:
    pos: np.ndarray        # (n, 3) float64, exact f32 values
    bones: np.ndarray      # (n, 4) uint8
    weights: np.ndarray    # (n, 4) float64, the stored ones and the implicit last
    basis: np.ndarray      # (n, 12) uint8, the three packed vectors as stored
    uv: np.ndarray         # (n, 2) float64
    idx: np.ndarray        # (m, 3) int64
    stride: int = 48
    raw_weights: np.ndarray = field(default=None, repr=False)  # (n, stride-36 / 4) f32 as stored

    @property
    def tris(self):
        return self.pos[self.idx]

    def weight(self, bone):
        return (self.weights * (self.bones == bone)).sum(1)


def read_mesh(data):
    chunk, size, n = struct.unpack_from('<III', data)
    if chunk != 3 or n == 0 or (size - 4) % n:
        raise ValueError('not a vertex chunk')
    stride = (size - 4) // n
    if stride not in (36, 40, 44, 48):
        raise ValueError(f'unknown vertex layout {stride}')
    at = 8 + size
    kind, isize, ni = struct.unpack_from('<III', data, at)
    if kind != 4 or isize != 4 + 2 * ni or ni % 3 or len(data) != at + 12 + 2 * ni:
        raise ValueError('not an index chunk')
    raw = np.frombuffer(data, np.uint8, n * stride, 12).reshape(n, stride)
    stored = (stride - 36) // 4
    f = lambda a, b: raw[:, a:b].copy().view('<f4')
    w = f(16, 16 + 4 * stored).astype(np.float64)
    weights = np.zeros((n, 4))
    weights[:, :stored] = w
    weights[:, stored] = 1 - w.sum(1)
    bones = raw[:, 12:16].copy()
    bones[:, stored + 1:] = bones[:, stored:stored + 1]  # unused slots: weight 0 anyway
    idx = np.frombuffer(data, '<u2', ni, at + 12).reshape(-1, 3).astype(np.int64)
    if idx.max() >= n:
        raise ValueError('index outside the vertices')
    b = 16 + 4 * stored
    return Mesh(f(0, 12).astype(np.float64), bones, weights, raw[:, b:b + 12].copy(),
                f(b + 12, b + 20).astype(np.float64), idx, stride, w.astype(np.float32))


def write_mesh(mesh):
    """The 48-byte four-influence layout (the exo models'); a mesh read from one writes back the
    same bytes."""
    n = len(mesh.pos)
    out = np.zeros((n, 48), np.uint8)
    out[:, 0:12] = mesh.pos.astype('<f4').view(np.uint8).reshape(n, 12)
    out[:, 12:16] = mesh.bones
    w = mesh.raw_weights if mesh.raw_weights is not None and mesh.raw_weights.shape[1] == 3 \
        else mesh.weights[:, :3].astype('<f4')
    out[:, 16:28] = np.ascontiguousarray(w, '<f4').view(np.uint8).reshape(n, 12)
    out[:, 28:40] = mesh.basis
    out[:, 40:48] = mesh.uv.astype('<f4').view(np.uint8).reshape(n, 8)
    vb, ib = out.tobytes(), mesh.idx.astype('<u2').tobytes()
    if len(mesh.idx) * 3 >= 1 << 32 or n > 65535:
        raise ValueError('too large for u16 indices')
    return (struct.pack('<III', 3, 4 + len(vb), n) + vb +
            struct.pack('<III', 4, 4 + len(ib), len(mesh.idx) * 3) + ib)


def packed(vectors):
    """Unit vectors as the packed basis stores them (bytes Z, Y, X, 0)."""
    v = np.clip(np.rint((vectors + 1) * 127.5), 0, 255).astype(np.uint8)
    return np.stack([v[:, 2], v[:, 1], v[:, 0], np.zeros(len(v), np.uint8)], 1)


def unpacked(bytes4):
    return np.stack([bytes4[:, 2], bytes4[:, 1], bytes4[:, 0]], 1) / 127.5 - 1


# Rays ------------------------------------------------------------------------------------------

def ray_hits(tris, origins, dirs, chunk=64):
    """(rays, ) nearest and farthest positive hit distance of each ray on the triangles (nan: no
    hit). Möller-Trumbore, both faces."""
    near = np.full(len(origins), np.nan)
    far = np.full(len(origins), np.nan)
    if len(tris) == 0:
        return near, far
    v0 = tris[:, 0]
    e1, e2 = tris[:, 1] - v0, tris[:, 2] - v0
    for s in range(0, len(origins), chunk):
        o = origins[s:s + chunk, None, :]
        d = dirs[s:s + chunk, None, :]
        p = np.cross(d, e2[None])
        det = (e1[None] * p).sum(2)
        ok = np.abs(det) > 1e-12
        inv = np.where(ok, 1 / np.where(ok, det, 1), 0)
        t0 = o - v0[None]
        u = (t0 * p).sum(2) * inv
        q = np.cross(t0, e1[None])
        v = (d * q).sum(2) * inv
        t = (e2[None] * q).sum(2) * inv
        hit = ok & (u >= -1e-9) & (v >= -1e-9) & (u + v <= 1 + 1e-9) & (t > 1e-6)
        tn = np.where(hit, t, np.inf).min(1)
        tf = np.where(hit, t, -np.inf).max(1)
        near[s:s + chunk] = np.where(np.isfinite(tn), tn, np.nan)
        far[s:s + chunk] = np.where(np.isfinite(tf), tf, np.nan)
    return near, far


def ring_rays(center_xz, heights, angles_deg):
    """Horizontal rays outward from a vertical axis: (origins, dirs, heights, angles) flattened
    height-major. Angle 0 is +z (the character's front), 90 is +x."""
    hs, an = np.meshgrid(heights, np.radians(angles_deg), indexing='ij')
    hs, an = hs.ravel(), an.ravel()
    origins = np.stack([np.full_like(hs, center_xz[0]), hs, np.full_like(hs, center_xz[1])], 1)
    dirs = np.stack([np.sin(an), np.zeros_like(an), np.cos(an)], 1)
    return origins, dirs, hs, np.degrees(an)


def see_through(meshes, origins, dirs):
    """Rays that hit none of the meshes' triangles: a hole seen from outside along them."""
    hit = np.zeros(len(origins), bool)
    for tris in meshes:
        near, _ = ray_hits(tris, origins, dirs)
        hit |= ~np.isnan(near)
    return ~hit


def angle_about(pos, center_xz):
    return np.degrees(np.arctan2(pos[:, 0] - center_xz[0], pos[:, 2] - center_xz[1]))


def radius_about(pos, center_xz):
    return np.hypot(pos[:, 0] - center_xz[0], pos[:, 2] - center_xz[1])


def compact(mesh, keep_tris):
    """The mesh with only the kept triangles and the vertices they use."""
    idx = mesh.idx[keep_tris]
    used = np.unique(idx)
    remap = np.full(len(mesh.pos), -1)
    remap[used] = np.arange(len(used))
    sub = lambda a: None if a is None else a[used]
    return Mesh(mesh.pos[used], mesh.bones[used], mesh.weights[used], mesh.basis[used],
                mesh.uv[used], remap[idx], 48, sub(mesh.raw_weights) if mesh.stride == 48 else None)


def copy(mesh):
    return Mesh(mesh.pos.copy(), mesh.bones.copy(), mesh.weights.copy(), mesh.basis.copy(),
                mesh.uv.copy(), mesh.idx.copy(), mesh.stride,
                None if mesh.raw_weights is None else mesh.raw_weights.copy())


def set_influences(mesh, v, bones, weights):
    mesh.bones[v] = bones
    mesh.weights[v] = weights
    if mesh.raw_weights is not None and mesh.raw_weights.shape[1] == 3:
        mesh.raw_weights[v] = np.asarray(weights[:3], np.float32)


def set_vectors(mesh, rows, slot, vectors):
    """Packed vector `slot` (0 normal, 1 tangent: dP/du, 2 binormal: dP/dv) of the rows, unit
    length, the aux byte kept."""
    v = vectors / np.linalg.norm(vectors, axis=1, keepdims=True)
    aux = mesh.basis[rows, 4 * slot + 3].copy()
    mesh.basis[rows, 4 * slot:4 * slot + 4] = packed(v)
    mesh.basis[rows, 4 * slot + 3] = aux


def set_normals(mesh, rows):
    """Normals of the rows from the triangles round them (area-weighted), vertices at one place
    (a UV seam's copies) sharing theirs."""
    P = mesh.pos[mesh.idx]
    fn = np.cross(P[:, 1] - P[:, 0], P[:, 2] - P[:, 0])
    _, place = np.unique(np.round(mesh.pos, 5), axis=0, return_inverse=True)
    place = place.ravel()
    acc = np.zeros((place.max() + 1, 3))
    for k in range(3):
        np.add.at(acc, place[mesh.idx[:, k]], fn)
    n = acc[place]
    good = rows & (np.linalg.norm(n, axis=1) > 1e-12)
    set_vectors(mesh, good, 0, n[good])


def set_uv_frame(mesh, rows):
    """Tangents and binormals of the rows from the triangles round them and their UVs (dP/du,
    dP/dv), made perpendicular to the stored normal."""
    P, U = mesh.pos[mesh.idx], mesh.uv[mesh.idx]
    e1, e2 = P[:, 1] - P[:, 0], P[:, 2] - P[:, 0]
    d1, d2 = U[:, 1] - U[:, 0], U[:, 2] - U[:, 0]
    r = d1[:, 0] * d2[:, 1] - d2[:, 0] * d1[:, 1]
    r = np.where(np.abs(r) > 1e-12, r, np.inf)
    t = (e1 * d2[:, 1:2] - e2 * d1[:, 1:2]) / r[:, None]
    b = (e2 * d1[:, 0:1] - e1 * d2[:, 0:1]) / r[:, None]
    T, B = np.zeros_like(mesh.pos), np.zeros_like(mesh.pos)
    for k in range(3):
        np.add.at(T, mesh.idx[:, k], t)
        np.add.at(B, mesh.idx[:, k], b)
    n = unpacked(mesh.basis[:, 0:4])
    n /= np.linalg.norm(n, axis=1, keepdims=True)
    T -= n * (T * n).sum(1, keepdims=True)
    B -= n * (B * n).sum(1, keepdims=True)
    good = rows & (np.linalg.norm(T, axis=1) > 1e-9) & (np.linalg.norm(B, axis=1) > 1e-9)
    set_vectors(mesh, good, 1, T[good])
    set_vectors(mesh, good, 2, B[good])


# The legs: the hem lowered ----------------------------------------------------------------------

# Each shin bone and the foot below it (render bind order).
FOOT_OF = {15: 16, 10: 11}


def lower_hem(legs, drop, cap_top, blend_top, ankle, shins, tuck_top, tuck_span, gap):
    """The exo legs with their closed bottom (the part below `cap_top`) moved `drop` metres down
    whole, the trousers between `cap_top` and `blend_top` stretched down to meet it: the same
    vertices, triangles and UVs (the camouflage masks and decals stay where they are), only
    positions, the packed basis of the stretched band and, near the ankle, the skin (the
    native full-length trousers' Leg/Foot blend there, so the hem follows the boot as theirs
    does) change.

    Below `tuck_top` the trousers narrow, over `tuck_span`, to `gap` outside the native tucked
    trousers (`ankle`, the stock `_s3` cut every tall boot is made around) measured from each
    shin's axis (`shins`: {side: (Leg, Foot) bind positions}, side 1 the +x leg): they go into a
    tall boot's shaft as stock trousers do, and over a low boot's collar the hem still meets it."""
    out = copy(legs)
    y = legs.pos[:, 1]
    f = np.clip((blend_top - y) / (blend_top - cap_top), 0, 1)
    out.pos[:, 1] = y - drop * f
    # The band's normals: a vertical stretch by s (dy'/dy) scales a normal's y by 1/s. Its tangents
    # and binormals follow the new triangles over the same UVs.
    s = 1 + drop / (blend_top - cap_top)
    band = (y < blend_top) & (y > cap_top)
    n = unpacked(out.basis[band, 0:4])
    n[:, 1] /= s
    set_vectors(out, band, 0, n)
    set_uv_frame(out, y < blend_top)
    new_y = out.pos[:, 1]
    for v in np.nonzero(new_y < 0.24)[0]:
        leg = int(out.bones[v][np.argmax(out.weights[v])])
        if leg not in FOOT_OF:
            raise ValueError(f'vertex {v} at the hem is on bone {leg}, not a shin')
        foot = np.interp(new_y[v], [0.10, 0.20, 0.24], [0.5, 0.1, 0.0])
        set_influences(out, v, [leg, FOOT_OF[leg], leg, leg], [1 - foot, foot, 0.0, 0.0])
    tuck(out, ankle, shins, tuck_top, tuck_span, gap)
    set_normals(out, out.pos[:, 1] < tuck_top)
    set_uv_frame(out, out.pos[:, 1] < tuck_top)
    return out


def shin_axis(shins, side, y):
    """The point of a shin's axis (Leg to Foot) at height y, as (x, z)."""
    leg, foot = (np.asarray(p, float) for p in shins[side])
    t = (leg[1] - y) / (leg[1] - foot[1])
    return (leg + t * (foot - leg))[[0, 2]]


def tuck(mesh, ankle, shins, top, span, gap):
    """Each vertex below `top` drawn in towards its shin's axis, to `gap` outside `ankle` along the
    same horizontal ray, by k = (top - y) / span (at most 1); never pushed out."""
    rows = np.nonzero(mesh.pos[:, 1] < top)[0]
    for side in (1, -1):
        mine = rows[mesh.pos[rows, 0] * side > 0]
        if not len(mine):
            continue
        tris = ankle.tris[(ankle.tris[:, :, 0] * side > 0).all(1)]
        centers = np.array([shin_axis(shins, side, y) for y in mesh.pos[mine, 1]])
        d = mesh.pos[mine][:, [0, 2]] - centers
        r = np.linalg.norm(d, axis=1)
        ok = r > 1e-6
        mine, centers, d, r = mine[ok], centers[ok], d[ok], r[ok]
        dirs = np.stack([d[:, 0] / r, np.zeros(len(r)), d[:, 1] / r], 1)
        origins = np.stack([centers[:, 0], mesh.pos[mine, 1], centers[:, 1]], 1)
        _, far = ray_hits(tris, origins, dirs)
        target = np.where(np.isnan(far), r, np.minimum(r, far + gap))
        k = np.clip((top - mesh.pos[mine, 1]) / span, 0, 1)
        new_r = r + k * (target - r)
        mesh.pos[mine, 0] = centers[:, 0] + d[:, 0] / r * new_r
        mesh.pos[mine, 2] = centers[:, 1] + d[:, 1] / r * new_r


# The neck piece -------------------------------------------------------------------------------

def rim(helmet, center_xz, step=5):
    """The helmet's lowest point in each `step`-degree sector about the neck's axis, the lower of
    each sector and its two neighbours (a sector's edge), as (angles, heights)."""
    a = angle_about(helmet.pos, center_xz)
    bins = np.floor((a + 180) / step).astype(int) % (360 // step)
    low = np.full(360 // step, np.inf)
    np.minimum.at(low, bins, helmet.pos[:, 1])
    low = np.minimum(low, np.minimum(np.roll(low, 1), np.roll(low, -1)))
    return np.arange(-180, 180, step) + step / 2, low


def rim_at(rim_table, angles):
    centers, heights = rim_table
    return np.interp(angles, np.r_[centers - 360, centers, centers + 360], np.r_[heights, heights, heights])


# How far inside the helmet's shell the neck stays where the shell is round it.
INSET = 0.005


def neck_piece(head, helmet, rise, uv_rect, low=1.40):
    """The native head's neck (`head`, the stock base character's head: the neck every stock collar
    is made around), the neck the exo helmets hide with the head (`remove_head_model`) and the exo
    body's collar gave back: its triangles from `low` up to `rise` metres above the helmet's rim,
    without the eyes. A vertex closer than INSET to the helmet (a horizontal ray from the neck's
    axis through it meets the shell, or a strap), or outside it, is drawn in to INSET inside. Its UVs are laid on `uv_rect` (u0, v0, u1, v1) of the helmet's texture: u round the
    neck (mirrored front to back, so no seam), v down it; its normals and skin are the native
    neck's, its tangents follow the new UVs."""
    neck = head.weight(NECK) > 0.3
    center = head.pos[neck][:, [0, 2]].mean(0)
    table = rim(helmet, center)
    a = angle_about(head.pos, center)
    top = rim_at(table, a) + rise
    face = (head.weight(LEFT_EYE) + head.weight(RIGHT_EYE)) > 0.01
    keep = (head.pos[:, 1] <= top) & (head.pos[:, 1] >= low) & ~face
    pos = head.pos.copy()
    v = np.nonzero(keep)[0]
    if len(v):
        origins = np.stack([np.full(len(v), center[0]), head.pos[v, 1], np.full(len(v), center[1])], 1)
        d = head.pos[v] - origins
        r = np.linalg.norm(d, axis=1)
        _, far = ray_hits(helmet.tris, origins, d / r[:, None])
        tight = ~np.isnan(far) & (r > far - INSET)
        pos[v[tight]] = origins[tight] + d[tight] * ((far[tight] - INSET) / r[tight])[:, None]
    head = Mesh(pos, head.bones, head.weights, head.basis, head.uv, head.idx, head.stride, head.raw_weights)
    piece = compact(head, keep[head.idx].all(1))
    u0, v0, u1, v1 = uv_rect
    pa = np.abs(angle_about(piece.pos, center)) / 180
    y = piece.pos[:, 1]
    ytop = y.max()
    piece.uv = np.stack([u0 + pa * (u1 - u0), v0 + (ytop - y) / (ytop - low) * (v1 - v0)], 1)
    piece.raw_weights = None
    set_uv_frame(piece, np.ones(len(piece.pos), bool))
    return piece, center
