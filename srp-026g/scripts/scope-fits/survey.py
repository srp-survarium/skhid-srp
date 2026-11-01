#!/usr/bin/env python3
"""survey.py: where 0.69d0 mounts each optic on the release-41 guns, and which guns each scope we sell
fits (mounts.tsv, fit.tsv; make_pack.py builds from them).

usage: python3 -I survey.py --donor <folder> --guns <folder> [--guns <folder> ...]

--donor  prepare_donor.sh's folder: 0.69d0's game.options (its weapons' slots and default modules,
         its modules' slots_to_attach_to and attach_params, module_to_item_links), the models of
         modules.tsv and the 0.69d0 models of guns.tsv (their locators).
--guns   a weapon pack's files/ folder (its gun models: the meshes and frames are 0.69d0's, vertex for
         vertex, so 0.69d0's locator positions hold for them).

A gun takes a scope (fit.tsv) when 0.69d0 links the scope's module to it (module_to_item_links) and
mounts it on a slot the gun has or a mount of modules.tsv adds (mounts.tsv): an optic attaches at
the slot's locator, 0.69d0's own placement (PROVEN in our data: patch 19's variants are 0.69d0's
module meshes moved to exactly those positions, the AK-12's and the M4A1's to 0.1 mm). Of a scope's
slots, the first whose chain adds only rails of modules.tsv, or modules our gun has baked in already
(the AKMSU's receiver cover), is taken; a chain that would need another module (a replaced
handguard or forend) is not. The Walther Competition II (optics.tsv's new item) also goes on each of
our live weapons 0.69d0 gives it to that has a collimator mount (../rifle-scopes/mounts.tsv), but the
HK416 and the MSBS, whose stock scope it is.
"""
import argparse
import hashlib
import math
import struct
import re
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import scopes_lib as lib  # noqa: E402

ALIASES = {'a_545': 'a545'}
STOCK = {'shop_scope_walther_comp2': {'hk416', 'msbs_carbine'}}


def rows(name):
    return [line.split('\t') for line in (HERE / name).read_text(encoding='utf-8').splitlines()
            if line and not line.startswith('#')]


def items(d, prefix, fields):
    out, i = [], 0
    while any(f'{prefix}[{i}]/{f}' in d for f in fields):
        out.append({f: d.get(f'{prefix}[{i}]/{f}') for f in fields})
        i += 1
    return out


def rotation(r):
    x, y, z = r or (0.0, 0.0, 0.0)
    cx, sx, cy, sy, cz, sz = math.cos(x), math.sin(x), math.cos(y), math.sin(y), math.cos(z), math.sin(z)
    rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
    ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
    rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    return rz @ ry @ rx


def bind_pose(path):
    """{bone: (translation, rotation)} of a skinned model's render/bind_pose (u16 count; each bone its
    name, zero-terminated, and float3 translation, rotation, scale)."""
    data = Path(path).read_bytes()
    out, at = {}, 2
    for _ in range(struct.unpack_from('<H', data, 0)[0]):
        end = data.index(b'\0', at)
        name = data[at:end].decode()
        t = np.array(struct.unpack_from('<3f', data, end + 1))
        r = struct.unpack_from('<3f', data, end + 13)
        out[name] = (t, r)
        at = end + 37
    return out


HALF_TURN = np.array([-1.0, 1.0, -1.0])


def locators(model_dir):
    """{name: (position, rotation)} of a model's locators, a skinned model's in one frame for all its
    bones: a locator hangs on its bone, whose bind frame is the model's origin moved and turned half
    a turn about Y (every weapon bone's, checked), so its position is the bone's origin turned plus
    its own. Our scope_point, aim and 0.69d0's slot locators are compared in that frame (the Minimi's
    scope rail hangs on its feed cover's bone)."""
    props = lib.leaves(model_dir / 'render/export_properties')
    bind = bind_pose(model_dir / 'render/bind_pose') if (model_dir / 'render/bind_pose').exists() else {}
    out = {}
    i = 0
    while f'/locators[{i}]/name' in props:
        name, pos = props[f'/locators[{i}]/name'], np.array(props[f'/locators[{i}]/position'])
        bone = props.get(f'/locators[{i}]/bone_name')
        if bind and bone:
            if bone not in bind:
                sys.exit(f'{model_dir}: {name} hangs on {bone}, which its bind pose lacks')
            t, r = bind[bone]
            # a locator on a bone turned otherwise is not placed (None): a caller that needs it stops
            pos = t * HALF_TURN + pos if np.allclose(np.abs(r), (math.pi, 0, math.pi), atol=1e-3) else None
        out[name] = (pos, props.get(f'/locators[{i}]/rotation'))
        i += 1
    return out


def shape_key(p):
    """A mesh's shape up to a translation (and the half turn about Y a baked module may carry)."""
    return [np.sort(np.round(q - q.mean(axis=0), 4), axis=0) for q in (p, p * np.array([-1.0, 1.0, -1.0]))]


def baked(gun_dir, module_dir):
    """Every LOD0 surface of the module is a surface of the gun, moved (the porter bakes default and
    bundled modules into the gun's mesh)."""
    ours = [lib.positions(gun_dir / 'render' / s / 'converted_model') for s in lib.surfaces(gun_dir)]
    for s in lib.surfaces(module_dir):
        p = lib.positions(module_dir / 'render' / s / 'converted_model')
        keys = shape_key(p)
        if not any(len(q) == len(p) and any(np.abs(np.sort(np.round(q - q.mean(axis=0), 4), axis=0) - k).max() < 2e-4
                                            for k in keys) for q in ours):
            return False
    return True


class Donor:
    def __init__(self, folder):
        self.root = Path(folder)
        self.game = lib.leaves(self.root / 'gameplay/game.options')
        self.links = {}
        for link in items(self.game, '/module_to_item_links', ['item', 'module']):
            self.links.setdefault(link['item'], set()).add(link['module'])
        self.mounts = {r[0].split('/')[-1]: r for r in rows('modules.tsv') if r[1] == 'rail'}

    def module(self, name):
        g = self.game
        attach = {}
        for a in items(g, f'/weapon_modules/{name}/slots_to_attach_to', ['slot', 'attach_params']):
            ap = f'/weapon_modules/{name}/{a["attach_params"]}'
            attach[a['slot']] = items(g, f'{ap}/slots_to_add', ['id', 'locator'])
        return g.get(f'/weapon_modules/{name}/model'), attach

    def module_dir(self, model):
        return lib.model_dir(self.root, model)


def survey_gun(donor, gun, key, model, first_at, gun_dir):
    """{optic link name: (slot, optic origin, [(rail module, its socket)])}: 0.69d0's locator frame."""
    g = donor.game
    locs = locators(donor.root / 'models' / f'{model}.skinned_model')
    ours = locators(gun_dir)
    slots = {}
    for s in items(g, f'/weapons/{key}/slots', ['id', 'locator']):
        if s['locator'] in locs:
            p, r = locs[s['locator']]
            if p is None:
                sys.exit(f'{model}: {s["locator"]} hangs on a bone not turned half about Y')
            slots[s['id']] = (p, r, [])
    usable = set(donor.links.get(key, ())) | {d['module'] for d in items(g, f'/weapons/{key}/default_modules', ['slot', 'module'])}
    grown = True
    while grown:
        grown = False
        for m in sorted(usable):
            mmodel, attach = donor.module(m)
            if not mmodel or re.search(r'(collimator|_scope)$', m):
                continue
            mdir = donor.module_dir(mmodel)
            if mdir is None:
                continue
            mlocs = locators(mdir)
            for slot, adds in attach.items():
                if slot not in slots:
                    continue
                socket, rot, chain = slots[slot]
                for a in adds:
                    if a['id'] in slots or a['locator'] not in mlocs:
                        continue
                    p, r = mlocs[a['locator']]
                    slots[a['id']] = (socket + rotation(rot) @ p, r, chain + [(m, mmodel, socket, rot)])
                    grown = True
    out = {}
    for m in sorted(donor.links.get(key, ())):
        if not re.search(r'(collimator|_scope)$', m):
            continue
        _, attach = donor.module(m)
        for slot in attach:
            if slot not in slots:
                continue
            p, r, chain = slots[slot]
            rails, ok = [], True
            for i, (cm, cmodel, socket, rot) in enumerate(chain):
                if max(abs(x) for x in (rot or (0, 0, 0))) > 1e-3:
                    ok = False
                    break
                last = i == len(chain) - 1
                if first_at != '-' and not last:
                    continue  # our gun is one piece there: the modules under the rail are ours
                if first_at != '-' and last:
                    if ours[first_at][0] is None:
                        sys.exit(f'{gun}: our {first_at} hangs on a bone not turned half about Y')
                    shift = ours[first_at][0] - socket
                    socket, p = socket + shift, p + shift
                if baked(gun_dir, donor.module_dir(cmodel)):
                    continue
                if cmodel.split('/')[-1] not in donor.mounts:
                    ok = False
                    break
                rails.append((cmodel, socket))
            if ok and max(abs(x) for x in (r or (0, 0, 0))) < 1e-3:
                out[m] = (slot, p, rails)
                break
    return out


def find_gun_dir(roots, gun):
    """(model directory, model reference, the files make_pack.py takes of the gun: its config and model)."""
    for root in roots:
        cfg = root / f'gameplay/items/weapons/wpn_{gun}.options'
        if cfg.exists():
            model = lib.leaves(cfg)['/object/model']
            d = lib.model_dir(root, model)
            used = [cfg] + sorted(p for p in d.rglob('*') if p.is_file())
            return d, model, [(str(p.relative_to(root)), p) for p in used]
    sys.exit(f'{gun}: no gameplay/items/weapons/wpn_{gun}.options in --guns')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--donor', required=True, type=Path)
    parser.add_argument('--guns', required=True, type=Path, action='append')
    args = parser.parse_args()
    donor = Donor(args.donor)
    optics = rows('optics.tsv')
    module_of = {r[1]: r[0] for r in optics}
    lines = ['# made by survey.py: each release-41 gun and optic 0.69d0 mounts on it: the slot, the optic\'s origin',
             '# (0.69d0\'s locator frame, metres) and the rails under it, each at its socket (- none)',
             '# gun\toptic\tslot\torigin\trails']
    fits = {r[0]: [] for r in optics}
    sums = []
    for gun, key, model, first_at in rows('guns.tsv'):
        gun_dir, _, used = find_gun_dir(args.guns, gun)
        sums += [f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {rel}' for rel, p in used]
        found = survey_gun(donor, gun, key, model, first_at, gun_dir)
        for m, (slot, p, rails) in sorted(found.items()):
            if m not in module_of:
                continue
            fits[module_of[m]].append(gun)
            rail = ' '.join(f'{r}@{",".join("%.5f" % x for x in s)}' for r, s in rails) or '-'
            lines.append('\t'.join([gun, m, slot, ','.join('%.5f' % x for x in p), rail]))
        print(f'{gun}: {len([m for m in found if m in module_of])} scopes')
    (HERE / 'mounts.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    (HERE / 'guns.sha256').write_text('\n'.join(sums) + '\n', encoding='utf-8')
    # the new items on our live weapons too: 0.69d0's links, a collimator mount of ours
    live_mounts = {r[0]: r for r in lib.rows('mounts.tsv')}
    for name, link, *_rest in optics:
        if _rest[1] == '-':
            continue
        for w in sorted(live_mounts):
            plain = re.sub(r'_premium(_\d+)?$', '', w)
            given = {k for k, v in donor.links.items() if link in v}
            if {w, plain, ALIASES.get(plain, plain)} & given and w not in STOCK.get(name, set()) \
                    and plain not in STOCK.get(name, set()) and live_mounts[w][1] != '-':
                fits[name].append(w)
    lines = ['# made by survey.py: each scope we sell, the guns it fits besides the rifle scopes\' (release 41\'s guns;',
             '# a new item of optics.tsv: our live weapons too)', '# scope\tguns']
    for name, *_ in optics:
        lines.append(f'{name}\t{" ".join(fits[name])}')
        print(f'{name}: {len(fits[name])}')
    (HERE / 'fit.tsv').write_text('\n'.join(lines) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
