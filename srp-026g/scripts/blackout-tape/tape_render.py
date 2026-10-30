"""CPU review renders of an item with its tape shell (render.py's rasteriser, with the shell's
alpha test and a little sheen on the tape). Normal maps are read as the game's (X in alpha,
Y in green with Y = -n.y against the binormal: measured on the stock maps, see tapelib)."""
import numpy as np
from PIL import Image
import render as RD

ALPHA_REF = 0.5


def _sample_bilinear(tex, uv):
    h, w = tex.shape[:2]
    x = np.clip(uv[:, 0] * w - 0.5, 0, w - 1.001)
    y = np.clip(uv[:, 1] * h - 0.5, 0, h - 1.001)
    x0, y0 = x.astype(int), y.astype(int)
    fx, fy = (x - x0)[:, None], (y - y0)[:, None]
    return ((tex[y0, x0] * (1 - fx) + tex[y0, x0 + 1] * fx) * (1 - fy)
            + (tex[y0 + 1, x0] * (1 - fx) + tex[y0 + 1, x0 + 1] * fx) * fy)


def render(parts, view='front', light='neutral', size=(220, 300), ss=2, frame=None):
    """parts: [dict(s=surface dict, dif=RGBA float 0..255 or None, nrm=RGBA or None,
    alpha_test=bool, sheen=float)]. Returns (PIL image, frame)."""
    right, up, fwd, fr = RD.VIEWS[view]
    if frame is not None:
        fr = frame
    else:
        P = np.concatenate([p['s']['pos'] for p in parts if not p.get('alpha_test')] or [parts[0]['s']['pos']])
        X, Y = P @ right, P @ up
        cx, cy = (X.min() + X.max()) / 2, (Y.min() + Y.max()) / 2
        half = max((X.max() - X.min()) / 2, (Y.max() - Y.min()) / 2 * size[0] / size[1]) * 1.12
        fr = (cx - half, cx + half, cy - half * size[1] / size[0], cy + half * size[1] / size[0])
    W, H = size[0] * ss, size[1] * ss
    sx, sy = W / (fr[1] - fr[0]), H / (fr[3] - fr[2])
    zbuf = np.full((H, W), -np.inf)
    out = np.zeros((H, W, 3))
    amb, key, keyc, fill = RD.LIGHTS[light]
    key = key / np.linalg.norm(key)
    if view == 'back':
        key = key * np.array([1, 1, -1])
    half_v = key + fwd
    half_v = half_v / np.linalg.norm(half_v)
    # Draw the alpha-tested shells last, so a shell's hole shows what is under it.
    for p in sorted(parts, key=lambda p: bool(p.get('alpha_test'))):
        s, dif, nrm = p['s'], p.get('dif'), p.get('nrm')
        P = s['pos']
        X = (P @ right - fr[0]) * sx
        Y = (fr[3] - P @ up) * sy
        Z = P @ fwd
        for t in s['tri']:
            xs, ys = X[t], Y[t]
            x0, x1 = int(max(np.floor(xs.min()), 0)), int(min(np.ceil(xs.max()), W - 1))
            y0, y1 = int(max(np.floor(ys.min()), 0)), int(min(np.ceil(ys.max()), H - 1))
            if x1 < x0 or y1 < y0:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            d = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
            if abs(d) < 1e-12:
                continue
            l0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / d
            l1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / d
            l2 = 1 - l0 - l1
            inside = (l0 >= 0) & (l1 >= 0) & (l2 >= 0)
            if not inside.any():
                continue
            z = l0 * Z[t[0]] + l1 * Z[t[1]] + l2 * Z[t[2]]
            sub = zbuf[y0:y1 + 1, x0:x1 + 1]
            win = inside & (z > sub)
            if not win.any():
                continue
            w = np.stack([l0[win], l1[win], l2[win]], 1)
            uv = w @ s['uv'][t]
            if dif is not None:
                texel = _sample_bilinear(dif, uv) if p.get('alpha_test') else RD.sample(dif, uv)
            else:
                texel = np.full((len(uv), 4), 50.0)
            keep = np.ones(len(uv), bool)
            if p.get('alpha_test'):
                keep = texel[:, 3] / 255.0 >= ALPHA_REF
                if not keep.any():
                    continue
            n = w @ s['nrm'][t]
            n /= np.linalg.norm(n, axis=1, keepdims=True) + 1e-12
            if nrm is not None:
                tg, bn = w @ s['tan'][t], w @ s['bin'][t]
                tg = tg - n * (tg * n).sum(1, keepdims=True)
                tg /= np.linalg.norm(tg, axis=1, keepdims=True) + 1e-12
                bn = np.cross(n, tg)
                m = (_sample_bilinear(nrm, uv) if p.get('alpha_test') else RD.sample(nrm, uv)) / 255.0
                nx, ny = m[:, 3] * 2 - 1, -(m[:, 1] * 2 - 1)
                nz = np.sqrt(np.clip(1 - nx * nx - ny * ny, 0, 1))
                pert = tg * nx[:, None] + bn * ny[:, None] + n * nz[:, None]
                ln = np.linalg.norm(pert, axis=1, keepdims=True)
                n = np.where(ln > 1e-6, pert / (ln + 1e-12), n)
            alb = RD.srgb_to_linear(texel[:, :3])
            col = alb * (amb + np.clip(n @ key, 0, None)[:, None] * keyc + np.clip(-(n @ key), 0, None)[:, None] * fill)
            sheen = p.get('sheen', 0.0)
            if sheen:
                col = col + sheen * (np.clip(n @ half_v, 0, 1) ** 24)[:, None] * keyc
            ys_, xs_ = np.nonzero(win)
            ys_, xs_ = ys_[keep], xs_[keep]
            sub[ys_, xs_] = z[win][keep]
            out[y0:y1 + 1, x0:x1 + 1][ys_, xs_] = col[keep]
    bg = np.isinf(zbuf)
    out[bg] = [0.18, 0.19, 0.2] if light == 'neutral' else [0.012, 0.013, 0.015]
    img = RD.linear_to_srgb(out).reshape(H // ss, ss, W // ss, ss, 3).mean((1, 3))
    return Image.fromarray((img * 255).round().astype(np.uint8)), fr
