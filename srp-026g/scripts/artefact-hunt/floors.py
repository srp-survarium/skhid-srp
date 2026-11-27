"""floors.py: a map's floors and walls, from the file `mapdata-dump` writes for its project
(crates/mapdata, `format.rs`: the collision models of the project and where each one stands).

Used by make_pack.py for the fit report of 0.46c2's points against 0.26g's maps: what a point
stands on, how far under or over it the floor is, what is over its head and how near a wall is.
Plain Python, no dependencies.

A body is the game's character as crates/mapdata has it (`grid::Body`): 1.8 m tall, 0.38 m round,
steps up 0.6 m; ground is a `walk` face, or a `by slope` one up to 60 degrees (normal y >= 0.5).
What a body meets is the walker set (a placement with cgroup & 2 and cmask & 4,
`project::CollisionFilter::WALKER`); a thing lying on the ground rests on that or on what a bullet
meets (cgroup & 8, cmask & 16).
"""
import math
import struct

MAGIC, END, VERSION = b'SRPMAPD\0', b'END\0', 2
BY_SLOPE, WALK, DONT_WALK = 0, 1, 2
MIN_GROUND_NORMAL_Y = 0.5
HEIGHT, RADIUS, STEP = 1.8, 0.38, 0.6
SUNK = 0.10
CELL = 2.0


class Reader:
    def __init__(self, data):
        self.data, self.at = data, 0

    def take(self, fmt):
        values = struct.unpack_from('<' + fmt, self.data, self.at)
        self.at += struct.calcsize('<' + fmt)
        return values

    def u32(self):
        return self.take('I')[0]

    def text(self):
        size = self.u32()
        self.at += size
        return self.data[self.at - size:self.at].decode('utf-8')

    def skip(self, size):
        self.at += size

    def shapes(self):
        self.skip(61 * self.u32())


def load(path):
    """{project, spawns [(position, rotation, team, priority, point_id, layer)], libraries
    [(name, vertices, triangles, walker)], placements [(library, position, rotation, cgroup, cmask)]}."""
    data = open(path, 'rb').read()
    r = Reader(data)
    if data[:8] != MAGIC or data[-4:] != END:
        raise ValueError(f'{path}: not a map data file')
    r.skip(8)
    if r.u32() != VERSION:
        raise ValueError(f'{path}: not map data version {VERSION}: write it again with mapdata-dump')
    out = {'project': r.text()}
    r.skip(24)
    out['spawns'] = []
    for _ in range(r.u32()):
        values = r.take('6f3i')
        out['spawns'].append((values[0:3], values[3:6], values[6], values[7], values[8], r.text()))
    for _ in range(r.u32()):            # containers
        r.skip(20)
        r.shapes()
    for _ in range(r.u32()):            # battery spawners
        r.skip(24)
        r.text()
    r.skip(12 * r.u32())                # stations
    for _ in range(r.u32()):            # zones
        r.skip(1)
        r.text()
        r.skip(16)
        r.text()
        if r.take('B')[0]:
            r.skip(14)
            r.skip(8 * r.u32())
        r.shapes()
    for _ in range(r.u32()):            # anomalies
        r.text()
        r.skip(12)
        for _ in range(r.u32()):
            r.text()
        r.skip(1 + 48 + 1 + 16 + 8)
        for _ in range(r.u32()):        # states
            r.skip(1 + 4 + 4 + 4 + 4 + 2)
            for _ in range(r.u32()):    # groups
                r.skip(3 + 16)
                for _ in range(r.u32()):
                    r.text()
    out['libraries'] = []
    for _ in range(r.u32()):
        name = r.text()
        r.skip(4)
        count = r.u32()
        vertices = struct.unpack_from(f'<{3 * count}f', data, r.at)
        r.skip(12 * count)
        count = r.u32()
        triangles = struct.unpack_from(f'<{3 * count}I', data, r.at)
        r.skip(12 * count)
        walker = data[r.at:r.at + count]
        r.skip(count)
        out['libraries'].append((name, vertices, triangles, walker))
    out['placements'] = []
    for _ in range(r.u32()):
        values = r.take('I6f2H')
        out['placements'].append((values[0], values[1:4], values[4:7], values[7], values[8]))
    if data[r.at:] != END:
        raise ValueError(f'{path}: {len(data) - r.at} bytes after the placements')
    return out


def rotation(angles):
    """crates/mapdata `Rotation::from_angles` (`math::create_rotation`): the rows i, j, k."""
    xs, xc = math.sin(angles[0]), math.cos(angles[0])
    ys, yc = math.sin(angles[1]), math.cos(angles[1])
    zs, zc = math.sin(angles[2]), math.cos(angles[2])
    return ((yc * zc, -yc * zs, ys),
            (ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            (-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc))


def point_triangle(p, a, b, c):
    """The distance from a point to a triangle (Ericson, Real-Time Collision Detection 5.1.5)."""
    ab = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    ac = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    ap = (p[0] - a[0], p[1] - a[1], p[2] - a[2])
    d1 = ab[0] * ap[0] + ab[1] * ap[1] + ab[2] * ap[2]
    d2 = ac[0] * ap[0] + ac[1] * ap[1] + ac[2] * ap[2]
    if d1 <= 0 and d2 <= 0:
        return math.dist(p, a)
    bp = (p[0] - b[0], p[1] - b[1], p[2] - b[2])
    d3 = ab[0] * bp[0] + ab[1] * bp[1] + ab[2] * bp[2]
    d4 = ac[0] * bp[0] + ac[1] * bp[1] + ac[2] * bp[2]
    if d3 >= 0 and d4 <= d3:
        return math.dist(p, b)
    vc = d1 * d4 - d3 * d2
    if vc <= 0 and d1 >= 0 and d3 <= 0:
        v = d1 / (d1 - d3)
        return math.dist(p, (a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]))
    cp = (p[0] - c[0], p[1] - c[1], p[2] - c[2])
    d5 = ab[0] * cp[0] + ab[1] * cp[1] + ab[2] * cp[2]
    d6 = ac[0] * cp[0] + ac[1] * cp[1] + ac[2] * cp[2]
    if d6 >= 0 and d5 <= d6:
        return math.dist(p, c)
    vb = d5 * d2 - d1 * d6
    if vb <= 0 and d2 >= 0 and d6 <= 0:
        w = d2 / (d2 - d6)
        return math.dist(p, (a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]))
    va = d3 * d6 - d5 * d4
    if va <= 0 and d4 - d3 >= 0 and d5 - d6 >= 0:
        w = (d4 - d3) / ((d4 - d3) + (d5 - d6))
        return math.dist(p, (b[0] + w * (c[0] - b[0]), b[1] + w * (c[1] - b[1]), b[2] + w * (c[2] - b[2])))
    denom = va + vb + vc
    if denom == 0:
        return math.dist(p, a)
    v, w = vb / denom, vc / denom
    return math.dist(p, (a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w))


class World:
    """The placed triangles of a map near the points asked about, in a grid of CELL metres.

    `near`: only models within `reach` metres (in plan) of one of these points are placed (a
    map has a million triangles; the report asks about a few hundred places)."""

    def __init__(self, mapdata, near, reach=12.0):
        self.cells = {}
        self.triangles = []         # (a, b, c, ground, walker set, min y, max y)
        wanted = {(int(p[0] // reach) + i, int(p[2] // reach) + j) for p in near for i in (-1, 0, 1) for j in (-1, 0, 1)}
        bounds = {}
        for library, position, angles, cgroup, cmask in mapdata['placements']:
            walker, bullet = bool(cgroup & 2 and cmask & 4), bool(cgroup & 8 and cmask & 16)
            if not walker and not bullet:
                continue
            _, vertices, indices, behaviour = mapdata['libraries'][library]
            if not indices:
                continue
            (ix, iy, iz), (jx, jy, jz), (kx, ky, kz) = rotation(angles)
            px, py, pz = position
            if library not in bounds:
                bounds[library] = max(math.sqrt(vertices[n] ** 2 + vertices[n + 1] ** 2 + vertices[n + 2] ** 2)
                                      for n in range(0, len(vertices), 3))
            radius = bounds[library]
            if radius < reach and not any((int((px + dx) // reach), int((pz + dz) // reach)) in wanted
                                          for dx in (-radius, radius) for dz in (-radius, radius)):
                continue
            world = [(px + ix * vertices[n] + jx * vertices[n + 1] + kx * vertices[n + 2],
                      py + iy * vertices[n] + jy * vertices[n + 1] + ky * vertices[n + 2],
                      pz + iz * vertices[n] + jz * vertices[n + 1] + kz * vertices[n + 2])
                     for n in range(0, len(vertices), 3)]
            for t in range(len(behaviour)):
                a, b, c = world[indices[3 * t]], world[indices[3 * t + 1]], world[indices[3 * t + 2]]
                x0, x1 = min(a[0], b[0], c[0]), max(a[0], b[0], c[0])
                z0, z1 = min(a[2], b[2], c[2]), max(a[2], b[2], c[2])
                if radius >= reach and not any((i, j) in wanted for i in range(int(x0 // reach), int(x1 // reach) + 1)
                                               for j in range(int(z0 // reach), int(z1 // reach) + 1)):
                    continue
                nx = (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1])
                ny = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
                nz = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
                length = math.sqrt(nx * nx + ny * ny + nz * nz)
                if length == 0:
                    continue
                kind = behaviour[t]
                ground = kind == WALK or (kind == BY_SLOPE and abs(ny) / length >= MIN_GROUND_NORMAL_Y)
                index = len(self.triangles)
                self.triangles.append((a, b, c, ground, walker, min(a[1], b[1], c[1]), max(a[1], b[1], c[1])))
                for i in range(int(x0 // CELL), int(x1 // CELL) + 1):
                    for j in range(int(z0 // CELL), int(z1 // CELL) + 1):
                        self.cells.setdefault((i, j), []).append(index)

    def column(self, x, z):
        """Every face over (x, z): [(height, ground, walker set)], lowest first."""
        out = []
        for index in self.cells.get((int(x // CELL), int(z // CELL)), ()):
            a, b, c, ground, walker, _, _ = self.triangles[index]
            det = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2])
            if abs(det) < 1e-9:
                continue
            u = ((x - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (z - a[2])) / det
            v = ((b[0] - a[0]) * (z - a[2]) - (x - a[0]) * (b[2] - a[2])) / det
            if u < 0 or v < 0 or u + v > 1:
                continue
            out.append((a[1] + u * (b[1] - a[1]) + v * (c[1] - a[1]), ground, walker))
        out.sort()
        return out

    def nearest(self, x, z, low, high, reach, walker_only=True):
        """The distance from the upright segment (x, low..high, z) to the nearest face, up to `reach`."""
        best = reach
        seen = set()
        samples = [low + (high - low) * n / 5 for n in range(6)]
        for i in range(int((x - reach) // CELL), int((x + reach) // CELL) + 1):
            for j in range(int((z - reach) // CELL), int((z + reach) // CELL) + 1):
                for index in self.cells.get((i, j), ()):
                    if index in seen:
                        continue
                    seen.add(index)
                    a, b, c, _, walker, y0, y1 = self.triangles[index]
                    if (walker_only and not walker) or y1 < low - best or y0 > high + best:
                        continue
                    if (min(a[0], b[0], c[0]) > x + best or max(a[0], b[0], c[0]) < x - best
                            or min(a[2], b[2], c[2]) > z + best or max(a[2], b[2], c[2]) < z - best):
                        continue
                    for y in samples:
                        best = min(best, point_triangle((x, y, z), a, b, c))
        return best

    def stand(self, point, lying=False):
        """How a point stands on the map: {below, above, drop, rise, head, wall}.

        below: the ground at or under it, or up to SUNK over it (0.26g's own respawn points lie
        up to 6 cm under their floor: level_03's); above: the lowest ground over that; drop and
        rise the distances to them; head: the room over `below` (or over the point) to the next
        face a body meets; wall: the distance from a body's middle (from a step's height to its
        head, over `below`) to the nearest face a body meets.
        `lying`: a thing on the ground, which rests on anything a body or a bullet meets."""
        x, y, z = point
        faces = self.column(x, z)
        floors = [h for h, ground, walker in faces if (ground and walker) or (lying and (ground or not walker))]
        under = [h for h in floors if h <= y + SUNK]
        over = [h for h in floors if h > y + SUNK]
        out = {'below': max(under) if under else None, 'above': min(over) if over else None}
        out['drop'] = y - out['below'] if under else None
        out['rise'] = out['above'] - y if over else None
        feet = out['below'] if under and y - out['below'] <= 1.0 else y
        ceilings = [h for h, _, walker in faces if walker and h > feet + 0.3]
        out['head'] = min(ceilings) - feet if ceilings else None
        out['wall'] = self.nearest(x, z, feet + STEP, feet + HEIGHT - 0.1, 1.0)
        return out

    def place(self, point, reach=8.0, step=0.25, clear=RADIUS, low=STEP, high=HEIGHT - 0.1, flat=0.0):
        """The nearest place to `point` where a body stands free: ground, HEIGHT of room over it,
        no face a body meets within `clear` of the upright from `low` to `high` over it (a body's
        middle; a box's, with other numbers) and, with `flat`, ground within 15 cm of the same
        height `flat` metres to each side. (distance, place), or None within `reach`."""
        x, y, z = point
        rings = int(reach / step)
        offsets = sorted((math.hypot(i * step, j * step), i * step, j * step)
                         for i in range(-rings, rings + 1) for j in range(-rings, rings + 1))
        best = None
        for plan, dx, dz in offsets:
            if plan > reach or (best and plan >= best[0]):
                break
            px, pz = x + dx, z + dz
            faces = self.column(px, pz)
            for h, ground, walker in faces:
                if not (ground and walker):
                    continue
                distance = math.sqrt(plan * plan + (h - y) ** 2)
                if best and distance >= best[0]:
                    continue
                if any(w and h + 0.3 < other < h + HEIGHT for other, _, w in faces):
                    continue
                if flat and not all(any(abs(o - h) <= 0.15 and g and w for o, g, w in self.column(px + ex, pz + ez))
                                    for ex, ez in ((flat, 0), (-flat, 0), (0, flat), (0, -flat))):
                    continue
                if self.nearest(px, pz, h + low, h + high, clear + 0.01) <= clear:
                    continue
                best = (distance, (px, h, pz))
        return best
