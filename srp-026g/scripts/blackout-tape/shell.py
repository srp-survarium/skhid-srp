"""A tape shell: an item surface's own cloth where the tape is wound, lifted a little off it and
drawn with the tape atlas (tapelib). It is that surface's faces (the outermost cloth of the limb,
pieces lying on top of it left out, as texband chose the band's faces), cut at the wrap's height
along the limb, every vertex lifted OFFSET_MM along the cloth's smooth normal and skinned exactly
as the cloth vertex it comes from, so in every pose it lies where the cloth lies. Its UVs are the
atlas cell of its limb and side: the angle round the limb's bone and the distance along it.

The game adds the shell to the character as one more part (exe patch 65: a record with base_model
`ported/blackout_tape/<base_model, '/' as '_'>__<surface>`, part `tape`, its material the team's).
"""
import struct
import numpy as np
import bandlib as BL
import model
import tapelib as T
import texband as TB

OFFSET_MM = 2.5
# A limb: the bone line (from, to), the bones whose weight makes a vertex the limb's, the bones
# past it (the forearm's, the shin's), where along the line the wrap's centre is.
LIMB = {
    'arm': dict(start='Arm', end='ForeArm', own=['Arm', 'Arm_Roll'], past=BL.FOREARM, at=0.65),
    'thigh': dict(start='UpLeg', end='Knee', own=['UpLeg', 'UpLeg_Roll'], past=['Knee', 'Leg', 'Foot', 'Toe'], at=0.45),
}
# The limbs an item of a slot may carry the tape round (a torso item's model may be a whole suit).
SLOT_LIMBS = {'torso': ('arm', 'thigh'), 'gloves': ('arm',), 'legs': ('thigh',)}
OWN_MIN = 0.6
PAST_MAX = 0.10
CUT_MM = T.HALF_MM - 3.0      # the shell is cut this far either side of the wrap's centre


def frame(bp, limb, side):
    """(start joint, unit axis, length, inner direction square to it): the angle round the limb
    starts at the inner side (toward the body's middle line), where the atlas cell's seam hides."""
    L = LIMB[limb]
    a, e = bp[side + L['start']], bp[side + L['end']]
    length = np.linalg.norm(e - a)
    d = (e - a) / length
    mid = np.array([0.0, a[1], a[2]]) if limb == 'thigh' else bp.get('Spin_1', np.array([0.0, a[1], a[2]]))
    inner = (mid - a) - d * ((mid - a) @ d)
    inner /= np.linalg.norm(inner)
    return a, d, length, inner


def limb_faces(s, names, bp, limb, side):
    """The faces of surface s that are the limb's outermost cloth at the wrap (texband's choice:
    the limb's weight, an edge flood from the cloth the rays from the bone meet first, no piece
    lying on top), and the vertices' own and past weights."""
    L = LIMB[limb]
    pos, tri = s['pos'], s['tri']
    a, d, length, inner = frame(bp, limb, side)
    own_ids = [names.index(side + b) for b in L['own'] if side + b in names]
    past_ids = [i for i, n in enumerate(names) if n.startswith(side) and any(n[len(side):].startswith(f) for f in L['past'])]
    ow = np.zeros(len(pos))
    pw = np.zeros(len(pos))
    for j in range(4):
        ow += np.isin(s['bones'][:, j], own_ids) * s['w'][:, j]
        pw += np.isin(s['bones'][:, j], past_ids) * s['w'][:, j]
    c = pos[tri].mean(1)
    sa = (c - a) @ d
    r = np.linalg.norm((c - a) - np.outer(sa, d), axis=1)
    h = sa / length
    ok = (ow[tri].max(1) >= OWN_MIN) & (r < 0.25) & (h > L['at'] - 0.45) & (h < L['at'] + 0.45)
    u = np.cross(d, inner)
    window = (L['at'] - 0.1, L['at'] + 0.1)
    reached = TB.flood(pos, tri, ok, a, d, length, inner, u, window)
    area = 0.5 * np.linalg.norm(np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]]), axis=1)
    island = BL.islands(tri, len(pos))
    el = reached & ~TB.top_pieces(pos, tri, reached, island, area, a, d)
    smm = ((pos - a) @ d - L['at'] * length) * 1000
    st = smm[tri]
    el &= (st.max(1) >= -CUT_MM) & (st.min(1) <= CUT_MM)
    return el, ow, pw


def smooth_normals(pos, nrm):
    """Each vertex's normal averaged over the vertices at its place (a UV seam splits a vertex)."""
    key = np.round(pos * 1e5).astype(np.int64)
    _, inv = np.unique(key, axis=0, return_inverse=True)
    inv = inv.reshape(-1)
    acc = np.zeros((inv.max() + 1, 3))
    np.add.at(acc, inv, nrm)
    out = acc[inv]
    return out / (np.linalg.norm(out, axis=1, keepdims=True) + 1e-12)


def dense_weights(s, nbones):
    w = np.zeros((len(s['pos']), nbones))
    for j in range(4):
        np.add.at(w, (np.arange(len(w)), s['bones'][:, j]), s['w'][:, j])
    return w


def clip_poly(poly, k, lo, hi):
    """Sutherland-Hodgman of a polygon (list of attribute rows) against lo <= row[k] <= hi."""
    for sign, bound in ((1, lo), (-1, hi)):
        out = []
        for i in range(len(poly)):
            p, q = poly[i], poly[(i + 1) % len(poly)]
            pin, qin = sign * (p[k] - bound) >= 0, sign * (q[k] - bound) >= 0
            if pin:
                out.append(p)
            if pin != qin:
                t = (bound - p[k]) / (q[k] - p[k])
                out.append(p + (q - p) * t)
        poly = out
        if not poly:
            break
    return poly


def build(base, surface, limbs):
    """The shell of one item surface round `limbs` (arm, thigh): dict(pos, nrm, tan, bin, uv, tri,
    bones, w, side: each vertex's (limb, side)) or None when the surface has no cloth at the wrap;
    plus the faces chosen per (limb, side)."""
    names, bp = BL.bind_pose(base)
    s = {n: x for n, x, _, _ in model.model(base)}[surface]
    nb = len(names)
    wd = dense_weights(s, nb)
    sn = smooth_normals(s['pos'], s['nrm'])
    verts, tris = [], []
    info = {}
    for limb, side in [(l, sd) for l in limbs for sd in T.SIDES]:
        el, ow, pw = limb_faces(s, names, bp, limb, side)
        a, d, length, inner = frame(bp, limb, side)
        u_ax = np.cross(d, inner)
        info[(limb, side)] = int(el.sum())
        rel = s['pos'] - a
        smm = (rel @ d - LIMB[limb]['at'] * length) * 1000
        radial = rel - np.outer(rel @ d, d)
        theta = np.mod(np.arctan2(radial @ u_ax, radial @ inner), 2 * np.pi)
        lifted = s['pos'] + sn * OFFSET_MM / 1000
        for fi in np.nonzero(el)[0]:
            t = s['tri'][fi]
            th = theta[t].copy()
            if th.max() - th.min() > np.pi:
                th = np.where(th < np.pi, th + 2 * np.pi, th)
            # rows: s, theta, x, y, z, nx, ny, nz, weights...
            rows = [np.concatenate([[smm[i], th[j]], lifted[i], sn[i], wd[i]]) for j, i in enumerate(t)]
            poly = clip_poly(rows, 0, -CUT_MM, CUT_MM)
            if len(poly) < 3:
                continue
            base_i = len(verts)
            for p in poly:
                verts.append((limb, side, p))
            for k in range(1, len(poly) - 1):
                tris.append((base_i, base_i + k, base_i + k + 1))
    if not tris:
        return None, info
    n = len(verts)
    out = dict(pos=np.zeros((n, 3)), nrm=np.zeros((n, 3)), tan=np.zeros((n, 3)), bin=np.zeros((n, 3)),
               uv=np.zeros((n, 2)), bones=np.zeros((n, 4), np.int64), w=np.zeros((n, 4)), side=[])
    for i, (limb, side, p) in enumerate(verts):
        a, d, length, inner = frame(bp, limb, side)
        smm, th, x, nv, wv = p[0], p[1], p[2:5], p[5:8], p[8:]
        nv = nv / (np.linalg.norm(nv) + 1e-12)
        rel = x - a
        radial = rel - d * (rel @ d)
        circ = np.cross(d, radial / (np.linalg.norm(radial) + 1e-12))
        tg = circ - nv * (circ @ nv)
        tg /= np.linalg.norm(tg) + 1e-12
        out['pos'][i], out['nrm'][i], out['tan'][i] = x, nv, tg
        out['bin'][i] = np.cross(nv, tg)
        out['uv'][i] = T.cell_uv(limb, side, th, smm)
        top = np.argsort(-wv, kind='stable')[:4]
        ww = wv[top]
        ww = np.where(ww > 1e-4, ww, 0)
        out['bones'][i] = np.where(ww > 0, top, top[0])
        out['w'][i] = ww / ww.sum()
        out['side'].append((limb, side))
    out['tri'] = np.array(tris, np.int64)
    # Winding as the cloth's: (b - a) x (c - a) along the normal.
    P = out['pos']
    tr = out['tri']
    fn = np.cross(P[tr[:, 1]] - P[tr[:, 0]], P[tr[:, 2]] - P[tr[:, 0]])
    flip = (fn * out['nrm'][tr].sum(1)).sum(1) < 0
    out['tri'][flip] = out['tri'][flip][:, [0, 2, 1]]
    return out, info


def front_sign(s):
    """Which way the stock surface winds a front face (+1: (b-a)x(c-a) along the normal)."""
    P, tr = s['pos'], s['tri']
    fn = np.cross(P[tr[:, 1]] - P[tr[:, 0]], P[tr[:, 2]] - P[tr[:, 0]])
    return np.sign((fn * s['nrm'][tr].sum(1)).sum(1).sum())


def _packed(v, alpha):
    v = v / (np.linalg.norm(v) + 1e-12)
    c = np.clip(np.floor((v + 1) * 127.5), 0, 255).astype(np.uint32)
    return (int(alpha) << 24) | (int(c[0]) << 16) | (int(c[1]) << 8) | int(c[2])


def _uv_fraction(x):
    r = x * 1024.0
    return int(np.clip(np.floor(255.5 * (r - np.clip(np.floor(r), -32768, 32767))), 0, 255))


def encode(sh, winding=1.0):
    """converted_model bytes: 48-byte vertices (position, 4 bones, 3 weights, packed normal,
    tangent and binormal with the UV fractions, UV), then 16-bit indices."""
    n = len(sh['pos'])
    assert 0 < n <= 65535
    out = bytearray(struct.pack('<III', 3, 4 + n * 48, n))
    for i in range(n):
        out += struct.pack('<3f', *sh['pos'][i])
        out += bytes(int(b) for b in sh['bones'][i])
        out += struct.pack('<3f', *sh['w'][i][:3])
        uv = sh['uv'][i]
        out += struct.pack('<III', _packed(sh['nrm'][i], 127), _packed(sh['tan'][i], _uv_fraction(uv[0])),
                           _packed(sh['bin'][i], _uv_fraction(uv[1])))
        out += struct.pack('<2f', *uv)
    tri = sh['tri'] if winding > 0 else sh['tri'][:, [0, 2, 1]]
    idx = tri.reshape(-1)
    out += struct.pack('<III', 4, 4 + len(idx) * 2, len(idx))
    out += struct.pack('<%dH' % len(idx), *idx)
    return bytes(out)
