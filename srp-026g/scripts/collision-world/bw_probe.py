"""What the game's own walker world has under a point: vertical probes of a capture's Bullet world.

    python bw_probe.py <capture.bwc> --rows <export-audit rows.tsv> --match <id> [--out rows-game.tsv]
    python bw_probe.py <capture.bwc> --at X Y Z [--at ...]

The walker world is every object the walking body's casts meet: group & 2 and mask & 4 (the body is group 4,
mask 2). Triangle meshes as they are; boxes, spheres, cylinders and capsules as fine meshes of Bullet's own
shape (48 sides, the margin's rounding left out: under 0.02 m).

For each audit row (the recorded feet of a body the export had no surface for): the game's surfaces under the
capsule's footprint (21 columns out to 0.38 m), and the verdict
  game-has    a surface of the game's world within 0.1 m of the feet's height under the footprint
  game-lacks  none: the body did not stand on the static world the game built for the map
"""
import argparse
import collections
import csv
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bw_diff  # noqa: E402

SIDES = 48


def primitive_mesh(shape):
    kind = shape["kind"]
    e = np.array(shape["implicit_dims"][:3], float)
    m = shape["margin"]
    if kind == "box":
        h = e + m
        pts = np.array([[sx * h[0], sy * h[1], sz * h[2]] for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)])
        quads = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        tris = [(q[0], q[1], q[2]) for q in quads] + [(q[0], q[2], q[3]) for q in quads]
        return pts, np.array(tris)
    if kind == "sphere":
        radius, half = shape["implicit_dims"][0] * shape["local_scaling"][0], 0.0
    elif kind == "capsule":
        up = shape.get("up_axis", 1)
        radius, half = e[(up + 2) % 3], e[up]
    elif kind == "cylinder":
        up = shape.get("up_axis", 1)
        radius, half = e[(up + 2) % 3] + m, e[up] + m
        pts = []
        for y in (-half, half):
            for s in range(SIDES):
                a = s * math.tau / SIDES
                pts.append((radius * math.cos(a), y, radius * math.sin(a)))
        pts += [(0, -half, 0), (0, half, 0)]
        tris = []
        for s in range(SIDES):
            n = (s + 1) % SIDES
            tris += [(s, n, SIDES + n), (s, SIDES + n, SIDES + s), (2 * SIDES, n, s), (2 * SIDES + 1, SIDES + s, SIDES + n)]
        return axis_swap(np.array(pts), up), np.array(tris)
    else:
        return None, None
    # sphere / capsule: rings of a sphere pulled apart along y
    rings = SIDES // 4
    pts = [(0, -half - radius, 0)]
    for sign, order in ((-1, range(rings - 1, -1, -1)), (1, range(0, rings))):
        for r in order:
            lat = r * (math.pi / 2) / rings
            y = sign * (half + radius * math.sin(lat))
            for s in range(SIDES):
                a = s * math.tau / SIDES
                pts.append((radius * math.cos(lat) * math.cos(a), y, radius * math.cos(lat) * math.sin(a)))
    pts.append((0, half + radius, 0))
    top = len(pts) - 1
    tris = []
    for s in range(SIDES):
        n = (s + 1) % SIDES
        tris.append((0, 1 + n, 1 + s))
        last = 1 + (2 * rings - 1) * SIDES
        tris.append((top, last + s, last + n))
        for r in range(2 * rings - 1):
            a, b = 1 + r * SIDES, 1 + (r + 1) * SIDES
            tris += [(a + s, a + n, b + n), (a + s, b + n, b + s)]
    pts = np.array(pts)
    if kind == "capsule":
        pts = axis_swap(pts, shape.get("up_axis", 1))
    return pts, np.array(tris)


def axis_swap(pts, up):
    if up == 1:
        return pts
    out = pts.copy()
    out[:, [1, up]] = pts[:, [up, 1]]
    return out


def walker_triangles(game):
    """(n, 3, 3) triangles of the game's walker world in the map frame, and the object index of each."""
    chunks, owners = [], []
    for obj in game.world["objects"]:
        if obj["group"] is None or not (obj["group"] & 2 and obj["mask"] & 4):
            continue
        for shape, M, t, _ in game.parts(obj):
            if shape is None:
                continue
            if shape["kind"] == "triangle_mesh":
                verts, idx, _, _ = game.mesh(shape)
            else:
                verts, idx = primitive_mesh(shape)
                if verts is None:
                    continue
            world = verts @ M.T + t
            chunks.append(world[idx])
            owners.append(np.full(len(idx), obj["index"]))
    return np.concatenate(chunks), np.concatenate(owners)


class Columns:
    """Vertical lines against a triangle soup, binned on a 2 m grid."""

    CELL = 2.0

    def __init__(self, tris):
        self.tris = tris
        lo = np.floor(tris[:, :, [0, 2]].min(axis=1) / self.CELL).astype(int)
        hi = np.floor(tris[:, :, [0, 2]].max(axis=1) / self.CELL).astype(int)
        self.cells = collections.defaultdict(list)
        for i, ((x0, z0), (x1, z1)) in enumerate(zip(lo.tolist(), hi.tolist())):
            for cx in range(x0, x1 + 1):
                for cz in range(z0, z1 + 1):
                    self.cells[(cx, cz)].append(i)
        self.cells = {k: np.array(v) for k, v in self.cells.items()}

    def heights(self, x, z):
        ids = self.cells.get((math.floor(x / self.CELL), math.floor(z / self.CELL)))
        if ids is None:
            return np.empty(0), np.empty(0, int)
        t = self.tris[ids]
        ax, az = t[:, 0, 0] - x, t[:, 0, 2] - z
        bx, bz = t[:, 1, 0] - x, t[:, 1, 2] - z
        cx, cz = t[:, 2, 0] - x, t[:, 2, 2] - z
        w0 = bx * cz - bz * cx
        w1 = cx * az - cz * ax
        w2 = ax * bz - az * bx
        area = w0 + w1 + w2
        ok = (np.abs(area) > 1e-12) & (((w0 >= 0) & (w1 >= 0) & (w2 >= 0)) | ((w0 <= 0) & (w1 <= 0) & (w2 <= 0)))
        y = (w0[ok] * t[ok, 0, 1] + w1[ok] * t[ok, 1, 1] + w2[ok] * t[ok, 2, 1]) / area[ok]
        return y, ids[ok]


def footprint():
    pts = [(0.0, 0.0)]
    for radius, count in ((0.19, 8), (0.38, 12)):
        pts += [(radius * math.cos(i * math.tau / count), radius * math.sin(i * math.tau / count)) for i in range(count)]
    return pts


def probe(columns, x, y, z):
    """(gap under the centre, nearest surface to the feet's height under the footprint, its object)."""
    ys, _ = columns.heights(x, z)
    under = ys[ys <= y + 0.3]
    gap = float(y - under.max()) if len(under) else None
    best, owner = None, None
    for dx, dz in footprint():
        ys, ids = columns.heights(x + dx, z + dz)
        if len(ys):
            i = int(np.abs(ys - y).argmin())
            if best is None or abs(ys[i] - y) < abs(best):
                best, owner = float(ys[i] - y), int(ids[i])
    return gap, best, owner


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--rows")
    ap.add_argument("--match")
    ap.add_argument("--at", nargs=3, type=float, action="append", default=[])
    ap.add_argument("--out")
    args = ap.parse_args()
    game = bw_diff.Game(args.capture)
    tris, owners = walker_triangles(game)
    columns = Columns(tris)
    print(f"walker world: {len(tris)} triangles of {len(set(owners.tolist()))} objects")
    for x, y, z in args.at:
        gap, best, tri = probe(columns, x, y, z)
        print(f"({x}, {y}, {z}): gap under the centre {gap}, nearest surface under the footprint {best} "
              f"(object {owners[tri] if tri is not None else None})")
    if not args.rows:
        return
    verdicts = collections.Counter()
    out_rows = []
    for row in csv.DictReader(open(args.rows, newline=""), delimiter="\t"):
        if row["match"] != args.match:
            continue
        x, y, z = float(row["x"]), float(row["y"]), float(row["z"])
        gap, best, tri = probe(columns, x, y, z)
        has = best is not None and abs(best) <= 0.1
        verdict = "game-has" if has else "game-lacks"
        verdicts[(row["kind"], verdict)] += 1
        out_rows.append((row["match"], row["place"], row["time"], row["kind"], row["gap"], x, y, z,
                         "" if gap is None else round(gap, 3), "" if best is None else round(best, 3),
                         "" if tri is None else int(owners[tri]), verdict))
    print(f"audit rows of match {args.match}: {sum(verdicts.values())}")
    for key, n in sorted(verdicts.items()):
        print(f"  {key}: {n}")
    if args.out:
        with open(args.out, "w", newline="") as out:
            out.write("match\tplace\ttime\taudit_kind\taudit_gap\tx\ty\tz\tgame_gap_centre\tgame_nearest_dy\tgame_object\tverdict\n")
            for r in out_rows:
                out.write("\t".join(str(v) for v in r) + "\n")


if __name__ == "__main__":
    main()
