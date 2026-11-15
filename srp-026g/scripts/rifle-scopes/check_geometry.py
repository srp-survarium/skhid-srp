#!/usr/bin/env python3
"""Check generated scope cameras and reticles on the real animated weapon assets.

python3 check_geometry.py --pack <pack> --live <released extraction> --donor <donor> --report <tsv>
This is geometric evidence, not proof of native shaders or the lobby lifecycle.
"""

import argparse
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as builder
import scopes_lib as lib


def check(pack, live, donor, report):
    files = pack / 'files'
    manifest = json.loads((pack / 'pack.json').read_text())
    # Geometry alone cannot detect missing native materials. Check every pack,
    # including scope-fits, against the same dependency closure as its builder.
    builder.check_textures({str(p.relative_to(files)): p.read_bytes()
                            for p in files.rglob('*') if p.is_file()}, live)
    if manifest['id'] == 'rifle-scopes':
        modules = {row['name']: row['module'] for row in lib.scopes()}
        aimed_models = {row['name']: 'weapons/modules/' + row['module'] + '/' + row['module_name'] + '_aim'
                        for row in lib.scopes()}
    elif manifest['id'] == 'scope-fits':
        table = Path(__file__).resolve().parents[1] / 'scope-fits/optics.tsv'
        optics = [line.split('\t') for line in table.read_text().splitlines() if line and not line.startswith('#')]
        modules = {row[0]: row[2] for row in optics}
        game = lib.leaves(donor / 'gameplay/game.options')
        aimed_models = {row[0]: game[f'/weapon_modules/{row[1]}/aimed_model'] for row in optics}
    else:
        raise ValueError('expected a rifle-scopes or scope-fits pack')
    weapons = builder.weapon_models(live)
    donor_modules = lib.Modules(donor)
    rows = ['variant\tweapon\tkind\ttime\terror_m\taim_bone\tmount_bone']
    count = 0
    for path in sorted((files / 'gameplay/items/scopes').glob('*.options')):
        scope = builder.posing.config(path)
        if 'srp_scope_fit' not in scope:
            continue
        name = scope['srp_scope_fit']['scope']
        for weapon in scope['srp_scope_fit']['weapons']:
            cfg_path, weapon_model = weapons[weapon]
            for modification in scope['modifications']:
                for modifier in modification['modifiers']:
                    if modifier['path'] == ['object', 'model']:
                        weapon_model = modifier['value']
            directory = lib.model_dir(files, weapon_model) or lib.model_dir(live, weapon_model)
            cfg = builder.posing.config(live / cfg_path)
            animation = cfg['states'].get('aimed_idle', {}).get('animations', cfg['states']['idle']['animations'])[0]
            aim = builder.posing.locator(directory, 'aim')
            mount = builder.posing.locator(directory, 'scope_point')
            aimed = lib.model_dir(files, scope['data']['aimed_model']) or lib.model_dir(live, scope['data']['aimed_model'])
            optical = scope['data']['hide_weapon_on_aim']
            if optical:
                module = donor_modules.model(modules[name])
                native = builder.posing.locator(module, 'aim')
                idle = lib.model_dir(files, scope['data']['idle_model'])
                # Recover the actual mount translation independently from the emitted
                # mesh, rather than trusting the builder's mounts.tsv or camera result.
                body = next(s for s in lib.surfaces(module) if not s.startswith('scope_lense'))
                original = lib.positions(module / 'render' / body / 'converted_model')
                mounted = lib.positions(idle / 'render' / body / 'converted_model')
                if original.shape != mounted.shape:
                    raise ValueError(f'{path.stem}: donor optical body topology changed')
                translation = (mounted - original).mean(axis=0)
                np.testing.assert_allclose(mounted - original, np.broadcast_to(translation, mounted.shape), atol=1e-7)
                native_aimed = lib.model_dir(donor, aimed_models[name])
                for surface in lib.surfaces(native_aimed):
                    source_points = lib.positions(native_aimed / 'render' / surface / 'converted_model')
                    aimed_points = lib.positions(aimed / 'render' / surface / 'converted_model')
                    np.testing.assert_allclose(aimed_points, source_points + translation, atol=1e-7,
                                               err_msg=f'{path.stem}: aimed {surface} moved away from the idle assembly')
                if aim.bone != mount.bone:
                    raise ValueError(f'{path.stem}: optical eye does not follow the attachment bone')
            else:
                emitter = [s for s in lib.surfaces(aimed) if builder.align.emitter(s)]
                if len(emitter) != 1:
                    raise ValueError(f'{path.stem}: expected one reticle emitter')
                point = lib.bbox_centre(aimed, emitter[0])
            for time in (0.0, 0.5, 1.0):
                rig = builder.posing.bones(live, directory, animation, time)
                optic_frame = builder.posing.HALF @ builder.posing.locator_matrix(mount, rig)
                camera = (builder.posing.matrix((0, 0, cfg['parameters'].get('aim_offset', 0)))
                          @ builder.posing.HALF @ builder.posing.locator_matrix(aim, rig))
                if optical:
                    expected = (builder.posing.matrix(native.position, native.rotation)
                                @ builder.posing.matrix(translation) @ optic_frame)
                    error = float(np.abs(camera - expected).max())
                    if error > 1e-6:
                        raise ValueError(f'{path.stem}@{time}: camera differs from 0.69 by {error}')
                else:
                    view_point = builder.posing.transform(point, optic_frame @ np.linalg.inv(camera))
                    error = float(np.abs(view_point[:2]).max())
                    if view_point[2] <= 0 or error > builder.EYE_TOLERANCE:
                        raise ValueError(f'{path.stem}@{time}: reticle misses viewing axis: {view_point}')
                rows.append(f'{path.stem}\t{weapon}\t{"optical" if optical else "collimator"}\t{time:g}\t{error:.9g}\t{aim.bone}\t{mount.bone}')
            count += 1
    report.write_text('\n'.join(rows) + '\n')
    print(f'{count} variants, {count * 3} animated camera checks passed; {report}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('pack', 'live', 'donor', 'report'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    check(args.pack, args.live, args.donor, args.report)
