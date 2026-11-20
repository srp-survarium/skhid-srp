"""Native attachment frames transferred through the receiver's measured geometry.

All matrices use row vectors. The weapon attachment needs the engine's half
turn; static module sockets already use their mesh frame. Applying that turn
again mirrors long handguard sockets away from their rails.
"""
import struct

import numpy as np

from audit import model, effective_module
from geometry import fit_frame, lib


def bind(directory):
    data = (directory / 'render/bind_pose').read_bytes()
    result, at = {}, 2
    for _ in range(struct.unpack_from('<H', data)[0]):
        end = data.index(0, at)
        values = struct.unpack_from('<9f', data, end + 1)
        result[data[at:end].decode()] = model.matrix(values[:3], values[3:6], values[6:])
        at = end + 37
    if at != len(data):
        raise ValueError(f'{directory}: trailing bind pose data')
    return result


def locator(directory, name):
    return next(row for row in model.config(directory / 'render/export_properties')['locators']
                if row['name'] == name)


def local(row):
    return model.matrix(row['position'], row.get('rotation', (0, 0, 0)))


def assembly(game, donor, directory, weapon, root, steps):
    """Optic and mount-part frames in the donor weapon's bind space."""
    loc = locator(directory, root)
    bones = bind(directory)
    bone = list(bones)[loc['bone_idx']]
    frame = model.HALF @ local(loc) @ bones[bone]
    parts = []
    for step in steps:
        module = effective_module(game, step['module'], weapon)
        part = lib.model_dir(donor, module['model'])
        if part is None:
            raise ValueError(f'missing module {module["model"]}')
        parts.append((step['module'], part, frame.copy()))
        frame = local(locator(part, step['locator'])) @ frame
    return bone, frame, parts


def transfer(live, donor):
    match = fit_frame(live, donor)
    transform = np.eye(4)
    transform[:3, :3], transform[3, :3] = match.rotation, match.translation
    return transform, match


def attach_frame(directory, donor_bone, optic_frame):
    bones = bind(directory)
    if donor_bone not in bones:
        raise ValueError(f'{directory}: missing attachment bone {donor_bone}')
    return list(bones).index(donor_bone), model.HALF @ optic_frame @ np.linalg.inv(bones[donor_bone])
