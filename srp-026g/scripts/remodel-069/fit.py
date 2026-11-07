#!/usr/bin/env python3
"""Where each remodel lies against its live model: sidecar.py's OFFSET.

usage: python3 -I fit.py <players' files> <remodel bundle overlay> [<0.69d0 item>...]

<players' files>          the live weapon models (sight-alignment's `align.py extract` folder)
<remodel bundle overlay>  make_pack.sh's bundle/overlay, made with no offsets
Prints, per gun, the shift (mm, mesh space) that lays the remodel's bind mesh on the live one (a
trimmed nearest-point fit of 6000 vertices each, run until it settles) and the median gap before and
after. 0.69d0 placed most guns as we do (gap 0); a gun it placed otherwise shows its shift and a gap
near 0 after it (the MP-153); a gap left after the shift is a shape of its own (the Uzi's sights).
"""
import struct
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import model  # noqa: E402

SAMPLE = 6000
KEPT = 0.6  # the nearest share of the points the shift follows: parts one gun has and the other not are left out


def bind_points(directory, rng):
    """A model's LOD0 vertex positions as written (bind pose, mesh space), a sample of them."""
    out = []
    for name in model.config(directory / 'settings')['lod_hierrarchy']['LOD0']['surfaces']:
        data = (directory / 'render' / name / 'converted_model').read_bytes()
        _, size, n = struct.unpack_from('<3I', data)
        stride = (size - 4) // n
        rows = np.frombuffer(data, np.uint8, n * stride, 12).reshape(n, stride)
        out.append(rows[:, :12].copy().view('<f4').reshape(n, 3).astype(float))
    p = np.concatenate(out)
    return p[rng.choice(len(p), min(len(p), SAMPLE), replace=False)]


def nearest(a, b):
    out = np.empty(len(a), int)
    for i in range(0, len(a), 500):
        out[i:i + 500] = ((a[i:i + 500, None, :] - b[None]) ** 2).sum(-1).argmin(1)
    return out


def gap(p, q):
    return float(np.median(np.linalg.norm(p[nearest(q, p)] - q, axis=1)))


def shift(p, q):
    t = np.zeros(3)
    for _ in range(80):
        d = p[nearest(q + t, p)] - (q + t)
        r = np.linalg.norm(d, axis=1)
        step = np.median(d[r <= np.quantile(r, KEPT)], axis=0)
        t += step
        if np.linalg.norm(step) < 2e-5:
            break
    return t


def main():
    live, overlay = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    rows = [line.split('\t') for line in (HERE / 'items.tsv').read_text().splitlines()
            if line and not line.startswith('#')]
    seen = set()
    for _, _, live_model, w, remodel in rows:
        if w in seen or (only and w not in only):
            continue
        seen.add(w)
        rng = np.random.default_rng(1)
        p = bind_points(model.model_dir(live, live_model), rng)
        q = bind_points(overlay / 'models' / (remodel + '.skinned_model'), rng)
        t = shift(p, q)
        print(f'{w}\t{np.round(t * 1000, 1).tolist()}\tgap {gap(p, q) * 1000:.1f} -> {gap(p, q + t) * 1000:.1f} mm')


if __name__ == '__main__':
    main()
