"""Keep existing variant IDs and optics; make their cameras follow the mount.

Older fits bake an animated offset into a camera on the weapon root. Their
reticles drift as the scope's bone moves. Collimators use the emitter axis;
legacy optical overlays retain their original idle view, on the correct bone.
"""
import shutil

import numpy as np

import mounts
from rail_alignment import Rails


def repair(live, work, builder, donor, compose):
    rs, model, lib = builder.rs, builder.model, builder.lib
    weapons = rs.weapon_models(live)
    files, report = {}, []
    edits = dict(_about='Existing scope cameras follow their attachment bone; IDs and compatibility stay unchanged.',
                 set_entry=[], replace_string=[])
    rails = Rails(donor, builder)
    for path in sorted((live / 'gameplay/items/scopes').glob('*.options')):
        scope = model.config(path)
        if 'srp_scope_fit' not in scope:
            continue
        guns = scope['srp_scope_fit']['weapons']
        first = guns[0]
        cfg_path, original_name = weapons[first]
        changed_modifier = None
        for modification in scope['modifications']:
            for modifier in modification['modifiers']:
                if modifier['path'] == ['object', 'model']:
                    original_name = modifier['value']
                    changed_modifier = modifier
        original = lib.model_dir(live, original_name)
        aim, mount = model.locator(original, 'aim'), model.locator(original, 'scope_point')
        optical = scope['data']['hide_weapon_on_aim']
        idle = lib.model_dir(live, scope['data']['idle_model'])
        aimed = lib.model_dir(live, scope['data']['aimed_model'])
        shift, own, rail = rails.measure(scope['srp_scope_fit']['scope'], idle)
        if optical and aim.bone == mount.bone and not shift.any():
            continue
        if shift.any():
            if scope['data']['idle_model'] == scope['data']['aimed_model']:
                raise ValueError(f'{path}: one shared idle/aimed model needs explicit handling')
            moved = rails.shifted(scope['srp_scope_fit']['scope'], idle, aimed, shift, own,
                                  compose, work, path.stem)
            for key, (reference, directory) in zip(('idle_model', 'aimed_model'), moved):
                edits['replace_string'].append({'files': str(path.relative_to(live)),
                                                 'from': scope['data'][key], 'to': reference})
                scope['data'][key] = reference
                files.update(rs.files_of(directory, work))
            idle, aimed = [directory for _, directory in moved]
        name = f'weapons/coverage/centered_{path.stem}'
        target = work / f'models/{name}.skinned_model'
        shutil.copytree(original, target)
        builder.put_locator(target, 'aim', mount.bone, model.matrix(aim.position, aim.rotation))
        required = []
        for gun in guns:
            cfg_path = weapons[gun][0]
            cfg = model.config(live / cfg_path)
            if optical:
                animation = cfg['states'].get('aimed_idle', {}).get('animations', cfg['states']['idle']['animations'])[0]
                rig = model.bones(live, original, animation, 0)
                offset = cfg['parameters'].get('aim_offset', 0)
                camera = model.matrix((0, 0, offset)) @ model.HALF @ model.locator_matrix(aim, rig)
                mounted = model.HALF @ model.locator_matrix(mount, rig)
                camera = camera @ np.linalg.inv(mounted) @ model.matrix(shift) @ mounted
                required.append(rs.locator_from_camera(camera @ np.linalg.inv(rig[mount.bone].pose), offset))
            else:
                required.append(rs.collimator_eye(live, cfg_path, target, aimed))
        for value in required[1:]:
            np.testing.assert_allclose(value, required[0], atol=1e-6,
                                       err_msg=f'{path.stem}: shared variant requires different weapon cameras')
        if not shift.any() and aim.bone == mount.bone and np.allclose(required[0], (aim.position, aim.rotation), atol=1e-6):
            shutil.rmtree(target)
            continue
        rs.moved_optical_eye(target, required[0])
        cfg_file = str(path.relative_to(live))
        if changed_modifier is None:
            value = dict(path=['object', 'model'], value=name, type=0)
            expected = {'id': scope['modifications'][0]['id']}
            expected.update({k.removeprefix('/modifications[0]/'): v for k, v in lib.leaves(path).items()
                             if k.startswith('/modifications[0]/modifiers[')})
            scope['modifications'][0]['modifiers'].append(value)
            edits['set_entry'].append({'file': cfg_file, 'list': 'modifications',
                                       'where': {'id': scope['modifications'][0]['id']}, 'from': expected,
                                       'to': {'modifiers': scope['modifications'][0]['modifiers']}})
        else:
            index = next(i for i, m in enumerate(scope['modifications']) if changed_modifier in m['modifiers'])
            edits['set_entry'].append({'file': cfg_file, 'list': f'modifications[{index}]/modifiers',
                                       'where': {'path[0]': 'object', 'path[1]': 'model'},
                                       'from': {'value': changed_modifier['value']}, 'to': {'value': name}})
            changed_modifier['value'] = name
        files[str(path.relative_to(live))] = builder.binary(scope)
        files.update(rs.files_of(target, work))
        report.append(dict(variant=path.stem, weapons=guns, optical=optical, model=name,
                           original_model=original_name, original_bone=aim.bone, bone=mount.bone,
                           optic_shift=shift.tolist(), rail=rail))
    return files, report, edits
