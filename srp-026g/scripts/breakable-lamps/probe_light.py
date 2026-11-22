"""The share of a room probe's light that is not its lamp's.

A ported map's room is lit mostly by its environment probes, whose cubemaps
the donor's editor baked with the lamps on. A lamp that breaks takes its own
light away, not what it put into the probe of its room. The donor ties no
probe (nor ambient light) to a lamp, and has no lamp-off bake: this splits a
probe's light by colour instead.

A probe's light is the mean of its irradiance cubemap (its `_diffuse.dds`),
as 0.26g's probe shader reads it (RGBM: the texel squared over its alpha
squared). It is taken as a mix of two colours: the lamp's (its light
object's colour) and the room's light without lamps (the mean colour of the
map's probes that hold no lamp). Where those two colours differ enough (a
fluorescent tube's lavender against the warm daylight), the mix's two
weights say how much of the probe is the lamp's. A kerosene lamp's flame
(no light object) or a warm bulb is the daylight's colour: no split.

A weather's sky can have the tubes' colour (London's rain is as blue as
them). The lamp is the same in every weather, and so is the probe (its
place and box): where a weather's own split cannot be made, the lamp's light
the day's split found in the probe of that name and place is taken out of
the weather's, if what is left is that weather's room colour (London
(v0.42)'s rain; its evening bake has no lamp in it, and its own split binds
nothing there).
"""
import math
import struct
from pathlib import Path

# The lamp's part of a probe's light from which the probe is the lamp's.
LAMP_SHARE = 0.4
# The least angle between the lamp's colour and the room's for a split.
SPLIT_ANGLE = 8.0
# What a probe keeps at least once its lamp is broken: light from the room's
# doors and windows the split cannot tell from the lamp's.
KEEPS_FLOOR = 0.15


def _rgb565(v):
    return ((v >> 11 & 31) / 31, (v >> 5 & 63) / 63, (v & 31) / 31)


def _dxt5_block(b):
    a0, a1 = b[0], b[1]
    if a0 > a1:
        alphas = [a0, a1] + [((6 - i) * a0 + (i + 1) * a1) / 7 for i in range(6)]
    else:
        alphas = [a0, a1] + [((4 - i) * a0 + (i + 1) * a1) / 5 for i in range(4)] + [0, 255]
    alpha_bits = int.from_bytes(b[2:8], 'little')
    c0, c1, bits = struct.unpack('<HHI', b[8:16])
    p0, p1 = _rgb565(c0), _rgb565(c1)
    colours = [p0, p1, tuple((2 * x + y) / 3 for x, y in zip(p0, p1)), tuple((x + 2 * y) / 3 for x, y in zip(p0, p1))]
    return [(colours[bits >> 2 * i & 3], alphas[alpha_bits >> 3 * i & 7] / 255) for i in range(16)]


def irradiance(path):
    """The mean linear light of a DXT5 cube map's first mip over its six faces."""
    data = Path(path).read_bytes()
    if data[:4] != b'DDS ' or data[84:88] != b'DXT5':
        raise ValueError(f'{path}: not a DXT5 DDS')
    height, width = struct.unpack('<II', data[12:20])
    mips = max(1, struct.unpack('<I', data[28:32])[0])
    total, count, offset = [0.0, 0.0, 0.0], 0, 128
    for _ in range(6):
        w, h = width, height
        for mip in range(mips):
            blocks = max(1, w // 4) * max(1, h // 4)
            if mip == 0:
                for k in range(blocks):
                    for rgb, a in _dxt5_block(data[offset + 16 * k:offset + 16 * k + 16]):
                        if a > 0:
                            for c in range(3):
                                total[c] += rgb[c] * rgb[c] / (a * a)
                        count += 1
            offset += 16 * blocks
            w, h = max(1, w // 2), max(1, h // 2)
    return [t / count for t in total]


def luminance(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def hue(c):
    """A colour scaled to luminance 1."""
    return [x / luminance(c) for x in c]


def _local(probe, point):
    """A point in the probe's frame (mapdata::Rotation::from_angles: row-vector Rx * Ry * Rz)."""
    x, y, z = probe['rotation']
    xs, xc, ys, yc, zs, zc = math.sin(x), math.cos(x), math.sin(y), math.cos(y), math.sin(z), math.cos(z)
    axes = ((yc * zc, -yc * zs, ys),
            (ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            (-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc))
    d = [p - q for p, q in zip(point, probe['position'])]
    return [sum(d[k] * axes[j][k] for k in range(3)) for j in range(3)]


def holds(probe, point):
    """Whether a point is inside a probe's volume: its box (half extents its scale) or its sphere."""
    if probe.get('geometry') == 1:
        return all(abs(a) <= b for a, b in zip(_local(probe, point), probe['scale']))
    return math.dist(probe['position'], point) < probe['radius']


def split(light, lamp, room):
    """(lamp's share, the share it keeps) of a probe's light `light` from the hues `lamp` and
    `room`, or None where the two hues are too alike to tell apart."""
    cos = sum(a * b for a, b in zip(lamp, room)) / math.sqrt(sum(a * a for a in lamp) * sum(b * b for b in room))
    if math.degrees(math.acos(min(1.0, cos))) < SPLIT_ANGLE:
        return None
    # Least squares: light = a * lamp + d * room.
    ll, lr, rr = (sum(a * b for a, b in zip(u, v)) for u, v in ((lamp, lamp), (lamp, room), (room, room)))
    bl, br = sum(a * b for a, b in zip(lamp, light)), sum(a * b for a, b in zip(room, light))
    det = ll * rr - lr * lr
    a, d = (bl * rr - br * lr) / det, (ll * br - lr * bl) / det
    total = luminance(light)
    return a / total, min(1.0, max(KEEPS_FLOOR, d / total))


def angle(a, b):
    cos = sum(x * y for x, y in zip(a, b)) / math.sqrt(sum(x * x for x in a) * sum(y * y for y in b))
    return math.degrees(math.acos(max(-1.0, min(1.0, cos))))


def lamp_probes(probes, lamp_lights, cubemap, day=None):
    """({lamp: [(probe, keeps)]}, {probe name: (position, the lamp's light in it)}): each diffuse
    probe that holds the light of one lamp alone and whose light is at least LAMP_SHARE that
    lamp's, with the share it keeps once the lamp is broken, and the lamp's light the split
    found in it. `lamp_lights` is [(lamp, light object)], `cubemap(probe)` a probe's irradiance
    file, `day` the day project's second result (for a weather's probes its own split cannot
    tell apart)."""
    diffuse = [p for p in probes if p.get('affect_diffuse', 1)]
    light_of = {id(p): irradiance(cubemap(p)) for p in diffuse}
    free = [p for p in diffuse if not any(holds(p, light['position']) for _, light in lamp_lights)]
    if not free:
        return {}, {}
    room = [sum(hue(light_of[id(p)])[c] for p in free) / len(free) for c in range(3)]
    found, lamp_light = {}, {}
    for probe in diffuse:
        inside = {lamp for lamp, light in lamp_lights if holds(probe, light['position'])}
        if len(inside) != 1:
            continue
        lamp = inside.pop()
        colours = [hue(light['color']) for owner, light in lamp_lights if owner == lamp]
        colour = [sum(c[k] for c in colours) / len(colours) for k in range(3)]
        light = light_of[id(probe)]
        result = split(light, colour, room)
        if result is not None:
            lamp_light[probe['full_name']] = (probe['position'], [result[0] * luminance(light) * c for c in colour])
        elif day and day.get(probe['full_name'], (None,))[0] == probe['position']:
            left = [a - b for a, b in zip(light, day[probe['full_name']][1])]
            if min(left) > 0 and angle(left, room) < SPLIT_ANGLE:
                result = 1 - luminance(left) / luminance(light), min(1.0, max(KEEPS_FLOOR, luminance(left) / luminance(light)))
        if result is not None and result[0] >= LAMP_SHARE:
            found.setdefault(lamp, []).append((probe, round(result[1], 3)))
    return found, lamp_light
