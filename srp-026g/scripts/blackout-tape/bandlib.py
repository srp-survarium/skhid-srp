"""Geometry helpers for the band tools: a model's bind pose, a surface's faces rasterised in UV
at its diffuse's resolution, UV islands, ray casts."""
import io
import struct
import numpy as np
from PIL import Image
import gameview as gv
import model

# Bones whose weight on a vertex makes it forearm or hand, after the side's name.
FOREARM = ['ForeArm', 'ForeArm_Roll', 'Hand', 'Thumb', 'Index', 'Middle', 'Ring', 'Pinky']


def bind_pose(base):
    """(bone names in the model's order, {name: bind position})."""
    b = gv.read('models/%s.skinned_model/render/bind_pose' % base)
    n = struct.unpack_from('<H', b, 0)[0]
    at, names, pos = 2, [], {}
    for _ in range(n):
        end = b.index(b'\0', at)
        name = b[at:end].decode()
        at = end + 1
        pos[name] = np.array(struct.unpack_from('<3f', b, at))
        names.append(name)
        at += 36
    return names, pos


def texture(name):
    return np.asarray(Image.open(io.BytesIO(gv.read('textures/%s.dds' % name))).convert('RGBA')).astype(np.float32)


def raster(uv, W, H):
    """Texels (ys, xs, barycentrics) whose centres a UV triangle covers (UVs wrap once into
    0..1); a sliver smaller than a texel still samples the texel it lies in."""
    uv = uv - np.floor(uv.min(0))
    x, y = uv[:, 0] * W, uv[:, 1] * H
    x0, x1 = int(max(np.floor(x.min()), 0)), int(min(np.ceil(x.max()), W - 1))
    y0, y1 = int(max(np.floor(y.min()), 0)), int(min(np.ceil(y.max()), H - 1))
    if x1 < x0 or y1 < y0:
        return None
    gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
    d = (y[1] - y[2]) * (x[0] - x[2]) + (x[2] - x[1]) * (y[0] - y[2])
    if abs(d) < 1e-12:
        return None
    l0 = ((y[1] - y[2]) * (gx - x[2]) + (x[2] - x[1]) * (gy - y[2])) / d
    l1 = ((y[2] - y[0]) * (gx - x[2]) + (x[0] - x[2]) * (gy - y[2])) / d
    l2 = 1 - l0 - l1
    inside = (l0 >= -1e-6) & (l1 >= -1e-6) & (l2 >= -1e-6)
    if not inside.any():
        cx, cy = int(np.clip(x.mean(), 0, W - 1)), int(np.clip(y.mean(), 0, H - 1))
        return np.array([cy]), np.array([cx]), np.array([[1 / 3, 1 / 3, 1 / 3]])
    return gy[inside].astype(int), gx[inside].astype(int), np.stack([l0[inside], l1[inside], l2[inside]], 1)


_PREP = {}


def prepare(base, surface):
    """(bone names, bind positions, surface, maps, material, H, W, per-face rasters), cached."""
    key = (base, surface)
    if key not in _PREP:
        names, bp = bind_pose(base)
        parts = {n: (s, maps, mat) for n, s, maps, mat in model.model(base)}
        s, maps, mat = parts[surface]
        dif = texture(maps['diffuse'])
        H, W = dif.shape[:2]
        _PREP[key] = (names, bp, s, maps, mat, H, W, [raster(s['uv'][t], W, H) for t in s['tri']])
    return _PREP[key]


def islands(tri, nv):
    """Each face's island: faces sharing a vertex index (converted models split vertices at
    UV seams, so these are UV islands)."""
    parent = np.arange(nv)

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    for t in tri:
        ra = find(t[0])
        for v in t[1:]:
            rb = find(v)
            if rb != ra:
                parent[rb] = ra
    return np.array([find(t[0]) for t in tri])


def ray_hits(o, d, P, T):
    """Distances along o + t d to every triangle of T it crosses, which, and the hits'
    barycentrics (u, v on edges 1 and 2)."""
    v0, v1, v2 = P[T[:, 0]], P[T[:, 1]], P[T[:, 2]]
    e1, e2 = v1 - v0, v2 - v0
    h = np.cross(d, e2)
    a = (e1 * h).sum(1)
    ok = np.abs(a) > 1e-12
    f = np.where(ok, 1 / np.where(ok, a, 1), 0)
    s = o - v0
    u = f * (s * h).sum(1)
    q = np.cross(s, e1)
    v = f * (q @ d)
    t = f * (e2 * q).sum(1)
    hit = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-5)
    return t[hit], np.nonzero(hit)[0], np.stack([1 - u[hit] - v[hit], u[hit], v[hit]], 1)


def arm_frame(bp, side):
    """(shoulder joint, unit axis to the elbow, upper-arm length, u, v square to it)."""
    a, e = bp[side + 'Arm'], bp[side + 'ForeArm']
    L = np.linalg.norm(e - a)
    d = (e - a) / L
    u = np.cross(d, [0, 0, 1.0])
    u /= np.linalg.norm(u)
    return a, d, L, u, np.cross(d, u)
