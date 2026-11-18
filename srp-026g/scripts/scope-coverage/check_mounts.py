#!/usr/bin/env python3
"""Check rendered rail sockets, independently of the builder's weapon locators.

Recover a rail's rigid transform from emitted vertex positions. Its native
socket must then coincide with the optic body origin. This detects mirrored
module offsets even when a camera perfectly follows the misplaced scope.
"""
import argparse
import json
from pathlib import Path
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import make_pack as b
from rail_alignment import Rails


def recover(source, emitted):
    a, c = source.mean(0), emitted.mean(0)
    u, _, vt = np.linalg.svd((source - a).T @ (emitted - c))
    rotation = u @ np.diag([1., 1., np.linalg.det(u @ vt)]) @ vt
    translation = c - a @ rotation
    np.testing.assert_allclose(source @ rotation + translation, emitted, atol=2e-6,
                               err_msg='rendered mount is not the native rigid part')
    frame = np.eye(4)
    frame[:3, :3], frame[3, :3] = rotation, translation
    return frame


def check(pack, donor):
    game = b.model.config(donor / 'gameplay/game.options')
    profiles = json.loads((b.HERE / 'legacy-mounts.json').read_text())
    rails = Rails(donor, b)
    checked = 0
    for row in json.loads((pack / 'coverage.json').read_text()):
        chain = json.loads(row['chain'])
        if profile := profiles.get(row['donor_weapon']):
            chain = [dict(module=profile['module'], locator=profile['locator'], slot='scope_coverage_rail')]
        if not chain:
            continue
        idle = b.lib.model_dir(pack / 'files', f'weapons/attach/coverage/{row["variant"]}')
        module = b.lib.model_dir(donor, rails.optics[row['scope']])
        origin = b.sf.translation(idle, module)
        if origin is None:
            raise ValueError(f'{row["variant"]}: cannot locate optic body')
        step = chain[-1]
        defaults = {r['slot']: r['module'] for r in b.audit.sequence(game['weapons'][row['donor_weapon']]['default_modules'])}
        if defaults.get(step['slot']) == step['module']:
            continue  # this socket belongs to a module baked into the weapon
        source = b.lib.model_dir(donor, game['weapon_modules'][step['module']]['model'])
        surface = b.lib.surfaces(source)[0]
        original = b.lib.positions(source / 'render' / surface / 'converted_model')
        emitted = b.lib.positions(idle / 'render' / surface / 'converted_model')
        frame = recover(original, emitted)
        socket = b.mounts.local(b.mounts.locator(source, step['locator'])) @ frame
        np.testing.assert_allclose(socket[3, :3], origin, atol=2e-5,
                                   err_msg=f'{row["variant"]}: optic does not sit at the rendered rail socket')
        np.testing.assert_allclose(socket[:3, :3], np.eye(3), atol=2e-5,
                                   err_msg=f'{row["variant"]}: optic turned away from rail socket')
        checked += 1
    for row in json.loads((pack / 'centering.json').read_text()):
        if not any(row['optic_shift']):
            continue
        cfg = b.model.config(pack / 'review/gameplay/items/scopes' / f'{row["variant"]}.options')
        idle = b.lib.model_dir(pack / 'files', cfg['data']['idle_model'])
        shift, _, rail = rails.measure(cfg['srp_scope_fit']['scope'], idle)
        if rail is None or shift.any():
            raise ValueError(f'{row["variant"]}: existing rail correction did not seat optic ({shift})')
        checked += 1
    print(f'{checked} rendered adapter assemblies: optic origin and orientation agree with the physical socket')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pack', type=Path, required=True)
    parser.add_argument('--donor', type=Path, required=True)
    args = parser.parse_args()
    check(args.pack, args.donor)
