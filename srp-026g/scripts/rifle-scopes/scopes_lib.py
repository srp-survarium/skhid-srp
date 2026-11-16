"""What survey.py and make_pack.py share: the tables, reading configs and meshes, the optics'
modules, our variants and each weapon's mount."""
import glob
import hashlib
import shutil
import struct
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'content-edit'))
import content_edit as ce  # noqa: E402

PICATINNY = ('aimpoint_compm4_collimator', 'eotech_552_collimator', 'docter_red_dot_sight_collimator',
             'leupold_dppro_collimator', 'walther_competition2_collimator', 'acog_ta31_4x32_scope',
             'elcan_specterdr01_1x4x_scope', 'ncstar_4_collimator', 'eotech_xps2_collimator',
             'belomo_pk01_collimator', 'hensoldt_rsas_collimator', 'aperture_scope', 'acog_ta01_4x32_scope',
             'leupold_3_7x4x8_scope')
# surfaces an aimed model of ours adds to its optic's: the collimators' red dot, the scopes' eyepiece
DOT = 'collimator_glowing_dot_s'
BACKGROUND = 'scope_aim_background_s'
EYEPIECE_LENS = 'scope_lense_1p_s'
GRID, REDDOT = 'scope_aimgrid_s', 'scope_reddot_s'


def rows(name):
    lines = (HERE / name).read_text(encoding='utf-8').splitlines()
    return [line.split('\t') for line in lines if line and not line.startswith('#')]


def scopes():
    out = []
    for dict_id, name, module, kind, fov, price, row, col, donor_zoom in rows('scopes.tsv'):
        out.append({'dict_id': int(dict_id), 'name': name, 'short': name.removeprefix('shop_scope_'),
                    'module': module, 'module_name': module.split('/')[-1], 'kind': kind, 'fov': float(fov),
                    'price': int(price), 'icon': (int(row), int(col)), 'donor_zoom': donor_zoom})
    return out


def leaves(path):
    """A config file's values by path ('/a/b[0]/c'): numbers, texts, float vectors as lists."""
    return leaves_of(Path(path).read_bytes())


def leaves_of(data):
    """leaves() of a config's bytes."""
    buf = bytearray(data)
    out = {}
    for p, t, v in ce.walk(buf):
        if t == ce.T_STR:
            out[p] = v.decode('utf-8', 'replace')
        elif t in (ce.T_INT, ce.T_FLOAT):
            out[p] = ce.as_number(t, v)
        elif t == ce.T_BOOL:
            out[p] = bool(v & 1)
        elif t > ce.T_STR:
            out[p] = list(struct.unpack('<%df' % (t - 4), v))
    return out


def positions(path):
    data = Path(path).read_bytes()
    kind, size, n = struct.unpack_from('<3I', data, 0)
    stride = (size - 4) // n if n else 28
    raw = np.frombuffer(data, dtype=np.uint8, count=stride * n, offset=12).reshape(n, stride)
    return raw[:, 0:12].copy().view('<f4').reshape(n, 3).astype(np.float64)


def static_026(data):
    """A static `converted_model` (28-byte vertices: float3 position, three basis words, half2 uv) in
    0.26g's layout, whichever build wrote it. 0.26g reads each basis vector (normal, tangent along u,
    bitangent along v) as a B8G8R8A8 word, a component `c` as the byte `127.5 * (c + 1)`, the normal's
    alpha 127 and the others' 255 (every stock 0.26g scope). 0.69d0 writes the same three bytes as one
    float, `(x << 16 | y << 8 | z) / 2^24`: 0.26g reading that float's bits as the word gets the
    vectors wrong (the x byte loses its top bit, the alpha is the float's exponent), and drew the
    0.69d0 optics nearly black with light streaks (seen in the game, rifle scopes @1..@4: the PK-01VI).
    Checked on the Aimpoint CompM4, whose mesh both builds ship: each 0.69d0 float times 2^24 is the
    0.26g word's bytes within 1. A mesh in neither layout is refused."""
    kind, size, n = struct.unpack_from('<3I', data, 0)
    stride = (size - 4) // n
    if kind != 3 or stride != 28 or 4 + stride * n != size:
        raise ValueError(f'not a static mesh (chunk {kind}, stride {stride})')
    raw = np.frombuffer(data, dtype=np.uint8, count=stride * n, offset=12).reshape(n, stride).copy()
    if (raw[:, 15] == 127).all():
        return bytes(data)
    floats = raw[:, 12:24].copy().view('<f4').reshape(n, 3).astype(np.float64)
    if not (np.isfinite(floats).all() and (floats >= 0).all() and (floats < 1).all()):
        raise ValueError("a basis in neither 0.26g's nor 0.69d0's layout")
    packed = np.minimum(np.round(floats * 16_777_216.0).astype(np.uint32), 0xff_ffff)
    words = packed | (np.array([127, 255, 255], dtype=np.uint32) << 24)
    raw[:, 12:24] = words.astype('<u4').view(np.uint8).reshape(n, 12)
    out = bytearray(data)
    out[12:12 + stride * n] = raw.tobytes()
    return bytes(out)


def model_026(model, work):
    """A model directory whose meshes are all in 0.26g's layout: `model` itself, or a copy of it under
    `work` with its 0.69d0 meshes rewritten (static_026)."""
    model = Path(model)
    meshes = sorted(model.glob('render/*/converted_model'))
    made = {m: static_026(m.read_bytes()) for m in meshes}
    if all(made[m] == m.read_bytes() for m in meshes):
        return model
    copy = Path(work) / 'layout_026' / hashlib.sha1(str(model.resolve()).encode()).hexdigest()[:16] / model.name
    if not copy.exists():
        shutil.copytree(model, copy)
        for m, data in made.items():
            (copy / m.relative_to(model)).write_bytes(data)
    return copy


def basis(raw):
    """(normal, tangent, bitangent) of 28-byte vertices in 0.26g's layout (static_026), each (n, 3)."""
    def word(k):
        return raw[:, 12 + 4 * k:15 + 4 * k][:, ::-1].astype(np.float64) / 127.5 - 1.0
    return word(0), word(1), word(2)


def locator(props, name):
    """(index, position, rotation) of a model's locator."""
    i = 0
    while f'/locators[{i}]/name' in props:
        if props[f'/locators[{i}]/name'] == name:
            return i, props[f'/locators[{i}]/position'], props.get(f'/locators[{i}]/rotation')
        i += 1
    return None


def model_dir(root, model):
    """A model reference ('weapons/attach/fit3/x') as its directory under `root`."""
    for suffix in ('.model', '.skinned_model'):
        d = Path(root) / 'models' / (model + suffix)
        if d.exists():
            return d
    return None


def surfaces(model):
    """The LOD0 surfaces of a model directory, in order."""
    props = leaves(model / 'settings')
    out, i = [], 0
    while f'/lod_hierrarchy/LOD0/surfaces[{i}]' in props:
        out.append(props[f'/lod_hierrarchy/LOD0/surfaces[{i}]'])
        i += 1
    return out


def bbox_centre(model, surface):
    p = positions(model / 'render' / surface / 'converted_model')
    return (p.min(axis=0) + p.max(axis=0)) / 2


class Modules:
    """The 0.69d0 optics of the donor (the new ones and references.tsv's): their surfaces' meshes and
    their eye point (the `aim` locator)."""

    def __init__(self, donor):
        self.donor = Path(donor)
        self.meshes = {}
        self.aim = {}
        for d in glob.glob(str(self.donor / 'models/weapons/modules/*/*/*.model')):
            name = Path(d).name[:-len('.model')]
            props = leaves(Path(d) / 'render/export_properties')
            found = locator(props, 'aim')
            if found:
                self.aim[name] = np.array(found[1])
            for s in glob.glob(d + '/render/*/converted_model'):
                self.meshes.setdefault(Path(s).parent.name, []).append((name, positions(s)))

    def model(self, module, aimed=False):
        name = module.split('/')[-1] + ('_aim' if aimed else '')
        d = self.donor / 'models/weapons/modules' / module.rsplit('/', 1)[0] / module.split('/')[-1] / (name + '.model')
        return d if d.exists() else None

    def match(self, surface, points):
        """(module, translation) when `points` are one of the modules' surface `surface` moved by
        a translation alone."""
        for name, base in self.meshes.get(surface, []):
            if len(base) != len(points):
                continue
            t = (points - base).mean(axis=0)
            if np.abs(points - base - t).max() < 1e-5:
                return name, t
        return None


def variants(live, modules):
    """Our scope variants (patch 19's: a scope item made for a weapon): its weapons, kind (C a
    collimator, S a scope), weapon model change, models, and the module found in its model with the
    weapon's mount translation."""
    out = []
    for cfg in sorted(glob.glob(str(Path(live) / 'gameplay/items/scopes/*.options'))):
        d = leaves(cfg)
        if '/data/idle_model' not in d or '/srp_scope_fit/scope' not in d:
            continue
        weapons, i = [], 0
        while f'/srp_scope_fit/weapons[{i}]' in d:
            weapons.append(d[f'/srp_scope_fit/weapons[{i}]'])
            i += 1
        change = None
        for i in range(4):
            if d.get(f'/modifications[0]/modifiers[{i}]/path[1]') == 'model':
                change = d[f'/modifications[0]/modifiers[{i}]/value']
        rec = {'name': Path(cfg).stem, 'scope': d['/srp_scope_fit/scope'], 'weapons': weapons,
               'kind': 'C' if d.get('/data/hide_weapon_on_aim') is False else 'S', 'change': change,
               'idle': d['/data/idle_model'], 'aim': d['/data/aimed_model'], 'module': None, 't': None,
               # 0 sets the weapon's stock scope (addons/rifle_scope_dict_id), 3 adds one
               'puts': d.get('/modifications[0]/modifiers[0]/type')}
        idle = model_dir(live, rec['idle'])
        if idle is None:
            continue
        rec['idle_dir'], rec['aim_dir'] = idle, model_dir(live, rec['aim'])
        own = set()
        for s in surfaces(idle):
            found = modules.match(s, positions(idle / 'render' / s / 'converted_model'))
            if found and found[0] in PICATINNY:
                rec['module'], rec['t'] = found
                own.add(s)
        if rec['module']:
            rec['rails'] = [s for s in surfaces(idle) if s not in own and not s.startswith('scope_lense')]
            aim_surfaces = surfaces(rec['aim_dir']) if rec['aim_dir'] else []
            rec['aim_rails'] = [s for s in aim_surfaces if s in rec['rails']]
        out.append(rec)
    return out
