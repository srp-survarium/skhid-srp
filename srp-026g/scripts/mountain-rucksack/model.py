"""The German mountain rucksack (Bundeswehr Gebirgsjaegerrucksack, Mil-Tec 14016001, olive):
the game mesh, its baked maps and its lobby icon, built in Blender from nothing but this file.

    blender -b -noaudio -P model.py -- --out <new folder> [--samples 16] [--size 1024]

Writes into <out>:
  rucksack.glb           the game mesh, skinned to the native bones by name (resource-porter's
                         build-armour-model reads it); glTF axes: +Y up, the wearer faces +Z
  rucksack_diffuse.png   olive Cordura, webbing, buckles, PVC bottom, with baked occlusion
  rucksack_normal.png    tangent-space normal map, OpenGL convention (+Y up the texture)
  rucksack_gloss.png     0.26g's gloss (texture_roughness: dark is matte)
  rucksack_fresnel.png   0.26g's Fresnel map (dark is a dielectric)
  rucksack_icon.png      the lobby icon's picture (three-quarter front, transparent)
  rucksack.blend         the mesh, its materials and the bake setup, for looking at it

The maps are baked from the mesh's own materials: each stuff's colour, gloss and Fresnel, and
its relief (a woven canvas, ribbed and stitched webbing, the buckles' slots, pebbled PVC) over
rounded edges (a bevel shader), into the tangent-space normal map.

Blender axes here: the wearer faces -Y, up is +Z, his left is +X; one unit is a metre. The native
player is in his bind pose (game = (-x, z, -y) of Blender). The pack's size is the shop's:
32 x 21 x 40 cm, 25 l. Its back panel and the shoulder straps follow the stock torsos' outer
surfaces (their 85th percentile, measured over the 22 stock torso_s1 models), so it sits on all of
them; resource-porter measures the fit against every stock torso again when it converts it.
"""
import argparse
import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Vector

# ---------------------------------------------------------------------------------------------
# Parts and what they are made of (the bake's materials; the game draws one material)
FABRIC, LID, POCKET, WEBBING, PLASTIC, METAL, PVC, PADDING, BINDING = range(9)
STUFFS = {
    # name: (base colour sRGB, gloss 0..1 (0.26g smap), Fresnel 0..1 (fmap), woven)
    FABRIC: ('fabric', (0.27, 0.28, 0.205), 0.10, 0.16, 1.0),
    LID: ('lid', (0.28, 0.29, 0.212), 0.10, 0.16, 1.0),
    POCKET: ('pocket', (0.265, 0.275, 0.20), 0.10, 0.16, 1.0),
    WEBBING: ('webbing', (0.21, 0.215, 0.16), 0.14, 0.18, 0.0),
    PLASTIC: ('plastic', (0.055, 0.058, 0.05), 0.42, 0.30, 0.0),
    METAL: ('metal', (0.36, 0.36, 0.34), 0.55, 0.75, 0.0),
    PVC: ('pvc', (0.16, 0.18, 0.14), 0.36, 0.24, 0.0),
    PADDING: ('padding', (0.255, 0.265, 0.195), 0.10, 0.16, 1.0),
    BINDING: ('binding', (0.22, 0.225, 0.165), 0.12, 0.17, 0.0),
}

# ---------------------------------------------------------------------------------------------
# The native bones the mesh is skinned to (the 0.26g player rig, game names). The wearer's left
# is Blender +X. Bind-pose heads (Blender axes) only place the export armature.
BONES = {
    'Spin': (0.0, 0.001, 1.067), 'Spin_1': (0.0, 0.04, 1.22), 'Chest': (0.0, 0.07, 1.357),
    'Neck': (0.0, 0.046, 1.509), 'Hip': (0.0, 0.001, 1.006),
    'LeftClavicle': (0.068, -0.013, 1.442), 'RightClavicle': (-0.068, -0.013, 1.442),
    'LeftArm': (0.202, 0.026, 1.444), 'RightArm': (-0.202, 0.026, 1.444),
}
# The pack rides the upper back as one rigid piece, as the stock back pieces do: the mean of the
# stock torsos' weights over the back panel's area (Chest at the top, Spin at the bottom).
BODY_WEIGHTS = {'Chest': 0.55, 'Spin_1': 0.27, 'Spin': 0.18}

# The rounded edges baked into the normal map, by stuff (metres)
BEVEL = {FABRIC: 0.008, LID: 0.006, POCKET: 0.006, WEBBING: 0.0012, PLASTIC: 0.002, METAL: 0.001,
         PVC: 0.008, PADDING: 0.005, BINDING: 0.002}

# ---------------------------------------------------------------------------------------------
# Shape (metres)
Z0, Z1 = 1.095, 1.47            # body bottom and top of its walls
LID_EDGE = 1.285                # the lid's lowest point, front centre
POCKET_Z = (1.12, 1.355)        # side pockets
STRAP_X = 0.072                 # the two front straps' centre lines
BUCKLE_Z = 1.245                # the front buckles' centre


def back_y(x, z):
    """The back panel (it rests on the torsos): straight above the shoulder blades, nearer the
    body at the small of the back, wrapping a little round the sides."""
    t = min(max((1.31 - z) / 0.21, 0.0), 1.0)
    return 0.212 - 0.034 * t ** 1.4 - 0.024 * (x / 0.16) ** 2


def half_width(z):
    t = (z - Z0) / (Z1 - Z0)
    return 0.160 - 0.012 * t


def depth(z):
    t = (z - Z0) / (Z1 - Z0)
    return 0.188 - 0.022 * t


def front_y(x, z):
    """The front face, bulging where the load fills it."""
    hw = half_width(z)
    t = min(max((z - Z0) / (Z1 - Z0), 0.0), 1.0)
    u = min(abs(x) / hw, 1.0)
    return back_y(0, z) + depth(z) + 0.016 * (1 - u * u) * math.sin(math.pi * (0.15 + 0.8 * t))


# ---------------------------------------------------------------------------------------------
class Builder:
    """Vertices, faces (each with its stuff) and per-loop UVs in metres, plus per-vertex
    pattern coordinates (along/across a strap) for the bake material."""

    def __init__(self):
        self.v, self.f, self.stuff, self.uv, self.island = [], [], [], [], []
        self.along, self.across, self.weights = [], [], []
        self.islands = 0

    def vert(self, p, along=0.0, across=0.0, weights=None):
        self.v.append(tuple(p))
        self.along.append(along)
        self.across.append(across)
        self.weights.append(weights or BODY_WEIGHTS)
        return len(self.v) - 1

    def face(self, idx, stuff, uvs):
        self.f.append(tuple(idx))
        self.stuff.append(stuff)
        self.uv.append(tuple(uvs))
        self.island.append(self.islands)

    def new_island(self):
        self.islands += 1

    def mark(self):
        return len(self.f)

    def flip_since(self, mark):
        """Turns the faces made since `mark` the other way (a mirrored or inside-out build)."""
        for i in range(mark, len(self.f)):
            self.f[i] = self.f[i][::-1]
            self.uv[i] = self.uv[i][::-1]

    def grid(self, rows, stuff, uvs=None, closed=False, flip=False):
        """Quads between rows of vertex indices; uvs[r][c] in metres (defaults: arc length)."""
        self.new_island()
        if uvs is None:
            uvs = arc_uvs([[self.v[i] for i in row] for row in rows], closed)
        for r in range(len(rows) - 1):
            n = len(rows[r]) if closed else len(rows[r]) - 1
            for c in range(n):
                c1 = (c + 1) % len(rows[r]) if closed else c + 1
                cu = c + 1  # the uv column (no wrap: the seam's uv is past the last)
                q = [rows[r][c], rows[r][c1], rows[r + 1][c1], rows[r + 1][c]]
                qu = [uvs[r][c], uvs[r][cu], uvs[r + 1][cu], uvs[r + 1][c]]
                if flip:
                    q, qu = q[::-1], qu[::-1]
                self.face(q, stuff, qu)


def arc_uvs(rows, closed):
    """UVs in metres: u the arc length along each row, v the mean distance between rows."""
    out = []
    v = 0.0
    for r, row in enumerate(rows):
        pts = [Vector(p) for p in row] + ([Vector(row[0])] if closed else [])
        if r:
            prev = [Vector(p) for p in rows[r - 1]] + ([Vector(rows[r - 1][0])] if closed else [])
            v += sum((a - b).length for a, b in zip(pts, prev)) / len(pts)
        u, line = 0.0, [(0.0, v)]
        for a, b in zip(pts, pts[1:]):
            u += (b - a).length
            line.append((u, v))
        out.append(line)
    return out


def superellipse(n, a, b, e=5.0, start=-math.pi / 2):
    """n points of |x/a|^e + |y/b|^e = 1, counter-clockwise from the back (start = -90 deg)."""
    pts = []
    for i in range(n):
        t = start + 2 * math.pi * i / n
        c, s = math.cos(t), math.sin(t)
        pts.append((a * math.copysign(abs(c) ** (2 / e), c), b * math.copysign(abs(s) ** (2 / e), s)))
    return pts


# ---------------------------------------------------------------------------------------------
def body(b, ring=26):
    """The bag: rings of a rounded rectangle from a rounded bottom up to the top of its walls."""
    levels = [(-0.012, 0.86), (-0.004, 0.96), (0.0, 1.0), (0.035, 1.0), (0.07, 1.0), (0.10, 1.0), (0.17, 1.0),
              (0.24, 1.0), (0.30, 1.0), (Z1 - Z0 - 0.012, 1.0), (Z1 - Z0, 0.985)]
    rows = []
    first = b.mark()
    for dz, k in levels:
        z = Z0 + 0.012 + dz
        hw, d = half_width(z) * k, depth(z) * k
        row = []
        # start at the back centre (the UV seam), the wearer's side of the bag first
        for x, y in superellipse(ring, hw, d / 2, 5.0, start=math.pi / 2):
            # y = +d/2 is the back panel, -d/2 the front
            yb, yf = back_y(x, z), front_y(x, z)
            row.append(b.vert((x, yb + (yf - yb) * (0.5 - y / d), z)))
        rows.append(row)
    # faces: the lowest band (rounded bottom edge) is PVC, the rest Cordura
    b.new_island()
    uvs = arc_uvs([[b.v[i] for i in r] for r in rows], True)
    for r in range(len(rows) - 1):
        stuff = PVC if r < 3 else FABRIC
        for c in range(ring):
            c1 = (c + 1) % ring
            q = [rows[r][c], rows[r][c1], rows[r + 1][c1], rows[r + 1][c]]
            qu = [uvs[r][c], uvs[r][c + 1], uvs[r + 1][c + 1], uvs[r + 1][c]]
            b.face(q, stuff, qu)
    # bottom: a fan to the centre
    b.new_island()
    low = rows[0]
    cz = b.v[low[0]][2] - 0.004
    centre = b.vert((0.0, sum(b.v[i][1] for i in low) / len(low), cz))
    for c in range(ring):
        c1 = (c + 1) % ring
        tri = [low[c], centre, low[c1]]
        b.face(tri, PVC, [(b.v[i][0], b.v[i][1]) for i in tri])
    # top: a fan (under the lid)
    b.new_island()
    top = rows[-1]
    centre = b.vert((0.0, sum(b.v[i][1] for i in top) / len(top), Z1 + 0.008))
    for c in range(ring):
        c1 = (c + 1) % ring
        tri = [top[c1], centre, top[c]]
        b.face(tri, FABRIC, [(b.v[i][0], b.v[i][1]) for i in tri])
    # the rings run clockwise seen from above (the back panel is the lower Y): outward is the
    # other way round
    b.flip_since(first)
    return rows


def lid_profile(u, n):
    """The lid's section at u (-1..1 across): from its seam on the back panel over the top and
    down the front to its rounded edge; points (y, z), the edge last."""
    edge = LID_EDGE + 0.05 * abs(u) ** 3.2
    yb = back_y(0, Z1) + 0.004
    yf_top = front_y(0, Z1 - 0.02) + 0.014
    pts = []
    # the top: a gentle dome from the back seam to the front shoulder
    for i in range(5):
        t = i / 4
        y = yb + (yf_top - 0.02 - yb) * t
        z = Z1 + 0.012 + 0.028 * math.sin(math.pi * min(t * 1.15, 1.0)) * (1 - 0.3 * abs(u) ** 2)
        pts.append((y, z))
    # the front shoulder
    pts.append((yf_top - 0.004, Z1 - 0.004))
    # down the front
    m = n - len(pts)
    for i in range(1, m + 1):
        t = i / m
        z = (Z1 - 0.004) + (edge - (Z1 - 0.004)) * t
        x = u * half_width(z)
        y = front_y(x, z) + 0.013 + 0.006 * math.sin(math.pi * t)
        pts.append((y, z))
    return pts


def lid(b, cols=17, n=13):
    """The lid: one shell over the top and the upper front, wider than the bag, its edge bound
    with tape and turned under (a lip), so it reads as cloth of some thickness."""
    rows = []
    for j in range(cols):
        u = -1 + 2 * j / (cols - 1)
        prof = lid_profile(u, n)
        row = []
        for i, (y, z) in enumerate(prof):
            hw = half_width(z) + 0.014
            x = u * hw
            # the sides droop a little and curl round the corners
            droop = max(abs(u) - 0.8, 0) / 0.2
            z -= 0.018 * droop * (i / (n - 1))
            # the edge carries the binding tape (`across` 1 there: the material's tape band)
            edge = i == n - 1 or (j in (0, cols - 1) and i >= 5)
            row.append(b.vert((x, y, z), across=1.0 if edge else 0.0))
        rows.append(row)
    # rows run across (u), columns along the profile: transpose so each grid row is one profile
    prof_rows = [[rows[j][i] for j in range(cols)] for i in range(n)]
    b.grid(prof_rows, LID)
    # the lip: the free edge (sides and front) turned under, 1.4 cm deep, bound with tape
    path = ([rows[0][i] for i in range(5, n)] + [rows[j][n - 1] for j in range(1, cols - 1)]
            + [rows[cols - 1][i] for i in range(n - 1, 4, -1)])
    inner = []
    for i, vi in enumerate(path):
        p = Vector(b.v[vi])
        q = Vector((p.x * 0.97, p.y - 0.010, p.z + 0.012))
        inner.append(b.vert(q))
    b.grid([path, inner], BINDING)
    return rows


def ribbon(b, pts, width, thick, stuff, ups=None, sections=4, weights=None, cap=True):
    """A strap along pts (Vectors): a flat box, width across `ups` (the strap's surface normal
    side), thick along it. UVs: u across, v along, in metres."""
    pts = [Vector(p) for p in pts]
    n = len(pts)
    rings = []
    along = 0.0
    lengths = [0.0]
    for a, c in zip(pts, pts[1:]):
        along += (c - a).length
        lengths.append(along)
    for i, p in enumerate(pts):
        t = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
        up = Vector(ups[i] if ups is not None else (0, 0, 1))
        side = t.cross(up).normalized()
        up = side.cross(t).normalized()
        hw, ht = width / 2, thick / 2
        if sections == 4:
            corners = [(-hw, -ht), (hw, -ht), (hw, ht), (-hw, ht)]
        else:  # a padded strap: a flattened octagon
            k = 0.62
            corners = [(-hw * k, -ht), (hw * k, -ht), (hw, -ht * 0.35), (hw, ht * 0.35),
                       (hw * k, ht), (-hw * k, ht), (-hw, ht * 0.35), (-hw, -ht * 0.35)]
        w = weights(i, p) if weights else None
        ring = []
        for cx, cy in corners:
            q = p + side * cx + up * cy
            ring.append(b.vert(q, along=lengths[i], across=(cx + hw) / width, weights=w))
        rings.append(ring)
    m = len(rings[0])
    first = b.mark()
    b.new_island()
    # unrolled: u around the section, v along
    per = [0.0]
    sec = [Vector(b.v[i]) for i in rings[0]]
    for a, c in zip(sec, sec[1:] + sec[:1]):
        per.append(per[-1] + (c - a).length)
    for i in range(n - 1):
        for c in range(m):
            c1 = (c + 1) % m
            q = [rings[i][c], rings[i][c1], rings[i + 1][c1], rings[i + 1][c]]
            qu = [(per[c], lengths[i]), (per[c + 1], lengths[i]), (per[c + 1], lengths[i + 1]),
                  (per[c], lengths[i + 1])]
            b.face(q, stuff, qu)
    if cap:
        for ring, rev in ((rings[0], True), (rings[-1], False)):
            b.new_island()
            idx = ring[::-1] if rev else ring
            uv = [((i % 2) * width, (i // 2) * thick) for i in range(len(idx))]
            if len(idx) == 4:
                b.face(idx, stuff, uv)
            else:
                pts2 = [Vector(b.v[i]) for i in idx]
                c0 = pts2[0]
                for k in range(1, len(idx) - 1):
                    b.face([idx[0], idx[k], idx[k + 1]], stuff,
                           [(0, 0), ((pts2[k] - c0).length, 0), ((pts2[k + 1] - c0).length, 0.01)])
    # the sections run clockwise about the strap's direction: outward is the other way round
    b.flip_since(first)
    return rings


def slab(b, centre, size, up, side, stuff, bevel=0.25, weights=None):
    """A bevelled block (a buckle): `size` = (across, along, thick); `up` its outward normal."""
    c, up, side = Vector(centre), Vector(up).normalized(), Vector(side).normalized()
    fwd = up.cross(side).normalized()
    sx, sy, sz = size[0] / 2, size[1] / 2, size[2] / 2
    k = 1 - bevel
    outline = [(-sx * k, -sy), (sx * k, -sy), (sx, -sy * k), (sx, sy * k), (sx * k, sy),
               (-sx * k, sy), (-sx, sy * k), (-sx, -sy * k)]
    lo = [b.vert(c + side * x + fwd * y - up * sz, weights=weights) for x, y in outline]
    hi_ = [b.vert(c + side * x * 0.9 + fwd * y * 0.94 + up * sz, along=y + sy, across=(x + sx) / size[0],
                  weights=weights) for x, y in outline]
    b.new_island()
    m = len(outline)
    per = [0.0]
    for i in range(m):
        a, d = Vector(outline[i]), Vector(outline[(i + 1) % m])
        per.append(per[-1] + (d - a).length)
    for i in range(m):
        i1 = (i + 1) % m
        b.face([lo[i], lo[i1], hi_[i1], hi_[i]], stuff,
               [(per[i], 0), (per[i + 1], 0), (per[i + 1], size[2]), (per[i], size[2])])
    b.new_island()
    top_uv = [(x + sx, y + sy) for x, y in outline]
    # the top as a fan of quads/tris
    for i in range(1, m - 1):
        b.face([hi_[0], hi_[i], hi_[i + 1]], stuff, [top_uv[0], top_uv[i], top_uv[i + 1]])
    return hi_


def dring(b, centre, up, side, radius=0.016, wire=0.0028, weights=None, seg=7):
    """A D-ring standing on a strap: a half loop above the strap's surface."""
    c, up, side = Vector(centre), Vector(up).normalized(), Vector(side).normalized()
    fwd = up.cross(side).normalized()
    rings = []
    for i in range(seg + 1):
        t = math.pi * i / seg
        p = c + side * (radius * math.cos(t)) + fwd * (radius * 0.9 * math.sin(t)) + up * 0.004
        tan = (side * -math.sin(t) + fwd * math.cos(t) * 0.9).normalized()
        n1 = up
        n2 = tan.cross(n1).normalized()
        ring = [b.vert(p + (n1 * math.cos(a) + n2 * math.sin(a)) * wire, weights=weights)
                for a in (0, math.pi / 2, math.pi, 3 * math.pi / 2)]
        rings.append(ring)
    b.new_island()
    for i in range(seg):
        for k in range(4):
            k1 = (k + 1) % 4
            b.face([rings[i][k], rings[i][k1], rings[i + 1][k1], rings[i + 1][k]], METAL,
                   [(k * wire * 1.6, i * radius * 0.45), ((k + 1) * wire * 1.6, i * radius * 0.45),
                    ((k + 1) * wire * 1.6, (i + 1) * radius * 0.45), (k * wire * 1.6, (i + 1) * radius * 0.45)])


# ---------------------------------------------------------------------------------------------
def on_surface(bvh, p, normal_hint, off):
    """p moved onto the nearest surface of bvh along normal_hint, then `off` out of it."""
    loc, nrm, _, _ = bvh.find_nearest(Vector(p))
    if loc is None:
        return Vector(p), Vector(normal_hint)
    if nrm.dot(Vector(normal_hint)) < 0:
        nrm = -nrm
    return loc + nrm * off, nrm


def front_straps(b, shell):
    """Two webbing straps sewn to the lid's top, down over the lid to a side-release buckle
    under its edge, on down the bag's front to the PVC bottom."""
    for sx in (-1, 1):
        x = sx * STRAP_X
        pts, ups = [], []
        # over the lid top from near the back seam, then down the front to the bottom
        prof = lid_profile(x / (half_width(Z1) + 0.014), 13)
        path = [Vector((x, y, z)) for y, z in prof[1:]]
        # below the lid edge: down the body front
        z = path[-1].z - 0.02
        while z > Z0 + 0.03:
            path.append(Vector((x, front_y(x, z) + 0.004, z)))
            z -= 0.035
        path.append(Vector((x, front_y(x, Z0 + 0.02) + 0.002, Z0 + 0.02)))
        for p in path:
            q, nrm = on_surface(shell, p, (0, 1, 0.6 if p.z > Z1 - 0.01 else 0.0), 0.0035)
            pts.append(q)
            ups.append(nrm)
        ribbon(b, pts, 0.025, 0.0032, WEBBING, ups)
        # D-rings: one at the lid's top edge, one on the flap
        for k in (5, 8):
            dring(b, pts[k] + ups[k] * 0.002, ups[k], (1, 0, 0))
        # the buckle under the lid's edge
        z = BUCKLE_Z
        p = Vector((x, front_y(x, z) + 0.0095, z))
        slab(b, p, (0.036, 0.058, 0.009), (0, 1, 0), (1, 0, 0), PLASTIC)
        # the tail of the strap hanging out of the buckle
        tail = [Vector((x, front_y(x, zz) + 0.0125, zz)) for zz in (z - 0.02, z - 0.05, z - 0.08)]
        ribbon(b, tail, 0.023, 0.0028, WEBBING, [(0, 1, 0)] * 3)


def side_pocket(b, sx):
    """A side pocket sewn on at its edges, with its flap and a strap with a buckle."""
    zlo, zhi = POCKET_Z
    rows = []
    ring = 12
    for k, z in enumerate((zlo, zlo + 0.008, zlo + 0.06, zlo + 0.13, zhi - 0.03, zhi)):
        yc = 0.5 * (back_y(sx * 0.15, z) + front_y(sx * 0.15, z)) + 0.012
        hw = half_width(z)
        dx = 0.072 * (0.9 if k == 0 else 1.0) * (0.92 if z > zhi - 0.04 else 1.0)
        dy = 0.064 * (0.92 if k == 0 else 1.0)
        row = []
        for px, py in superellipse(ring, dx / 2, dy, 4.0, start=math.pi):
            x = sx * (hw - 0.012 + dx / 2 + px)
            row.append(b.vert((x, yc + py, z)))
        rows.append(row)
    b.grid(rows, POCKET, closed=True, flip=sx < 0)
    b.new_island()
    low = rows[0]
    c = b.vert((sum(b.v[i][0] for i in low) / ring, sum(b.v[i][1] for i in low) / ring, zlo - 0.004))
    for i in range(ring):
        i1 = (i + 1) % ring
        tri = [low[i1], c, low[i]] if sx < 0 else [low[i], c, low[i1]]
        b.face(tri, PVC, [(b.v[t][0], b.v[t][1]) for t in tri])
    # the flap: a slightly domed lid over the pocket's mouth, overhanging it
    top = rows[-1]
    cx = sum(b.v[i][0] for i in top) / ring
    cy = sum(b.v[i][1] for i in top) / ring
    outer = []
    for i in top:
        p = Vector(b.v[i])
        d = Vector((p.x - cx, p.y - cy, 0))
        outer.append(b.vert((cx + d.x * 1.12, cy + d.y * 1.1, zhi - 0.012)))
    mid = [b.vert((cx + (b.v[i][0] - cx) * 0.6, cy + (b.v[i][1] - cy) * 0.6, zhi + 0.012)) for i in top]
    peak = b.vert((cx, cy, zhi + 0.016))
    b.grid([outer, [top[i] for i in range(ring)]], BINDING, closed=True, flip=sx < 0)
    b.grid([[top[i] for i in range(ring)], mid], POCKET, closed=True, flip=sx < 0)
    b.new_island()
    for i in range(ring):
        i1 = (i + 1) % ring
        tri = [mid[i1], mid[i], peak] if sx < 0 else [mid[i], mid[i1], peak]
        b.face(tri, POCKET, [(b.v[t][0], b.v[t][1]) for t in tri])
    # the strap down the pocket's outer face, and its buckle
    x_out = max(abs(b.v[i][0]) for i in rows[3])
    yc = 0.5 * (back_y(sx * 0.15, 1.25) + front_y(sx * 0.15, 1.25)) + 0.012
    pts = [Vector((sx * (x_out + 0.003 + (0.004 if z > zhi - 0.01 else 0)), yc, z))
           for z in (zhi + 0.018, zhi - 0.005, zhi - 0.04, zhi - 0.08, zhi - 0.11)]
    ribbon(b, pts, 0.022, 0.003, WEBBING, [(sx, 0, 0)] * len(pts))
    slab(b, (sx * (x_out + 0.008), yc, zhi - 0.06), (0.032, 0.05, 0.008), (sx, 0, 0), (0, 1, 0), PLASTIC)


def handle(b):
    """The carry handle on the lid's top, near the back."""
    y = back_y(0, Z1) + 0.035
    pts = []
    for i in range(9):
        t = i / 8
        x = -0.055 + 0.11 * t
        z = Z1 + 0.034 + 0.045 * math.sin(math.pi * t) ** 0.8
        pts.append(Vector((x, y, z)))
    ribbon(b, pts, 0.026, 0.006, WEBBING, [(0, 1, 0)] * len(pts))


# ---------------------------------------------------------------------------------------------
# The shoulder straps. Their path over the left shoulder (+X; the right is its mirror), on the
# stock torsos' outer surface (85th percentile) plus a centimetre: from the bag's top over the
# trapezius and the clavicle, down the chest to the adjuster, then a webbing under the armpit
# back to the bag's lower corner.
PADDED = [  # (x, y, z): from under the bag's top, over the shoulder, down the chest
    (0.050, 0.200, 1.440), (0.077, 0.182, 1.487), (0.084, 0.166, 1.523), (0.090, 0.150, 1.562),
    (0.097, 0.116, 1.590), (0.104, 0.071, 1.602), (0.111, 0.027, 1.598), (0.117, -0.012, 1.579),
    (0.124, -0.042, 1.552), (0.131, -0.064, 1.522), (0.138, -0.084, 1.498), (0.140, -0.104, 1.476),
    (0.140, -0.123, 1.451), (0.141, -0.142, 1.420), (0.141, -0.156, 1.390), (0.142, -0.174, 1.358),
    (0.143, -0.186, 1.328), (0.145, -0.186, 1.298), (0.147, -0.180, 1.270), (0.149, -0.174, 1.245)]
WEBBING_PATH = [  # from the adjuster under the armpit, round the flank, to the bag's lower corner
    (0.149, -0.176, 1.240), (0.160, -0.150, 1.215), (0.194, -0.092, 1.195), (0.206, -0.035, 1.183),
    (0.205, 0.020, 1.170), (0.200, 0.074, 1.158), (0.180, 0.118, 1.145), (0.158, 0.155, 1.131),
    (0.146, 0.170, 1.122)]
# Their weights along the way: the stock torsos' own where the strap lies on them (the mean of
# the six nearest torso vertices on each of the 22 torso_s1 models), so a strap moves with the
# cloth under it; the bag's at both ends, where it is sewn on. "Clavicle" is the strap's side.
STRAP_WEIGHTS = [  # (parameter 0..1 over PADDED, weights)
    (0.00, BODY_WEIGHTS),
    (0.08, {'Chest': 0.86, 'Clavicle': 0.14}),
    (0.16, {'Chest': 0.87, 'Neck': 0.07, 'Clavicle': 0.06}),
    (0.26, {'Chest': 0.77, 'Neck': 0.20, 'Clavicle': 0.03}),
    (0.37, {'Chest': 0.76, 'Neck': 0.18, 'Clavicle': 0.06}),
    (0.47, {'Chest': 0.69, 'Clavicle': 0.28, 'Neck': 0.03}),
    (0.58, {'Chest': 0.59, 'Clavicle': 0.39, 'Spin_1': 0.02}),
    (0.68, {'Chest': 0.61, 'Clavicle': 0.26, 'Spin_1': 0.13}),
    (0.79, {'Chest': 0.57, 'Spin_1': 0.29, 'Clavicle': 0.14}),
    (0.89, {'Chest': 0.46, 'Spin_1': 0.42, 'Spin': 0.09, 'Clavicle': 0.03}),
    (1.00, {'Spin_1': 0.48, 'Chest': 0.28, 'Spin': 0.24}),
]
WEBBING_WEIGHTS = [
    (0.00, {'Spin_1': 0.48, 'Chest': 0.28, 'Spin': 0.24}),
    (0.25, {'Spin_1': 0.45, 'Spin': 0.40, 'Chest': 0.15}),
    (0.50, {'Spin': 0.45, 'Spin_1': 0.43, 'Chest': 0.06, 'Hip': 0.06}),
    (0.70, {'Spin': 0.50, 'Spin_1': 0.32, 'Hip': 0.15, 'Chest': 0.03}),
    (1.00, BODY_WEIGHTS),
]


def blend(table, t, side):
    for (t0, w0), (t1, w1) in zip(table, table[1:]):
        if t0 <= t <= t1:
            k = (t - t0) / (t1 - t0) if t1 > t0 else 0
            out = {}
            for w, f in ((w0, 1 - k), (w1, k)):
                for bone, v in w.items():
                    name = side + bone if bone == 'Clavicle' else bone
                    out[name] = out.get(name, 0) + v * f
            return out
    return table[-1][1]


def shoulder_straps(b):
    for sx, side in ((1, 'Left'), (-1, 'Right')):
        pad = [Vector((sx * x, y, z)) for x, y, z in PADDED]
        n = len(pad)
        # the strap lies on the body: its "up" is away from the torso's axis
        ups = []
        for p in pad:
            out = Vector((p.x * 0.6, p.y, (p.z - 1.36) * 0.9)).normalized()
            ups.append(out)
        ribbon(b, pad, 0.058, 0.015, PADDING, ups, sections=8,
               weights=lambda i, p, s=side: blend(STRAP_WEIGHTS, i / (n - 1), s))
        web = [Vector((sx * x, y, z)) for x, y, z in WEBBING_PATH]
        m = len(web)
        wups = [Vector((p.x, p.y * 0.4, 0)).normalized() for p in web]
        ribbon(b, web, 0.025, 0.003, WEBBING, wups,
               weights=lambda i, p, s=side: blend(WEBBING_WEIGHTS, i / (m - 1), s))
        # the ladder-lock adjuster where the pad ends
        p = pad[-1]
        slab(b, p + Vector((0, -0.006, -0.008)), (0.04, 0.03, 0.007), (0, -1, -0.15), (1, 0, 0), PLASTIC,
             weights=blend(STRAP_WEIGHTS, 1.0, side))


# ---------------------------------------------------------------------------------------------
def build_low():
    b = Builder()
    body(b)
    lid(b)
    # a temporary shell (bag + lid) the straps are laid on
    shell = to_object(b, 'shell')
    from mathutils.bvhtree import BVHTree
    bvh = BVHTree.FromObject(shell, bpy.context.evaluated_depsgraph_get())
    front_straps(b, bvh)
    bpy.data.objects.remove(shell)
    for sx in (-1, 1):
        side_pocket(b, sx)
    handle(b)
    shoulder_straps(b)
    return b


def to_object(b, name, with_uv=True):
    me = bpy.data.meshes.new(name)
    me.from_pydata(b.v, [], b.f)
    me.validate()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    if with_uv:
        uvl = me.uv_layers.new(name='UVMap')
        for poly, uvs in zip(me.polygons, b.uv):
            for li, uv in zip(poly.loop_indices, uvs):
                uvl.data[li].uv = uv
    return ob


def finish_low(b):
    ob = to_object(b, 'rucksack')
    me = ob.data
    # one material slot per stuff (the bake reads them), faces assigned
    for k in sorted(STUFFS):
        me.materials.append(stuff_material(k))
    me.polygons.foreach_set('material_index', b.stuff)
    for p in me.polygons:
        p.use_smooth = True
    # sharp edges where the surface turns hard (straps' edges, buckles), smooth elsewhere
    me.set_sharp_from_angle(angle=math.radians(60))
    # pattern coordinates for the bake material
    for name, values in (('along', b.along), ('across', b.across)):
        a = me.attributes.new(name, 'FLOAT', 'POINT')
        a.data.foreach_set('value', values)
    # vertex groups: the native bones
    groups = {}
    for i, w in enumerate(b.weights):
        total = sum(w.values())
        for bone, v in w.items():
            if v <= 0:
                continue
            g = groups.get(bone) or ob.vertex_groups.new(name=bone)
            groups[bone] = g
            g.add([i], v / total, 'REPLACE')
    pack_uvs(ob)
    return ob


def pack_uvs(ob):
    bpy.context.view_layer.objects.active = ob
    for o in bpy.context.selected_objects:
        o.select_set(False)
    ob.select_set(True)
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.select_all(action='SELECT')
    bpy.ops.uv.pack_islands(rotate=True, margin=0.006)
    bpy.ops.object.mode_set(mode='OBJECT')


# ---------------------------------------------------------------------------------------------
# The bake's materials: colour, gloss and Fresnel as the values the bakes read,
# and the cloth's relief as a bump the normal bake carries.
def stuff_material(k):
    name, colour, gloss, fresnel, weave = STUFFS[k]
    m = bpy.data.materials.get('st_' + name)
    if m:
        return m
    m = bpy.data.materials.new('st_' + name)
    m.use_nodes = True
    nt = m.node_tree
    nodes, links = nt.nodes, nt.links
    bsdf = nodes['Principled BSDF']
    lin = [srgb_to_linear(c) for c in colour]
    # colour variation: large blotches (fading, dirt) and fine mottling
    tc = nodes.new('ShaderNodeTexCoord')
    noise = nodes.new('ShaderNodeTexNoise')
    noise.inputs['Scale'].default_value = 9.0
    noise.inputs['Detail'].default_value = 6.0
    links.new(tc.outputs['Object'], noise.inputs['Vector'])
    ramp = nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = 0.35
    ramp.color_ramp.elements[0].color = [c * 0.86 for c in lin] + [1]
    ramp.color_ramp.elements[1].position = 0.7
    ramp.color_ramp.elements[1].color = [min(c * 1.12, 1) for c in lin] + [1]
    links.new(noise.outputs['Fac'], ramp.inputs['Fac'])
    colour_out = ramp.outputs['Color']
    if k == LID:
        # the lid's bound edge: a band of darker tape along it
        edge = nodes.new('ShaderNodeAttribute')
        edge.attribute_name = 'across'
        band = nodes.new('ShaderNodeMath')
        band.operation = 'GREATER_THAN'
        band.inputs[1].default_value = 0.62
        links.new(edge.outputs['Fac'], band.inputs[0])
        mix = nodes.new('ShaderNodeMix')
        mix.data_type = 'RGBA'
        links.new(band.outputs[0], mix.inputs['Factor'])
        links.new(colour_out, mix.inputs[6])
        mix.inputs[7].default_value = [srgb_to_linear(c) for c in STUFFS[BINDING][1]] + [1]
        colour_out = mix.outputs[2]
    links.new(colour_out, bsdf.inputs['Base Color'])
    bsdf.inputs['Roughness'].default_value = 1 - gloss
    m['gloss'] = gloss
    m['fresnel'] = fresnel
    # relief
    # rounded edges (the bevel the low mesh has not got), then the relief over them
    bevel = nodes.new('ShaderNodeBevel')
    bevel.samples = 16
    bevel.inputs['Radius'].default_value = BEVEL[k]
    bump = nodes.new('ShaderNodeBump')
    bump.inputs['Distance'].default_value = 0.0006
    height = relief(nodes, links, k, tc)
    links.new(height, bump.inputs['Height'])
    links.new(bevel.outputs['Normal'], bump.inputs['Normal'])
    links.new(bump.outputs['Normal'], bsdf.inputs['Normal'])
    # the bakes' target
    target = nodes.new('ShaderNodeTexImage')
    target.name = 'bake_target'
    nodes.active = target
    return m


def srgb_to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def relief(nodes, links, k, tc):
    """A height (0..1) for the stuff: woven cloth, ribbed webbing with stitched edges, a
    buckle's slots, pebbled PVC."""
    def node(kind, **inputs):
        n = nodes.new(kind)
        for key, v in inputs.items():
            n.inputs[key].default_value = v
        return n

    def math_(op, a, b=None, c=None):
        n = nodes.new('ShaderNodeMath')
        n.operation = op
        for slot, v in zip((0, 1, 2), (a, b, c)):
            if v is None:
                continue
            if isinstance(v, (int, float)):
                n.inputs[slot].default_value = v
            else:
                links.new(v, n.inputs[slot])
        return n.outputs[0]

    along = nodes.new('ShaderNodeAttribute')
    along.attribute_name = 'along'
    across = nodes.new('ShaderNodeAttribute')
    across.attribute_name = 'across'
    obj = tc.outputs['Object']
    if k in (FABRIC, LID, POCKET, PADDING):
        # canvas: a fine weave (two crossing waves in object space) and soft folds
        w1 = node('ShaderNodeTexWave', Scale=900.0, Distortion=0.0)
        w1.wave_type = 'BANDS'
        w1.bands_direction = 'X'
        links.new(obj, w1.inputs['Vector'])
        w2 = node('ShaderNodeTexWave', Scale=900.0, Distortion=0.0)
        w2.bands_direction = 'Z'
        links.new(obj, w2.inputs['Vector'])
        folds = node('ShaderNodeTexNoise', Scale=14.0, Detail=3.0)
        links.new(obj, folds.inputs['Vector'])
        weave = math_('MULTIPLY', math_('ADD', w1.outputs['Fac'], w2.outputs['Fac']), 0.12)
        h = math_('ADD', weave, math_('MULTIPLY', folds.outputs['Fac'], 1.6))
        if k == PADDING:
            # quilted: a seam across the pad every 6 cm
            seam = math_('PINGPONG', along.outputs['Fac'], 0.03)
            h = math_('ADD', h, math_('MULTIPLY', math_('SMOOTH_MIN', seam, 0.01, 0.01), 8.0))
        return h
    if k in (WEBBING, BINDING):
        # ribbed webbing: ribs across the strap every 1.5 mm, stitched 2 mm in from each edge
        ribs = math_('SINE', math_('MULTIPLY', along.outputs['Fac'], 2 * math.pi / 0.0015))
        h = math_('MULTIPLY', ribs, 0.15)
        edge = math_('MINIMUM', across.outputs['Fac'], math_('SUBTRACT', 1.0, across.outputs['Fac']))
        line = math_('LESS_THAN', math_('ABSOLUTE', math_('SUBTRACT', edge, 0.1)), 0.03)
        dash = math_('GREATER_THAN', math_('SINE', math_('MULTIPLY', along.outputs['Fac'], 2 * math.pi / 0.004)), -0.2)
        stitch = math_('MULTIPLY', line, dash)
        return math_('SUBTRACT', h, math_('MULTIPLY', stitch, 0.9))
    if k == PLASTIC:
        # a side-release buckle: two slots across the top and the release tabs
        a = along.outputs['Fac']
        s1 = math_('LESS_THAN', math_('ABSOLUTE', math_('SUBTRACT', a, 0.012)), 0.003)
        s2 = math_('LESS_THAN', math_('ABSOLUTE', math_('SUBTRACT', a, 0.046)), 0.003)
        return math_('MULTIPLY', math_('ADD', s1, s2), -1.5)
    if k == PVC:
        pebble = node('ShaderNodeTexVoronoi', Scale=500.0)
        links.new(obj, pebble.inputs['Vector'])
        return math_('MULTIPLY', pebble.outputs['Distance'], 0.5)
    return math_('MULTIPLY', 0.0, 0.0)


# ---------------------------------------------------------------------------------------------
def bake(low, size, samples):
    """Bakes the game mesh's own materials (colour, the bevelled and embossed normals,
    occlusion, gloss and Fresnel) into its single UV layout."""
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = samples
    images = {}
    for name in ('colour', 'normal', 'ao', 'gloss', 'fresnel'):
        img = bpy.data.images.new('bake_' + name, size, size, alpha=False, float_buffer=True)
        img.colorspace_settings.name = 'Non-Color' if name != 'colour' else 'Linear Rec.709'
        images[name] = img
    for o in scene.objects:
        o.select_set(False)
    low.select_set(True)
    bpy.context.view_layer.objects.active = low
    bk = scene.render.bake
    bk.use_selected_to_active = False
    bk.margin = 8
    materials = [slot.material for slot in low.material_slots]

    def run(kind, image, **kw):
        for m in materials:
            m.node_tree.nodes['bake_target'].image = image
        bpy.ops.object.bake(type=kind, **kw)

    run('DIFFUSE', images['colour'], pass_filter={'COLOR'})
    run('NORMAL', images['normal'], normal_space='TANGENT')
    scene.cycles.samples = max(samples, 64)
    run('AO', images['ao'])
    scene.cycles.samples = samples
    # gloss and Fresnel: each stuff's constant in place of its colour
    for key in ('gloss', 'fresnel'):
        saved = {}
        for m in materials:
            bsdf = m.node_tree.nodes['Principled BSDF']
            link = bsdf.inputs['Base Color'].links[0]
            saved[m.name] = link.from_socket
            m.node_tree.links.remove(link)
            bsdf.inputs['Base Color'].default_value = (m[key], m[key], m[key], 1)
        run('DIFFUSE', images[key], pass_filter={'COLOR'})
        for m in materials:
            bsdf = m.node_tree.nodes['Principled BSDF']
            m.node_tree.links.new(saved[m.name], bsdf.inputs['Base Color'])
    return images


def pixels(img):
    a = np.empty(img.size[0] * img.size[1] * 4, np.float32)
    img.pixels.foreach_get(a)
    return a.reshape(img.size[1], img.size[0], 4)


def save(arr, path, srgb):
    h, w = arr.shape[:2]
    img = bpy.data.images.new(os.path.basename(path), w, h, alpha=False)
    if not srgb:
        img.colorspace_settings.name = 'Non-Color'
    rgba = np.ones((h, w, 4), np.float32)
    rgba[..., :3] = np.clip(arr[..., :3], 0, 1)
    img.pixels.foreach_set(rgba.ravel())
    img.filepath_raw = path
    img.file_format = 'PNG'
    img.save()


def compose(images, out):
    colour = pixels(images['colour'])[..., :3]
    ao = pixels(images['ao'])[..., :1]
    ao = 0.35 + 0.65 * ao ** 1.3
    # the colour bake is linear (a float image); occlusion darkens it before it goes to sRGB
    lin = colour * ao
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(np.maximum(lin, 0), 1 / 2.4) - 0.055)
    save(srgb, os.path.join(out, 'rucksack_diffuse.png'), True)
    save(pixels(images['normal']), os.path.join(out, 'rucksack_normal.png'), False)
    g = pixels(images['gloss'])[..., :3] * (0.6 + 0.4 * ao)
    save(g, os.path.join(out, 'rucksack_gloss.png'), False)
    save(pixels(images['fresnel'])[..., :3], os.path.join(out, 'rucksack_fresnel.png'), False)


def render_icon(low, maps, path):
    """The lobby icon's picture: the pack from the front, a little from the side and above, lit
    as the stock icons are, on a transparent background."""
    scene = bpy.context.scene
    m = bpy.data.materials.new('icon')
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes['Principled BSDF']
    bsdf.inputs['Roughness'].default_value = 0.8
    tex = nt.nodes.new('ShaderNodeTexImage')
    tex.image = bpy.data.images.load(os.path.join(maps, 'rucksack_diffuse.png'))
    nt.links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
    nrm = nt.nodes.new('ShaderNodeTexImage')
    nrm.image = bpy.data.images.load(os.path.join(maps, 'rucksack_normal.png'))
    nrm.image.colorspace_settings.name = 'Non-Color'
    nmap = nt.nodes.new('ShaderNodeNormalMap')
    nt.links.new(nrm.outputs['Color'], nmap.inputs['Color'])
    nt.links.new(nmap.outputs['Normal'], bsdf.inputs['Normal'])
    # the pack without the shoulder straps' loops (they are on the wearer, not in the picture)
    shown = low.copy()
    shown.data = low.data.copy()
    shown.modifiers.clear()
    scene.collection.objects.link(shown)
    bm = bmesh.new()
    bm.from_mesh(shown.data)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces
                               if f.calc_center_median().y < 0.175 or f.calc_center_median().z > 1.555],
                     context='FACES')
    bm.to_mesh(shown.data)
    bm.free()
    shown.data.materials.clear()
    shown.data.materials.append(m)
    low.hide_render = True
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = 64
    scene.render.film_transparent = True
    scene.render.resolution_x = scene.render.resolution_y = 512
    scene.view_settings.view_transform = 'Standard'
    world = bpy.data.worlds.new('icon')
    world.use_nodes = True
    world.node_tree.nodes['Background'].inputs[0].default_value = (0.5, 0.52, 0.55, 1)
    world.node_tree.nodes['Background'].inputs[1].default_value = 0.35
    scene.world = world
    lights = []
    for name, rot, energy in (('key', (50, 0, 200), 4.2), ('rim', (65, 0, 40), 1.5)):
        light = bpy.data.objects.new(name, bpy.data.lights.new(name, 'SUN'))
        light.data.energy = energy
        light.rotation_euler = [math.radians(a) for a in rot]
        scene.collection.objects.link(light)
        lights.append(light)
    cam = bpy.data.objects.new('icon_cam', bpy.data.cameras.new('icon_cam'))
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam.data.type = 'ORTHO'
    cam.data.ortho_scale = 0.62
    target = Vector((0.0, 0.29, 1.30))
    a, e = math.radians(198), math.radians(12)   # in front of the pack's face, a little to its right
    cam.location = target + Vector((math.sin(a) * math.cos(e), -math.cos(a) * math.cos(e), math.sin(e))) * 2
    cam.rotation_euler = (target - cam.location).to_track_quat('-Z', 'Y').to_euler()
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    for o in lights + [cam, shown]:
        bpy.data.objects.remove(o)
    low.hide_render = False


def armature_for(low):
    arm = bpy.data.armatures.new('rig')
    ob = bpy.data.objects.new('rig', arm)
    bpy.context.scene.collection.objects.link(ob)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.mode_set(mode='EDIT')
    root = arm.edit_bones.new('Root')
    root.head, root.tail = (0, 0, 0), (0, 0, 0.1)
    for name, head in BONES.items():
        e = arm.edit_bones.new(name)
        e.head = head
        e.tail = Vector(head) + Vector((0, 0, 0.05))
        e.parent = root
    bpy.ops.object.mode_set(mode='OBJECT')
    low.parent = ob
    mod = low.modifiers.new('rig', 'ARMATURE')
    mod.object = ob
    return ob


def export_glb(low, rig, path):
    for o in bpy.context.scene.objects:
        o.select_set(False)
    low.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = low
    bpy.ops.export_scene.gltf(filepath=path, export_format='GLB', use_selection=True,
                              export_tangents=True, export_normals=True, export_texcoords=True,
                              export_skins=True, export_all_influences=False, export_materials='NONE',
                              export_animations=False, export_yup=True, export_apply=False)


def main():
    argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--size', type=int, default=1024)
    ap.add_argument('--samples', type=int, default=16)
    ap.add_argument('--no-bake', action='store_true')
    args = ap.parse_args(argv)
    os.makedirs(args.out, exist_ok=False)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    b = build_low()
    low = finish_low(b)
    tris = sum(len(p.vertices) - 2 for p in low.data.polygons)
    print(f'rucksack: {len(low.data.vertices)} vertices, {tris} triangles')
    if not args.no_bake:
        compose(bake(low, args.size, args.samples), args.out)
        render_icon(low, args.out, os.path.join(args.out, 'rucksack_icon.png'))
    rig = armature_for(low)
    export_glb(low, rig, os.path.join(args.out, 'rucksack.glb'))
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(args.out, 'rucksack.blend'))


if __name__ == '__main__':
    main()
