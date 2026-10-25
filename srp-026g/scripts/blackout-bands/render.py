"""A small CPU rasteriser for review renders (no GPU): skinned models in the bind pose or posed
(the poser's converted_models), their diffuse and normal maps, a key and fill light."""
import io
import numpy as np
from PIL import Image
import gameview as gv
import model

LIGHTS = {
    # ambient, key direction (toward the light), key colour, fill
    'neutral': (np.array([0.32, 0.32, 0.33]), np.array([0.45, 0.55, 0.70]), np.array([0.95, 0.93, 0.90]), 0.18),
    'dark': (np.array([0.02, 0.023, 0.03]), np.array([-0.3, 0.8, 0.5]), np.array([0.09, 0.09, 0.1]), 0.02),
}
VIEWS = {
    # right, up, forward (toward the camera), frame (x0, x1, y0, y1) in the right/up axes
    'front': (np.array([-1.0, 0, 0]), np.array([0, 1.0, 0]), np.array([0, 0, 1.0]), (-0.55, 0.55, 0.95, 1.75)),
    'back': (np.array([1.0, 0, 0]), np.array([0, 1.0, 0]), np.array([0, 0, -1.0]), (-0.55, 0.55, 0.95, 1.75)),
    'side': (np.array([0, 0, -1.0]), np.array([0, 1.0, 0]), np.array([-1.0, 0, 0]), (-0.35, 0.35, 0.95, 1.75)),
    'right': (np.array([0, 0, 1.0]), np.array([0, 1.0, 0]), np.array([1.0, 0, 0]), (-0.35, 0.35, 0.95, 1.75)),
}
_tex = {}


def srgb_to_linear(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(c):
    c = np.clip(c, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def texture(key, loader):
    if key not in _tex:
        try:
            _tex[key] = np.asarray(Image.open(io.BytesIO(loader())).convert('RGBA')).astype(np.float32)
        except Exception:
            _tex[key] = None
    return _tex[key]


def game_texture(name):
    return texture(('game', name), lambda: gv.read('textures/%s.dds' % name)) if name else None


def override(name, rgba):
    """Draw texture `name` as `rgba` (float [h, w, 4] 0..255) until cleared with None."""
    if rgba is None:
        _tex.pop(('game', name), None)
    else:
        _tex[('game', name)] = rgba.astype(np.float32)


def sample(tex, uv):
    h, w = tex.shape[:2]
    x = (uv[:, 0] % 1.0) * w
    y = (uv[:, 1] % 1.0) * h
    return tex[np.clip(y.astype(int), 0, h - 1), np.clip(x.astype(int), 0, w - 1)]


def render(bases, view='front', light='neutral', size=(440, 320), ss=2, frame=None, geometry=None,
           fit='all', face_rgb=None):
    """RGB image of the models `bases` (a base_model or a list; posed from `geometry` when given).
    face_rgb: {(base, surface): [n_faces, 3] sRGB or NaN rows}: those faces drawn flat."""
    if isinstance(bases, str):
        bases = [bases]
    right, up, fwd, fr = VIEWS[view]
    parts_geo = [(b, n, s, maps) for b in bases for n, s, maps, _ in model.model(b, geometry)]
    if frame is not None:
        fr = frame
    elif fit is not None:
        P = fit if isinstance(fit, np.ndarray) else np.concatenate([s['pos'] for _, _, s, _ in parts_geo])
        X, Y = P @ right, P @ up
        cx, cy = (X.min() + X.max()) / 2, (Y.min() + Y.max()) / 2
        half = max((X.max() - X.min()) / 2, (Y.max() - Y.min()) / 2 * size[0] / size[1]) * (1.15 if isinstance(fit, str) else 1.8)
        fr = (cx - half, cx + half, cy - half * size[1] / size[0], cy + half * size[1] / size[0])
    W, H = size[0] * ss, size[1] * ss
    sx, sy = W / (fr[1] - fr[0]), H / (fr[3] - fr[2])
    zbuf = np.full((H, W), -np.inf)
    out = np.zeros((H, W, 3))
    amb, key, keyc, fill = LIGHTS[light]
    key = key / np.linalg.norm(key)
    if view == 'back':
        key = key * np.array([1, 1, -1])
    for b, name, s, maps in parts_geo:
        dif, nrm = game_texture(maps.get('diffuse')), game_texture(maps.get('normal'))
        frgb = (face_rgb or {}).get((b, name))
        P = s['pos']
        X = (P @ right - fr[0]) * sx
        Y = (fr[3] - P @ up) * sy
        Z = P @ fwd
        for ti, t in enumerate(s['tri']):
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
            n = w @ s['nrm'][t]
            n /= np.linalg.norm(n, axis=1, keepdims=True) + 1e-12
            if nrm is not None:
                tg, bn = w @ s['tan'][t], w @ s['bin'][t]
                m = sample(nrm, uv) / 255.0
                nx, ny = m[:, 3] * 2 - 1, m[:, 1] * 2 - 1
                nz = np.sqrt(np.clip(1 - nx * nx - ny * ny, 0, 1))
                pert = tg * nx[:, None] + bn * ny[:, None] + n * nz[:, None]
                ln = np.linalg.norm(pert, axis=1, keepdims=True)
                n = np.where(ln > 1e-6, pert / (ln + 1e-12), n)
            if frgb is not None and not np.isnan(frgb[ti][0]):
                alb = np.tile(srgb_to_linear(np.asarray(frgb[ti], float)), (len(uv), 1))
            else:
                alb = srgb_to_linear(sample(dif, uv)[:, :3]) if dif is not None else np.full((len(uv), 3), 0.2)
            col = alb * (amb + np.clip(n @ key, 0, None)[:, None] * keyc + np.clip(-(n @ key), 0, None)[:, None] * fill)
            sub[win] = z[win]
            out[y0:y1 + 1, x0:x1 + 1][win] = col
    bg = np.isinf(zbuf)
    out[bg] = [0.18, 0.19, 0.2] if light == 'neutral' else [0.012, 0.013, 0.015]
    img = linear_to_srgb(out).reshape(H // ss, ss, W // ss, ss, 3).mean((1, 3))
    return Image.fromarray((img * 255).round().astype(np.uint8)), fr
