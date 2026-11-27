"""lab-capsule-glass@1: each half of a capsule's glass cut into 16 two-sided shards over its whole surface,
each with its glass's outward normal and its long side."""
import math
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bottle-shards'))
from test_bottle_shards import mesh  # noqa: E402

import importlib.util

_spec = importlib.util.spec_from_file_location('lab_capsule_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)


def pane(axis=(0.0, 0.016)):
    """A half cylinder of radius 0.69 m about `axis` (x, z) from 0.61 to 3.07 m up, in long slivers, as the
    capsule's glass."""
    triangles = []
    for k in range(10):
        a0, a1 = math.pi * k / 10, math.pi * (k + 1) / 10
        p = lambda a, y: (axis[0] + 0.69 * math.cos(a), y, axis[1] + 0.69 * math.sin(a), a / math.pi, y / 3.07)
        triangles += [[p(a0, 0.61), p(a1, 0.61), p(a1, 3.07)], [p(a0, 0.61), p(a1, 3.07), p(a0, 3.07)]]
    return mesh(triangles)


class Capsule(unittest.TestCase):
    def test_the_pane_breaks_into_16_shards_none_empty_spread_over_it(self):
        data = pane()
        shards = pack.bottle.shards(data, pack.cut(data))
        self.assertEqual(len(shards), pack.SHARDS)
        centres = [c for *_, c in shards]
        heights = sorted(c[1] for c in centres)
        self.assertLess(heights[0], 1.2)
        self.assertGreater(heights[-1], 2.5)
        for blob, points, triangles, centre in shards:
            self.assertEqual(triangles % 2, 0, 'each triangle and its back')
            self.assertLess(triangles, 2000, 'a shard a few hundred triangles')
            low, high = pack.bottle.mesh_tools.bounds(points)
            self.assertLess(max(hi - lo for lo, hi in zip(low, high)), 1.6, 'pieces, not the pane')

    def test_the_glass_s_axis_is_its_cylinder_s(self):
        points = pack.ware.points_of(pane((-0.817, -0.016)))
        ax, az = pack.axis_of(points)
        self.assertAlmostEqual(ax, -0.817, places=3)
        self.assertAlmostEqual(az, -0.016, places=3)

    def test_a_shard_s_long_side_is_its_greatest_spread(self):
        tall = [(0.01 * (k % 3), 0.1 * k, 0.0) for k in range(10)]
        along = pack.principal(tall)
        self.assertGreater(abs(along[1]), 0.99)
        self.assertAlmostEqual(math.hypot(*along), 1.0, places=6)

    def test_both_halves_are_their_own_models(self):
        self.assertEqual(pack.CAPSULE.broken, 'props/chemicalflaskcentre/srp_lab_flask_broken')
        self.assertEqual(pack.DOOR.broken, 'props/chemicalflaskcentre/srp_lab_flaskdoor_broken')
        self.assertEqual(len(pack.DOOR.shard_names), pack.SHARDS)
        self.assertFalse(set(pack.CAPSULE.shard_names) & set(pack.DOOR.shard_names))


if __name__ == '__main__':
    unittest.main()
