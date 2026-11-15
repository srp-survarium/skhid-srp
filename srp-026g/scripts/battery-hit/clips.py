"""The engine's skeleton clips (`.b-spline` / `.bones_names`) in Python: read,
evaluate with the engine's own spline basis (resource-porter
skeleton_animation.rs, `get_spline_params` transcribed), fit, write; the rig
from a `.skeleton`; world poses in the engine's row-vector convention."""
import math
import struct
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as content  # noqa: E402

UNUSED = -1e9
KNOT_EPS = 1e-7


class Channel:
    def __init__(self, knots, points):
        self.knots, self.points = list(knots), list(points)

    @staticmethod
    def constant(value, end):
        return Channel([0, 0, 0, 0, end], [value] * 4 + [UNUSED])

    def k(self, i):
        last = len(self.knots) - 1
        return self.knots[last if i < 0 or i > last else i]

    def p(self, i):
        last = len(self.points) - 1
        return self.points[last if i < 0 or i > last else i]

    def coefficients(self, i):
        k = self.k
        dm1, d0, d1, d2, d3 = (k(i - 1) - k(i - 2), k(i) - k(i - 1), k(i + 1) - k(i), k(i + 2) - k(i + 1),
                               k(i + 3) - k(i + 2))
        k0 = 1 / (d1 * (d1 + d0) * (d1 + d0 + dm1))
        k10 = k0
        k11 = 1 / (d1 * (d1 + d0) * (d1 + d0 + d2))
        k12 = 1 / (d1 * (d1 + d2) * (d1 + d0 + d2))
        k20, k21 = k11, k12
        k22 = 1 / (d1 * (d1 + d2) * (d1 + d2 + d3))
        k3 = k22
        f = [[k3, -k20 - k21 - k22, k10 + k11 + k12, -k0],
             [0, k20 * (d1 - 2 * d0) + k21 * (d1 + d2 - d0) + k22 * (d1 + d2 + d3),
              k10 * (dm1 + d0 - 2 * d1) + k11 * (-2 * d1 - d2 + d0) + k12 * (-2 * (d1 + d2)), k0 * 3 * d1],
             [0, k20 * (2 * d0 * d1 - d0 * d0) + k21 * (d0 * (d1 + d2)),
              k10 * (d1 * d1 - 2 * d1 * (dm1 + d0)) + k11 * (-d0 * d1 + (d1 - d0) * (d1 + d2))
              + k12 * ((d1 + d2) * (d1 + d2)), -k0 * 3 * d1 * d1],
             [0, k20 * (d1 * d0 * d0), k10 * (d1 * d1 * (dm1 + d0)) + k11 * (d1 * d0 * (d1 + d2)),
              k0 * d1 * d1 * d1]]
        p = [self.p(i), self.p(i - 1), self.p(i - 2), self.p(i - 3)]
        c = lambda row: sum(a * b for a, b in zip(row, p))
        return [c(f[3]), c(f[2]), c(f[1]), c(f[0])]

    def spans(self):
        out = []
        if len(self.knots) < 5:
            return out
        for i in range(3, len(self.knots) - 1):
            if self.knots[i + 1] - self.knots[i] >= KNOT_EPS:
                out.append((self.knots[i], self.coefficients(i)))
        return out

    def evaluate(self, t):
        spans = self.spans()
        if not spans:
            return self.points[0]
        t = min(max(t, spans[0][0]), self.knots[-1])
        start, c = next((s for s in reversed(spans) if s[0] <= t), spans[0])
        u = t - start
        return u * (u * (u * c[3] + c[2]) + c[1]) + c[0]

    @staticmethod
    def fit(times, values):
        if all(v == values[0] for v in values):
            return Channel.constant(values[0], times[-1])
        knots = [times[0]] * 4 + list(times[1:])
        pairs = len(knots)
        basis = np.zeros((len(times), pairs - 1))
        for j in range(pairs - 1):
            points = [0.0] * pairs
            points[j] = 1.0
            probe = Channel(knots, points)
            spans = probe.spans()
            for r, t in enumerate(times):
                basis[r, j] = probe.evaluate(t)
        a = basis.T @ basis + 1e-6 * np.eye(pairs - 1)
        solved = np.linalg.solve(a, basis.T @ np.array(values, dtype=float))
        return Channel(knots, [float(x) for x in solved] + [UNUSED])


def parse_names(data):
    return [content.name_at(data, content.rec(data, at)[0]).decode()
            for at in content.children(data, content.find(data, 'bones_names'))]


def parse_clip(data):
    """The clip's per-bone nine channels and its animation type."""
    bones, events, kind = struct.unpack_from('<HBB', data, 0)
    tracks = content.clip_parse(data)[2]
    offset, out = 0, []
    for _ in range(bones):
        channels = []
        for _ in range(9):
            n = struct.unpack_from('<I', tracks, offset)[0]
            pairs = [struct.unpack_from('<ff', tracks, offset + 4 + 8 * i) for i in range(n)]
            offset += 4 + 8 * n
            channels.append(Channel([a for a, _ in pairs], [b for _, b in pairs]))
        out.append(channels)
    return out, kind


def write_clip(channels_per_bone, kind=0):
    out = bytearray(struct.pack('<HBB', len(channels_per_bone), 0, kind) + bytes(0x48 * len(channels_per_bone)))
    for channels in channels_per_bone:
        for c in channels:
            assert len(c.knots) >= 5 and len(c.knots) == len(c.points)
            out += struct.pack('<I', len(c.knots))
            for a, b in zip(c.knots, c.points):
                out += struct.pack('<ff', a, b)
    content.clip_parse(out)
    return bytes(out)


def clip_end(channels_per_bone):
    return max(c.knots[-1] for ch in channels_per_bone for c in ch)


def rotation(a):
    """`create_rotation(float3)`, the engine's XYZ Euler basis (row vectors)."""
    sx, sy, sz = map(math.sin, a)
    cx, cy, cz = map(math.cos, a)
    return np.array([[cy * cz, -cy * sz, sy],
                     [sy * sx * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy],
                     [-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy]])


def euler(b):
    sy = max(-1.0, min(1.0, b[0, 2]))
    y = math.asin(sy)
    if math.sqrt(1 - sy * sy) < 1e-6:
        return [math.atan2(b[1, 0], b[1, 1]), y, 0.0]
    return [math.atan2(-b[1, 2], b[2, 2]), y, math.atan2(-b[0, 1], b[0, 0])]


def skeleton_parents(data):
    parents = {}
    for path, kind, _ in content.walk(data):
        if path and kind == content.T_NAMED:
            parts = path.split('/')[1:]
            parents[parts[-1]] = parts[-2] if len(parts) > 1 else None
    return parents


def skeleton_masks(data):
    """Each bone's `mask` (the skeleton's bone groups: 1 the spine, head,
    Root, Weapon, Item; 2 the arms from the clavicles to the fingers; 4 the
    hips and legs). An animation moves a bone only if its lexeme's bone mask
    and the bone's share a bit (computed_local_bone_transform 0x3B20F2)."""
    masks = {}
    for path, kind, value in content.walk(data):
        if path.endswith('/mask') and kind == content.T_INT:
            masks[path.split('/')[-2]] = value
    return masks


def local_pose(clip, names, t):
    """Each bone's local (rotation basis, translation) at frame t."""
    out = {}
    for name, ch in zip(names, clip):
        v = [c.evaluate(t) for c in ch]
        out[name] = (rotation(v[3:6]), np.array(v[:3]))
    return out


def world_pose(local, parents, order):
    """Model-space (basis, position): a child's row vector times its local, then its parent's."""
    world = {}
    for name in order:
        r, p = local[name]
        parent = parents.get(name)
        if parent is not None and parent in world:
            pr, pp = world[parent]
            r, p = r @ pr, p @ pr + pp
        world[name] = (r, p)
    return world
