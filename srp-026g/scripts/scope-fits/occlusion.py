#!/usr/bin/env python3
"""occlusion.py: what stands in a scope variant's line of sight (make_pack.py checks every variant
with it; run alone it checks a built pack).

usage: python3 -I occlusion.py --pack <make_pack.py's folder> --live <folder> --guns <folder> [--guns ...] \\
           [--out <tsv>]
       python3 -I occlusion.py --live <folder> --donor <folder> [--out <tsv>]   (the players' own variants)

A collimator's line of sight: from the eye of the variant's weapon model (its `aim` locator, the aimed
view's camera) rays go in a cone about its red dot: DIRECTIONS around, out to WINDOW_CAP degrees. The
optic's own body (its surfaces but the lenses) bounds the window: in each direction the window reaches
to the first ray the body stops. A scope's (the weapon is hidden when aiming): rays along its axis (the
centres of its lens surface's two disks) from its eyepiece's disk, the window that disk. Rays inside
the window are cast against the weapon model's triangles and the rails under the optic: anything of
them hit is seen through the optic. A variant passes when nothing is hit inside CLEAR of its window in
any direction (the reticle's line of sight and the window round it: the gun's rails and handguard may
show at the window's rim, as on a real rifle; a sight standing in it may not).

Frames: the locators' (survey.locators): a skinned weapon's vertices turned half about Y, the optic's
model at scope_point turned half about Y (as the weapon draws a scope).
"""
import argparse
import importlib.util
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import render_icon as ri  # noqa: E402
import scopes_lib as lib  # noqa: E402

_spec = importlib.util.spec_from_file_location('scope_fits_survey', HERE / 'survey.py')
survey = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(survey)

HALF_TURN = np.array([-1.0, 1.0, -1.0])
DIRECTIONS = 24
WINDOW_CAP = 20.0  # degrees: wider than any optic's window seen from its eye
WINDOW_STEP = 0.2  # degrees
FRACTIONS = (0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9)
CLEAR = 0.6  # of the window: nothing of the gun inside it
NEAR = 0.005  # metres: hits nearer the eye than this are the eye's own geometry (none is)
LENS = 'scope_lense_3p_s'
RETICLES = (lib.REDDOT, lib.DOT, 'collimator_glowing_dot_green_s', lib.GRID)


def triangles(model_dir, names, skinned):
    """(triangles (n, 3, 3), their surface names) of `names` of a model, in the locators' frame:
    a skinned weapon's turned half about Y; a static model's as it is (the caller places it)."""
    tris, labels = [], []
    for s in names:
        f = model_dir / 'render' / s / 'converted_model'
        p, _uv, ix = ri.mesh(f.read_bytes())
        if skinned:
            p = p * HALF_TURN
        t = p[ix.astype(np.int64)]
        tris.append(t)
        labels += [s] * len(t)
    if not tris:
        return np.zeros((0, 3, 3)), np.array([], dtype=object)
    return np.concatenate(tris), np.array(labels, dtype=object)


def placed(model_dir, names, sp):
    """A scope model's triangles as the weapon draws it at scope_point (turned half about Y)."""
    t, labels = triangles(model_dir, names, False)
    return sp + t * HALF_TURN, labels


def hits(origin, dirs, tris):
    """For each ray (origin, dirs[i]) the distance to the nearest triangle hit (inf for none) and its
    index (-1). Moeller-Trumbore, both faces."""
    n = len(dirs)
    best = np.full(n, np.inf)
    which = np.full(n, -1)
    if len(tris) == 0:
        return best, which
    v0, e1, e2 = tris[:, 0], tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0]
    s = origin - v0
    q = np.cross(s, e1)
    for i0 in range(0, n, 64):
        d = dirs[i0:i0 + 64]
        h = np.cross(d[:, None, :], e2[None, :, :])
        a = np.einsum('tk,rtk->rt', e1, h)
        ok = np.abs(a) > 1e-12
        f = np.where(ok, 1.0 / np.where(ok, a, 1.0), 0.0)
        u = f * np.einsum('tk,rtk->rt', s, h)
        v = f * np.einsum('rk,tk->rt', d, q)
        t = f * np.einsum('tk,tk->t', e2, q)[None, :]
        good = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > NEAR)
        t = np.where(good, t, np.inf)
        j = t.argmin(axis=1)
        best[i0:i0 + 64] = t[np.arange(len(d)), j]
        which[i0:i0 + 64] = np.where(np.isfinite(best[i0:i0 + 64]), j, -1)
    return best, which


def parallel_hits(origins, direction, tris):
    """For rays from each of `origins` along one `direction`: as hits()."""
    n = len(origins)
    best = np.full(n, np.inf)
    which = np.full(n, -1)
    if len(tris) == 0:
        return best, which
    d = direction / np.linalg.norm(direction)
    v0, e1, e2 = tris[:, 0], tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0]
    h = np.cross(d, e2)
    a = np.einsum('tk,tk->t', e1, h)
    ok = np.abs(a) > 1e-12
    f = np.where(ok, 1.0 / np.where(ok, a, 1.0), 0.0)
    for i0 in range(0, n, 64):
        s = origins[i0:i0 + 64, None, :] - v0[None, :, :]
        u = f * np.einsum('rtk,tk->rt', s, h)
        q = np.cross(s, e1[None, :, :])
        v = f * np.einsum('k,rtk->rt', d, q)
        t = f * np.einsum('tk,rtk->rt', e2, q)
        good = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > NEAR)
        t = np.where(good, t, np.inf)
        j = t.argmin(axis=1)
        best[i0:i0 + 64] = t[np.arange(len(s)), j]
        which[i0:i0 + 64] = np.where(np.isfinite(best[i0:i0 + 64]), j, -1)
    return best, which


def around(axis):
    """Two unit vectors square to `axis` and each other, and the DIRECTIONS angles about it."""
    a = axis / np.linalg.norm(axis)
    u = np.cross(a, [0.0, 1.0, 0.0])
    if np.linalg.norm(u) < 1e-6:
        u = np.cross(a, [1.0, 0.0, 0.0])
    u /= np.linalg.norm(u)
    return a, u, np.cross(a, u), np.arange(DIRECTIONS) * 2 * np.pi / DIRECTIONS


def cone(axis, theta_deg):
    """Ray directions at `theta_deg` from `axis`, DIRECTIONS of them around it."""
    a, u, v, phi = around(axis)
    th = np.radians(np.broadcast_to(theta_deg, (DIRECTIONS,)))
    return (np.cos(th)[:, None] * a + np.sin(th)[:, None] * (np.cos(phi)[:, None] * u + np.sin(phi)[:, None] * v))


def check(eye, axis, optic, gun):
    """`optic` and `gun`: (triangles, labels). Returns the window (degrees, per direction), the
    smallest fraction of the window at which the gun is hit (1.0: none inside FRACTIONS'), and each
    surface hit inside CLEAR with its nearest hit (from the eye, metres: x, y, z)."""
    window = np.full(DIRECTIONS, np.nan)
    for theta in np.arange(0.0, WINDOW_CAP + 1e-9, WINDOW_STEP):
        open_ = np.isnan(window)
        if not open_.any():
            break
        d, _ = hits(eye, cone(axis, theta), optic[0])
        stopped = open_ & np.isfinite(d)
        window[stopped] = max(theta - WINDOW_STEP, 0.0)
    window[np.isnan(window)] = WINDOW_CAP
    first = np.ones(DIRECTIONS)
    seen = {}
    for frac in FRACTIONS:
        dirs = cone(axis, window * frac)
        d, j = hits(eye, dirs, gun[0])
        hit = np.isfinite(d)
        first[hit & (first == 1.0)] = frac
        if frac < CLEAR:
            for k in np.where(hit)[0]:
                name = gun[1][j[k]]
                if name not in seen or d[k] < np.linalg.norm(seen[name]):
                    seen[name] = d[k] * dirs[k]
    return window, float(first.min()), seen


def check_scope(eyepiece, radius, axis, gun):
    """A scope's: rays along its axis from its eyepiece's disk (centre and rings of FRACTIONS of
    `radius`): as check(), the window the eyepiece's radius (mm)."""
    a, u, v, phi = around(axis)
    rim = np.cos(phi)[:, None] * u + np.sin(phi)[:, None] * v
    first = np.ones(DIRECTIONS)
    seen = {}
    for frac in FRACTIONS:
        origins = eyepiece + radius * frac * rim
        d, j = parallel_hits(origins, a, gun[0])
        hit = np.isfinite(d)
        first[hit & (first == 1.0)] = frac
        if frac < CLEAR:
            for k in np.where(hit)[0]:
                name = gun[1][j[k]]
                if name not in seen or d[k] < np.linalg.norm(seen[name]):
                    seen[name] = origins[k] + d[k] * a - eyepiece
    return float(first.min()), seen


def lens_disks(idle_dir, sp):
    """A scope's eyepiece and objective (the two disks of its lens surface, scope_lense_3p_s): the
    centres and radii, the eyepiece the rear one (+z), the locators' frame."""
    lens = idle_dir / 'render' / LENS / 'converted_model'
    if not lens.exists():
        raise SystemExit(f'{idle_dir}: no {LENS}')
    p, _uv, _ix = ri.mesh(lens.read_bytes())
    p = sp + p * HALF_TURN
    z = p[:, 2]
    mid = (z.min() + z.max()) / 2
    disks = [p[z > mid], p[z <= mid]]
    if z.max() - z.min() < 0.01 or not all(len(x) for x in disks):
        raise SystemExit(f'{idle_dir}: its lens surface is not two disks')
    return [((x.min(0) + x.max(0)) / 2, (x.max(0) - x.min(0))[:2].min() / 2) for x in disks]


def variant_check(weapon_dir, idle_dir, aimed_dir, kind, optic_names, rail_names):
    """The check of one variant: (passes, window min/max (a collimator's: degrees; a scope's: its
    eyepiece's radius, mm), clear fraction, {surface: nearest hit from the eye})."""
    locs = survey.locators(weapon_dir)
    sp, eye = locs['scope_point'][0], locs['aim'][0]
    gun_t, gun_l = triangles(weapon_dir, lib.surfaces(weapon_dir), True)
    rail_t, rail_l = placed(idle_dir, [s for s in rail_names if not s.startswith('scope_lense')], sp)
    gun = (np.concatenate([gun_t, rail_t]), np.concatenate([gun_l, rail_l]))
    if kind == 'S':
        (rear, radius), (front, _) = lens_disks(idle_dir, sp)
        clear, seen = check_scope(rear, radius, front - rear, gun)
        return clear >= CLEAR, radius * 1000, radius * 1000, clear, seen
    names = lib.surfaces(aimed_dir)
    dot = next((s for s in RETICLES if s in names), None)
    if dot is None:
        raise SystemExit(f'{aimed_dir}: no reticle')
    axis = sp + lib.bbox_centre(aimed_dir, dot) * HALF_TURN - eye
    optic = placed(idle_dir, [s for s in optic_names if not s.startswith('scope_lense')], sp)
    window, clear, seen = check(eye, axis, optic, gun)
    return clear >= CLEAR, float(window.min()), float(window.max()), clear, seen


def describe(seen):
    return ' '.join(f'{s}@{",".join("%+.3f" % x for x in at)}' for s, at in sorted(seen.items())) or '-'


RAIL_WORDS = ('picatin', 'rail', 'lastochka', 'dovetail', 'mount')


def pack_rows(args, weapons):
    """A built pack's variants (variants.tsv): (name, kind, weapon model, idle, aimed, optic, rails)."""
    files = args.pack / 'files'
    roots = [files] + args.guns + [args.live]
    for scope, gun, _id, change, _t in survey.rows('variants.tsv'):
        vname = f'fitn_{scope.removeprefix("shop_scope_")}_{gun}'
        d = lib.leaves(files / f'gameplay/items/scopes/{vname}.options')
        cfg = next((Path(r) / f'gameplay/items/weapons/wpn_{gun}.options' for r in roots
                    if (Path(r) / f'gameplay/items/weapons/wpn_{gun}.options').exists()), None)
        model = change if change != '-' else lib.leaves(cfg)['/object/model'] if cfg else weapons[gun][1]
        wdir = next(m for m in (lib.model_dir(r, model) for r in roots) if m)
        idle, aimed = lib.model_dir(files, d['/data/idle_model']), lib.model_dir(files, d['/data/aimed_model'])
        names = lib.surfaces(idle)
        rails = [s for s in names if any(w in s for w in RAIL_WORDS)]
        yield vname, kind_of(d), wdir, idle, aimed, [s for s in names if s not in rails], rails


def live_rows(args, weapons):
    """The players' variants (patch 19's and the rifle scopes'), each on each weapon it fits."""
    for v in lib.variants(args.live, lib.Modules(args.donor)):
        if v['aim_dir'] is None:
            continue
        names = lib.surfaces(v['idle_dir'])
        rails = v.get('rails') or [s for s in names if any(w in s for w in RAIL_WORDS)]
        for w in v['weapons']:
            if w not in weapons:
                continue
            wdir = lib.model_dir(args.live, v['change'] or weapons[w][1])
            locs = survey.locators(wdir) if wdir else {}
            if any(locs.get(k, (None,))[0] is None for k in ('scope_point', 'aim')):
                print(f'{v["name"]}@{w}: its scope_point or aim is not placed (a bone turned otherwise): not checked')
                continue
            yield f'{v["name"]}@{w}', v['kind'], wdir, v['idle_dir'], v['aim_dir'], [s for s in names if s not in rails], rails


def kind_of(d):
    return 'C' if d.get('/data/hide_weapon_on_aim') is False else 'S'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--pack', type=Path)
    parser.add_argument('--live', required=True, type=Path)
    parser.add_argument('--guns', type=Path, action='append', default=[])
    parser.add_argument('--donor', type=Path)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    if bool(args.pack) == bool(args.donor):
        sys.exit('--pack (and --guns) checks a built pack, --donor the players\' own variants: one of them')
    spec = importlib.util.spec_from_file_location('rifle_scopes_make_pack', HERE.parent / 'rifle-scopes' / 'make_pack.py')
    rs = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(rs)
    weapons = rs.weapon_models(args.live)
    lines = []
    for vname, kind, wdir, idle, aimed, optic, rails in (pack_rows if args.pack else live_rows)(args, weapons):
        ok, wmin, wmax, clear, seen = variant_check(wdir, idle, aimed, kind, optic, rails)
        line = f'{vname}\t{kind}\t{"pass" if ok else "FAIL"}\t{wmin:.1f}\t{wmax:.1f}\t{clear:.1f}\t{describe(seen)}'
        print(line, flush=True)
        lines.append(line)
    if args.out:
        args.out.write_text('# variant\tkind\tresult\twindow min\twindow max\tclear\tsurfaces hit inside CLEAR, each at its '
                            'nearest hit from the eye (m)\n' + '\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
