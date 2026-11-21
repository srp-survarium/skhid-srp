#!/usr/bin/env python3
"""The scope fits of the STK2 AK-74s: every optic the live AK-74M takes, on each new gun, as the
AK-74M's own fit of it (docs/notes/weapons-stk2.md). Pack weapons-stk2-scopes and the edit list
weapons-stk2-scopes-1.json (the lobby's compatibility rows and the guns' `addons` placeholder).

    python3 -I scope_fits.py --live <tree> --guns <tree> --out <pack folder> --version <n>

<live>: the live files (stock and every listed patch) at their game paths: the dictionary, the
AK-74M's variants (gameplay/items/scopes/), their idle and aimed models and their weapon models;
<guns>: the weapons-stk2 pack's files (the new guns' models).

Each AK-74M variant becomes one per new gun:
- the optic is the AK-74M's (its idle and aimed models as they are), less the AK-74M's dovetail
  adapter with its rail (`picatini_lastochka_s`): it stands on the new gun's own top rail, moved by
  the difference of the two rails' tops (and centres), as are the gun's scope_point and eye
  (`aim`) of the AK-74M's variant model, so the optic, its reticle and the eye keep the places the
  AK-74M's fit gives them on each other (the two PSO-1s clamp to the dovetail, which the guns
  share with the AK-74M: they stay where they are, and the side mount with the top rail comes off);
- the gun's model is a copy of its own with those two locators and its iron sights gone
  (guns.json's `under_optic`);
- the variant's config is the AK-74M's with its own id, models and modification id
  (1,100,000 + dict id, as scope-coverage's), for the new gun alone.
The variant ids are variants.tsv's (claimed: 4700-4799).

Needs NumPy and SciPy.
"""
import argparse
import copy
import hashlib
import json
import shutil
import struct
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'content-edit'))
sys.path.insert(0, str(HERE.parent / 'sight-alignment'))
sys.path.insert(0, str(HERE.parent / 'rifle-scopes'))
import align  # noqa: E402
import content_edit as ce  # noqa: E402
import model as M  # noqa: E402
import sights  # noqa: E402
from donor import GUNS, islands  # noqa: E402

TEMPLATE = 'ak_74m'
ADAPTER = 'picatini_lastochka_s'
HALF = M.matrix(angles=(0.0, np.pi, 0.0))
MODIFICATION_BASE = 1_100_000
IDS = range(4700, 4800)


def config_bytes(value):
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, ce.append(buf, b'\0'), '', ce.as_table(value))
    return bytes(buf)


def vector_config_bytes(value):
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, ce.append(buf, b'\0'), '', ce.as_table(value, vectors=True))
    return bytes(buf)


def bind(directory):
    data = (directory / 'render/bind_pose').read_bytes()
    out, at = {}, 2
    for _ in range(struct.unpack_from('<H', data)[0]):
        end = data.index(0, at)
        v = struct.unpack_from('<9f', data, end + 1)
        out[data[at:end].decode()] = M.matrix(v[:3], v[3:6], v[6:])
        at = end + 37
    return out


def locator_frame(directory, name):
    """The locator's frame in the model's (mesh) frame and its bone's name."""
    loc = M.locator(directory, name)
    bones = bind(directory)
    bone = list(bones)[loc.bone]
    return M.matrix(loc.position, loc.rotation) @ bones[bone], bone


def positions(path):
    data = path.read_bytes()
    _, size, n = struct.unpack_from('<3I', data)
    stride = (size - 4) // n
    return np.frombuffer(data, np.uint8, n * stride, 12).reshape(n, stride)[:, :12].copy().view('<f4').reshape(n, 3)


def adapter_rail(idle, scope_frame):
    """The AK-74M adapter's rail: its top and its centre across, in the gun's mesh frame."""
    p = positions(idle / f'render/{ADAPTER}/converted_model').astype(np.float64)
    p = np.c_[p, np.ones(len(p))] @ HALF @ scope_frame
    top = p[:, 1].max()
    teeth = p[p[:, 1] > top - 0.004]
    return top, (teeth[:, 0].min() + teeth[:, 0].max()) / 2


def set_locator(directory, name, frame, bone):
    """`name` at `frame` (mesh frame) on `bone`, written as the locator's own position and angles."""
    path = directory / 'render/export_properties'
    props = M.config(path)
    bones = bind(directory)
    local = frame @ np.linalg.inv(bones[bone])
    for loc in props['locators']:
        if loc['name'] == name:
            loc['position'] = local[3, :3].tolist()
            loc['rotation'] = align.angles(local[:3, :3])
            loc['bone_idx'] = list(bones).index(bone)
            loc['bone_name'] = bone
            np.testing.assert_allclose(M.matrix(loc['position'], loc['rotation']), local, atol=1e-6)
            break
    else:
        raise ValueError(f'{directory}: no {name} locator')
    path.write_bytes(vector_config_bytes(props))


def leave_out(directory, surfaces):
    """The model without `surfaces` (its settings' LOD0 and materials, their folders)."""
    path = directory / 'settings'
    settings = M.config(path)
    lod = settings['lod_hierrarchy']['LOD0']['surfaces']
    settings['lod_hierrarchy']['LOD0']['surfaces'] = [s for s in lod if s not in surfaces]
    settings['material_settings'] = {k: v for k, v in settings['material_settings'].items() if k not in surfaces}
    path.write_bytes(config_bytes(settings))
    for s in surfaces:
        shutil.rmtree(directory / 'render' / s)


def under_optic(directory, rules, dovetail):
    """The gun's copy with its iron sights gone (and its side mount, under a dovetail optic)."""
    gone = []
    for surface in sorted({r['surface'] for r in rules['cut'] + rules['flatten']} | ({rules['mount']['surface']} if dovetail else set())):
        mesh = sights.Skinned(directory / 'render' / surface / 'converted_model')
        p = mesh.positions()
        label = islands(p, mesh.tris)
        drop = np.zeros(len(p), bool)
        boxes = [np.asarray(r['box']) for r in rules['cut'] if r['surface'] == surface]
        if dovetail and surface == rules['mount']['surface']:
            boxes.append(np.array([[-1.0, -1.0, -1.0], [1.0, 1.0, rules['mount']['below_z']]]))
        for i in np.unique(label):
            q = p[label == i]
            if any((q.min(0) >= b[0]).all() and (q.max(0) <= b[1]).all() for b in boxes):
                drop[label == i] = True
        for rule in (r for r in rules['flatten'] if r['surface'] == surface):
            b = np.asarray(rule['box'])
            for i in np.unique(label):
                q = p[label == i]
                if (q.min(0) >= b[0]).all() and (q.max(0) <= b[1]).all():
                    at = np.flatnonzero((label == i) & (p[:, 1] > rule['y']))
                    mesh.raw[at, 4:8] = np.frombuffer(struct.pack('<f', rule['y']), np.uint8)
        if not drop.any() and not rules['flatten']:
            raise ValueError(f'{directory.name} {surface}: nothing of the sights found')
        keep = ~drop[mesh.tris].any(axis=1)
        if not keep.any():
            gone.append(surface)
            continue
        mesh.keep(keep)
        mesh.write()
    if gone:
        leave_out(directory, gone)


def our_variants(live, dictionary):
    """The AK-74M's variants: (config path, config)."""
    out = []
    for path in sorted((live / 'gameplay/items/scopes').glob('*.options')):
        cfg = M.config(path)
        fit = cfg.get('srp_scope_fit')
        if fit and TEMPLATE in [dictionary.get(w, w) for w in fit['weapons']]:
            out.append((path, cfg))
    return out


def registry(path, pairs):
    """Stable dict ids by (gun, scope): variants.tsv, extended at its end for new pairs."""
    rows = [line.split('\t') for line in path.read_text().splitlines() if line and not line.startswith('#')] if path.exists() else []
    ids = {(g, s): int(i) for i, g, s in rows}
    for pair in pairs:
        if pair not in ids:
            free = next(i for i in IDS if i not in ids.values())
            ids[pair] = free
    path.write_text('# made by scope_fits.py: each variant\'s dict id (claimed 4700-4799), never reused\n# id\tgun\tscope\n'
                    + ''.join(f'{i}\t{g}\t{s}\n' for (g, s), i in sorted(ids.items(), key=lambda x: x[1])))
    return ids


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--live', type=Path, required=True)
    ap.add_argument('--guns', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--version', required=True)
    args = ap.parse_args()
    if args.out.exists():
        sys.exit(f'{args.out} is already there')
    files = args.out / 'files'
    dictionary = M.config(args.live / 'gameplay/db_static_dictionaries')
    names = {e['dict_id']: e['name'] for e in dictionary['items_dict']}
    taken = set(names)
    variants = our_variants(args.live, names)
    pairs = [(gun, cfg['srp_scope_fit']['scope']) for gun in GUNS['guns'] for _, cfg in variants]
    ids = registry(HERE / 'variants.tsv', pairs)
    if taken & set(ids.values()):
        sys.exit(f'dict ids {sorted(taken & set(ids.values()))} are taken')
    rail = GUNS['rail']
    entries, report, compat = [], [], []
    for gun, spec in GUNS['guns'].items():
        gun_dir = args.guns / f'models/{spec["model"]}.skinned_model'
        for path, cfg in variants:
            scope = cfg['srp_scope_fit']['scope']
            dict_id = ids[(gun, scope)]
            short = scope.removeprefix('shop_scope_')
            name = f'fitk_{short}_{gun}'
            mods = {tuple(m['path']): m['value'] for x in cfg['modifications'] for m in x['modifiers']}
            ours = args.live / f'models/{mods[("object", "model")]}.skinned_model'
            scope_frame, scope_bone = locator_frame(ours, 'scope_point')
            aim_frame, aim_bone = locator_frame(ours, 'aim')
            idle = args.live / f'models/{cfg["data"]["idle_model"]}.model'
            dovetail = not (idle / 'render' / ADAPTER).exists()
            if dovetail:
                shift = np.zeros(3)
                models = cfg['data']['idle_model'], cfg['data']['aimed_model']
            else:
                top, centre = adapter_rail(idle, scope_frame)
                shift = np.array([rail['x'] - centre, rail['top'] - top, 0.0])
                models = f'weapons/attach/stk2/{name}', f'weapons/attach/stk2/{name}_aim'
                for source, target in zip((cfg['data']['idle_model'], cfg['data']['aimed_model']), models):
                    out = files / f'models/{target}.model'
                    shutil.copytree(args.live / f'models/{source}.model', out)
                    if (out / 'render' / ADAPTER).exists():
                        leave_out(out, [ADAPTER])
            move = M.matrix(shift)
            weapon_model = f'weapons/ak74_tactical/stk2_fits/{gun}_{short}'
            weapon_dir = files / f'models/{weapon_model}.skinned_model'
            shutil.copytree(gun_dir, weapon_dir)
            set_locator(weapon_dir, 'scope_point', scope_frame @ move, scope_bone)
            set_locator(weapon_dir, 'aim', aim_frame @ move, aim_bone)
            under_optic(weapon_dir, spec['under_optic'], dovetail)
            variant = copy.deepcopy(cfg)
            modification = MODIFICATION_BASE + dict_id
            variant['parameters']['modification'] = modification
            variant['data']['idle_model'], variant['data']['aimed_model'] = models
            variant['modifications'] = [{'id': modification, 'modifiers': [
                {'path': ['addons'], 'value': ['rifle_scope_dict_id', dict_id], 'type': 3},
                {'path': ['object', 'model'], 'value': weapon_model, 'type': 0}]}]
            variant['srp_scope_fit'] = {'scope': scope, 'weapons': [gun]}
            target = files / f'gameplay/items/scopes/{name}.options'
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(config_bytes(variant))
            if M.config(target) != variant:
                sys.exit(f'{target}: does not read back')
            entries.append(dict(base_cost=0, cfg_name=f'gameplay/items/scopes/{name}.options', dict_id=dict_id,
                                is_premium=False, is_stack=False, item_category=22, name=name))
            compat.append((scope, gun))
            report.append(dict(dict_id=dict_id, gun=gun, scope=scope, variant=name, from_variant=path.stem,
                               dovetail=dovetail, shift_mm=[round(float(x) * 1000, 2) for x in shift]))
            print(f'{dict_id}\t{gun}\t{short}\tshift {np.round(shift * 1000, 2)} mm{"  (dovetail)" if dovetail else ""}')
    paths = sorted(str(p.relative_to(files)) for p in files.rglob('*') if p.is_file())
    pack = dict(format_version=1, id='weapons-stk2-scopes', version=str(args.version), target_build='0.26g',
                description="The STK2 AK-74s' scope fits: every optic the AK-74M takes (scripts/weapons-stk2/scope_fits.py)",
                operations=[dict(op='add_file', path=p) for p in paths]
                + [dict(op='dictionary_add', table='items_dict', entries=entries)],
                server={'items': [{'dict_id': e['dict_id']} for e in entries]})
    (args.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    (args.out / 'fits.json').write_text(json.dumps(report, indent=2) + '\n')
    scope_ids = {n: i for i, n in names.items()}
    gun_ids = {'ak74_tactical': 2296, 'ak74_tactical_carbine': 2297}
    configs = {g: f'gameplay/items/weapons/wpn_{g}.options' for g in GUNS['guns']}
    additions = [dict(files=[configs[g]], table='', key='addons', value={'mount': 1}) for g in GUNS['guns']]
    additions += [dict(files=['gameplay/db_static_dictionaries'], table='items_compatibility',
                       key=f'compatibility_{scope_ids[s]}{gun_ids[g]:05d}',
                       value={'first_item': gun_ids[g], 'second_item': scope_ids[s]}) for s, g in compat]
    edit = dict(_about="The STK2 AK-74s' optics: the lobby's rows of gun and shop optic, and the guns' `addons` "
                       "placeholder a variant's modification adds its scope to; generated by weapons-stk2/scope_fits.py.",
                add_value=additions)
    (HERE.parent / 'content-edit/weapons-stk2-scopes-1.json').write_text(json.dumps(edit, indent=2) + '\n')
    print(f'{len(entries)} variants; pack.json sha256', hashlib.sha256((args.out / 'pack.json').read_bytes()).hexdigest())


if __name__ == '__main__':
    main()
