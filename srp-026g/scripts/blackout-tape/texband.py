"""The band per texel, for one torso surface and a window along the upper arm.

Faces decide eligibility only: the sleeve cloth of the upper arm, reached through shared
edges from seeds on the cloth (the innermost arm-weighted faces on rays out from the bone at
the window's middle), at least one vertex weighted >= ARM_MIN to Arm + Arm_Roll, and not a
piece lying on top of another island (pad, plate, pouch, strap: LAYER). Inside eligible faces
each texel is judged on its own bind-pose point (barycentric at the texel centre): its
interpolated Arm + Arm_Roll weight >= ARM_MIN, its forearm/hand weight <= FOREARM_MAX, and its
signed distance to the window's two planes (square to the Arm -> ForeArm line) gives an
anti-aliased coverage over AA texels. A texel any other face samples is never painted; painted
values grow GUTTER texels into the gutter (texels no face samples) for mips.

A window is (from, to) in fractions of the upper arm from the shoulder joint to the elbow.
"""
import numpy as np
import bandlib as BL

ARM_MIN = 0.6
FOREARM_MAX = 0.10
AA = 1.5
GUTTER = 2
LAYER = 0.04
SIDES = ['Left', 'Right']


def model_band(base, surface, window):
    names, bp, s, maps, mat, H, W, rasters = BL.prepare(base, surface)
    pos, tri = s['pos'], s['tri']
    nf = len(tri)
    area = 0.5 * np.linalg.norm(np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]]), axis=1)
    island = BL.islands(tri, len(pos))
    cov = np.zeros((H, W), np.float32)
    elig_tex = np.zeros((H, W), bool)
    hole_tex = np.zeros((H, W), bool)
    other_tex = np.zeros((H, W), bool)
    any_tex = np.zeros((H, W), bool)
    eligible = np.zeros(nf, bool)
    excluded = np.zeros(nf, bool)       # arm-weighted faces across the window left out (pieces on top, not connected)
    side_of = np.full(nf, -1)
    band_area = np.zeros(nf)            # each eligible face's area inside the band
    weights = []
    stats = {'band_area': np.zeros(2), 'hole_after': np.zeros(2), 'hole_before': np.zeros(2), 'forearm_area': np.zeros(2)}
    for k, side in enumerate(SIDES):
        a, d, L, u, v = BL.arm_frame(bp, side)
        up_ids = [names.index(side + b) for b in ['Arm', 'Arm_Roll'] if side + b in names]
        fore_ids = [i for i, n in enumerate(names) if n.startswith(side) and any(n[len(side):].startswith(f) for f in BL.FOREARM)]
        uw = np.zeros(len(pos))
        fw = np.zeros(len(pos))
        for j in range(4):
            uw += np.isin(s['bones'][:, j], up_ids) * s['w'][:, j]
            fw += np.isin(s['bones'][:, j], fore_ids) * s['w'][:, j]
        weights.append((uw, fw))
        c = pos[tri].mean(1)
        sa = (c - a) @ d
        r = np.linalg.norm((c - a) - np.outer(sa, d), axis=1)
        h = sa / L
        ok = (uw[tri].max(1) >= ARM_MIN) & (r < 0.2) & (h > window[0] - 0.3) & (h < window[1] + 0.3)
        reached = flood(pos, tri, ok, a, d, L, u, v, window)
        el = reached & ~top_pieces(pos, tri, reached, island, area, a, d)
        vs = (pos - a) @ d / L
        crosses = (vs[tri].max(1) >= window[0] - 0.02) & (vs[tri].min(1) <= window[1] + 0.02)
        excluded |= ok & crosses & ~el & (side_of < 0)
        el &= crosses & (side_of < 0)
        eligible |= el
        side_of[el] = k
        strict = uw[tri].min(1) >= ARM_MIN
        for fi in np.nonzero(el)[0]:
            ra = rasters[fi]
            if ra is None:
                continue
            ys, xs, w = ra
            t = tri[fi]
            p = w @ pos[t]
            ta = area[fi] / max(len(ys), 1)
            texel_world = np.sqrt(ta)
            sd = (p - a) @ d
            inside = np.minimum(sd - window[0] * L, window[1] * L - sd)
            c_ = np.clip(0.5 + inside / (AA * texel_world), 0, 1)
            wt, ft = w @ uw[t], w @ fw[t]
            ok_t = (wt >= ARM_MIN) & (ft <= FOREARM_MAX)
            in_slab = inside >= 0
            band_area[fi] = ta * (in_slab & ok_t).sum()
            stats['band_area'][k] += band_area[fi]
            stats['hole_after'][k] += ta * (in_slab & (wt < ARM_MIN)).sum()
            stats['forearm_area'][k] += ta * (in_slab & (wt >= ARM_MIN) & (ft > FOREARM_MAX)).sum()
            if not strict[fi]:
                stats['hole_before'][k] += ta * in_slab.sum()
            elig_tex[ys, xs] = True
            hole_tex[ys[in_slab & ~ok_t], xs[in_slab & ~ok_t]] = True
            np.maximum.at(cov, (ys, xs), np.where(ok_t, c_, 0).astype(np.float32))
    for fi in range(nf):
        ra = rasters[fi]
        if ra is None:
            continue
        any_tex[ra[0], ra[1]] = True
        if not eligible[fi]:
            other_tex[ra[0], ra[1]] = True
    return dict(base=base, surface=surface, material=mat, diffuse=maps['diffuse'], s=s, bp=bp, names=names,
                window=window, cov=cov, elig_tex=elig_tex, hole_tex=hole_tex, other_tex=other_tex, any_tex=any_tex,
                eligible=eligible, excluded=excluded, side_of=side_of, band_area=band_area, weights=weights,
                stats=stats, area=area)


def texel_in_band(r, k, fi, bary):
    """Is the point `bary` of face fi (side k) painted?"""
    s = r['s']
    t = s['tri'][fi]
    a, d, L, _, _ = BL.arm_frame(r['bp'], SIDES[k])
    h = ((bary @ s['pos'][t]) - a) @ d / L
    uw, fw = r['weights'][k]
    return (r['window'][0] <= h <= r['window'][1]) and bary @ uw[t] >= ARM_MIN and bary @ fw[t] <= FOREARM_MAX


def flood(pos, tri, ok, a, d, L, u, v, window):
    fs = np.nonzero(ok)[0]
    reached = np.zeros(len(tri), bool)
    if not len(fs):
        return reached
    seeds = set()
    for hm in [window[0] + (window[1] - window[0]) * f for f in (0.3, 0.5, 0.7)]:
        o = a + d * hm * L
        for ang in np.linspace(0, 2 * np.pi, 32, endpoint=False):
            t, hit, _ = BL.ray_hits(o, u * np.cos(ang) + v * np.sin(ang), pos, tri[fs])
            if len(t):
                seeds.add(int(fs[hit[np.argmin(t)]]))
    edges = {}
    for fi in fs:
        t = tri[fi]
        for k in range(3):
            edges.setdefault((min(t[k], t[(k + 1) % 3]), max(t[k], t[(k + 1) % 3])), []).append(fi)
    nbr = {fi: [] for fi in fs}
    for fl in edges.values():
        for x in fl:
            nbr[x].extend(y for y in fl if y != x)
    stack = list(seeds)
    for x in stack:
        reached[x] = True
    while stack:
        x = stack.pop()
        for y in nbr[x]:
            if not reached[y]:
                reached[y] = True
                stack.append(y)
    return reached


def top_pieces(pos, tri, sel, island, area, a, d):
    """Islands of `sel` (other than the largest) whose faces mostly lie within LAYER on top of
    another island of it: pads, plates, pouches, straps over the cloth."""
    fs = np.nonzero(sel)[0]
    out = np.zeros(len(tri), bool)
    if not len(fs):
        return out
    on_top = np.zeros(len(tri), bool)
    T = tri[fs]
    for fi in fs:
        c = pos[tri[fi]].mean(0)
        w = c - a
        inward = -(w - d * (w @ d))
        n = np.linalg.norm(inward)
        if n < 1e-6:
            continue
        inward /= n
        t, hit, _ = BL.ray_hits(c + inward * 1e-4, inward, pos, T)
        on_top[fi] = ((t < LAYER) & (island[fs[hit]] != island[fi])).any()
    ids = island[fs]
    areas = {i: area[fs][ids == i].sum() for i in np.unique(ids)}
    main = max(areas, key=areas.get)
    for i in areas:
        if i != main:
            m = fs[ids == i]
            if area[m][on_top[m]].sum() >= 0.5 * area[m].sum():
                out[m] = True
    return out


def material_band(models, window):
    """For every (base, surface) of one material: their bands, and the painted coverage the
    material's diffuse gets (never on a texel another face of any of them samples; grown into
    the unused gutter for mips)."""
    rs = [model_band(b, s, window) for b, s in models]
    cov = np.zeros_like(rs[0]['cov'])
    other = np.zeros_like(rs[0]['other_tex'])
    used = np.zeros_like(rs[0]['any_tex'])
    for r in rs:
        cov = np.maximum(cov, r['cov'])
        other |= r['other_tex']
        used |= r['any_tex']
    painted = np.where(other, 0, cov)
    g = painted.copy()
    for _ in range(GUTTER):
        n = g.copy()
        n[1:] = np.maximum(n[1:], g[:-1])
        n[:-1] = np.maximum(n[:-1], g[1:])
        n[:, 1:] = np.maximum(n[:, 1:], g[:, :-1])
        n[:, :-1] = np.maximum(n[:, :-1], g[:, 1:])
        g = np.where(used, g, n)
    return rs, g, dict(shared_texels=int(((cov > 0) & other).sum()), band_texels=int((cov > 0).sum()))
