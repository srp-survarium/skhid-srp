#!/usr/bin/env python3
"""Before/after previews of the STK2 AK-74s (docs/notes/weapons-stk2.md): the live AK-74M on the
left, the new gun on the right, the same camera and the same fixed exposure.

    python3 -I preview.py --live <tree> --guns <tree> --out <folder>

<live>: the live files at their game paths (the AK-74M's model, materials, textures and clips,
the player's skeleton, the hud hands, the rifles' animation sets); <guns>: the port's overlay
(the new guns' models, materials, textures and configs). Writes models.png (side and
three-quarter views) and fp_<gun>.png: first-person frames of the idle, the aim, the reload (both
clips) and the draw, posed as the game poses them:

- the player: the state's hands clip (additive: rotation base x additive, translations added)
  on its stance (the rifles' set: stand_hud's on_site_idle, aimed_stand_hud's
  aimed_on_site_idle when aiming);
- the gun on the player's Weapon bone turned half about Y, posed by the state's item clip;
- the camera at the Item bone (65 degrees vertical, near 0.05 m); aimed, at the gun's `aim`
  locator (calculate_aim_alignment: translate(aim_offset) x rotation_y(pi) x locator x bone),
  the view narrowed by the gun's aim_zoom_factor.

Each frame is measured too (contacts.tsv): how deep the hud hands go into the gun (the hands'
vertices behind the gun's surface at their nearest gun vertex, within 2 cm), the left and the right
hand apart, for both guns. hands_<gun>.png: the same frames seen from the gun's left, close (both hands, the
magazine).

Needs NumPy, SciPy and Pillow.
"""
import argparse
import math
import struct
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy.spatial import cKDTree

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'battery-hit'))
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
import clips  # noqa: E402
import model as M  # noqa: E402

HALF = clips.rotation([0, math.pi, 0])
FOV, NEAR = 65.0, 0.05
LIGHT = np.array([-0.35, 0.55, -0.75])
LIGHT /= np.linalg.norm(LIGHT)
EXPOSURE = 1.6
ANIM = 'resources/animations/single/weapons/ak'
SETS = 'gameplay/items/weapons/common/rifles.user_animations'


def mat(r, p):
    m = np.eye(4)
    m[:3, :3], m[3, :3] = r, p
    return m


def read_clip(root, stem):
    stem = stem.removeprefix('resources/')
    tracks, kind = clips.parse_clip((root / (stem + '.b-spline')).read_bytes())
    return dict(zip(clips.parse_names((root / (stem + '.bones_names')).read_bytes()), tracks)), kind


def clip_end(clip):
    return max(c.knots[-1] for ch in clip[0].values() for c in ch)


def sample(clip, bone, t):
    ch = clip[0].get(bone)
    if ch is None:
        return None
    v = [c.evaluate(t) for c in ch]
    return clips.rotation(v[3:6]), np.array(v[:3])


def bind_pose(path):
    data = path.read_bytes()
    out, at = {}, 2
    for _ in range(struct.unpack_from('<H', data)[0]):
        end = data.index(0, at)
        v = struct.unpack_from('<9f', data, end + 1)
        out[data[at:end].decode()] = mat(np.diag(v[6:]) @ clips.rotation(v[3:6]), v[:3])
        at = end + 37
    return out


def texture(root, name):
    path = root / f'textures/{name}.dds'
    return np.asarray(Image.open(path).convert('RGBA'), np.float32) / 255.0


class Surface:
    """A skinned surface with its diffuse and normal maps."""

    def __init__(self, path, diffuse, normal):
        b = path.read_bytes()
        _, size, n = struct.unpack_from('<3I', b)
        stride = (size - 4) // n
        raw = np.frombuffer(b, np.uint8, n * stride, 12).reshape(n, stride)
        self.pos = raw[:, :12].copy().view('<f4').reshape(n, 3).astype(np.float64)
        count = (stride - 36) // 4 + 1
        self.bones = raw[:, 12:12 + count].astype(np.int64)
        self.weights = np.ones((n, count))
        if count > 1:
            self.weights[:, :-1] = raw[:, 16:16 + 4 * (count - 1)].copy().view('<f4').reshape(n, count - 1)
            self.weights[:, -1] = 1 - self.weights[:, :-1].sum(1)

        def basis(at):
            w = raw[:, at:at + 4].astype(np.float64)
            return np.c_[w[:, 2], w[:, 1], w[:, 0]] / 127.5 - 1

        at = 16 + 4 * (count - 1)
        self.nrm, self.tan, self.bit = basis(at), basis(at + 4), basis(at + 8)
        self.uv = raw[:, -8:].copy().view('<f4').reshape(n, 2).astype(np.float64)
        _, isize, icount = struct.unpack_from('<3I', b, 8 + size)
        self.tri = np.frombuffer(b, '<u2', icount, 20 + size).reshape(-1, 3).astype(np.int64)
        self.diffuse, self.normal = diffuse, normal

    def posed(self, skin):
        """Positions and vectors under `skin` (a 4x4 per bone: inverse bind x world)."""
        out = []
        for v, w in ((self.pos, 1.0), (self.nrm, 0.0), (self.tan, 0.0), (self.bit, 0.0)):
            h = np.c_[v, np.full(len(v), w)]
            acc = np.zeros((len(v), 4))
            for k in range(self.bones.shape[1]):
                acc += np.einsum('ni,nij->nj', h, skin[self.bones[:, k]]) * self.weights[:, k:k + 1]
            out.append(acc[:, :3])
        return out


class Model:
    def __init__(self, root, model):
        d = root / f'models/{model}.skinned_model'
        self.bind = bind_pose(d / 'render/bind_pose')
        self.names = list(self.bind)
        settings = M.config(d / 'settings')
        self.surfaces = []
        for s in settings['lod_hierrarchy']['LOD0']['surfaces']:
            material = settings['material_settings'][s]['material_name']
            effect = M.config(root / f'material_instances/{material}.material')['material']['g_stage']['effect']
            self.surfaces.append(Surface(d / f'render/{s}/converted_model',
                                         texture(root, effect['texture_diffuse']['value']),
                                         texture(root, effect['texture_normal']['value'])))
        self.dir = d

    def meshes(self, world):
        skin = np.stack([np.linalg.inv(self.bind[n]) @ world[n] for n in self.names])
        return [(s, *s.posed(skin)) for s in self.surfaces]


class Gun(Model):
    def __init__(self, root, live, config):
        cfg = M.config(root / config) if (root / config).exists() else M.config(live / config)
        super().__init__(root if (root / f'models/{cfg["object"]["model"]}.skinned_model').exists() else live,
                         cfg['object']['model'])
        self.cfg = cfg
        skeleton = M.config(self.dir / 'skeleton')['skeleton'].removeprefix('resources/')
        self.parents = clips.skeleton_parents((live / f'{skeleton}.skeleton').read_bytes())
        self.aim = M.locator(self.dir, 'aim')

    def world(self, item, t, weapon):
        local = {}
        for n in self.names:
            s = sample(item, n, t) if item else None
            if s is None:
                p = self.parents.get(n)
                m = self.bind[n] @ np.linalg.inv(self.bind[p]) if p in self.bind else self.bind[n]
                s = (m[:3, :3], m[3, :3])
            local[n] = s
        root = mat(HALF, np.zeros(3)) @ weapon
        return {n: mat(r, p) @ root for n, (r, p) in clips.world_pose(local, self.parents, self.names).items()}

    def eye(self, world):
        """The aimed camera: translate(aim_offset) x rotation_y(pi) x locator x its bone."""
        offset = self.cfg['parameters'].get('aim_offset', 0.0)
        bone = world[self.names[self.aim.bone]]
        return mat(np.eye(3), [0, 0, offset]) @ mat(HALF, np.zeros(3)) @ M.matrix(self.aim.position, self.aim.rotation) @ bone


class Player:
    def __init__(self, live):
        sk = (live / 'animations/skeletons/scavengers_01.skeleton').read_bytes()
        self.parents = clips.skeleton_parents(sk)
        self.order = []
        seen = set()

        def visit(n):
            if n not in seen:
                if self.parents.get(n):
                    visit(self.parents[n])
                seen.add(n)
                self.order.append(n)
        for n in self.parents:
            visit(n)
        self.hands = Model(live, 'character/human/black_market/black_market_01_hand_hud')
        dominant = np.concatenate([s.bones[np.arange(len(s.bones)), s.weights.argmax(1)] for s in self.hands.surfaces])
        names = np.array(self.hands.names)[dominant]
        self.palm = {side: np.array([n.startswith(side) and 'Arm' not in n and 'Clavicle' not in n for n in names])
                     for side in ('Left', 'Right')}

    def world(self, base, base_t, add, add_t):
        local = {}
        for n in self.order:
            b = sample(base, n, base_t) or (np.eye(3), np.zeros(3))
            a = sample(add, n, add_t) if add else None
            if a is not None:
                b = (b[0] @ a[0], b[1] + a[1]) if add[1] == 1 else a
            local[n] = b
        return {n: mat(r, p) for n, (r, p) in clips.world_pose(local, self.parents, self.order).items()}


def draw(meshes, camera, size, fov, near=NEAR, ss=2):
    """Textured, normal-mapped, depth-tested perspective render; camera rows: right, up, forward."""
    W, H = size[0] * ss, size[1] * ss
    rgb = np.zeros((H, W, 3), np.float32)
    rgb[:] = (0.16, 0.17, 0.19)
    depth = np.full((H, W), np.inf, np.float32)
    focal = H / (2 * math.tan(math.radians(fov) / 2))
    inv = np.linalg.inv(camera)
    light = LIGHT
    for surface, pos, nrm, tan, bit in meshes:
        v = np.c_[pos, np.ones(len(pos))] @ inv
        v = v[:, :3]
        n_v, t_v, b_v = (x @ inv[:3, :3] for x in (nrm, tan, bit))
        tex, nmap = surface.diffuse, surface.normal
        th, tw = tex.shape[:2]
        nh, nw = nmap.shape[:2]
        for tri in surface.tri:
            z = v[tri, 2]
            if z.min() < near:
                continue
            sx = v[tri, 0] / z * focal + W / 2
            sy = H / 2 - v[tri, 1] / z * focal
            x0, x1 = max(0, int(sx.min())), min(W - 1, int(math.ceil(sx.max())))
            y0, y1 = max(0, int(sy.min())), min(H - 1, int(math.ceil(sy.max())))
            if x0 > x1 or y0 > y1:
                continue
            d = (sy[1] - sy[2]) * (sx[0] - sx[2]) + (sx[2] - sx[1]) * (sy[0] - sy[2])
            if abs(d) < 1e-9:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + .5, np.arange(y0, y1 + 1) + .5)
            w0 = ((sy[1] - sy[2]) * (gx - sx[2]) + (sx[2] - sx[1]) * (gy - sy[2])) / d
            w1 = ((sy[2] - sy[0]) * (gx - sx[2]) + (sx[0] - sx[2]) * (gy - sy[2])) / d
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not inside.any():
                continue
            # perspective-correct barycentrics
            p0, p1, p2 = w0 / z[0], w1 / z[1], w2 / z[2]
            s = p0 + p1 + p2
            p0, p1, p2 = p0 / s, p1 / s, p2 / s
            zz = 1 / s
            region = depth[y0:y1 + 1, x0:x1 + 1]
            inside &= zz < region
            if not inside.any():
                continue
            a, b, c = p0[inside], p1[inside], p2[inside]
            uv = a[:, None] * surface.uv[tri[0]] + b[:, None] * surface.uv[tri[1]] + c[:, None] * surface.uv[tri[2]]
            col = tex[(np.mod(uv[:, 1], 1) * th).astype(int) % th, (np.mod(uv[:, 0], 1) * tw).astype(int) % tw]
            nm = nmap[(np.mod(uv[:, 1], 1) * nh).astype(int) % nh, (np.mod(uv[:, 0], 1) * nw).astype(int) % nw]
            nx, ny = nm[:, 3] * 2 - 1, nm[:, 1] * 2 - 1
            nz = np.sqrt(np.clip(1 - nx * nx - ny * ny, 0, 1))
            interp = lambda q: a[:, None] * q[tri[0]] + b[:, None] * q[tri[1]] + c[:, None] * q[tri[2]]
            nn = interp(t_v) * nx[:, None] + interp(b_v) * ny[:, None] + interp(n_v) * nz[:, None]
            nn /= np.maximum(np.linalg.norm(nn, axis=1, keepdims=True), 1e-9)
            lit = 0.30 + 0.85 * np.maximum(nn @ light, 0)
            half = light + np.array([0, 0, -1.0])
            half /= np.linalg.norm(half)
            spec = 0.25 * np.maximum(nn @ half, 0) ** 24
            shade = (col[:, :3] ** 2.2) * lit[:, None] * EXPOSURE + spec[:, None]
            region[inside] = zz[inside]
            rgb[y0:y1 + 1, x0:x1 + 1][inside] = np.clip(shade, 0, 1) ** (1 / 2.2)
    img = Image.fromarray((rgb * 255).astype(np.uint8))
    return img.resize(size, Image.LANCZOS)


def camera_of(m):
    """A 4x4 whose rows are the camera's right, up, forward and place (row vectors)."""
    return m


def depth(player, hand_meshes, gun_meshes):
    """(left, right): the deepest vertex of each hand (its palm and fingers) behind the gun's surface,
    at the nearest of the gun's vertices and triangle centres within 2 cm, metres."""
    gp, gn = [], []
    for surface, pos, nrm, _, _ in gun_meshes:
        tri = surface.tri
        face = np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]])
        face /= np.maximum(np.linalg.norm(face, axis=1, keepdims=True), 1e-12)
        gp += [pos, pos[tri].mean(1)]
        gn += [nrm / np.maximum(np.linalg.norm(nrm, axis=1, keepdims=True), 1e-12), face]
    gp, gn = np.concatenate(gp), np.concatenate(gn)
    tree = cKDTree(gp)
    hp = np.concatenate([m[1] for m in hand_meshes])
    out = []
    for side in ('Left', 'Right'):
        d, i = tree.query(hp[player.palm[side]])
        behind = -np.einsum('ij,ij->i', hp[player.palm[side]] - gp[i], gn[i])
        hit = (d < 0.02) & (behind > 0)
        out.append(float(behind[hit].max()) if hit.any() else 0.0)
    return out


def look_at(eye, target, up):
    f = target - eye
    f /= np.linalg.norm(f)
    r = np.cross(up, f)
    r /= np.linalg.norm(r)
    m = np.eye(4)
    m[0, :3], m[1, :3], m[2, :3], m[3, :3] = r, np.cross(f, r), f, eye
    return m


def frame(player, gun, stance, hands, t_hands, item, t_item, aimed, size, record=None, close=False):
    pw = player.world(stance, 0.0, hands, t_hands)
    gw = gun.world(item, t_item, pw['Weapon'])
    if record is not None:
        record.append(depth(player, player.hands.meshes(pw), gun.meshes(gw)))
    if close:
        # The gun from its left, a little above and behind, on its magazine well: the hands on it.
        view = pw['Item']
        well = np.array([0.0, -0.05, 0.12, 1.0]) @ np.linalg.inv(gun.bind['offset']) @ gw['offset']
        target = well[:3]
        eye = target - view[0, :3] * 0.6 + view[1, :3] * 0.18 - view[2, :3] * 0.15
        return draw(player.hands.meshes(pw) + gun.meshes(gw), look_at(eye, target, view[1, :3]), size, 45.0)
    if aimed:
        cam = gun.eye(gw)
        fov = 2 * math.degrees(math.atan(math.tan(math.radians(FOV / 2)) / gun.cfg['parameters'].get('aim_zoom_factor', 1.0)))
    else:
        cam = pw['Item']
        fov = FOV
    return draw(player.hands.meshes(pw) + gun.meshes(gw), cam, size, fov)


def label(img, text):
    ImageDraw.Draw(img).text((8, 6), text, fill=(235, 235, 235))
    return img


def fp_sheet(live, guns, ours, out, size=(640, 360), close=False):
    player = Player(live)
    sets = M.config(live / SETS)
    base, aimed_base = (read_clip(live, sets[k][0]) for k in ('stand_hud', 'aimed_stand_hud'))
    live_gun = Gun(live, live, 'gameplay/items/weapons/wpn_ak_74m.options')
    new_gun = Gun(guns, live, ours)
    rows, contacts = [], []

    def state(gun, key):
        st = gun.cfg['states'][key]
        return st['user_animations'], st['animations']

    for title, key, times, aimed in (('idle', 'idle', [0.0], False), ('aim', 'idle', [0.0], True),
                                     ('draw', 'show', [0.25, 0.6, 1.0], False),
                                     ('reload, tactical', 'reload', [0.15, 0.35, 0.55, 0.8], False),
                                     ('reload, empty', 'reload', [0.15, 0.35, 0.55, 0.8], False)):
        clip_index = 1 if title == 'reload, empty' else 0
        for share in times:
            tiles = []
            for gun in (live_gun, new_gun):
                users, items = state(gun, key)
                hands = read_clip(live if (live / (users[clip_index].removeprefix('resources/') + '.b-spline')).exists() else guns,
                                  users[clip_index])
                item_name = items[min(clip_index, len(items) - 1)]
                item = read_clip(live if (live / (item_name.removeprefix('resources/') + '.b-spline')).exists() else guns, item_name)
                end = clip_end(hands)
                t = share * end
                got = []
                img = frame(player, gun, aimed_base if aimed else base, hands, t, item, min(t, clip_end(item)), aimed,
                            size, got, close)
                name = 'AK-74M (live)' if gun is live_gun else ours.rsplit('/', 1)[1].removesuffix('.options')
                contacts.append(f'{name}\t{title}\t{share:.2f}\t{got[0][0] * 1000:.1f}\t{got[0][1] * 1000:.1f}')
                tiles.append(label(img, f'{name}  {title}  {share:.0%}  into the gun: left {got[0][0] * 1000:.0f} mm, right {got[0][1] * 1000:.0f} mm'))
            rows.append(tiles)
    sheet = Image.new('RGB', (size[0] * 2 + 8, (size[1] + 8) * len(rows)), (10, 10, 10))
    for i, (a, b) in enumerate(rows):
        sheet.paste(a, (0, i * (size[1] + 8)))
        sheet.paste(b, (size[0] + 8, i * (size[1] + 8)))
    sheet.save(out)
    with open(out.parent / 'contacts.tsv', 'a') as f:
        f.write(''.join(c + '\n' for c in contacts))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--live', type=Path, required=True)
    ap.add_argument('--guns', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--config', action='append', required=True, help='a new gun\'s config path')
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for config in args.config:
        name = config.rsplit('/', 1)[1].removesuffix('.options')
        fp_sheet(args.live, args.guns, config, args.out / f'fp_{name}.png')
        fp_sheet(args.live, args.guns, config, args.out / f'hands_{name}.png', close=True)
        print('wrote', args.out / f'fp_{name}.png', args.out / f'hands_{name}.png')


if __name__ == '__main__':
    main()
