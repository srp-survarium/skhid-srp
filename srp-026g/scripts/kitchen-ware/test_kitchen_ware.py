"""kitchen-ware@1: a thing's capsule, the jugs' shards at the maps' scale, settings without collision."""
import math
import struct
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bottle-shards'))
from test_bottle_shards import mesh  # noqa: E402

import importlib.util

_spec = importlib.util.spec_from_file_location('kitchen_ware_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)


def cylinder(radius, height, sides=12, rings=3):
    """A closed-side cylinder about the y axis, its base at 0."""
    triangles = []
    for r in range(rings):
        y0, y1 = height * r / rings, height * (r + 1) / rings
        for k in range(sides):
            a0, a1 = 2 * math.pi * k / sides, 2 * math.pi * (k + 1) / sides
            p = lambda a, y: (radius * math.cos(a), y, radius * math.sin(a), k / sides, y / height)
            triangles += [[p(a0, y0), p(a1, y0), p(a1, y1)], [p(a0, y0), p(a1, y1), p(a0, y1)]]
    return mesh(triangles)


class KitchenWare(unittest.TestCase):
    def test_a_jug_breaks_into_eight_shards_each_about_its_centre(self):
        data = cylinder(0.1, 0.31)
        shards = pack.bottle.shards(data, pack.cut_of(data))
        self.assertEqual(len(shards), pack.SHARDS)
        for blob, points, triangles, centre in shards:
            low, high = pack.mesh_tools.bounds(points)
            self.assertTrue(all(abs(lo + hi) < 1e-6 for lo, hi in zip(low, high)))
            self.assertGreater(triangles, 4)
            self.assertTrue(0 <= centre[1] <= 0.31 and math.hypot(centre[0], centre[2]) <= 0.1 + 1e-6)

    def test_the_jugs_shards_are_cut_at_the_scale_the_maps_place_them(self):
        data = cylinder(0.1, 0.31)
        big = pack.scaled(data, 1.225)
        top = max(p[1] for p in pack.points_of(big))
        self.assertAlmostEqual(top, 0.31 * 1.225, places=5)
        # its vectors and UVs as they were
        self.assertEqual([r[12:] for r in pack.mesh_tools.mesh(big)[0]], [r[12:] for r in pack.mesh_tools.mesh(data)[0]])
        self.assertEqual(set(pack.SCALE), {n for n in pack.CERAMIC if n.endswith('pitcher_01')})

    def test_settings_lose_their_collision_and_keep_the_surfaces_asked(self):
        T = pack.mesh_tools
        settings = T.serialize((T.T_NAMED, [
            ('lod_hierrarchy', (T.T_NAMED, [('LOD0', (T.T_NAMED, [('surfaces', (T.T_INDEXED, [(T.T_STR, 'a_s'), (T.T_STR, 'b_s')]))]))])),
            ('material_settings', (T.T_NAMED, [('a_s', (T.T_NAMED, [])), ('b_s', (T.T_NAMED, []))])),
            ('collision_enabled', (T.T_BOOL, 1))]))
        out = T.parse(bytearray(pack.settings_of(settings, keep={'b_s'})))
        kids = dict(out[1])
        self.assertEqual(pack.lod0(out), ['b_s'])
        self.assertEqual([k for k, _ in kids['material_settings'][1]], ['b_s'])
        self.assertEqual({k: kids[k][1] for k in pack.cap_pack.COLLISION}, {k: 0 for k in pack.cap_pack.COLLISION})


if __name__ == '__main__':
    unittest.main()
