"""render_icon.py: the shop icons of the lobby's consumables atlas, and the views of the review sheet
(review.py), drawn with 0.26g's materials as its renderer reads them.

**The icon** (icon()): 0.69d0's own picture of the optic when it ships one (prepare_donor.sh: its
module's `ui_desc.icon_renderer` names flash_movies/textures/ui_icons/modules/module_renderer_<n>.png),
else the model rendered (render()); either fitted to the stock scope icons' frame, measured on
LobbyMenu_AmmoIcons column 4, rows 1 to 12 (171x83 cells):
- the object, its glow included, spans 73 px of height from y 8 (all twelve are 72 or 73 px tall)
  unless 125 px of width limits it (the TA31's), centred on x 88 (FRAME, CENTRE_X);
- a soft black glow around it (their pixels within 6 px of the object: black, alpha about 10);
- seen from the front left and above, the objective nearest, turned to rise to the right (VIEW), as
  0.69d0's own pictures are;
- the alpha anti-aliased (coverage of SCALE x SCALE samples a pixel).
0.69d0's picture of the CompM4 fitted so is the stock icon's angle and size.

**The shading** (0.26g's g-buffer and light passes, gbuffer_pass.ps and point_light_accumulator.ps,
disassembled from the shipped shaders): the diffuse texture is sRGB (its .dds's last byte 1), the
light linear; the normal map's alpha is x and its green y, flipped (`n = (2a - 1, 1 - 2g, z)`) in
the vertices' tangent frame (scopes_lib.basis); the roughness slot (`_smap`) is a gloss g, its red,
`s = 1 - (min + (max - min) g)`, the GGX width `0.5 s^6 + 0.1 s`; the fresnel slot (`_fmap`) is F0,
its red, between the material's min and max; Schlick's Fresnel takes the specular and leaves the
diffuse `NL (1 - F)`. Lit by LIGHTS, a sky and a soft box the glossy parts reflect, set so that
the stock CompM4's model and textures come out as bright as its stock icon (mean sRGB 58 and 55).
Needs Pillow and numpy.
"""
import struct
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

import scopes_lib as lib

CELL = (171, 83)
SCALE = 4
FRAME = (27, 9, 150, 80)  # x0, y0, x1, y1 of our object, so that with its glow it spans the stock ones (26, 8, 151, 81)
CENTRE_X = 88
GLOW = (2.5, 0.28)  # the glow's blur radius (px) and strength
VIEW = (-40.0, 25.0, 20.0)  # yaw, pitch, roll (degrees): the objective to the front left, from above
TEXTURE_SIZE = 512
# (towards the light in view space, linear colour): key from the upper left front, a cool rim from
# behind on the right, a dim fill from below
LIGHTS = [((-0.55, 0.65, 0.55), (3.0, 2.9, 2.75)),
          ((0.75, 0.35, -0.55), (1.5, 1.9, 2.6)),
          ((0.35, -0.6, 0.7), (0.2, 0.2, 0.22))]
SKY = ((0.08, 0.09, 0.1), (0.01, 0.01, 0.012))  # linear radiance above, below (the ambient light)
# what glossy parts reflect besides: a soft box (direction towards it, radiance, angular radius)
SOFTBOX = ((-0.3, 0.6, 0.75), 10.0, 0.4)
LENS = {'albedo': (0.01, 0.012, 0.015), 'gloss': 0.95, 'f0': 0.06}


def to_linear(c):
    c = np.asarray(c, dtype=np.float64)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def to_srgb(c):
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def mesh(data):
    """(positions, uv, triangles, (normal, tangent, bitangent)) of a converted_model: a static
    mesh's 28-byte vertices (0.69d0's made 0.26g's, scopes_lib.static_026) or a skinned one's 36-byte
    ones (position, bone, the three basis words, float2 uv)."""
    kind, size, count = struct.unpack_from('<3I', data, 0)
    stride = (size - 4) // count
    if stride == 28:
        data = lib.static_026(data)
    raw = np.frombuffer(data, dtype=np.uint8, count=stride * count, offset=12).reshape(count, stride)
    p = raw[:, 0:12].copy().view('<f4').reshape(count, 3).astype(np.float64)
    if stride == 28:
        uv = raw[:, 24:28].copy().view('<f2').reshape(count, 2).astype(np.float64)
        frame = lib.basis(raw)
    else:
        uv = raw[:, 28:36].copy().view('<f4').reshape(count, 2).astype(np.float64)
        frame = lib.basis(np.concatenate([raw[:, 0:12], raw[:, 16:28]], axis=1))
    at = 12 + stride * count
    kind, size, n = struct.unpack_from('<3I', data, at)
    ix = np.frombuffer(data, dtype='<u2', count=n, offset=at + 12).reshape(-1, 3)
    return p, uv, ix, frame


def find(roots, path):
    for root in roots:
        p = Path(root) / path
        if p.exists():
            return p
    return None


_textures = {}


def texture(roots, name):
    """A texture (textures/<name>.dds) as floats in [0, 1], RGBA, at most TEXTURE_SIZE wide; None
    when no root has it."""
    t = find(roots, f'textures/{name}.dds') if name else None
    if t is None:
        return None
    if t not in _textures:
        im = Image.open(t).convert('RGBA')
        if im.width > TEXTURE_SIZE:
            im = im.resize((TEXTURE_SIZE, TEXTURE_SIZE * im.height // im.width), Image.Resampling.BOX)
        _textures[t] = np.asarray(im, dtype=np.float64) / 255.0
    return _textures[t]


class Material:
    """What 0.26g's g-buffer pass takes of a material: albedo, normal map, gloss and F0 maps (absent
    where the material has none: its constant then) and their ranges."""

    def __init__(self, albedo=(0.5, 0.5, 0.5), gloss=0.2, f0=0.04, maps=None, ranges=((0.0, 1.0), (0.0, 1.0))):
        self.albedo, self.gloss, self.f0 = np.array(albedo, dtype=np.float64), gloss, f0
        self.maps = maps or {}
        self.ranges = ranges

    @classmethod
    def of(cls, roots, name):
        """The material `material_instances/<name>.material` of the roots; a lens (no g-buffer
        stage) as dark glass; a material or texture that is nowhere is an error."""
        path = find(roots, f'material_instances/{name}.material')
        if path is None:
            raise FileNotFoundError(f'material {name}: in none of {[str(r) for r in roots]}')
        d = lib.leaves(path)
        e = '/material/g_stage/effect/'
        if e + 'effect_id' not in d:
            return cls(**LENS)

        def value(key, default):
            return d.get(f'{e}{key}/value', default)
        maps = {}
        for slot, switch in (('texture_diffuse', 'use_tdiffuse'), ('texture_normal', 'use_nmap'),
                             ('texture_roughness', 'use_troughness'), ('texture_fresnel', 'use_tfresnel')):
            if value(switch, False):
                found = texture(roots, value(slot, ''))
                if found is None:
                    raise FileNotFoundError(f'material {name}: {slot} {value(slot, "")!r} is in none of the roots')
                maps[slot] = found
        colour = value('constant_diffuse', [1.0, 1.0, 1.0, 1.0])[:3]
        return cls(albedo=to_linear(colour), gloss=float(value('constant_roughness', 0.0)),
                   f0=float(value('constant_fresnel', 0.0)), maps=maps,
                   ranges=((float(value('constant_roughness_min', 0.0)), float(value('constant_roughness_max', 1.0))),
                           (float(value('constant_fresnel_min', 0.0)), float(value('constant_fresnel_max', 1.0)))))

    def sample(self, uv):
        """(albedo linear (n, 3), tangent-space normal (n, 3) or None, s, f0) at texture coordinates."""
        def at(t):
            h, w = t.shape[:2]
            x = (np.mod(uv[:, 0], 1.0) * w).astype(int).clip(0, w - 1)
            y = (np.mod(uv[:, 1], 1.0) * h).astype(int).clip(0, h - 1)
            return t[y, x]
        n = len(uv)
        albedo = np.tile(self.albedo, (n, 1))
        if 'texture_diffuse' in self.maps:
            albedo = albedo * to_linear(at(self.maps['texture_diffuse'])[:, :3])
        normal = None
        if 'texture_normal' in self.maps:
            t = at(self.maps['texture_normal'])
            x, y = t[:, 3] * 2 - 1, 1 - t[:, 1] * 2
            normal = np.stack([x, y, np.sqrt(np.clip(1 - x * x - y * y, 0, 1))], axis=1)
        (rlo, rhi), (flo, fhi) = self.ranges
        rough = rlo + (rhi - rlo) * at(self.maps['texture_roughness'])[:, 0] if 'texture_roughness' in self.maps \
            else np.full(n, self.gloss)
        f0 = flo + (fhi - flo) * at(self.maps['texture_fresnel'])[:, 0] if 'texture_fresnel' in self.maps \
            else np.full(n, self.f0)
        return albedo, normal, np.clip(1.0 - rough, 0, 1), np.clip(f0, 0, 1)


class Surface:
    """One mesh of a model to draw: positions (moved, turned), uv, triangles, tangent frame, material."""

    def __init__(self, p, uv, ix, frame, material):
        self.p, self.uv, self.ix, self.frame, self.material = p, uv, ix, frame, material


def parts(model, roots, offset=(0, 0, 0), flip=False, skip=(), renames=None, materials=None):
    """The Surfaces of a model directory: moved by `offset` and, with `flip`, turned 180 degrees about
    Y first (a scope model as the weapon draws it at scope_point). `renames`: the material a donor
    material name is drawn with (make_pack.py's material_renames); `materials`: a function of a
    material name giving a Material to use instead, or None (review.py's flat colours)."""
    settings = lib.leaves(Path(model) / 'settings')
    out = []
    turned = np.array([-1.0, 1.0, -1.0]) if flip else np.ones(3)
    for s in lib.surfaces(Path(model)):
        if any(s.startswith(k) for k in skip):
            continue
        p, uv, ix, frame = mesh((Path(model) / 'render' / s / 'converted_model').read_bytes())
        name = settings.get(f'/material_settings/{s}/material_name', '')
        name = (renames or {}).get(name, name)
        material = (materials and materials(name)) or Material.of(roots, name)
        out.append(Surface(p * turned + np.array(offset), uv, ix, tuple(v * turned for v in frame), material))
    return out


def turn(yaw, pitch, roll=0.0):
    def rz(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])

    def ry(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])

    def rx(a):
        c, s = np.cos(a), np.sin(a)
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    return rz(np.radians(roll)) @ rx(np.radians(pitch)) @ ry(np.radians(yaw))


def raster(views, size, project):
    """The g-buffer of view-space Surfaces: per pixel the nearest surface's index (-1 none), uv and
    tangent frame. project: points -> (x, y, depth), depth larger nearer."""
    w, h = size
    index = np.full((h, w), -1)
    depth = np.full((h, w), -np.inf)
    uv = np.zeros((h, w, 2))
    frame = np.zeros((h, w, 3, 3))
    for k, s in enumerate(views):
        sx, sy, z = project(s.p)
        n, t, b = s.frame
        for a, bb, c in s.ix:
            tri = [a, bb, c]
            xs, ys = sx[tri], sy[tri]
            if not (np.isfinite(xs).all() and np.isfinite(ys).all()):
                continue
            x0, x1 = int(max(0, np.floor(xs.min()))), int(min(w - 1, np.ceil(xs.max())))
            y0, y1 = int(max(0, np.floor(ys.min()))), int(min(h - 1, np.ceil(ys.max())))
            if x1 < x0 or y1 < y0 or (x1 - x0) * (y1 - y0) > w * h:
                continue
            d = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
            if abs(d) < 1e-12:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            l0 = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / d
            l1 = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / d
            l2 = 1 - l0 - l1
            inside = (l0 >= -1e-9) & (l1 >= -1e-9) & (l2 >= -1e-9)
            if not inside.any():
                continue
            zz = l0 * z[a] + l1 * z[bb] + l2 * z[c]
            region = depth[y0:y1 + 1, x0:x1 + 1]
            nearer = inside & (zz > region)
            if not nearer.any():
                continue
            w3 = np.stack([l0[nearer], l1[nearer], l2[nearer]], axis=1)
            region[nearer] = zz[nearer]
            index[y0:y1 + 1, x0:x1 + 1][nearer] = k
            uv[y0:y1 + 1, x0:x1 + 1][nearer] = w3 @ s.uv[tri]
            frame[y0:y1 + 1, x0:x1 + 1][nearer] = np.stack([w3 @ n[tri], w3 @ t[tri], w3 @ b[tri]], axis=1)
    return index, uv, frame


def unit(v):
    return v / np.maximum(np.linalg.norm(v, axis=-1, keepdims=True), 1e-9)


def shade(views, index, uv, frame, eye):
    """Linear colour of each pixel of a g-buffer (black where nothing is); `eye`: the direction
    towards the camera per pixel (h, w, 3)."""
    h, w = index.shape
    colour = np.zeros((h, w, 3))
    lights = [(unit(np.array(d, dtype=np.float64)), np.array(c)) for d, c in LIGHTS]
    above, below = (np.array(c) for c in SKY)
    for k, s in enumerate(views):
        mask = index == k
        if not mask.any():
            continue
        albedo, tnormal, smooth, f0 = s.material.sample(uv[mask])
        f = frame[mask]
        nv, tv, bv = unit(f[:, 0]), unit(f[:, 1]), unit(f[:, 2])
        v = eye[mask]
        back = (nv * v).sum(1) < 0
        nv[back], tv[back], bv[back] = -nv[back], -tv[back], -bv[back]
        n = nv if tnormal is None else unit(tnormal[:, 0:1] * tv + tnormal[:, 1:2] * bv + tnormal[:, 2:3] * nv)
        nn = np.clip((n * v).sum(1), 1e-4, 1)
        width = 0.5 * smooth ** 6 + 0.1 * smooth
        alpha2 = width ** 2
        kk = 0.5 * smooth
        out = np.zeros((mask.sum(), 3))
        for d, c in lights:
            nl = np.clip(n @ d, 0, 1)
            hv = unit(d + v)
            nh = np.clip((n * hv).sum(1), 0, 1)
            vh = np.clip((v * hv).sum(1), 0, 1)
            fd = f0 + (1 - f0) * (1 - nl) ** 5
            fs = f0 + (1 - f0) * (1 - vh) ** 5
            dist = np.minimum(alpha2 / (np.pi * (nh * nh * (alpha2 - 1) + 1) ** 2), 1000.0)
            g = (nl / np.maximum(nl * (1 - kk) + kk, 1e-6)) * (nn / np.maximum(nn * (1 - kk) + kk, 1e-6))
            spec = dist * fs * np.minimum(g / np.maximum(4 * nl * nn, 1e-6), 1.0) * nl
            out += (albedo * (nl * (1 - fd))[:, None] + spec[:, None]) * c
        sky = below + (above - below) * (n[:, 1:2] * 0.5 + 0.5)
        r = 2 * nn[:, None] * n - v
        seen = below + (above - below) * (np.clip(r[:, 1:2], -1, 1) * 0.5 + 0.5)
        box, radiance, radius = SOFTBOX
        spread = radius + width
        cos = np.clip(r @ unit(np.array(box, dtype=np.float64)), -1, 1)
        seen = seen + (radiance * (radius / spread) ** 2 * np.exp(-(np.arccos(cos) / spread) ** 2))[:, None]
        fr = (f0 + (1 - f0) * (1 - nn) ** 5)[:, None]
        out += albedo * sky * (1 - fr) + seen * fr
        colour[mask] = out
    return colour


def render(items, size, yaw, pitch, roll=0.0, frame=None, background=None):
    """Surfaces seen orthographically from (yaw, pitch) degrees, turned by `roll` in the picture, fitted into `frame` (x0, y0, x1, y1
    of `size`, the whole of it by default) and centred there: RGBA with anti-aliased alpha, or RGB
    over `background`."""
    r = turn(yaw, pitch, roll)
    views = [Surface(s.p @ r.T, s.uv, s.ix, tuple(v @ r.T for v in s.frame), s.material) for s in items]
    allp = np.concatenate([v.p for v in views])
    lo, hi = allp[:, :2].min(axis=0), allp[:, :2].max(axis=0)
    x0, y0, x1, y1 = frame or (0, 0, size[0], size[1])
    w, h = size[0] * SCALE, size[1] * SCALE
    fit = SCALE * min((x1 - x0) / (hi[0] - lo[0]), (y1 - y0) / (hi[1] - lo[1]))
    cx, cy = SCALE * (x0 + x1) / 2, SCALE * (y0 + y1) / 2
    centre = (lo + hi) / 2

    def project(p):
        return (p[:, 0] - centre[0]) * fit + cx, cy - (p[:, 1] - centre[1]) * fit, p[:, 2]
    index, uv, gframe = raster(views, (w, h), project)
    eye = np.zeros((h, w, 3))
    eye[:, :, 2] = 1.0
    colour = to_srgb(shade(views, index, uv, gframe, eye))
    cover = (index >= 0).astype(np.float64)
    if background is not None:
        colour = np.where(cover[:, :, None] > 0, colour, np.array(background) / 255.0)
        return Image.fromarray((colour * 255).round().astype(np.uint8), 'RGB').resize(size, Image.Resampling.BOX)
    return downsampled(colour, cover, size)


def downsampled(colour, cover, size):
    """RGBA of SCALE x SCALE samples a pixel: the alpha their coverage, the colour their mean over
    the covered ones."""
    w, h = size
    c = (colour * cover[:, :, None]).reshape(h, SCALE, w, SCALE, 3).sum(axis=(1, 3))
    a = cover.reshape(h, SCALE, w, SCALE).sum(axis=(1, 3))
    rgb = np.where(a[:, :, None] > 0, c / np.maximum(a, 1)[:, :, None], 0)
    out = np.dstack([rgb * 255, a / (SCALE * SCALE) * 255]).round().clip(0, 255).astype(np.uint8)
    return Image.fromarray(out, 'RGBA')


def perspective(items, size, eye, fov_deg, near=0.02, background=(70, 80, 95)):
    """Surfaces in weapon space seen from `eye` along -Z (the weapon's muzzle), vertical field of
    view `fov_deg`, over `background`."""
    w, h = size[0] * SCALE, size[1] * SCALE
    f = (h / 2) / np.tan(np.radians(fov_deg) / 2)
    views = [Surface(s.p - np.array(eye), s.uv, s.ix, s.frame, s.material) for s in items]

    def project(p):
        dz = -p[:, 2]
        ok = dz > near
        with np.errstate(divide='ignore', invalid='ignore'):
            x = np.where(ok, w / 2 + f * p[:, 0] / dz, np.nan)
            y = np.where(ok, h / 2 - f * p[:, 1] / dz, np.nan)
        return x, y, -dz
    index, uv, gframe = raster(views, (w, h), project)
    gy, gx = np.mgrid[0:h, 0:w] + 0.5
    towards = unit(np.dstack([-(gx - w / 2) / f, (gy - h / 2) / f, np.ones((h, w))]))
    colour = to_srgb(shade(views, index, uv, gframe, towards))
    colour = np.where((index >= 0)[:, :, None], colour, np.array(background) / 255.0)
    return Image.fromarray((colour * 255).round().astype(np.uint8), 'RGB').resize(size, Image.Resampling.BOX)


def framed(picture):
    """An RGBA picture of the object alone (transparent around it) as the stock icons have theirs:
    scaled into FRAME, centred on CENTRE_X, with their glow, in a CELL."""
    picture = picture.convert('RGBA')
    picture = picture.crop(picture.getchannel('A').point(lambda a: 255 if a > 8 else 0).getbbox())
    x0, y0, x1, y1 = FRAME
    s = min((x1 - x0) / picture.width, (y1 - y0) / picture.height)
    size = (max(1, round(picture.width * s)), max(1, round(picture.height * s)))
    a = np.asarray(picture, dtype=np.float64)
    pre = np.dstack([a[:, :, :3] * a[:, :, 3:4] / 255, a[:, :, 3]])
    pre = np.dstack([np.asarray(Image.fromarray(pre[:, :, k].astype(np.float32), 'F').resize(size, Image.Resampling.LANCZOS))
                     for k in range(4)]).clip(0, 255)
    rgb = np.where(pre[:, :, 3:4] > 0, pre[:, :, :3] * 255 / np.maximum(pre[:, :, 3:4], 1e-6), 0)
    obj = Image.fromarray(np.dstack([rgb, pre[:, :, 3]]).round().clip(0, 255).astype(np.uint8), 'RGBA')
    at = (round(CENTRE_X - size[0] / 2), round((y0 + y1) / 2 - size[1] / 2))
    mask = Image.new('L', CELL, 0)
    mask.paste(obj.getchannel('A'), at)
    glow = np.asarray(mask.filter(ImageFilter.GaussianBlur(GLOW[0])), dtype=np.float64) * GLOW[1]
    cell = Image.new('RGBA', CELL, (0, 0, 0, 0))
    cell.putalpha(Image.fromarray(glow.round().clip(0, 255).astype(np.uint8), 'L'))
    cell.alpha_composite(obj, at)
    return cell


def icon(model, roots, picture=None, renames=None):
    """A model's shop icon: `picture` (0.69d0's own, a PNG path) when given, else the model rendered
    from VIEW (its materials found in `roots`, `renames` as parts()); framed as the stock icons."""
    if picture is not None:
        return framed(Image.open(picture))
    return framed(render(parts(model, roots, renames=renames), (CELL[0] * 2, CELL[1] * 2), *VIEW))
