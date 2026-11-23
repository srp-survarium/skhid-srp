"""stands.py: a concrete slab under an Artifact Hunt base's box where it stands on rough ground (the owner,
2026-11-22: "I wish we could put them everywhere", "essentially everywhere it stands on rough ground").

Used by make_pack.py (the nine stock levels) and mode-maps/make.py (the other team maps), each with the
`.mapdata` of the maps' Team Deathmatch projects (`mapdata-dump`), the floors a box stands on.

- Rough ground: the walker floor under the box's footprint is a terrain model (its name has `terrain`,
  `terain` or `ground`), or it is more than ROUGH m higher at one point of the footprint than at another.
  Anything else is a built surface (a building's or a street's floor, a platform): no slab.
- The slab is Mamayev Kurgan's, `props/garbage/concrete_plita2`, whose top is a flat plane FLAT_TOP over its
  origin (model units; 0.46c2's team 1 box there rests on it). The flat part is the rectangle FLAT (centre,
  half sizes, turn: measured on its walker mesh); at SCALE it holds a box of 1.28 x 2.48 m, which the
  footprint's longer side goes along.
- The box's foot goes RISE over the highest floor under its footprint (0.46c2's Mamayev team 1: 0.50 m on
  the slab at x1.3; scaled to the slab here), and with it the base, its use shapes and its props, where
  the slab's part over the ground (VISIBLE, in the flat rectangle's frame) cuts into no other model and
  covers no spawn point; else FLUSH over it (0.46c2's Mamayev team 0: 0.04 m), a concrete pad that rises
  over nothing near it.
"""
import math

import floors

SLAB = 'props/garbage/concrete_plita2'
SCALE = 0.8
FLAT_TOP = 1.029
FLAT = ((-0.93, 0.20), (1.55, 0.80), math.radians(145))
RISE = 0.30
FLUSH = 0.04
# The slab's top over the ground line at RISE, in the flat rectangle's frame (model units: u along its long
# side, w across), measured on its walker mesh: the part that can cut into what stands by.
VISIBLE = ((-3.28, -1.69), (2.81, 2.54))
ROUGH = 0.10
TERRAIN = ('terrain', 'terain', 'ground')
# A container model's footprint on its foot (render/export_properties' bounding box): centre and half
# sizes across, in its own frame.
FOOTPRINTS = {
    'item/storage_device/box_atlant8c': ((0.0, 0.0), (0.606, 0.616)),
}


def turn(angle, x, z):
    """The game's yaw (floors.rotation with only y): model (x, z) to world."""
    return math.cos(angle) * x - math.sin(angle) * z, math.sin(angle) * x + math.cos(angle) * z


def yaw(rotation):
    """A placement's turn in plan, from its whole rotation (a model turned (pi, a, pi) is turned pi - a)."""
    axes = floors.rotation(rotation)
    return math.atan2(axes[0][2], axes[0][0])


def footprint_points(model, position, rotation, n=5):
    (cx, cz), (hx, hz) = FOOTPRINTS[model]
    out = []
    for i in range(n):
        for j in range(n):
            x, z = turn(yaw(rotation), cx + hx * (2 * i / (n - 1) - 1), cz + hz * (2 * j / (n - 1) - 1))
            out.append((position[0] + x, position[2] + z))
    return out


class Floors:
    """A map's walker floors near the asked places, each face with its model's name."""

    def __init__(self, mapdata, near, reach=8.0):
        self.faces = []
        for library, position, angles, cgroup, cmask in mapdata['placements']:
            if not (cgroup & 2 and cmask & 4):
                continue
            name, vertices, indices, behaviour = mapdata['libraries'][library]
            if not indices:
                continue
            (ix, iy, iz), (jx, jy, jz), (kx, ky, kz) = floors.rotation(angles)
            px, py, pz = position
            world = [(px + ix * vertices[n] + jx * vertices[n + 1] + kx * vertices[n + 2],
                      py + iy * vertices[n] + jy * vertices[n + 1] + ky * vertices[n + 2],
                      pz + iz * vertices[n] + jz * vertices[n + 1] + kz * vertices[n + 2])
                     for n in range(0, len(vertices), 3)]
            for t in range(len(behaviour)):
                a, b, c = world[indices[3 * t]], world[indices[3 * t + 1]], world[indices[3 * t + 2]]
                if not any(min(a[0], b[0], c[0]) - reach < x < max(a[0], b[0], c[0]) + reach
                           and min(a[2], b[2], c[2]) - reach < z < max(a[2], b[2], c[2]) + reach for x, _, z in near):
                    continue
                nx = (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1])
                ny = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
                nz = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
                length = math.sqrt(nx * nx + ny * ny + nz * nz)
                if length == 0:
                    continue
                kind = behaviour[t]
                ground = kind == floors.WALK or (kind == floors.BY_SLOPE and abs(ny) / length >= floors.MIN_GROUND_NORMAL_Y)
                self.faces.append((a, b, c, ground, name.removeprefix('resources/models/').split('.model/')[0]))

    def under(self, x, z, low, high):
        """The highest ground face of the map's own (a prop's or an item's is dressing, not ground) over
        (x, z) from low to high: (height, model), or None."""
        best = None
        for a, b, c, ground, name in self.faces:
            if not ground or {'props', 'item'} & set(name.split('/')):
                continue
            det = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2])
            if abs(det) < 1e-9:
                continue
            u = ((x - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (z - a[2])) / det
            v = ((b[0] - a[0]) * (z - a[2]) - (x - a[0]) * (b[2] - a[2])) / det
            if u < 0 or v < 0 or u + v > 1:
                continue
            h = a[1] + u * (b[1] - a[1]) + v * (c[1] - a[1])
            if low <= h <= high and (best is None or h > best[0]):
                best = (h, name)
        return best

    def walls(self, corners, low, high):
        """The models of the faces that are not ground, from low to high, with a vertex inside the
        quadrilateral `corners` (a slab's outline in plan): what a slab there would cut into."""
        def inside(x, z):
            sign = None
            for (x0, z0), (x1, z1) in zip(corners, corners[1:] + corners[:1]):
                side = (x1 - x0) * (z - z0) - (z1 - z0) * (x - x0)
                if sign is None:
                    sign = side > 0
                elif (side > 0) != sign:
                    return False
            return True
        return sorted({name for a, b, c, ground, name in self.faces if not ground
                       and any(low < p[1] < high and inside(p[0], p[2]) for p in (a, b, c))})


def ground(world, model, position, rotation):
    """The floor under a box's footprint (from 1 m under its foot to 0.5 m over it): (highest, lowest,
    models) of the points that have one (a box may overhang an edge), or None where most have none."""
    found = [world.under(x, z, position[1] - 1.0, position[1] + 0.5) for x, z in footprint_points(model, position, rotation)]
    found = [f for f in found if f is not None]
    if 2 * len(found) <= len(footprint_points(model, position, rotation)):
        return None
    heights = [h for h, _ in found]
    return max(heights), min(heights), sorted({name for _, name in found})


def rough(floor):
    """Why the floor is rough ground, or None for a built surface."""
    top, bottom, models = floor
    terrain = [m for m in models if any(word in m.rsplit('/', 1)[-1] for word in TERRAIN)]
    if terrain:
        return f'terrain ({", ".join(terrain)})'
    if top - bottom > ROUGH:
        return f'uneven: {top - bottom:.2f} m under its footprint ({", ".join(models)})'
    return None


def place(world, model, position, rotation, floor_top, points):
    """The slab under a box: of its placements (the footprint at five places along the flat
    rectangle, the slab turned either way round under it), the first RISE high whose part over the ground
    cuts into no other model and covers none of `points` [(name, position)]; else FLUSH, the first that
    covers none, else the one that covers fewest. (the box's new foot, the slab's place, its outline,
    what it cuts into, the points it covers)."""
    (_, _), (hx, hz) = FOOTPRINTS[model]
    room = FLAT[1][0] - max(hx, hz) / SCALE
    # a square footprint fits the rectangle across it too
    turns = (0.0, math.pi) + ((math.pi / 2, 3 * math.pi / 2) if abs(hx - hz) < 0.05 else ())
    best = None
    for rise in (RISE, FLUSH):
        for slide in (0.0, -room / 2, room / 2, -room, room):
            for half_turn in turns:
                foot, at, outline = slab(model, position, rotation, floor_top, slide, half_turn, rise)
                covered = [name for name, p in points if inside(outline, p[0], p[2], 0.3) and floor_top - 1.0 < p[1] < foot + 0.5]
                cut = world.walls(outline, floor_top + 0.05, foot + 0.1) if rise == RISE else []
                if not cut and not covered:
                    return foot, at, outline, cut, covered
                if rise == FLUSH and (best is None or len(covered) < len(best[4])):
                    best = (foot, at, outline, cut, covered)
    return best


def inside(polygon, x, z, margin):
    """(x, z) in the convex `polygon` (counter-clockwise in plan) or within `margin` of it."""
    for (x0, z0), (x1, z1) in zip(polygon, polygon[1:] + polygon[:1]):
        if ((x1 - x0) * (z - z0) - (z1 - z0) * (x - x0)) / math.hypot(x1 - x0, z1 - z0) < -margin:
            return False
    return True


def slab(model, position, rotation, floor_top, slide=0.0, half_turn=0.0, rise=RISE):
    """The slab under a box at `position` (its foot) turned `rotation`, the box's new foot RISE over
    floor_top, the footprint `slide` along the flat rectangle's long side (model units): (the box's new foot
    height, the slab's place {position, rotation, scale}, its outline in plan)."""
    (cx, cz), (hx, hz) = FOOTPRINTS[model]
    (fx, fz), _, flat_turn = FLAT
    fx, fz = fx + math.cos(-flat_turn) * slide, fz + math.sin(-flat_turn) * slide
    # the rectangle's long side (u) along the footprint's longer side
    box_turn = yaw(rotation)
    slab_turn = box_turn + flat_turn + (0.0 if hx >= hz else math.pi / 2) + half_turn
    foot = floor_top + rise
    ox, oz = turn(box_turn, cx, cz)
    sx, sz = turn(slab_turn, SCALE * fx, SCALE * fz)
    origin = (position[0] + ox - sx, foot - SCALE * FLAT_TOP, position[2] + oz - sz)
    (u0, w0), (u1, w1) = VISIBLE
    corners = [(FLAT[0][0] + math.cos(-flat_turn) * u - math.sin(-flat_turn) * w,
                FLAT[0][1] + math.sin(-flat_turn) * u + math.cos(-flat_turn) * w)
               for u, w in ((u0, w0), (u1, w0), (u1, w1), (u0, w1))]
    outline = [turn(slab_turn, SCALE * x, SCALE * z) for x, z in corners]
    outline = [(origin[0] + x, origin[2] + z) for x, z in outline]
    slab_turn = math.atan2(math.sin(slab_turn), math.cos(slab_turn))
    return foot, {'position': origin, 'rotation': (0.0, slab_turn, 0.0), 'scale': (SCALE, SCALE, SCALE)}, outline


def entries(place, vector):
    """The slab's render visual and its two collision objects, as 0.26g's projects write a scaled one."""
    values = {'position': vector(place['position']), 'rotation': vector(place['rotation']),
              'scale': vector(place['scale'])}
    suffix = ''.join(f'[{v:.2f}]' for v in place['scale'])
    visual = {'lib_name': SLAB, 'visual_name': SLAB, **values}
    solids = [{'lib_name': f'resources/models/{SLAB}.model/{part}#{suffix}', 'cgroup': cgroup, 'cmask': cmask, **values}
              for part, cgroup, cmask in (('collision', 8, 16), ('walker_collision', 2, 4))]
    return visual, solids
