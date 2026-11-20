#!/usr/bin/env python3
"""Build additive standalone scope fits from the complete donor attachment graph."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

import numpy as np
from scipy.spatial import cKDTree

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import audit
import mounts
import repair_existing
from inputs import fingerprint
from geometry import lib, rotated_static

sys.path.insert(0, str(HERE.parent / 'scope-fits'))
# Explicit name avoids importing this module as the older pack builder.
import importlib.util
spec = importlib.util.spec_from_file_location('scope_fits_builder', HERE.parent / 'scope-fits/make_pack.py')
sf = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sf)
rs, ce, model = sf.rs, sf.rs.ce, audit.model


def binary(value):
    buf = bytearray(24)
    record = ce.value_record(buf, ce.append(buf, b'\0'), '', ce.as_table(value))
    buf[:24] = record
    return bytes(buf)


def put_locator(directory, name, bone, frame):
    path = directory / 'render/export_properties'
    locs = model.config(path)['locators']
    value = dict(name=name, position=frame[3, :3].tolist(), rotation=list(rs.align.angles(frame[:3, :3])),
                 bone_idx=bone, bone_name=list(mounts.bind(directory))[bone])
    np.testing.assert_allclose(mounts.local(value), frame, atol=1e-6)
    buf = bytearray(path.read_bytes())
    found = next((r for r in locs if r['name'] == name), None)
    if found:
        ce.set_entry(buf, 'locators', {'name': name},
                     {k: found[k] for k in value if k != 'name'},
                     {k: v for k, v in value.items() if k != 'name'})
    else:
        ce.append_entry(buf, 'locators', len(locs), 'name', value, vectors=True)
    path.write_bytes(buf)


def remove_default(directory, source, frame):
    """Remove only triangles belonging to the replaced, baked default module.

    Require the entire donor module to be present; a guessed bounding box must
    never cut holes in the receiver. Mesh coordinates are compared in bind space.
    """
    points = model.transform(np.concatenate([lib.positions(source / 'render' / s / 'converted_model')
                                             for s in lib.surfaces(source)]), frame)
    surfaces = [(s, rs.sights.Skinned(directory / 'render' / s / 'converted_model'))
                for s in lib.surfaces(directory)]
    all_points = np.concatenate([m.positions() for _, m in surfaces])
    distances, _ = cKDTree(all_points).query(points)
    if distances.max() > .0002:
        raise ValueError(f'{directory}: replaced {source.name} does not match baked geometry ({distances.max()} m)')
    tree = cKDTree(points)
    removed, empty = 0, []
    for name, mesh in surfaces:
        distance, _ = tree.query(mesh.positions())
        match = (distance[mesh.tris] < .0002).all(axis=1)
        removed += int(match.sum())
        if match.all():
            empty.append(name)
        elif match.any():
            mesh.keep(~match)
            mesh.write()
    if not removed:
        raise ValueError(f'{directory}: no default module triangles removed')
    sf.leave_out(directory, empty)
    return removed


def variant(shop, row, dict_id, name, weapon_model, stock):
    result = copy.deepcopy(shop)
    modification = 1_100_000 + dict_id
    result['parameters']['modification'] = modification
    result['data']['idle_model'] = f'weapons/attach/coverage/{name}'
    result['data']['aimed_model'] = f'weapons/attach/coverage/{name}_aim'
    addon = ({'path': ['addons', 'rifle_scope_dict_id'], 'value': dict_id, 'type': 0} if stock else
             {'path': ['addons'], 'value': ['rifle_scope_dict_id', dict_id], 'type': 3})
    result['modifications'] = [{'id': modification, 'modifiers': [addon,
                               {'path': ['object', 'model'], 'value': weapon_model, 'type': 0}]}]
    result['srp_scope_fit'] = {'scope': row['scope'], 'weapons': [row['weapon']]}
    return binary(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ('live', 'donor', 'compose', 'out'):
        parser.add_argument('--' + arg, type=Path, required=True)
    parser.add_argument('--only', help='Debug one weapon without writing the registry or edit list')
    args = parser.parse_args()
    pinned = json.loads((HERE / 'inputs.json').read_text())
    for key in ('live', 'donor'):
        if fingerprint(getattr(args, key)) != pinned[key]:
            raise ValueError(f'{key}: prepared input differs from inputs.json')
    if args.out.exists():
        raise ValueError('output must be a new directory')
    rows = sorted((r for r in audit.audit(args.live, args.donor) if r['status'] == 'missing'),
                  key=lambda r: (r['weapon'], r['scope']))
    registry = [(4000 + i, row) for i, row in enumerate(rows)]
    registered = [(int(i), gun, scope) for i, gun, scope in audit.rows(HERE / 'variants.tsv')]
    if registered != [(i, r['weapon'], r['scope']) for i, r in registry]:
        raise ValueError('coverage changed: review the stable variant registry before rebuilding')
    if args.only and not any(r['weapon'] == args.only for r in rows):
        raise ValueError(f'{args.only}: no missing combinations')
    dictionary = model.config(args.live / 'gameplay/db_static_dictionaries')
    ids = {e['name']: e['dict_id'] for e in dictionary['items_dict']}
    if set(ids.values()) & {i for i, _ in registry} or registry[-1][0] >= 5000:
        raise ValueError('scope coverage dictionary range is not free')
    game = model.config(args.donor / 'gameplay/game.options')
    donor = sf.survey.Donor(args.donor)
    weapons = rs.weapon_models(args.live)
    donor_models = dict(audit.rows(HERE / 'weapons.tsv'))
    legacy = json.loads((HERE / 'legacy-mounts.json').read_text())
    roots = sf.Roots([], args.live)
    variants = sf.our_variants(args.live)
    optics = sf.survey.rows('optics.tsv')
    pu = game['weapon_modules']['pu_35x22_scope']['model']
    optics.append(['shop_scope_pu_3_5x', 'pu_35x22_scope', pu.removeprefix('weapons/modules/').rsplit('/', 1)[0],
                   '-', '-', '-', '-', '-'])
    # Walther is already a shop item in the input; reuse its shipped geometry.
    optics = [r[:3] + ['-'] + r[4:] for r in optics]
    fits = {r['scope']: True for r in rows}
    files, entries, report, placeholders = {}, [], [], set()
    args.out.mkdir(parents=True)
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        refs = sf.references(args, work, donor, optics, fits, variants, weapons, roots)
        transforms = {}
        for dict_id, row in registry:
            gun, scope = row['weapon'], row['scope']
            if args.only and gun != args.only:
                continue
            cfg_path, base = weapons[gun]
            cfg = model.config(args.live / cfg_path)
            original = roots.model(base)
            native = lib.model_dir(args.donor, donor_models[row['donor_weapon']])
            steps = json.loads(row['chain'])
            if profile := legacy.get(row['donor_weapon']):
                if steps:
                    raise ValueError(f'{gun}: legacy profile is only valid for direct rail optics')
                transform = np.eye(4)
                bone = profile['bone']
                directory = lib.model_dir(args.donor, game['weapon_modules'][profile['module']]['model'])
                placed = model.matrix(profile['position'], profile['rotation'])
                frame = mounts.local(mounts.locator(directory, profile['locator'])) @ placed
                parts = [(profile['module'], directory, placed)]
                steps = [dict(module=profile['module'], slot='scope_coverage_rail', locator=profile['locator'])]
            else:
                if gun not in transforms:
                    transforms[gun] = mounts.transfer(original, native)[0]
                transform = transforms[gun]
                bone, frame, parts = mounts.assembly(game, args.donor, native, row['donor_weapon'], row['root'], steps)
            frame = frame @ transform
            bone_index, locator_frame = mounts.attach_frame(original, bone, frame)
            ref = refs[scope]
            short = scope.removeprefix('shop_scope_')
            name = f'fitc_{short}_{gun}'
            changed = f'weapons/coverage/{gun}_{short}'
            weapon_out = work / f'models/{changed}.skinned_model'
            shutil.copytree(original, weapon_out)
            put_locator(weapon_out, 'scope_point', bone_index, locator_frame)
            # Bind before choosing the eye; otherwise a single animated pose gets
            # baked into the camera of an unrelated bone.
            aim = mounts.locator(weapon_out, 'aim')
            put_locator(weapon_out, 'aim', bone_index, mounts.local(aim))
            rail_parts = []
            defaults = {r['slot']: r['module'] for r in audit.sequence(game['weapons'][row['donor_weapon']]['default_modules'])}
            for step, (module_name, directory, part_frame) in zip(steps, parts):
                relative = part_frame @ transform @ np.linalg.inv(frame)
                default = defaults.get(step['slot'])
                if default and default != module_name:
                    old = lib.model_dir(args.donor, game['weapon_modules'][default]['model'])
                    remove_default(weapon_out, old, part_frame @ transform)
                elif default == module_name:
                    continue  # already baked into the weapon
                if not np.allclose(relative[:3, :3], np.eye(3), atol=1e-6):
                    directory = rotated_static(directory, relative[:3, :3], work / 'parts' / name / directory.name)
                rail_parts.append((directory, relative[3, :3], []))
            family = row['donor_weapon']
            sf.under_optic(weapon_out, sf.sight_changes().get(gun, sf.sight_changes().get(family, [])), donor, gun)
            sight_rules = rs.sight_changes(HERE.parent / 'rifle-scopes/sights.tsv')
            changes = sight_rules.get(gun, sight_rules.get(family, []))
            swapped = rs.swap_parts(changes, lambda m: donor.module_dir(f'weapons/modules/{m}'), gun, original)
            if swapped:
                old_scope = mounts.locator(original, 'scope_point')
                old_bones = mounts.bind(original)
                old_frame = model.HALF @ mounts.local(old_scope) @ list(old_bones.values())[old_scope['bone_idx']]
                for part, translation, chosen in swapped:
                    relative = model.matrix(translation) @ old_frame @ np.linalg.inv(frame)
                    if not np.allclose(relative[:3, :3], np.eye(3), atol=1e-6):
                        part = rotated_static(part, relative[:3, :3], work / 'swapped' / name / part.name)
                    rail_parts.append((part, relative[3, :3], chosen))
            rs.under_optic(weapon_out, changes,
                           lambda m: donor.module_dir(f'weapons/modules/{m}'), gun)
            idle = work / f'models/weapons/attach/coverage/{name}.model'
            aimed = work / f'models/weapons/attach/coverage/{name}_aim.model'
            materials = {**rs.material_renames(), **ref.get('native_materials', {})}
            if ref['kind'] == 'S':
                rs.compose(args.compose, [(ref['native'], np.zeros(3), [])] + rail_parts, materials, idle)
                rs.compose(args.compose, [(ref['native_aim'], np.zeros(3), [])], materials, aimed)
                needed = rs.optical_eye(roots.layered(), cfg_path, weapon_out, ref['native'], np.zeros(3))
            else:
                rs.compose(args.compose, [(ref['idle'], -ref['t'], ref['own'])] + rail_parts, materials, idle)
                rs.compose(args.compose, [(ref['aim'], -ref['t'], ref['aim_own'])] + rail_parts, materials, aimed)
                needed = rs.collimator_eye(roots.layered(), cfg_path, weapon_out, aimed)
            rs.moved_optical_eye(weapon_out, needed)
            for directory in (idle, aimed):
                rs.wrap_uv(directory)
            for directory in (weapon_out, idle, aimed):
                files.update(rs.files_of(directory, work))
            shop = model.config(args.live / f'gameplay/items/scopes/{scope}.options')
            path = f'gameplay/items/scopes/{name}.options'
            files[path] = variant(shop, row, dict_id, name, changed, 'rifle_scope_dict_id' in cfg.get('addons', {}))
            entries.append(dict(base_cost=0, cfg_name=path, dict_id=dict_id, is_premium=False,
                                is_stack=False, item_category=22, name=name))
            if not cfg.get('addons'):
                placeholders.add(gun)
            report.append(dict(row, dict_id=dict_id, variant=name, bone=bone, model=changed))
            print(f'{dict_id}: {gun} / {short}', flush=True)
        if not args.only:
            centered, repairs, centering_edits = repair_existing.repair(args.live, work, sys.modules[__name__],
                                                                       args.donor, args.compose)
            # Existing configs are changed after prior edit lists, not replaced
            # with a post-edit snapshot before those lists run again.
            for path, data in centered.items():
                if path.startswith('gameplay/'):
                    preview = args.out / 'review' / path
                    preview.parent.mkdir(parents=True, exist_ok=True)
                    preview.write_bytes(data)
                else:
                    files[path] = data
            (HERE.parent / 'content-edit/scope-centering-1.json').write_text(json.dumps(centering_edits, indent=2) + '\n')
            (args.out / 'centering.json').write_text(json.dumps(repairs, indent=2) + '\n')
            print(f'{len(repairs)} existing variant cameras corrected; item IDs retained', flush=True)
        have = set((args.live / 'paths.txt').read_text().splitlines())
        for p in args.donor.rglob('*'):
            rel = str(p.relative_to(args.donor))
            if p.is_file() and rel.startswith(('material_instances/', 'textures/')) and rel not in have:
                files[rel] = p.read_bytes()
        files.update(sf.pbr_026.convert({k: v for k, v in files.items() if k.startswith('textures/')}))
        rs.check_textures(files, args.live)
        for path, data in files.items():
            target = args.out / 'files' / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
    pack = dict(format_version=1, id='scope-coverage', version='2', target_build='0.26g',
                description='Complete standalone 0.69 scope fits, retaining existing variants',
                operations=[dict(op='add_file', path=p) for p in sorted(files)] +
                           [dict(op='dictionary_add', table='items_dict', entries=entries)],
                server={'items': [{'dict_id': e['dict_id']} for e in entries]})
    (args.out / 'pack.json').write_text(json.dumps(pack, indent=2) + '\n')
    (args.out / 'coverage.json').write_text(json.dumps(report, indent=2) + '\n')
    if not args.only:
        additions = [dict(files=[weapons[g][0]], table='', key='addons', value={'mount': 1})
                     for g in sorted(placeholders)]
        additions += [dict(files=['gameplay/db_static_dictionaries'], table='items_compatibility',
                           key=f'compatibility_{ids[r["scope"]]}{ids[r["weapon"]]:05d}',
                           value={'first_item': ids[r['weapon']], 'second_item': ids[r['scope']]}) for r in rows]
        edit = dict(_about='Standalone scope coverage; generated by scope-coverage/make_pack.py.', add_value=additions)
        (HERE.parent / 'content-edit/scope-coverage-1.json').write_text(json.dumps(edit, indent=2) + '\n')
        (HERE / 'variants.tsv').write_text('# id\tweapon\tscope\n' + ''.join(
            f'{i}\t{r["weapon"]}\t{r["scope"]}\n' for i, r in registry))
    print('pack.json sha256', hashlib.sha256((args.out / 'pack.json').read_bytes()).hexdigest())


if __name__ == '__main__':
    main()
