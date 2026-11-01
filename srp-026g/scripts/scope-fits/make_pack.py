#!/usr/bin/env python3
"""make_pack.py: the scope fits pack: every scope we sell on the release-41 guns 0.69d0 gives it to,
and the Walther Competition II as a shop scope (srp-026g/docs/notes/weapon_scopes.md, section 7).

usage: python3 -I make_pack.py --donor <folder> --live <folder> --guns <folder> [--guns ...] \\
           --compose <vostok-port> --out <packs folder>/scope-fits-<version>

--donor    prepare_donor.sh's folder (0.69d0's optics, rails and game.options; donor.sha256).
--live     prepare_live.sh's folder of the players' files with the rifle scopes in (our scope
           items and their variants, the weapons' configs and models, the attachments' materials).
--guns     a weapon pack's files/ folder (the release-41 guns' configs and models; guns.sha256).
--compose  a resource-porter vostok-port with `compose-model`.
--out      the pack's folder (a new one).

Each variant (a scope item made for one gun, as patch 19's and the rifle scopes' are) is made of a
reference of its scope, moved from the reference's mount to the gun's:
- **the reference**: a variant of ours of that scope (the same models, materials, red dot and
  eyepiece players know), the 0.69d0 module's mesh found in it (exactly, or by its bounding box
  where our mesh is the same shape written in another order: the Kobra, the DeltaPoint, the old
  PSO-1), so its translation and the eye's offset from the optic's origin are known. The Walther
  Competition II has no variant of ours: its reference is 0.69d0's module, drawn with the HK416's
  stock Walther material (the same mesh and UVs, turned) and the red dot of ours at its eye, 0.6 m
  on, as the rifle scopes' collimators are made;
- **the mount** (survey.py, mounts.tsv): the translation (the scope frame: the weapon draws a scope
  at scope_point turned 180 degrees about Y) putting the optic's origin where 0.69d0 puts it on the
  gun, with the rails 0.69d0 puts under it (modules.tsv's, 0.69d0's meshes at their sockets) or, on
  a live weapon (the Walther), our mount's (../rifle-scopes/mounts.tsv: its translation and rails);
- **the models**: the reference's optic surfaces moved by the difference, with the gun's rails; a
  collimator's aimed model too (its red dot with it); a scope's aimed model (its eyepiece) moved so
  it sits before the gun's eye as it sits before the reference's;
- **the gun's model**: a collimator's eye on the optic's sight line: an existing model with that
  eye (0.3 mm), else a copy of the gun's model with its `aim` locator moved
  (models/weapons/<..>_n_<scope>); a scope keeps the gun's own model (the weapon is hidden when
  aiming; the eyepiece sits before the gun's own eye), or a copy with its eye levelled
  (<..>_n_optic); either copy with its iron sights as under an optic (sights.tsv: folded, cut);
- **the check**: nothing of the gun's model or the rails in the variant's line of sight
  (occlusion.py; occlusion.tsv), a release-41 gun's variant failing it stops the build;
- **its item**: the config of a type-3 variant of ours of the same form (the reference's own when
  it is one), its ids, models, fit and gun changed, and its data and texts the scope's (read back,
  nothing else differs); dict ids from FIRST_VARIANT; its dictionary row.
The Walther Competition II's shop item (optics.tsv) is made from the CompM4's, with the texts our
HK416's stock Walther already has; its icon rendered from 0.69d0's model.
The edit list (../content-edit/scope-fits-1.json): an `addons { mount: 1 }` placeholder in each gun
that takes a scope and has no `addons` (a type-3 modifier adds only to a table with a child,
weapon_scopes.md section 2), and an items_compatibility row for each gun and scope it fits.
"""
import argparse
import hashlib
import importlib.util
import json
import shutil
import sys
import tempfile
import zlib
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import pbr_026  # noqa: E402
import render_icon  # noqa: E402
import scopes_lib as lib  # noqa: E402


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rs = _load('rifle_scopes_make_pack', HERE.parent / 'rifle-scopes' / 'make_pack.py')  # compose, configs, eyes
survey = _load('scope_fits_survey', HERE / 'survey.py')
occlusion = _load('scope_fits_occlusion', HERE / 'occlusion.py')
sights = _load('scope_fits_sights', HERE / 'sights.py')

PACK_ID = 'scope-fits'
VERSION = '3'
FIRST_VARIANT = 2901
LAST_VARIANT = 3299
CATEGORY = 22
ICON_ATLAS = 'LobbyMenu_AmmoIcons'
BBOX_TOLERANCE = 0.0002
DOT_DISTANCE = 0.6
PREFERRED = ('ak_12', 'm4a1', 'a_545', 'ash_12', 'hk416', 'fn_fnc', 'msbs_carbine', 'ak_74m', 'akmn', 'svd', 'vss',
             'toz_122', 'rem_700')
SHOP_TEMPLATE = 'shop_scope_aimpoint_compm4'
# the HK416's stock Walther: the Walther Competition II's material and texts
STOCK_WALTHER = 'walther_comp2'
WALTHER_MATERIAL = 'weapons/attach/walther_comp2/walther_comp2'


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def check_sums(roots, table, what):
    """Every file of `table` (sha256 and path) under one of `roots`, its bytes those."""
    for line in (HERE / table).read_text().splitlines():
        sha, path = line.split(None, 1)
        f = next((Path(r) / path.strip() for r in roots if (Path(r) / path.strip()).exists()), None)
        if f is None or sha256(f.read_bytes()) != sha:
            sys.exit(f'{what}: {path.strip()} is not the file this pack is made of (sha256 {sha}; {table})')


def bbox(model, chosen):
    p = np.concatenate([lib.positions(model / 'render' / s / 'converted_model') for s in chosen])
    return p.min(axis=0), p.max(axis=0)


def translation(model, module_dir):
    """The module's translation in our model, from its surfaces of the same name: each moved alone,
    exactly, else the same bounding box moved (a surface of another shape is not used: our Leupold
    Mk4's body has a bigger turret than 0.69d0's); the body's surfaces, else the lenses (our old PSO-1's
    lens is another disk than 0.69d0's, the Mk4's the same), all that are used agreeing."""
    names = set(lib.surfaces(module_dir))
    found = {False: [], True: []}
    for s in (s for s in lib.surfaces(model) if s in names):
        p = lib.positions(model / 'render' / s / 'converted_model')
        q = lib.positions(module_dir / 'render' / s / 'converted_model')
        lens = s.startswith('scope_lense')
        if len(p) == len(q):
            t = (p - q).mean(axis=0)
            if np.abs(p - q - t).max() < 1e-5:
                found[lens].append(t)
                continue
        (a, b), (c, d) = bbox(model, [s]), bbox(module_dir, [s])
        if np.abs((b - a) - (d - c)).max() <= BBOX_TOLERANCE:
            found[lens].append((a + b) / 2 - (c + d) / 2)
    for ts in (found[False], found[True]):
        if ts and np.abs(np.array(ts) - ts[0]).max() <= BBOX_TOLERANCE:
            return np.array(ts[0])
    return None


EYE_AXIS = 0.0002


def reticle_offset(weapon_model, aimed_model, shift, level=False):
    """(x, y) of an aimed model's reticle (its red dot, else its grid) from the weapon model's eye, in
    the locators' frame (the aimed model drawn at scope_point turned half about Y); None where it has
    no reticle, or (with `level`) the model's eye is turned (the view tilted off that line)."""
    locs = survey.locators(weapon_model)
    sp, aim = locs['scope_point'][0], locs['aim'][0]
    rot = locs['aim'][1] or [0.0, 0.0, 0.0]
    if sp is None or aim is None or (level and max(abs(x) for x in rot) > 1e-6):
        return None
    names = lib.surfaces(aimed_model)
    for surface in (lib.REDDOT, lib.DOT, 'collimator_glowing_dot_green_s', lib.GRID):
        if surface in names:
            c = lib.bbox_centre(aimed_model, surface) + shift
            return (sp + c * np.array([-1.0, 1.0, -1.0]) - aim)[:2]
    return None


def scope_frame_eye(model_dir):
    """A weapon model's eye in the scope frame (from its scope_point, x and z mirrored: the weapon draws a
    scope there turned half about Y), its scope_point, and its aim locator's bone origin (survey.locators'
    frame)."""
    locs = survey.locators(model_dir)
    sp, aim = locs['scope_point'][0], locs['aim'][0]
    if sp is None or aim is None:
        return None
    raw = lib.locator(lib.leaves(model_dir / 'render/export_properties'), 'aim')[1]
    return np.array([sp[0] - aim[0], aim[1] - sp[1], sp[2] - aim[2]]), sp, aim - np.array(raw)


def mount_translation(sp, origin):
    """The scope-frame translation putting an optic's origin at `origin` (the locator frame)."""
    return np.array([sp[0] - origin[0], origin[1] - sp[1], sp[2] - origin[2]])


class Roots:
    """Files by game path: the gun packs' first, then the players'."""

    def __init__(self, guns, live):
        self.roots = [Path(g) for g in guns] + [Path(live)]

    def file(self, path):
        return next((r / path for r in self.roots if (r / path).exists()), None)

    def model(self, model):
        return next((d for d in (lib.model_dir(r, model) for r in self.roots) if d), None)


def model_change(d):
    return next((d[f'/modifications[0]/modifiers[{i}]/value'] for i in range(4)
                 if d.get(f'/modifications[0]/modifiers[{i}]/path[1]') == 'model'), None)


def our_variants(live):
    """Every single-weapon scope variant of ours: config, leaves, scope, weapon, form."""
    out = []
    for cfg in sorted((Path(live) / 'gameplay/items/scopes').glob('*.options')):
        d = lib.leaves(cfg)
        weapons = [d[f'/srp_scope_fit/weapons[{i}]'] for i in range(8) if f'/srp_scope_fit/weapons[{i}]' in d]
        if '/srp_scope_fit/scope' not in d or len(weapons) != 1 or '/data/idle_model' not in d:
            continue
        out.append({'name': cfg.stem, 'cfg': cfg, 'leaves': d, 'scope': d['/srp_scope_fit/scope'], 'weapon': weapons[0],
                    'change': model_change(d), 'puts': d.get('/modifications[0]/modifiers[0]/type'),
                    'kind': 'C' if d.get('/data/hide_weapon_on_aim') is False else 'S'})
    return out


def references(args, work, donor, optics, fits, variants, weapons, roots):
    """Each scope's reference: models, optic surfaces, translation, the eye from the optic's origin,
    its kind and the config leaves its item's data and texts come from."""
    out = {}
    for name, link, module, new_id, *_ in optics:
        if not fits.get(name):
            continue
        mdir = donor.module_dir(f'weapons/modules/{module}/{module.split("/")[-1]}')
        if mdir is None:
            sys.exit(f'{name}: {module} is not in --donor (modules.tsv)')
        if new_id != '-':
            out[name] = new_reference(args, work, mdir, variants)
            continue
        best = None
        for v in variants:
            if v['scope'] != name:
                continue
            idle, aim = lib.model_dir(args.live, v['leaves']['/data/idle_model']), lib.model_dir(args.live, v['leaves']['/data/aimed_model'])
            if idle is None or aim is None:
                continue
            t = translation(idle, mdir)
            if t is None:
                continue
            names = set(lib.surfaces(mdir))
            own = [s for s in lib.surfaces(idle) if s in names]
            rails = [s for s in lib.surfaces(idle) if s not in names]
            eye = scope_frame_eye(roots.model(v['change'] or weapons[v['weapon']][1]))
            if eye is None:
                continue  # its weapon's scope_point or aim hangs on a bone turned otherwise
            e = eye[0]
            rank = (bool(rails), PREFERRED.index(v['weapon']) if v['weapon'] in PREFERRED else len(PREFERRED), v['name'])
            rec = {'idle': idle, 'aim': aim, 'own': own, 'aim_own': [s for s in lib.surfaces(aim) if s not in rails],
                   't': t, 'd': e - t, 'e': e, 'kind': v['kind'], 'leaves': v['leaves'], 'variant': v,
                   'off': reticle_offset(roots.model(v['change'] or weapons[v['weapon']][1]), aim, np.zeros(3))}
            if rec['off'] is None:
                continue
            if best is None or rank < best[0]:
                best = (rank, rec)
        if best is None:
            sys.exit(f'{name}: no variant of ours in which its module is found')
        out[name] = best[1]
    return out


def new_reference(args, work, mdir, variants):
    """A new collimator's reference: 0.69d0's module at the origin, the HK416's stock Walther material
    on it, its aimed model with the red dot of ours at its eye (as the rifle scopes' collimators)."""
    idle_src = mdir
    aim_src = mdir.parent / (mdir.name[:-len('.model')] + '_aim.model')
    aim_src = aim_src if aim_src.exists() else idle_src
    eye = np.array(lib.locator(lib.leaves(mdir / 'render/export_properties'), 'aim')[1])
    material = lib.leaves(mdir / 'settings')
    renames = {material[f'/material_settings/{s}/material_name']: WALTHER_MATERIAL
               for s in lib.surfaces(mdir) if not s.startswith('scope_lense')}
    dot_from = next(v for v in variants if v['name'] == rs.RETICLE_FROM[0])
    dot_dir = lib.model_dir(args.live, dot_from['leaves']['/data/aimed_model'])
    centre = lib.bbox_centre(dot_dir, rs.RETICLE_FROM[1])
    reticle = work / 'models/srp/reticle_dot.model'
    if not reticle.exists():
        rs.compose(args.compose, [(dot_dir, -centre, [rs.RETICLE_FROM[1]])], {}, reticle)
    idle = work / 'models/srp/reference_walther.model'
    aim = work / 'models/srp/reference_walther_aim.model'
    own = [s for s in lib.surfaces(idle_src) if not s.startswith('scope_lense')]
    rs.compose(args.compose, [(idle_src, np.zeros(3), own)], renames, idle)
    aim_own = [s for s in lib.surfaces(aim_src) if not s.startswith('scope_lense')]
    rs.compose(args.compose, [(aim_src, np.zeros(3), aim_own), (reticle, np.array([eye[0], eye[1], DOT_DISTANCE]), [])],
               renames, aim)
    stock = lib.leaves(Path(args.live) / f'gameplay/items/scopes/{STOCK_WALTHER}.options')
    return {'idle': idle, 'aim': aim, 'own': lib.surfaces(idle), 'aim_own': lib.surfaces(aim), 't': np.zeros(3),
            'd': np.array([eye[0], eye[1], 0.0]), 'e': None, 'kind': 'C', 'leaves': stock, 'variant': None,
            'off': np.zeros(2)}


def template_for(ref, kind, needs_model, variants, name):
    """A type-3 variant config of ours of the variant's form (a weapon model set or not): the
    reference's own when it is one, else one of the same scope, else of another of the kind."""
    v = ref['variant']
    if v and v['puts'] == 3 and (v['change'] is not None) == needs_model:
        return v
    same = [x for x in variants if x['puts'] == 3 and x['kind'] == kind and (x['change'] is not None) == needs_model]
    same.sort(key=lambda x: (x['scope'] != name, x['scope'] != SHOP_TEMPLATE, x['name']))
    if not same:
        sys.exit(f'{name}: no type-3 {kind} variant of ours {"setting" if needs_model else "keeping"} a weapon model')
    return same[0]


def variant_config(template, source, name, gun, dict_id, vname, change, fov, icon):
    """`template`'s config made the variant's: its ids, models, fit and gun, the weapon model it sets;
    its data numbers and texts `source`'s (the scope's own: its reference's config or shop item)."""
    t = template['leaves']
    values = {'parameters/modification': (t['/parameters/modification'], 990000 + dict_id),
              'modifications[0]/id': (t['/modifications[0]/id'], 990000 + dict_id),
              'modifications[0]/modifiers[0]/value[1]': (t['/modifications[0]/modifiers[0]/value[1]'], dict_id),
              'data/fov_factor': (t['/data/fov_factor'], fov),
              'ui_desc/icon[0]': (t['/ui_desc/icon[0]'], icon[0]), 'ui_desc/icon[1]': (t['/ui_desc/icon[1]'], icon[1])}
    for key in ('type', 'near_plane_factor', 'change_scope_factor'):
        if f'/data/{key}' in source:
            if f'/data/{key}' not in t:
                sys.exit(f'{vname}: the template {template["name"]} has no data/{key}')
            values[f'data/{key}'] = (t[f'/data/{key}'], source[f'/data/{key}'])
    strings = {t['/data/idle_model']: f'weapons/attach/fit5/{vname}',
               t['/data/aimed_model']: f'weapons/attach/fit5/{vname}_aim',
               t['/srp_scope_fit/scope']: name,
               t['/srp_scope_fit/weapons[0]']: gun,
               t['/ui_desc/text_descriptions/name']: source['/ui_desc/text_descriptions/name'],
               t['/ui_desc/text_descriptions/description']: source['/ui_desc/text_descriptions/description']}
    if template['change']:
        strings[template['change']] = change
    strings = {k: v for k, v in strings.items() if k != v}
    values = {k: v for k, v in values.items() if v[0] != v[1]}
    return rs.edited_config(template['cfg'].read_bytes(), values, strings)


def check_variant(data, name, gun, dict_id, vname, change, kind):
    tmp = Path(tempfile.mkdtemp()) / 'v.options'
    tmp.write_bytes(data)
    d = lib.leaves(tmp)
    shutil.rmtree(tmp.parent)
    problems = []
    if d.get('/srp_scope_fit/scope') != name:
        problems.append('scope')
    if [d.get(f'/srp_scope_fit/weapons[{i}]') for i in range(2)] != [gun, None]:
        problems.append('weapons')
    if d.get('/modifications[0]/modifiers[0]/type') != 3 or d.get('/modifications[0]/modifiers[0]/value[1]') != dict_id:
        problems.append('modifier')
    if d.get('/modifications[0]/modifiers[0]/value[0]') != 'rifle_scope_dict_id':
        problems.append('modifier key')
    if d.get('/parameters/modification') != d.get('/modifications[0]/id'):
        problems.append('modification id')
    if d.get('/data/idle_model') != f'weapons/attach/fit5/{vname}' or d.get('/data/aimed_model') != f'weapons/attach/fit5/{vname}_aim':
        problems.append('models')
    if model_change(d) != change:
        problems.append(f'weapon model {model_change(d)}')
    if (d.get('/data/hide_weapon_on_aim') is False) != (kind == 'C'):
        problems.append('kind')
    if problems:
        sys.exit(f'{vname}: {", ".join(problems)}')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--guns', required=True, type=Path, action='append')
    parser.add_argument('--compose', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out}: already there')
    check_sums([args.donor], 'donor.sha256', '--donor')
    check_sums(args.guns, 'guns.sha256', '--guns')
    work = Path(tempfile.mkdtemp())
    try:
        build(args, work)
    finally:
        shutil.rmtree(work)


def mounts_new():
    out = {}
    for gun, optic, slot, origin, rails in survey.rows('mounts.tsv'):
        rr = []
        if rails != '-':
            for r in rails.split():
                model, at = r.split('@')
                rr.append((model, np.array([float(x) for x in at.split(',')])))
        out[(gun, optic)] = (np.array([float(x) for x in origin.split(',')]), rr)
    return out


def build(args, work):
    donor = survey.Donor(args.donor)
    roots = Roots(args.guns, args.live)
    optics = survey.rows('optics.tsv')
    fits = {name: guns.split() for name, *rest in survey.rows('fit.tsv') for guns in [rest[0] if rest else '']}
    weapons = rs.weapon_models(args.live)
    for gun, *_ in survey.rows('guns.tsv'):
        cfg = f'gameplay/items/weapons/wpn_{gun}.options'
        if roots.file(cfg) is None:
            sys.exit(f'{gun}: no {cfg} in --guns')
        weapons[gun] = (cfg, lib.leaves(roots.file(cfg))['/object/model'])
    changes = sight_changes()
    new_guns = {g for g, *_ in survey.rows('guns.tsv')}
    variants = our_variants(args.live)
    refs = references(args, work, donor, optics, fits, variants, weapons, roots)
    new_mounts = mounts_new()
    live_mounts = rs.read_mounts()
    live_variants = {v['name']: v for v in lib.variants(args.live, lib.Modules(args.donor))}
    renames = {r[2]: 'weapons/loc2/' + r[3][len('loc2:'):] for r in survey.rows('modules.tsv')
               if r[1] == 'rail' and r[3].startswith('loc2:')}
    files, entries, compat, report, icons, axis, seen = {}, [], [], [], [], [], []
    eyes = {}  # gun -> [(model reference, its directory)]: the models with a moved eye made here
    next_id = FIRST_VARIANT
    placeholders = []
    for name, link, module, new_id, price, row, col, fov in optics:
        guns = fits.get(name, [])
        if not guns:
            continue
        ref = refs[name]
        kind = ref['kind']
        short = name.removeprefix('shop_scope_')
        if new_id == '-':
            shop = lib.leaves(Path(args.live) / f'gameplay/items/scopes/{name}.options')
            source, scope_fov = shop, shop['/data/fov_factor']
            icon = (shop['/ui_desc/icon[0]'], shop['/ui_desc/icon[1]'])
        else:
            source, scope_fov, icon = ref['leaves'], float(fov), (int(row), int(col))
        first = None
        for gun in guns:
            cfg, base_model = weapons[gun]
            base_dir = roots.model(base_model)
            from_dir = base_dir
            if scope_frame_eye(base_dir) is None:
                sys.exit(f'{gun}: its scope_point or aim hangs on a bone turned otherwise')
            e_gun, sp, aim_bone = scope_frame_eye(base_dir)
            if (gun, link) in new_mounts:
                origin, rails = new_mounts[(gun, link)]
                t = mount_translation(sp, origin)
                rail_parts = [(donor.module_dir(m), mount_translation(sp, at), []) for m, at in rails]
                aim_rail_parts = rail_parts
            else:
                mount = live_mounts[gun].get(kind) or live_mounts[gun].get('S' if kind == 'C' else 'C')
                mv = live_variants[mount['variant']]
                t = mount['t']
                rail_parts = [(mv['idle_dir'], np.zeros(3), mount['rails'])] if mount['rails'] else []
                aim_r = [s for s in mount['rails'] if mv['aim_dir'] and s in lib.surfaces(mv['aim_dir'])]
                aim_rail_parts = [(mv['aim_dir'], np.zeros(3), aim_r)] if aim_r else []
                # a live weapon's eye moves on its mount variant's model (the Kedr's adds its scope_point)
                from_dir = roots.model(mv['change'] or base_model)
                if scope_frame_eye(from_dir) is None:
                    sys.exit(f'{gun}: its scope_point or aim hangs on a bone turned otherwise')
                e_gun, sp, aim_bone = scope_frame_eye(from_dir)
            shift = t - ref['t']
            vname = f'fitn_{short}_{gun}'
            idle_out = work / f'models/weapons/attach/fit5/{vname}.model'
            aim_out = work / f'models/weapons/attach/fit5/{vname}_aim.model'
            rs.compose(args.compose, [(ref['idle'], shift, ref['own'])] + rail_parts, renames, idle_out)
            if kind == 'C':
                eye = t + ref['d']
                aim_parts = [(ref['aim'], shift, ref['aim_own'])] + aim_rail_parts
            else:
                eye = e_gun
                aim_parts = [(ref['aim'], eye - ref['e'], ref['aim_own'])]
            rs.compose(args.compose, aim_parts, renames, aim_out)
            rs.wrap_uv(idle_out)  # the aperture sight's texture coordinates past 1 (the rifle scopes' @3)
            rs.wrap_uv(aim_out)
            files.update(rs.files_of(idle_out, work))
            files.update(rs.files_of(aim_out, work))
            change = None
            sighted = changes.get(gun, [])
            if kind == 'S' and (sighted or aim_turned(from_dir)):
                # the gun's model with its sights as under an optic (sights.tsv) and its eye looking
                # straight along the bore (an aim locator turned about x tilts the view: the eyepiece,
                # set before the eye along the bore, then shows its reticle off the screen's centre,
                # where the bullets go; seen in the game on the AK-74M, 10 px at x4), its eye its own
                change = f'{base_model}_n_optic'
                target = work / 'models' / (change + '.skinned_model')
                if not target.exists():
                    shutil.copytree(from_dir, target)
                    level_eye(target)
                    under_optic(target, sighted, donor, gun)
                    files.update(rs.files_of(target, work))
            if kind == 'C':
                # the aim locator's own position: the eye less its bone's origin
                needed = (sp[0] - eye[0] - aim_bone[0], sp[1] + eye[1] - aim_bone[1])
                has_point = lib.locator(lib.leaves(base_dir / 'render/export_properties'), 'scope_point')
                candidates = ([(base_model, base_dir)] if has_point and not sighted else []) + eyes.get(gun, [])
                candidates += [(v['change'], lib.model_dir(args.live, v['change'])) for v in live_variants.values()
                               if v['weapons'] == [gun] and v['change'] and not sighted]
                candidates = [(m, d) for m, d in candidates if d and not aim_turned(d)]
                change = rs.eye_model(candidates, base_model, needed)
                if change is False:
                    change = f'{base_model}_n_{short}'
                    target = work / 'models' / (change + '.skinned_model')
                    shutil.copytree(from_dir, target)
                    rs.moved_eye(target, needed)
                    under_optic(target, sighted, donor, gun)
                    files.update(rs.files_of(target, work))
                    eyes.setdefault(gun, []).append((change, target))
                elif change is None:
                    change = base_model
            gl = lib.leaves(roots.file(cfg))
            if '/addons/rifle_scope_dict_id' in gl:
                sys.exit(f'{gun}: has a stock scope; this pack adds scopes to weapons without one')
            if not any(k.startswith('/addons/') for k in gl) and gun not in placeholders:
                placeholders.append(gun)
            dict_id = next_id
            next_id += 1
            if dict_id > LAST_VARIANT:
                sys.exit(f'more variants than the claimed ids {FIRST_VARIANT}..{LAST_VARIANT}')
            template = template_for(ref, kind, change is not None, variants, name)
            path = f'gameplay/items/scopes/{vname}.options'
            files[path] = variant_config(template, source, name, gun, dict_id, vname, change, scope_fov, icon)
            check_variant(files[path], name, gun, dict_id, vname, change, kind)
            # the eye on the scope's axis: the reticle before the weapon model's eye as before the reference's
            eye_model = work / 'models' / (change + '.skinned_model') if change and (work / 'models' / (change + '.skinned_model')).exists() \
                else roots.model(change or base_model)
            off = reticle_offset(eye_model, aim_out, np.zeros(3), level=True)
            if off is None or np.abs(off - ref['off']).max() > EYE_AXIS:
                sys.exit(f'{vname}: its reticle is {off} from the eye, the reference\'s {ref["off"]}')
            axis.append(f'{vname}\t{off[0] * 1000:+.2f}\t{off[1] * 1000:+.2f}\t{ref["off"][0] * 1000:+.2f}\t{ref["off"][1] * 1000:+.2f}')
            # nothing of the gun or its rails in the line of sight (occlusion.py): a release-41 gun's
            # variant must pass; a live weapon's (the Walther's) is as its live variants are (recorded)
            ok, wmin, wmax, clear, hit = occlusion.variant_check(
                eye_model, idle_out, aim_out, kind, ref['own'], [s for s in lib.surfaces(idle_out) if s not in ref['own']])
            result = 'pass' if ok and wmax > 0 else 'FAIL' if gun in new_guns else 'live'  # a shut window checks nothing
            seen.append(f'{vname}\t{kind}\t{result}\t{wmin:.1f}\t{wmax:.1f}\t{clear:.1f}\t{occlusion.describe(hit)}')
            if result == 'FAIL':
                sys.exit(f'{vname}: the gun stands in its line of sight: {occlusion.describe(hit)} (clear {clear:.1f} of its '
                         f'window, {wmin:.1f} to {wmax:.1f} degrees)')
            entries.append({'base_cost': 0, 'cfg_name': path, 'dict_id': dict_id, 'is_premium': False,
                            'is_stack': False, 'item_category': CATEGORY, 'name': vname})
            compat.append((gun, name))
            first = first or vname
            report.append(f'{name}\t{gun}\t{dict_id}\t{change or "-"}\t{",".join("%.5f" % x for x in t)}')
        if new_id != '-':
            icons.append(shop_item(args, files, entries, name, int(new_id), int(price), icon, scope_fov, first,
                                   ref['leaves'], ref['idle']))
    (HERE / 'eyes.tsv').write_text('# made by make_pack.py: each variant\'s reticle (red dot, else grid) from its weapon model\'s eye, '
                                   'mm (x, y), and its reference\'s: equal within EYE_AXIS\n# variant\tx\ty\treference x\treference y\n'
                                   + '\n'.join(axis) + '\n', encoding='utf-8')
    (HERE / 'occlusion.tsv').write_text(
        '# made by make_pack.py (occlusion.py): each variant\'s line of sight; window: a collimator\'s, degrees about its red dot\n'
        '# from its weapon model\'s eye (the optic\'s body bounds it), a scope\'s, its eyepiece\'s radius in mm (rays along its\n'
        '# axis); clear: the fraction of the window nothing of the gun or its rails stands in (1.0 none in 0.9); hits inside\n'
        '# CLEAR, each where it is from the eye (metres); result pass, FAIL (a release-41 gun: the build stops) or live (a live\n'
        '# weapon: as its live variants are)\n# variant\tkind\tresult\twindow min\twindow max\tclear\tsurfaces hit\n'
        + '\n'.join(seen) + '\n', encoding='utf-8')
    write_pack(args, files, entries, report, icons)
    write_edit_list(args.live, compat, weapons, placeholders, roots)


def aim_turned(model):
    """Whether a model's aim locator is turned (the view through it tilted)."""
    rotation = lib.locator(lib.leaves(model / 'render/export_properties'), 'aim')[2]
    return bool(rotation) and max(abs(x) for x in rotation) > 1e-6


def level_eye(model):
    """A copied model's aim locator unturned, where it is."""
    path = model / 'render/export_properties'
    buf = bytearray(path.read_bytes())
    i, _, rotation = lib.locator(lib.leaves(path), 'aim')
    if rotation is not None:
        rs.set_float3(buf, f'locators[{i}]/rotation', [0.0, 0.0, 0.0])
    path.write_bytes(bytes(buf))


def remove_key(buf, table, key):
    """The child `key` of the named table `table` taken out: the later children's records move down one
    place (the table stays in name-CRC order), its count drops by one."""
    ce = rs.ce
    off = ce.find(buf, table)
    kids = ce.children(buf, off)
    crc = zlib.crc32(key.encode()) & 0xffffffff
    hit = [i for i, c in enumerate(kids) if ce.rec(buf, c)[2] == crc]
    if len(hit) != 1:
        sys.exit(f'{table}: {len(hit)} children named {key}, not 1')
    for dst, src in zip(kids[hit[0]:], kids[hit[0] + 1:]):
        buf[dst:dst + 24] = buf[src:src + 24]
    d, i, c, t, cnt = ce.rec(buf, off)
    ce.REC.pack_into(buf, off, d, i, c, t, cnt - 1)


def leave_out(model, surfaces):
    """A copied model without `surfaces`: out of its LOD lists and its render folder."""
    if not surfaces:
        return
    path = model / 'settings'
    buf = bytearray(path.read_bytes())
    d = lib.leaves(path)
    for s in surfaces:
        lists = sorted({k.rsplit('[', 1)[0] for k, v in d.items() if k.startswith('/lod_hierrarchy/') and v == s})
        if not lists:
            sys.exit(f'{model}: no surface {s}')
        for lst in lists:
            rs.ce.remove_item(buf, lst.lstrip('/'), s)
        remove_key(buf, 'material_settings', s)
        shutil.rmtree(model / 'render' / s)
    path.write_bytes(bytes(buf))
    left = lib.leaves(path)
    if any(v in surfaces for k, v in left.items() if k.startswith('/lod_hierrarchy/')) or \
            any(k.startswith(f'/material_settings/{x}/') for k in left for x in surfaces):
        sys.exit(f'{model}: a surface left out is still listed')


def sight_changes():
    """sights.tsv's changes under an optic, per gun: (how, surface, the rest), `keep` left out."""
    out = {}
    for row in survey.rows('sights.tsv'):
        how = row[-1].split()
        if how[0] == 'keep':
            continue
        if len(how) not in {'fold': (4,), 'cut': (2, 3), 'lower': (4,)}.get(how[0], ()):
            sys.exit(f'sights.tsv: {row[0]}: {row[-1]!r} is not keep, fold <surface> <up> <folded>, cut <surface> [box] '
                     'or lower <surface> <box> <height>')
        out.setdefault(row[0], []).append(how)
    return out


def under_optic(model, changes, donor, gun):
    """A copied weapon model's sights as sights.tsv has them under an optic: folded (sights.fold: the
    up module the port baked in made 0.69d0's folded one), cut (sights.cut: a surface's islands
    inside a box; leave_out: a whole surface), lowered (sights.lower: a part of an island cut flat)."""
    for how, surface, *rest in changes:
        if how == 'cut' and not rest:
            leave_out(model, [surface])
            done = 'left out'
        elif how == 'cut':
            done = sights.cut(model / 'render' / surface, [float(x) for x in rest[0].split(',')])
        elif how == 'lower':
            done = sights.lower(model / 'render' / surface, [float(x) for x in rest[0].split(',')], float(rest[1]))
        else:
            up, folded = (donor.module_dir(f'weapons/modules/{m}') for m in rest)
            if up is None or folded is None:
                sys.exit(f'{gun}: {rest} not in --donor (modules.tsv)')
            names = lib.surfaces(up)
            if len(names) != 1 or lib.surfaces(folded) != names:
                sys.exit(f'{gun}: {rest}: not one surface each, the same')
            done = sights.fold(model / 'render' / surface, up / 'render' / names[0] / 'converted_model',
                               folded / 'render' / names[0] / 'converted_model')
        print(f'{gun}: {model.name}: {how} {surface} {" ".join(rest)}: {done}')


def shop_item(args, files, entries, name, dict_id, price, icon, fov, first, texts, model):
    tpath = Path(args.live) / f'gameplay/items/scopes/{SHOP_TEMPLATE}.options'
    tl = lib.leaves(tpath)
    values = {'parameters/modification': (tl['/parameters/modification'], 991000 + dict_id),
              'modifications[0]/id': (tl['/modifications[0]/id'], 991000 + dict_id),
              'data/fov_factor': (tl['/data/fov_factor'], fov),
              'ui_desc/icon[0]': (tl['/ui_desc/icon[0]'], icon[0]),
              'ui_desc/icon[1]': (tl['/ui_desc/icon[1]'], icon[1])}
    strings = {tl['/data/idle_model']: f'weapons/attach/fit5/{first}',
               tl['/data/aimed_model']: f'weapons/attach/fit5/{first}_aim',
               tl['/ui_desc/text_descriptions/name']: texts['/ui_desc/text_descriptions/name'],
               tl['/ui_desc/text_descriptions/description']: texts['/ui_desc/text_descriptions/description']}
    path = f'gameplay/items/scopes/{name}.options'
    files[path] = rs.edited_config(tpath.read_bytes(), values, strings)
    entries.append({'base_cost': price, 'cfg_name': path, 'dict_id': dict_id, 'is_premium': False,
                    'is_stack': False, 'item_category': CATEGORY, 'name': name})
    source = f'icons/{ICON_ATLAS}/r{icon[0]:02}c{icon[1]:02}.png'
    (args.out / source).parent.mkdir(parents=True, exist_ok=True)
    render_icon.icon(model, [args.donor, args.live]).save(args.out / source)
    return {'row': icon[0], 'col': icon[1], 'source': source}


def write_pack(args, files, entries, report, icons):
    # the donor's rail materials and the textures they name (prepare_donor.sh: only those the players lack)
    for p in sorted(args.donor.rglob('*')):
        rel = str(p.relative_to(args.donor))
        if p.is_file() and (rel.startswith('material_instances/') or rel.startswith('textures/')):
            files[rel] = p.read_bytes()
    # the rails' 0.69d0 textures as 0.26g draws a 0.69d0 weapon's (the rifle scopes' pbr_026.py)
    files.update(pbr_026.convert({k: v for k, v in files.items() if k.startswith('textures/')}))
    operations = [{'op': 'add_file', 'path': path} for path in sorted(files)]
    operations.append({'op': 'dictionary_add', 'table': 'items_dict', 'entries': entries})
    if icons:
        operations.append({'op': 'atlas_icons_add', 'swf': 'flash_movies/lobby_menu.swf', 'symbol': ICON_ATLAS,
                           'icons': icons})
    pack = {
        'format_version': 1, 'id': PACK_ID, 'version': VERSION, 'target_build': '0.26g',
        'description': 'scope fits (srp-026g/docs/notes/weapon_scopes.md, section 7): every scope we sell on the '
                       'release-41 guns 0.69d0 gives it to, and the Walther Competition II as a shop scope (2120)',
        'operations': operations,
        'server': {'items': [{'dict_id': e['dict_id']} for e in entries]},
        'notes': ['made by srp-026g/scripts/scope-fits/make_pack.py from guns.tsv, optics.tsv, modules.tsv, '
                  'mounts.tsv, fit.tsv, donor.sha256 and guns.sha256', 'donor: Survarium 0.69d0, prepare_donor.sh'],
    }
    for path, data in files.items():
        target = args.out / 'files' / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (args.out / 'pack.json').write_text(json.dumps(pack, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (HERE / 'variants.tsv').write_text('# made by make_pack.py: each variant, the weapon model it sets (- the gun\'s '
                                       'own) and its mount translation (the scope frame, metres)\n'
                                       '# scope\tgun\tdict id\tweapon model\ttranslation\n' + '\n'.join(report) + '\n',
                                       encoding='utf-8')
    digest = sha256((args.out / 'pack.json').read_bytes())
    print(f'{args.out}: {len(files)} files, {len(entries)} items, pack.json sha256 {digest}')
    print(f'manifest row: pack\t{PACK_ID}@{VERSION}\t{digest}\t{pack["description"]}')


def write_edit_list(live, compat, weapons, placeholders, roots):
    ids = {}
    d = lib.leaves(Path(live) / 'gameplay/db_static_dictionaries')
    i = 0
    while f'/items_dict[{i}]/name' in d:
        ids[d[f'/items_dict[{i}]/name']] = d[f'/items_dict[{i}]/dict_id']
        i += 1
    for r in roots.roots[:-1]:
        for op in json.loads((r.parent / 'pack.json').read_text())['operations']:
            if op['op'] == 'dictionary_add':
                for e in op['entries']:
                    ids.setdefault(e['name'], e['dict_id'])
    for name, link, module, new_id, *_ in survey.rows('optics.tsv'):
        if new_id != '-':
            ids[name] = int(new_id)
    rows = [{'files': ['gameplay/db_static_dictionaries'], 'table': 'items_compatibility',
             'key': f'compatibility_{ids[s]}{ids[w]:05d}', 'value': {'first_item': ids[w], 'second_item': ids[s]}}
            for w, s in sorted(compat, key=lambda p: (ids[p[1]], ids[p[0]]))]
    adds = [{'files': [weapons[g][0]], 'table': '', 'key': 'addons', 'value': {'mount': 1}} for g in placeholders]
    about = ("The scope fits (pack scope-fits; srp-026g/docs/notes/weapon_scopes.md, section 7): an addons { mount: 1 } "
             f"placeholder in the {len(adds)} release-41 guns that take a scope and have no addons (a type-3 modifier "
             "adds only to a table with a child), and an items_compatibility row for each gun and scope it fits "
             f"({len(rows)}; the lobby movie's scope pairs; scripts/scope-fits/fit.tsv). Made by "
             "scripts/scope-fits/make_pack.py.")
    lines = ['{', ' "_about": ' + json.dumps(about, ensure_ascii=False) + ',', ' "add_value": [']
    entries = adds + rows
    lines += ['  ' + json.dumps(e, ensure_ascii=False) + (',' if i < len(entries) - 1 else '') for i, e in enumerate(entries)]
    lines += [' ]', '}']
    (HERE.parent / 'content-edit' / 'scope-fits-1.json').write_text('\n'.join(lines) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
