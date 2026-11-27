import math
from pathlib import Path
import struct
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_joins import FileView, audit, bcw, joins  # noqa: E402

HEAD_BONE, NECK_BONE = 44, 21
LEG = {1: 15, -1: 10}
FOOT = {1: 16, -1: 11}


def tube(center, low, high, radius, bone, sides=24, rings=4):
    """An open cylinder about a vertical axis, every vertex on one bone."""
    pos, idx = [], []
    for k in range(rings + 1):
        y = low + (high - low) * k / rings
        for i in range(sides):
            a = 2 * math.pi * i / sides
            pos.append((center[0] + radius * math.sin(a), y, center[1] + radius * math.cos(a)))
    for k in range(rings):
        for i in range(sides):
            a, b = k * sides + i, k * sides + (i + 1) % sides
            idx += [(a, b, a + sides), (b, b + sides, a + sides)]
    n = len(pos)
    return joins.Mesh(np.array(pos), np.full((n, 4), bone, np.uint8), np.tile([1.0, 0, 0, 0], (n, 1)),
                      np.full((n, 12), 127, np.uint8), np.zeros((n, 2)), np.array(idx))


def merged(*meshes):
    out = meshes[0]
    for m in meshes[1:]:
        out = joins.Mesh(np.r_[out.pos, m.pos], np.r_[out.bones, m.bones], np.r_[out.weights, m.weights],
                         np.r_[out.basis, m.basis], np.r_[out.uv, m.uv], np.r_[out.idx, m.idx + len(out.pos)])
    return out


def bind():
    """A 69-bone render bind with the shins' Leg and Foot where a body has them."""
    frames = {LEG[1]: (0.13, 0.47, 0), LEG[-1]: (-0.13, 0.47, 0), FOOT[1]: (0.15, 0.1, 0), FOOT[-1]: (-0.15, 0.1, 0)}
    out = struct.pack('<H', 69)
    for i in range(69):
        out += f'B{i}'.encode() + b'\0' + struct.pack('<9f', *frames.get(i, (0, 0, 0)), 0, 0, 0, 1, 1, 1)
    return out


def item(files, slot, name, surfaces, **data):
    """An item config whose data names the surfaces, each a model of its own."""
    parts = []
    for k, mesh in enumerate(surfaces):
        base = f'{name}_{k}'
        files[f'models/{base}.skinned_model/render/s/converted_model'] = joins.write_mesh(mesh)
        files[f'models/{base}.skinned_model/render/bind_pose'] = bind()
        parts.append({'base_model': base, 'part_name': 's'})
    entry = dict(parts[0], **data)
    if parts[1:]:
        entry['additional_models'] = parts[1:]
    files[f'gameplay/items/armour/{slot}/{name}.options'] = bcw.dump({'data': entry})


def world(helmet_neck, low_boot_top):
    files = {}
    files['models/head.skinned_model/render/h/converted_model'] = joins.write_mesh(
        merged(tube((0, -0.04), 1.48, 1.62, 0.06, NECK_BONE), tube((0, -0.04), 1.62, 1.80, 0.09, HEAD_BONE)))
    files['gameplay/bodyparts/default'] = bcw.dump(
        {'body_parts': {'head': {'data': {'base_model': 'head', 'part_name': 'h'}}}})
    item(files, 'torso', 'vest', [tube((0, -0.04), 1.10, 1.50, 0.10, 19)])
    helmet = [tube((0, -0.04), 1.56, 1.80, 0.11, HEAD_BONE)]
    if helmet_neck:
        helmet.append(tube((0, -0.04), 1.45, 1.60, 0.06, NECK_BONE))
    item(files, 'helmet', 'closed', helmet, remove_head_model=True)
    shins = [tube((s * 0.14, 0), 0.21, 0.80, 0.06, LEG[s]) for s in (1, -1)]
    item(files, 'torso', 'suit', shins, additional_model_slots=['back', 'pants'])
    item(files, 'boots', 'low', [tube((s * 0.14, 0), 0.0, low_boot_top, 0.07, FOOT[s]) for s in (1, -1)])
    item(files, 'boots', 'exo', [tube((s * 0.14, 0), 0.0, 0.23, 0.07, FOOT[s]) for s in (1, -1)])
    return FileView(files)


def holes(rows, rule, piece, other):
    return next(r for r in rows if r['rule'] == rule and r['piece'].endswith(f'/{piece}.options')
                and r['with'].endswith(f'/{other}.options'))['holes']


class JoinTests(unittest.TestCase):
    def test_a_closed_helmet_without_a_neck_leaves_the_collar_open(self):
        rows = audit(world(helmet_neck=False, low_boot_top=0.15))
        self.assertGreater(holes(rows, 'neck', 'closed', 'vest'), 0)
        self.assertGreater(holes(rows, 'shins', 'suit', 'low'), 0)
        self.assertEqual(holes(rows, 'shins', 'suit', 'exo'), 0)

    def test_a_neck_of_its_own_and_boots_over_the_hem_close_them(self):
        rows = audit(world(helmet_neck=True, low_boot_top=0.24))
        self.assertEqual([r for r in rows if r['holes']], [])

    def test_a_body_without_the_trousers_slot_is_not_a_shin_case(self):
        rows = audit(world(helmet_neck=True, low_boot_top=0.24))
        self.assertFalse(any(r['rule'] == 'shins' and r['piece'].endswith('/vest.options') for r in rows))


if __name__ == '__main__':
    unittest.main()
