"""lab-jars@1: a rack's glass found as its ten jars, and a broken jar's stump under a jagged rim."""
import math
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bottle-shards'))
from test_bottle_shards import mesh  # noqa: E402

import importlib.util

_spec = importlib.util.spec_from_file_location('lab_jars_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)


def jar(x, z, sides=8):
    """A jar's wall: a cylinder 36 cm across from 0.8 to 1.86 m up at (x, z)."""
    triangles = []
    for k in range(sides):
        a0, a1 = 2 * math.pi * k / sides, 2 * math.pi * (k + 1) / sides
        p = lambda a, y: (x + 0.18 * math.cos(a), y, z + 0.18 * math.sin(a), a / (2 * math.pi), y)
        triangles += [[p(a0, 0.8), p(a1, 0.8), p(a1, 1.86)], [p(a0, 0.8), p(a1, 1.86), p(a0, 1.86)]]
    return triangles


def rack():
    return mesh([t for i in range(pack.JARS) for t in jar(-1.7 + 0.6 * (i // 2), -0.39 + 0.79 * (i % 2))])


class Jars(unittest.TestCase):
    def test_the_glass_is_ten_jars_in_order(self):
        found = pack.parts(rack())
        self.assertEqual(len(found), pack.JARS)
        centres = [c for c, _, _ in found]
        self.assertEqual(centres, sorted(centres, key=lambda c: (round(c[0], 1), round(c[2], 1))))
        for centre, (low, high), group in found:
            self.assertAlmostEqual(high[0] - low[0], 0.36, places=2)
            self.assertAlmostEqual(high[1] - low[1], 1.06, places=2)
            self.assertEqual(len(group), 16)

    def test_a_stump_is_the_jar_s_bottom_under_a_jagged_rim(self):
        data = mesh(jar(0.0, 0.0, sides=16))
        centre, low = [0.0, 1.33, 0.0], [-0.18, 0.8, -0.18]
        blob, points, triangles = pack.stump(data, 3, centre, low)
        heights = [p[1] - (low[1] - centre[1]) for p in points]
        self.assertGreater(triangles, 50)
        self.assertLess(max(heights), pack.RIM + pack.RIM_JITTER + 0.05)
        self.assertGreater(max(heights), pack.RIM - pack.RIM_JITTER)
        rims = {round(pack.rim(3, a / 10), 3) for a in range(63)}
        self.assertGreater(len(rims), 10, 'jagged, not level')
        self.assertTrue(all(pack.RIM - pack.RIM_JITTER <= r <= pack.RIM + pack.RIM_JITTER for r in rims))

    def test_the_glass_over_the_rim_breaks_into_strips_around_the_jar(self):
        data = mesh(jar(0.0, 0.0, sides=16))
        centre, low = [0.0, 1.33, 0.0], [-0.18, 0.8, -0.18]
        over = pack.blob_of(pack.split(data, 3, centre, low, pack.OVER_STEP)[1], [0, 0, 0])[0]
        shards = pack.bottle.shards(over, pack.cut(over, centre))
        self.assertEqual(len(shards), pack.SHARDS)
        for _, points, triangles, c in shards:
            bottom, top = pack.mesh_tools.bounds(points)
            self.assertGreater(top[1] - bottom[1], 0.7, "a strip the jar's height")
            self.assertLess(max(top[0] - bottom[0], top[2] - bottom[2]), 0.35, 'a part of the way round, not a ring')
            self.assertGreater(math.hypot(c[0], c[2]), 0.05, 'off the axis: an outward normal')

    def test_the_rack_s_collision_loses_its_jars_glass_alone(self):
        prim = lambda mtl: (3, [('type', (1, 2)), ('mtl', (1, mtl))])
        tree = (3, [('primitives', (4, [prim(0), prim(1), prim(0), prim(1), prim(1)])),
                    ('mtl_list', (4, [(5, 'coll_glass'), (5, 'coll_metall_thick')]))])
        data, kept, dropped = pack.without_glass(bytes(pack.ware.serialize(tree)))
        self.assertEqual((kept, dropped), (3, 2))
        left = dict(pack.parse(bytearray(data))[1])
        self.assertEqual([dict(p[1])['mtl'][1] for p in left['primitives'][1]], [1, 1, 1])
        self.assertEqual(left['mtl_list'][1], [(5, 'coll_glass'), (5, 'coll_metall_thick')])


if __name__ == '__main__':
    unittest.main()
