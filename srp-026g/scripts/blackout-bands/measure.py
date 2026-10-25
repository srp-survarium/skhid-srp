#!/usr/bin/env python3
"""Measure the per-texel band of every sleeve material in bands.tsv at candidate windows (and
at each row's own window): what can be painted, holes, texels shared with other faces, how
much of the ring is seen (cloth) or covered (pads, plates, collars, other parts), how the band
stretches and how far its edges lean from square to the posed upper arm in the review poses
(pose.py first). Writes $BANDS_WORK/measure.json.

usage: measure.py bands.tsv [--jobs N] [--only MATERIAL]
"""
import argparse
import json
import os
import sys
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import bandsenv as E
import bandlib as BL
import model
import texband as TB
from pose import POSES, posed_dir

CENTRES = [0.30, 0.37, 0.44, 0.51, 0.58, 0.65]
WIDTHS = [0.14, 0.20]
ANGLES = 48
REACH = 0.06   # a hit this far beyond the sleeve's radius is the body, not the sleeve


def windows():
    out = []
    for c in CENTRES:
        for w in WIDTHS:
            w0, w1 = round(max(c - w / 2, 0.15), 2), round(min(c + w / 2, 0.80), 2)
            out.append((w0, w1))
    return sorted(set(out))


def read_table(path):
    lines = open(path).read().splitlines()
    head = next(l for l in lines if not l.startswith('#')).split('\t')
    rows = [dict(zip(head, l.split('\t'))) for l in lines if l and not l.startswith('#')][1:]
    return lines, head, rows


def joints(base, pose):
    out = {}
    for line in open(os.path.join(posed_dir(pose), 'models', base + '.skinned_model', 'joints.tsv')):
        n, x, y, z = line.split('\t')
        out[n] = np.array([float(x), float(y), float(z)])
    return out


def posed_surface(base, surface, pose):
    return dict((n, s) for n, s, _, _ in model.model(base, posed_dir(pose)))[surface]


def visibility(r):
    """Per side: rays out from the bone at three heights in the window; the outermost hit near
    the sleeve is painted band (cloth) or something else over it (covered)."""
    parts = [(n, s) for n, s, _, _ in model.model(r['base'])]
    out = []
    for k, side in enumerate(TB.SIDES):
        a, d, L, u, v = BL.arm_frame(r['bp'], side)
        s = r['s']
        c = s['pos'][s['tri']].mean(1)
        el = r['eligible'] & (r['side_of'] == k)
        rad = np.linalg.norm((c - a) - np.outer((c - a) @ d, d), axis=1)
        cloth = hits = 0
        for h in np.linspace(r['window'][0], r['window'][1], 5)[1:-1]:
            near = el & (np.abs((c - a) @ d / L - h) < 0.05)
            ref = np.percentile(rad[near], 30) if near.sum() >= 3 else 0.08
            o = a + d * h * L
            for ang in np.linspace(0, 2 * np.pi, ANGLES, endpoint=False):
                dirn = u * np.cos(ang) + v * np.sin(ang)
                best = None
                for n, ps in parts:
                    t, f, b = BL.ray_hits(o, dirn, ps['pos'], ps['tri'])
                    keep = t < ref + REACH
                    if keep.any():
                        j = np.argmax(np.where(keep, t, -1))
                        if best is None or t[j] > best[0]:
                            best = (t[j], n, f[j], b[j])
                if best is None:
                    continue
                hits += 1
                _, n, fi, b = best
                if n == r['surface'] and r['eligible'][fi] and r['side_of'][fi] == k and TB.texel_in_band(r, k, fi, b):
                    cloth += 1
        out.append(cloth / max(hits, 1))
    return out


def cut_points(r, k, w):
    """(face, bary) where side k's band crosses the plane at w, with arm weight there."""
    s = r['s']
    a, d, L, _, _ = BL.arm_frame(r['bp'], TB.SIDES[k])
    uw, fw = r['weights'][k]
    pts = []
    for fi in np.nonzero(r['eligible'] & (r['side_of'] == k))[0]:
        t = s['tri'][fi]
        h = (s['pos'][t] - a) @ d / L - w
        for i in range(3):
            j = (i + 1) % 3
            if (h[i] >= 0) != (h[j] >= 0):
                f = h[i] / (h[i] - h[j])
                b = np.zeros(3)
                b[i], b[j] = 1 - f, f
                if b @ uw[t] >= TB.ARM_MIN and b @ fw[t] <= TB.FOREARM_MAX:
                    pts.append((fi, b))
    return pts


def plane_angle(P, axis):
    if len(P) < 3:
        return float('nan')
    _, _, vt = np.linalg.svd(P - P.mean(0), full_matrices=False)
    return float(np.degrees(np.arccos(min(1.0, abs(vt[2] @ axis)))))


def posed_measures(r):
    """Per pose: stretch (band-area weighted posed/bind face area: mean, p95; worst side) and
    lean (worst edge angle to the posed upper arm)."""
    out = {}
    cuts = {(k, w): cut_points(r, k, w) for k in [0, 1] for w in r['window']}
    for pose in POSES:
        try:
            s = posed_surface(r['base'], r['surface'], pose)
            jt = joints(r['base'], pose)
        except (OSError, KeyError):
            continue
        p, t = s['pos'], s['tri']
        pa = 0.5 * np.linalg.norm(np.cross(p[t[:, 1]] - p[t[:, 0]], p[t[:, 2]] - p[t[:, 0]]), axis=1)
        ratio = pa / np.maximum(r['area'], 1e-12)
        means, p95s, leans = [], [], []
        for k, side in enumerate(TB.SIDES):
            fs = np.nonzero(r['side_of'] == k)[0]
            w = r['band_area'][fs]
            if w.sum() <= 0:
                continue
            rr = ratio[fs]
            order = np.argsort(rr)
            cw = np.cumsum(w[order]) / w.sum()
            means.append(float((rr * w).sum() / w.sum()))
            p95s.append(float(rr[order][min(np.searchsorted(cw, 0.95), len(rr) - 1)]))
            axis = jt[side + 'ForeArm'] - jt[side + 'Arm']
            axis /= np.linalg.norm(axis)
            for wv in r['window']:
                P = np.array([b @ p[t[fi]] for fi, b in cuts[(k, wv)]]).reshape(-1, 3)
                leans.append(plane_angle(P, axis))
        leans = [x for x in leans if not np.isnan(x)]
        if means:
            out[pose] = dict(stretch_mean=max(means), stretch_p95=max(p95s), lean=max(leans) if leans else float('nan'))
    return out


def measure_window(models, window):
    rs, g, info = TB.material_band(models, window)
    res = dict(window=window, shared_texels=info['shared_texels'], models={})
    for r in rs:
        st = r['stats']
        want = st['band_area'] + st['hole_after'] + st['forearm_area']
        res['models'][r['base']] = dict(
            paintable=float(np.min(st['band_area'] / np.maximum(want, 1e-12))),
            band_cm2=float(st['band_area'].sum() * 1e4), hole_before_cm2=float(st['hole_before'].sum() * 1e4),
            hole_after_cm2=float(st['hole_after'].sum() * 1e4), cloth=visibility(r), poses=posed_measures(r))
    return res


def material_task(args):
    material, models, extra = args
    wins = windows()
    for w in extra:
        if w not in wins:
            wins.append(w)
    out = {'material': material, 'models': [b for b, _ in models], 'windows': []}
    for w in wins:
        try:
            out['windows'].append(measure_window(models, w))
        except Exception as e:  # a window with no band on a model
            out['windows'].append(dict(window=w, error=str(e)))
    BL._PREP.clear()
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('table')
    ap.add_argument('--jobs', type=int, default=8)
    ap.add_argument('--only', action='append', default=[])
    a = ap.parse_args()
    _, _, rows = read_table(a.table)
    by_mat, extra = {}, {}
    for r in rows:
        by_mat.setdefault(r['material'], []).append((r['base_model'], r['surface']))
        extra.setdefault(r['material'], set()).add((float(r['window_from']), float(r['window_to'])))
    todo = [(m, v, sorted(extra[m])) for m, v in sorted(by_mat.items()) if not a.only or m in a.only]
    path = E.work('measure.json')
    done = json.load(open(path)) if os.path.exists(path) else {}
    with Pool(a.jobs) as p:
        for res in p.imap_unordered(material_task, todo):
            done[res['material']] = res
            json.dump(done, open(path, 'w'))
            print('measured', res['material'], flush=True)
    print(path)


if __name__ == '__main__':
    main()
