#!/usr/bin/env python3
"""Verify existing-fit repairs preserve identity and optical view, without drift."""
import argparse
import copy
import json
from pathlib import Path
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as builder
rs, model, lib = builder.rs, builder.model, builder.lib


def without_model(scope):
    value = copy.deepcopy(scope)
    for modification in value['modifications']:
        modification['modifiers'] = [m for m in modification['modifiers'] if m['path'] != ['object', 'model']]
    value['data'].pop('idle_model')
    value['data'].pop('aimed_model')
    return value


def camera(cfg, directory, rig):
    return model.matrix((0, 0, cfg['parameters'].get('aim_offset', 0))) @ model.HALF @ model.locator_matrix(model.locator(directory, 'aim'), rig)


def check(files, live, repairs, configs=None):
    configs = configs or files
    weapons = rs.weapon_models(live)
    total = 0
    for row in repairs:
        path = f'gameplay/items/scopes/{row["variant"]}.options'
        before, after = model.config(live / path), model.config(configs / path)
        if without_model(before) != without_model(after):
            raise ValueError(f'{path}: identity or scope behavior changed beyond its weapon camera model')
        old = lib.model_dir(live, row['original_model'])
        new = lib.model_dir(files, row['model'])
        mount = model.locator(new, 'scope_point')
        if model.locator(new, 'aim').bone != mount.bone:
            raise ValueError(f'{path}: camera does not follow its mount')
        aimed = lib.model_dir(files, after['data']['aimed_model']) or lib.model_dir(live, after['data']['aimed_model'])
        if not row['optical']:
            point = lib.bbox_centre(aimed, next(s for s in lib.surfaces(aimed) if rs.align.emitter(s)))
        for gun in row['weapons']:
            cfg = model.config(live / weapons[gun][0])
            idle = cfg['states'].get('aimed_idle', {}).get('animations', cfg['states']['idle']['animations'])[0]
            animations = {idle}
            for key, state in cfg['states'].items():
                if 'reload' in key and isinstance(state, dict):
                    animations.update(state.get('animations', []))
            original_rig = model.bones(live, old, idle, 0)
            original_camera = camera(cfg, old, original_rig)
            old_mount = model.HALF @ model.locator_matrix(model.locator(old, 'scope_point'), original_rig)
            original_camera = original_camera @ np.linalg.inv(old_mount) @ model.matrix(row['optic_shift']) @ old_mount
            first = camera(cfg, new, model.bones(live, new, idle, 0))
            if row['optical']:
                np.testing.assert_allclose(first, original_camera, atol=1e-6,
                                           err_msg=f'{path}: legacy optical view moved at idle')
            local_camera = None
            for animation in sorted(animations):
                for time in (0., .5, 1.):
                    rig = model.bones(live, new, animation, time)
                    frame = model.HALF @ model.locator_matrix(mount, rig)
                    view = frame @ np.linalg.inv(camera(cfg, new, rig))
                    if local_camera is None:
                        local_camera = view
                    np.testing.assert_allclose(view, local_camera, atol=1e-6,
                                               err_msg=f'{path}: scope drifts in {animation}@{time}')
                    if not row['optical']:
                        p = model.transform(point, view)
                        if p[2] <= 0 or abs(p[:2]).max() > rs.EYE_TOLERANCE:
                            raise ValueError(f'{path}: emitter misses camera axis: {p}')
                    total += 1
    print(f'{len(repairs)} existing variants preserve identity; {total} idle/reload camera checks passed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('files', 'live', 'repairs'):
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--configs', type=Path)
    args = parser.parse_args()
    check(args.files, args.live, json.loads(args.repairs.read_text()), args.configs)
