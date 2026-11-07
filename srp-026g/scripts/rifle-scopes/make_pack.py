#!/usr/bin/env python3
"""make_pack.py: the rifle scopes pack (srp-026g/docs/notes/weapon_scopes.md, section 6), and the
content-edit list that goes with it.

usage: python3 -I make_pack.py --donor <folder> --live <folder> --compose <vostok-port> --out <packs folder>/rifle-scopes-<version>

--donor    prepare_donor.sh's folder (0.69d0's optics, their materials and textures).
--live     prepare_live.sh's folder (our scope items and their models, the weapons' configs and models).
--compose  a resource-porter vostok-port with `compose-model` (the optic, the weapon's rails and a
           baked reticle in one model).
--out      the pack's folder (a new one).

Writes the pack (pack.json, files/) and ../content-edit/rifle-scopes-1.json. For each scope of
scopes.tsv and each weapon fit.tsv gives it, as patch 19's variants are made (mounts.tsv,
survey.py):
- **its models** (models/weapons/attach/fit4/fitn_<scope>_<weapon>{,_aim}): the optic's 0.69d0
  model moved by the weapon's mount translation, with the mount's rails; the aimed model: a
  collimator's own (without its eyepiece lens) and the red dot of ours (patch 21's
  collimator_glowing_dot, the reticle model RETICLE) baked in at its eye point, 0.6 m on along the
  sight line, where every collimator of ours has it; a scope's: the eyepiece of the weapon's scope
  variant of ours (background and lens) with the new scope's own reticle grid and dot centred where
  that variant's are. Materials as materials.tsv says (loc2: ours, else the donor's made again).
- **the weapon's model**: a scope's variant takes its mount variant's (the aimed view is that
  variant's eyepiece, so its eye); a collimator's, when its eye line moves (the variant sets
  `object/model`, as ours do), the weapon's `aim` locator on the optic's axis looking at its red
  dot (the optic drawn at scope_point turned half about Y, the gun posed by its idle clip as the
  game draws it); a model of ours with that eye already (0.3 mm, 0.2 mrad) is taken as it is, else the mount
  variant's weapon model is copied with only its export_properties changed
  (models/weapons/<..>_n_<scope>).
- **its item** (a variant, dict id from 2701): its config made from a variant of ours of its kind
  (with or without a weapon model change) by content_edit.py's edits, read back; its dictionary row.
For each scope: its shop item (dict ids of scopes.tsv: 2113..2119) made from the CompM4's (a
collimator) or the TA31's (a scope), its price (base_cost: seller 0 sells it), its texts
(texts.tsv) and its icon (in column 4 of the consumables atlas: 0.69d0's own picture of the optic,
the donor's icons/<module>.png, framed as the stock icons by render_icon.py). Before it writes the pack,
check_textures(): every texture the pack's models draw is shipped and is a DDS 0.26g reads.
The edit list: an items_compatibility row for each weapon and scope it fits (the lobby movie's
scope pairs) and the CompM4 at x2 (data/fov_factor 0.8 -> 0.5 on its item and every variant of it).
The texts players see, and the CompM4's name with x2, are in client-patches/localization/*.tsv
(localization-1.json compiles every language from them, last).
"""
import argparse
import hashlib
import json
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pbr_026  # noqa: E402
import render_icon  # noqa: E402
import scopes_lib as lib  # noqa: E402
import sights  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'sight-alignment'))
import model as posing  # noqa: E402
import align  # noqa: E402
from scopes_lib import ce  # noqa: E402

PACK_ID = 'rifle-scopes'
VERSION = '15'
LANGUAGES = ('english', 'german', 'original', 'polish', 'russian', 'ukrainian')
FIRST_VARIANT = 2701
CATEGORY = 22
ICON_ATLAS = 'LobbyMenu_AmmoIcons'
DOT_DISTANCE = 0.6
EYE_TOLERANCE = 0.0003
EYE_TURN = 0.0002  # rad
EYE_RELIEF = 0.045  # m from a collimator's rear to the camera, at least (the AKMN's PK-01VI 0.049, most 0.06 to 0.2)
# the red dot every collimator of ours has (patch 21), as a model of its own centred at its origin
RETICLE_FROM = ('fitn_aimpoint_compm4_m4a1', lib.DOT)
COMPM4 = 'shop_scope_aimpoint_compm4'
COMPM4_FOV = (0.8, 0.5)
SHOP_TEMPLATE = {'C': 'shop_scope_aimpoint_compm4', 'S': 'shop_scope_acog_ta31'}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def porter_text(text):
    """The porter reads a config's strings as Windows-1251 (byte 0x98 as U+0098): a pack names the
    UTF-8 bytes so read."""
    return ''.join('\x98' if b == 0x98 else bytes([b]).decode('cp1251') for b in text.encode('utf-8'))


def read_mounts():
    mounts = {}
    for row in lib.rows('mounts.tsv'):
        w = row[0]
        for kind, (variant, t, rails) in (('C', row[1:4]), ('S', row[4:7])):
            if variant != '-':
                mounts.setdefault(w, {})[kind] = {'variant': variant, 't': np.array([float(x) for x in t.split(',')]),
                                                  'rails': [] if rails == '-' else rails.split()}
    return mounts


def read_fits():
    return {name: weapons.split() for name, *rest in lib.rows('fit.tsv') for weapons in [rest[0] if rest else '']}


def read_texts():
    header, *table = lib.rows('texts.tsv')
    if tuple(header) != ('key',) + LANGUAGES:
        sys.exit(f'texts.tsv: the columns are key and {", ".join(LANGUAGES)}')
    return {row[0]: dict(zip(LANGUAGES, row[1:])) for row in table}


def compose(port, parts, materials, out):
    """One model of the parts' surfaces (compose-model), each part's meshes in 0.26g's layout
    (scopes_lib.static_026: a 0.69d0 module's basis made 0.26g's words; the rifle scopes @1..@4 kept
    0.69d0's, and the optics drew nearly black)."""
    with tempfile.TemporaryDirectory() as layout:
        args = [str(port), 'compose-model', '--output', str(out)]
        for model, t, chosen in parts:
            spec = f'{lib.model_026(model, layout)}=%.6f,%.6f,%.6f' % tuple(t)
            if chosen:
                spec += ':' + ','.join(chosen)
            args += ['--part', spec]
        for frm, to in materials.items():
            args += ['--material', f'{frm}={to}']
        subprocess.run(args, check=True, capture_output=True)


def material_renames():
    out = {}
    for name, how in lib.rows('materials.tsv'):
        if how.startswith('loc2:'):
            out[f'weapons/modules/{name}'] = 'weapons/loc2/' + how[len('loc2:'):]
    return out


def weapon_models(live):
    """dict name -> (config path, model reference) of every weapon config of ours."""
    out = {}
    d = lib.leaves(Path(live) / 'gameplay/db_static_dictionaries')
    i = 0
    while f'/items_dict[{i}]/name' in d:
        cfg = d[f'/items_dict[{i}]/cfg_name']
        if cfg.startswith('gameplay/items/weapons/wpn_'):
            model = lib.leaves(Path(live) / cfg).get('/object/model')
            if model:
                out[d[f'/items_dict[{i}]/name']] = (cfg, model)
        i += 1
    return out


def eye_model(candidates, base_model, needed):
    """The model of `candidates` whose `aim` locator is `needed` (position, rotation; collimator_eye):
    its position within EYE_TOLERANCE, its rotation within EYE_TURN; None for the weapon's own, its
    reference for another; False for none."""
    for model, d in candidates:
        found = lib.locator(lib.leaves(d / 'render/export_properties'), 'aim') if d else None
        if found and np.abs(np.array(found[1]) - needed[0]).max() < EYE_TOLERANCE \
                and np.abs(np.array(found[2] or [0.0, 0.0, 0.0]) - needed[1]).max() < EYE_TURN:
            return None if model == base_model else model
    return False


def posed_frame(root, cfg, directory):
    """The optic's frame (row vectors, 4x4) in the frame of the aim locator's bone of the weapon model
    `directory`: the optic drawn at scope_point turned half about Y, the weapon posed by its idle
    clip as the sight-alignment tool poses it (matched with the game; `root` has the clip and the
    skeleton). The clip may move the bone scope_point hangs on off the aim locator's: the UMP-45's
    3 cm aside, the Rem 870's 6 cm down; read unposed, the variants' red dots showed that far off
    the screen's centre (@6)."""
    states = posing.config(root / cfg)['states']
    animation = states.get('aimed_idle', {}).get('animations', states['idle']['animations'])[0]  # posing.weapon's
    rig = posing.bones(root, directory, animation, 0)
    aim = posing.locator(directory, 'aim')
    frame = posing.HALF @ posing.locator_matrix(posing.locator(directory, 'scope_point'), rig)
    if aim.bone is not None:
        frame = frame @ np.linalg.inv(rig[aim.bone].pose)
    return frame


def collimator_eye(root, cfg, directory, aimed):
    """The aim locator (position, rotation) of the weapon model `directory` that looks along the
    optic's axis at the red dot of the aimed model `aimed` (posed_frame), as the sight-alignment
    tool's on_axis puts it (the 9A-91's Cobra), and at least EYE_RELIEF behind the optic. A clip may
    turn the optic's bone off the eye's too (the RPD, the Vepr, the Magnum): an eye only moved onto
    the dot then looked across the optic, the dot centred but the optic beside it (@9); the RPD's eye
    sat inside the PK-01VI."""
    frame = posed_frame(root, cfg, directory)
    aim = posing.locator(directory, 'aim')
    offset = posing.config(root / cfg)['parameters'].get('aim_offset', 0)
    names = lib.surfaces(aimed)
    dot = posing.transform(lib.bbox_centre(aimed, next(n for n in names if align.emitter(n))), frame)
    loc = align.on_axis({'id': str(directory), 'mount': frame, 'marker': dot,
                         'locator': {'position': aim.position, 'rotation': aim.rotation}, 'offset': offset})
    position = np.array(loc['position'])
    # the camera: aim_offset on from the locator (the RPD's 0.1 m), as model.weapon draws it
    camera = (posing.matrix((0, 0, offset)) @ posing.HALF @ posing.matrix(position, loc['rotation']))[3, :3]
    ahead = (dot - camera) / np.linalg.norm(dot - camera)
    optic = posing.transform(np.concatenate([lib.positions(aimed / 'render' / n / 'converted_model')
                                             for n in names if not align.emitter(n)]), frame)
    rear = ((optic - camera) @ ahead).min()
    if rear < EYE_RELIEF:
        position -= ahead * (EYE_RELIEF - rear)
    return position, np.array(loc['rotation'])


def moved_eye(model, needed):
    """A copied weapon model's `aim` locator set to `needed` (position, rotation; collimator_eye)."""
    path = model / 'render/export_properties'
    buf = bytearray(path.read_bytes())
    i, _, rotation = lib.locator(lib.leaves(path), 'aim')
    if rotation is None:
        sys.exit(f'{model}: its aim locator has no rotation to set')
    set_float3(buf, f'locators[{i}]/position', [float(v) for v in needed[0]])
    set_float3(buf, f'locators[{i}]/rotation', [float(v) for v in needed[1]])
    path.write_bytes(bytes(buf))


def wrap_uv(model):
    """Each triangle of a model's meshes with its texture coordinates in [0, 1]: a triangle wholly past
    an edge is moved back by whole textures. 0.69d0 lets them run past (its aperture sight's body to
    u = 1.91: the texture repeats); 0.26g clamps a weapon texture there, and drew the edge's texels
    smeared over those faces (seen in the game, @2: the aperture sight's body garbled). A surface with a
    triangle across an edge is left as it is."""
    for f in sorted(Path(model).glob('render/*/converted_model')):
        data = bytearray(f.read_bytes())
        kind, size, n = struct.unpack_from('<3I', data, 0)
        stride = (size - 4) // n
        at = 12 + stride * n
        _, _, count = struct.unpack_from('<3I', data, at)
        tris = np.frombuffer(bytes(data), dtype='<u2', count=count, offset=at + 12).reshape(-1, 3)
        raw = np.frombuffer(bytes(data), dtype=np.uint8, count=stride * n, offset=12).reshape(n, stride).copy()
        uv = raw[:, 24:28].copy().view('<f2').reshape(n, 2).astype(np.float64)
        if uv.min() >= -0.01 and uv.max() <= 1.01:
            continue
        shift = np.zeros((n, 2))
        whole = True
        for t in tris:
            lo = np.floor(uv[t].mean(axis=0))
            if (uv[t].min(axis=0) < lo - 0.01).any() or (uv[t].max(axis=0) > lo + 1.01).any() or \
                    ((shift[t] != 0) & (shift[t] != lo)).any():
                whole = False  # a surface laid across the edges on purpose (our eyepiece's tiled background)
                break
            shift[t] = lo
        if not whole or not shift.any():
            continue
        uv -= shift
        raw[:, 24:28] = uv.astype('<f2').view(np.uint8).reshape(n, 4)
        data[12:12 + stride * n] = raw.tobytes()
        f.write_bytes(bytes(data))


def aim_turned(model):
    """Whether a model's aim locator is turned (the view through it tilted)."""
    rotation = lib.locator(lib.leaves(model / 'render/export_properties'), 'aim')[2]
    return bool(rotation) and max(abs(x) for x in rotation) > 1e-6


def sight_changes(table):
    """A sights table's changes under an optic (its last column), per gun (its first): (how, surface,
    the rest); `keep` left out. sights.tsv here (the live guns); scope fits' has the release-41 guns."""
    out = {}
    for line in Path(table).read_text(encoding='utf-8').splitlines():
        if not line or line.startswith('#'):
            continue
        row = line.split('\t')
        how = row[-1].split()
        if how[0] == 'keep':
            continue
        if len(how) not in {'fold': (4,), 'turn': (4,), 'refold': (5,), 'swap': (4,), 'cut': (3,), 'lower': (4,)}.get(how[0], ()):
            sys.exit(f'{table}: {row[0]}: {row[-1]!r} is not keep, fold|turn <surface> <up> <folded>, refold <surface> '
                     '<up> <folded> <their surface>, swap <surface> <up> <other>, cut <surface> <box> or lower <surface> <box> <height>')
        out.setdefault(row[0], []).append(how)
    return out


def under_optic(model, changes, modules, gun):
    """A copied weapon model's sights as a sights table has them under an optic (sights.py): folded
    or turned (0.69d0's up module found in the surface made its folded one), refolded (found by its
    positions, a port's own texture mapping kept), swapped (taken out: the optic's model carries the
    other module, swap_parts), cut (a surface's islands inside a box), lowered (a part of an island
    cut flat)."""
    for how, surface, *rest in changes:
        if how == 'refold':
            up, folded = (modules(m) for m in rest[:2])
            if up is None or folded is None:
                sys.exit(f'{gun}: {rest} not in the donor')
            done = sights.refold(model / 'render' / surface, up / 'render' / rest[2] / 'converted_model',
                                 folded / 'render' / rest[2] / 'converted_model')
        elif how == 'swap':
            done = sights.take_out(model / 'render' / surface, one_surface(modules(rest[0]), gun) / 'converted_model')
        elif how == 'cut':
            done = sights.cut(model / 'render' / surface, [float(x) for x in rest[0].split(',')])
        elif how == 'lower':
            done = sights.lower(model / 'render' / surface, [float(x) for x in rest[0].split(',')], float(rest[1]))
        else:
            up, folded = (modules(m) for m in rest)
            if up is None or folded is None:
                sys.exit(f'{gun}: {rest} not in the donor')
            names = lib.surfaces(up)
            if len(names) != 1 or lib.surfaces(folded) != names:
                sys.exit(f'{gun}: {rest}: not one surface each, the same')
            mover = sights.turn if how == 'turn' else sights.fold
            done = mover(model / 'render' / surface, up / 'render' / names[0] / 'converted_model',
                         folded / 'render' / names[0] / 'converted_model')
        print(f'{gun}: {model.name}: {how} {surface} {" ".join(rest)}: {done}')


def one_surface(module, gun):
    """A donor module's one surface's folder."""
    if module is None:
        sys.exit(f'{gun}: a sights.tsv module not in the donor')
    names = lib.surfaces(module)
    if len(names) != 1:
        sys.exit(f'{module}: {len(names)} surfaces, not one')
    return module / 'render' / names[0]


def swap_parts(changes, modules, gun, weapon_dir):
    """The modules an optic's models carry for the sights its weapon model takes out (swap rows: 0.69d0
    puts the other module where the sight was, the M4A1's plain gas block): each (module, its
    translation in the optic's frame, all its surfaces). The optic is drawn at scope_point turned half
    about Y; the sight's translation is found in the weapon's surface (sights.found_exactly by its
    positions, the frame the surface is written in)."""
    out = []
    for how, surface, *rest in changes:
        if how != 'swap':
            continue
        gun_surface = sights.Skinned(weapon_dir / 'render' / surface / 'converted_model')
        up = sights.Static(one_surface(modules(rest[0]), gun) / 'converted_model')
        found = sights.found_exactly(gun_surface, gun_surface.positions(), up, uv=False)
        if not found:
            sys.exit(f'{gun}: {rest[0]} is not in {weapon_dir.name}/{surface}')
        sp = lib.locator(lib.leaves(weapon_dir / 'render/export_properties'), 'scope_point')[1]
        out.append((modules(rest[1]), found[0] - np.array(sp) * sights.HALF_TURN, []))
    return out


def level_eye(model):
    """A copied weapon model's aim locator unturned, where it is."""
    path = model / 'render/export_properties'
    buf = bytearray(path.read_bytes())
    i, _, rotation = lib.locator(lib.leaves(path), 'aim')
    if rotation is not None:
        set_float3(buf, f'locators[{i}]/rotation', [0.0, 0.0, 0.0])
    path.write_bytes(bytes(buf))


def set_float3(buf, path, value):
    """A float3 at `path` set to `value`, in three numbers of its own: the format keeps equal values
    once (the Rem 870's aim locator turned as its scope_point is, the same 12 bytes), so an edit
    in place changed both (@7: its scope_point's 0.008 rad roll gone with the eye's)."""
    off = ce.find(buf, path)
    _, name, crc, kind, count = ce.rec(buf, off)
    buf.extend(bytes(-len(buf) % 4))
    ce.REC.pack_into(buf, off, ce.append(buf, struct.pack('<3f', *value)), name, crc, kind, count)


def edited_config(template, values, strings):
    """`template` with each number of `values` (path: (from, to)) and string of `strings` (from:
    to) changed; read back, nothing else differs."""
    buf = bytearray(template)
    for path, (frm, to) in values.items():
        ce.set_number(buf, path, frm, to, as_float=isinstance(to, float))
    for frm, to in strings.items():
        if ce.replace_string(buf, frm, to) < 1:
            sys.exit(f'the template holds no {frm!r}')
    old = {p: v for p, t, v in ce.walk(bytearray(template))}
    new = {p: v for p, t, v in ce.walk(buf)}
    if set(old) != set(new):
        sys.exit('an edited config lost or gained a value')
    allowed = {'/' + p for p in values}
    for p in old:
        if old[p] != new[p] and p not in allowed and not (isinstance(old[p], bytes) and old[p].decode('utf-8', 'replace') in strings):
            sys.exit(f'an edited config changed {p}')
    return bytes(buf)


def files_of(directory, root):
    """(game path, bytes) of every file under a model directory composed in `root`."""
    out = {}
    for p in sorted(Path(directory).rglob('*')):
        if p.is_file():
            out[str(p.relative_to(root))] = p.read_bytes()
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--compose', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    work = Path(tempfile.mkdtemp())
    try:
        build(args, work)
    finally:
        shutil.rmtree(work)


def check_donor(donor):
    """Every file of donor.sha256 in the donor folder, its bytes those."""
    for line in (lib.HERE / 'donor.sha256').read_text().splitlines():
        sha, path = line.split(None, 1)
        f = donor / path.strip()
        if not f.exists() or sha256(f.read_bytes()) != sha:
            sys.exit(f'--donor: {path.strip()} is not the file this pack is made of (sha256 {sha})')


def build(args, work):
    check_donor(args.donor)
    modules = lib.Modules(args.donor)
    variants = {v['name']: v for v in lib.variants(args.live, modules)}
    mounts, fits, texts = read_mounts(), read_fits(), read_texts()
    renames = material_renames()
    weapons = weapon_models(args.live)
    # the reticle model: RETICLE_FROM's dot, centred
    rec = variants[RETICLE_FROM[0]]
    dot_centre = lib.bbox_centre(rec['aim_dir'], RETICLE_FROM[1])
    reticle = work / 'models/srp/reticle_dot.model'
    compose(args.compose, [(rec['aim_dir'], -dot_centre, [RETICLE_FROM[1]])], {}, reticle)
    files, entries, compat, icons = {}, [], [], []
    templates = {}
    for v in variants.values():
        if not v['module'] or len(v['weapons']) != 1:
            continue
        templates.setdefault((v['kind'], v['change'] is not None, v['puts']), v)
    next_id = FIRST_VARIANT
    report = []
    changes = sight_changes(Path(__file__).resolve().parent / 'sights.tsv')

    def donor_module(name):
        d = args.donor / 'models/weapons/modules' / (name + '.model')
        return d if d.exists() else None
    for scope in lib.scopes():
        kind, short = scope['kind'], scope['short']
        idle_src = modules.model(scope['module'])
        aim_src = modules.model(scope['module'], aimed=True) or idle_src
        eye = modules.aim[scope['module_name']]
        first = None
        for w in fits.get(scope['name'], []):
            mount = mounts[w].get(kind) or mounts[w].get('S' if kind == 'C' else 'C')
            ref = variants[mount['variant']]
            t = mount['t']
            name = f'fitn_{short}_{w}'
            ref_dir = 'models/weapons/attach/fit4'
            idle_out = work / f'{ref_dir}/{name}.model'
            aim_out = work / f'{ref_dir}/{name}_aim.model'
            # the weapon's model: its eye at the optic's
            # (a variant's weapon model may differ from the weapon's own beyond its eye: the Kedr's
            # adds its scope_point; a new one is made from the mount variant's)
            cfg, base_model = weapons[w]
            from_model = ref['change'] or base_model
            from_dir = lib.model_dir(args.live, from_model)
            found = lib.locator(lib.leaves(from_dir / 'render/export_properties'), 'scope_point')
            if found is None:
                sys.exit(f'{w}: its model {from_model} has no scope_point')
            swapped = swap_parts(changes.get(w, []), donor_module, w, from_dir)
            idle_parts = [(idle_src, t, [])] + swapped
            if mount['rails']:
                idle_parts.append((ref['idle_dir'], np.zeros(3), mount['rails']))
            compose(args.compose, idle_parts, renames, idle_out)
            if kind == 'C':
                own = [s for s in lib.surfaces(aim_src) if not s.startswith('scope_lense')]
                aim_parts = [(aim_src, t, own)]
                rails = [s for s in mount['rails'] if s in (lib.surfaces(ref['aim_dir']) if ref['aim_dir'] else [])]
                if rails:
                    aim_parts.append((ref['aim_dir'], np.zeros(3), rails))
                aim_parts.append((reticle, t + np.array([eye[0], eye[1], DOT_DISTANCE]), []))
                aim_parts += swapped
            else:
                # the mount variant's eyepiece (background, lens) with the scope's reticle, its red dot
                # (on the optic's own eye line, 0.69d0's) where that eyepiece's is, both moved so the dot
                # is before the weapon's eye, the gun posed by its idle clip (the eyepiece as the mount
                # variant has it put the A-545's TA01 143 px low, @7)
                ref_aim = ref['aim_dir']
                here = lib.bbox_centre(ref_aim, lib.REDDOT)
                there = lib.bbox_centre(aim_src, lib.REDDOT)
                frame = posed_frame(args.live, cfg, from_dir)
                aim_xy = np.array(lib.locator(lib.leaves(from_dir / 'render/export_properties'), 'aim')[1][:2])
                move = np.zeros(3)
                move[:2] = (aim_xy - posing.transform(here, frame)[:2]) @ np.linalg.inv(frame[:2, :2])
                aim_parts = [(ref_aim, move, [lib.BACKGROUND, lib.EYEPIECE_LENS]),
                             (aim_src, move + here - there, [lib.GRID, lib.REDDOT])]
            compose(args.compose, aim_parts, renames, aim_out)
            wrap_uv(idle_out)
            wrap_uv(aim_out)
            files.update(files_of(idle_out, work))
            files.update(files_of(aim_out, work))
            base_dir = lib.model_dir(args.live, base_model)
            # the weapon's own model only where the mount variant keeps it (the M4A1's mount variants
            # set a flat-top model, without the carry handle the stock one has: seen in the game, the
            # NcSTAR 4 under the handle, @1)
            has_point = lib.locator(lib.leaves(base_dir / 'render/export_properties'), 'scope_point')
            candidates = [(base_model, base_dir)] if has_point and ref['change'] is None else []
            for v in variants.values():
                if v['weapons'] == [w] and v['change']:
                    candidates.append((v['change'], lib.model_dir(args.live, v['change'])))
            candidates = [(m, d) for m, d in candidates if d]
            sighted = changes.get(w, [])
            if kind == 'S':
                # a scope's aimed view is the mount variant's eyepiece: its eye, its weapon model; a
                # model whose aim locator is turned about x tilts the view, and the reticle, set along
                # the bore, shows off the screen's centre (seen in the game, @1: the AK-74M's TA01 10 px,
                # its Leupold 3-7 16 px above): the model again with that locator unturned
                change = ref['change']
                if aim_turned(from_dir) or sighted:
                    # (and with its sights as under an optic, sights.tsv)
                    change = f'{base_model}_n_scope'
                    target = work / 'models' / (change + '.skinned_model')
                    if not target.exists():
                        shutil.copytree(from_dir, target)
                        level_eye(target)
                        under_optic(target, sighted, donor_module, w)
                        files.update(files_of(target, work))
            else:
                needed = collimator_eye(args.live, cfg, from_dir, aim_out)
                # a gun whose sights change under an optic takes a model of ours made so
                change = False if sighted else eye_model(candidates, base_model, needed)
                if change is False:
                    change = f'{base_model}_n_{short}'
                    target = work / 'models' / (change + '.skinned_model')
                    shutil.copytree(from_dir, target)
                    moved_eye(target, needed)
                    under_optic(target, sighted, donor_module, w)
                    files.update(files_of(target, work))
            # the variant's item: made from the mount variant when it is of the scope's kind and both
            # set the weapon's model or neither, else from a variant of ours that sets the scope as
            # the weapon takes it (a stock scope's set, type 0; else an add, type 3) and the model or
            # not (none that keeps the model: one that may sets it to the weapon's own)
            puts = 0 if '/addons/rifle_scope_dict_id' in lib.leaves(args.live / cfg) else 3
            if ref['kind'] == kind and ref['puts'] == puts and (ref['change'] is None) == (change is None):
                template = ref
            else:
                if change is None and (kind, False, puts) not in templates:
                    change = base_model
                template = templates[(kind, change is not None, puts)]
            tbuf = (args.live / 'gameplay/items/scopes' / f'{template["name"]}.options').read_bytes()
            tl = lib.leaves(args.live / 'gameplay/items/scopes' / f'{template["name"]}.options')
            dict_id = next_id
            next_id += 1
            values = {'parameters/modification': (tl['/parameters/modification'], 990000 + dict_id),
                      'modifications[0]/id': (tl['/modifications[0]/id'], 990000 + dict_id),
                      **({'modifications[0]/modifiers[0]/value': (tl['/modifications[0]/modifiers[0]/value'], dict_id)}
                         if puts == 0 else
                         {'modifications[0]/modifiers[0]/value[1]': (tl['/modifications[0]/modifiers[0]/value[1]'], dict_id)}),
                      'data/fov_factor': (tl['/data/fov_factor'], scope['fov']),
                      'ui_desc/icon[0]': (tl['/ui_desc/icon[0]'], scope['icon'][0]),
                      'ui_desc/icon[1]': (tl['/ui_desc/icon[1]'], scope['icon'][1])}
            strings = {tl['/data/idle_model']: f'weapons/attach/fit4/{name}',
                       tl['/data/aimed_model']: f'weapons/attach/fit4/{name}_aim',
                       tl['/srp_scope_fit/scope']: scope['name'],
                       tl['/srp_scope_fit/weapons[0]']: w,
                       tl['/ui_desc/text_descriptions/name']: f'st_{scope["name"]}_name',
                       tl['/ui_desc/text_descriptions/description']: f'st_{scope["name"]}_description'}
            if change:
                strings[template['change']] = change
            path = f'gameplay/items/scopes/{name}.options'
            files[path] = edited_config(tbuf, values, strings)
            entries.append({'base_cost': 0, 'cfg_name': path, 'dict_id': dict_id, 'is_premium': False,
                            'is_stack': False, 'item_category': CATEGORY, 'name': name})
            compat.append((w, scope['dict_id']))
            first = first or name
            report.append(f'{scope["name"]}\t{w}\t{dict_id}\t{change or "-"}')
        if not first:
            sys.exit(f'{scope["name"]}: fits no weapon')
        # the shop item
        tname = SHOP_TEMPLATE[kind]
        tpath = args.live / 'gameplay/items/scopes' / f'{tname}.options'
        tl = lib.leaves(tpath)
        values = {'parameters/modification': (tl['/parameters/modification'], 991000 + scope['dict_id']),
                  'modifications[0]/id': (tl['/modifications[0]/id'], 991000 + scope['dict_id']),
                  'data/fov_factor': (tl['/data/fov_factor'], scope['fov']),
                  'ui_desc/icon[0]': (tl['/ui_desc/icon[0]'], scope['icon'][0]),
                  'ui_desc/icon[1]': (tl['/ui_desc/icon[1]'], scope['icon'][1])}
        strings = {tl['/data/idle_model']: f'weapons/attach/fit4/{first}',
                   tl['/data/aimed_model']: f'weapons/attach/fit4/{first}_aim',
                   tl['/ui_desc/text_descriptions/name']: f'st_{scope["name"]}_name',
                   tl['/ui_desc/text_descriptions/description']: f'st_{scope["name"]}_description'}
        path = f'gameplay/items/scopes/{scope["name"]}.options'
        files[path] = edited_config(tpath.read_bytes(), values, strings)
        entries.append({'base_cost': scope['price'], 'cfg_name': path, 'dict_id': scope['dict_id'],
                        'is_premium': False, 'is_stack': False, 'item_category': CATEGORY, 'name': scope['name']})
        row, col = scope['icon']
        source = f'icons/{ICON_ATLAS}/r{row:02}c{col:02}.png'
        (args.out / source).parent.mkdir(parents=True, exist_ok=True)
        picture = args.donor / 'icons' / f'{scope["module_name"]}.png'
        render_icon.icon(idle_src, [args.donor, args.live], picture=picture if picture.exists() else None,
                         renames=renames).save(args.out / source)
        icons.append({'row': row, 'col': col, 'source': source})
    # the donor's materials and textures the pack needs
    for p in sorted(args.donor.rglob('*')):
        rel = str(p.relative_to(args.donor))
        if p.is_file() and (rel.startswith('material_instances/') or rel.startswith('textures/')):
            files[rel] = p.read_bytes()
    # their textures as 0.26g draws a 0.69d0 weapon's (@4; pbr_026.py)
    files.update(pbr_026.convert({k: v for k, v in files.items() if k.startswith('textures/')}))
    check_textures(files, args.live)
    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    keys = sorted({f'st_{s["name"]}_{k}' for s in lib.scopes() for k in ('name', 'description')})
    for language in LANGUAGES:
        operations.append({'op': 'localization_add', 'languages': [language],
                           'strings': {key: porter_text(texts[key][language]) for key in keys}})
    operations.append({'op': 'atlas_icons_add', 'swf': 'flash_movies/lobby_menu.swf', 'symbol': ICON_ATLAS,
                       'icons': icons})
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': 'rifle scopes from 0.69d0 (srp-026g/docs/notes/weapon_scopes.md, section 6): '
                       + ', '.join(f'{s["dict_id"]} {s["short"]}' for s in lib.scopes()),
        'operations': operations,
        'server': {'items': [{'dict_id': e['dict_id']} for e in entries]},
        'notes': ['made by srp-026g/scripts/rifle-scopes/make_pack.py from scopes.tsv, mounts.tsv, fit.tsv, '
                  'materials.tsv, texts.tsv and donor.sha256', 'donor: Survarium 0.69d0, prepare_donor.sh'],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (lib.HERE / 'variants.tsv').write_text('# made by make_pack.py: each variant, the weapon model it sets (- the weapon\'s own)\n'
                                           '# scope\tweapon\tdict id\tweapon model\n' + '\n'.join(report) + '\n',
                                           encoding='utf-8')
    write_edit_list(args.live, compat, weapons)
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, {len(entries)} items, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


SLOTS = (('texture_diffuse', 'use_tdiffuse', 1), ('texture_normal', 'use_nmap', 0),
         ('texture_roughness', 'use_troughness', 0), ('texture_fresnel', 'use_tfresnel', 0))


def check_textures(files, live):
    """Every model of the pack draws: each surface's material is in the pack or the players' files
    (--live's paths.txt), and each texture a g-buffer material of the pack or of --live binds (its
    use_ switch on) is too; a texture of the pack is a DDS 0.26g reads (DXT1 or DXT5, its mips and
    the sRGB byte: 1 for a diffuse, else 0; a normal map DXT5). Stops the build, listing each one
    that is not (the owner, 2026-11-01: "some scopes are completely untextured")."""
    have = set((Path(live) / 'paths.txt').read_text(encoding='utf-8').split())
    problems, notes, seen = [], [], set()

    def material_leaves(name):
        """The material's values; None when only its name is known (in the players' files); False
        when it is nowhere."""
        path = f'material_instances/{name}.material'
        if path in files:
            return lib.leaves_of(files[path])
        if (Path(live) / path).exists():
            return lib.leaves(Path(live) / path)
        return None if path in have else False
    for path in sorted(files):
        if not path.endswith('/settings') or '/models/' not in '/' + path:
            continue
        for key, name in lib.leaves_of(files[path]).items():
            if not key.endswith('/material_name') or name in seen:
                continue
            seen.add(name)
            leaves = material_leaves(name)
            if leaves is False:
                problems.append(f'{path}: material {name} is in neither the pack nor the players\' files')
                continue
            if leaves is None or '/material/g_stage/effect/effect_id' not in leaves:
                continue
            for slot, switch, srgb in SLOTS:
                if not leaves.get(f'/material/g_stage/effect/{switch}/value'):
                    continue
                texture = leaves.get(f'/material/g_stage/effect/{slot}/value', '')
                dds = f'textures/{texture}.dds'
                if dds in files:
                    try:
                        made = pbr_026.Dds(files[dds])
                    except ValueError as e:
                        problems.append(f'{name}: {slot} {dds}: {e}')
                        continue
                    if made.srgb != srgb or (slot == 'texture_normal') != (bytes(files[dds][84:88]) == b'DXT5'):
                        problems.append(f'{name}: {slot} {dds}: sRGB byte {made.srgb}, {bytes(files[dds][84:88])!r}')
                elif not texture or dds not in have:
                    line = f'{name}: {slot} {texture!r} is in neither the pack nor the players\' files'
                    # a players' material as they have it (stock 0.26g's weapons/vepr12_attach names a
                    # vepr_attach_nmap no archive has): noted, the pack does not make it worse
                    (problems if f'material_instances/{name}.material' in files else notes).append(line)
    for line in notes:
        print(f'textures: as the players have it: {line}')
    if problems:
        sys.exit('textures:\n  ' + '\n  '.join(problems))
    print(f'textures: the {len(seen)} materials the pack\'s models draw have their textures')


def write_edit_list(live, compat, weapons):
    ids = {}
    d = lib.leaves(Path(live) / 'gameplay/db_static_dictionaries')
    i = 0
    while f'/items_dict[{i}]/name' in d:
        ids[d[f'/items_dict[{i}]/name']] = d[f'/items_dict[{i}]/dict_id']
        i += 1
    rows = [{'files': ['gameplay/db_static_dictionaries'], 'table': 'items_compatibility',
             'key': f'compatibility_{scope}{ids[w]:05d}', 'value': {'first_item': ids[w], 'second_item': scope}}
            for w, scope in sorted(compat, key=lambda p: (p[1], ids[p[0]]))]
    compm4 = [COMPM4] + sorted(v.stem for v in (Path(live) / 'gameplay/items/scopes').glob('*.options')
                               if lib.leaves(v).get('/srp_scope_fit/scope') == COMPM4)
    sets = [{'file': f'gameplay/items/scopes/{n}.options', 'path': 'data/fov_factor', 'from': COMPM4_FOV[0],
             'to': COMPM4_FOV[1]} for n in compm4]
    about = ("The rifle scopes (the owner, 2026-10-26; pack rifle-scopes; srp-026g/docs/notes/weapon_scopes.md, "
             f"section 6): {len(rows)} items_compatibility rows, each weapon and new scope it fits (the lobby movie's "
             "scope pairs; scripts/rifle-scopes/fit.tsv), and the Aimpoint CompM4 at x2 as 0.46c2 and 0.69d0 have it: "
             f"data/fov_factor 0.8 -> 0.5 on its item and its {len(compm4) - 1} variants (its name with x2 is in "
             "client-patches/localization). Made by scripts/rifle-scopes/make_pack.py.")
    lines = ['{', ' "_about": ' + json.dumps(about, ensure_ascii=False) + ',']
    for op, entries in (('set', sets), ('add_value', rows)):
        lines.append(f' "{op}": [')
        lines += ['  ' + json.dumps(e, ensure_ascii=False) + (',' if i < len(entries) - 1 else '')
                  for i, e in enumerate(entries)]
        lines.append(' ],' if op != 'add_value' else ' ]')
    lines.append('}')
    (lib.HERE.parent / 'content-edit' / 'rifle-scopes-1.json').write_text('\n'.join(lines) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
