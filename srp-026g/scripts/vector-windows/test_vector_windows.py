"""vector-windows@1: a building's glass found as its partitions' bays (two-sided parts in a plane, meeting along
it) and its flat glass, and a broken bay's glass left along its frame."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bottle-shards'))
from test_bottle_shards import mesh  # noqa: E402

import importlib.util

_spec = importlib.util.spec_from_file_location('vector_windows_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)


def quad(x0, x1, y0, y1, z):
    """A pane across z: two triangles (x, y, z, u, v)."""
    a, b, c, d = (x0, y0, z, 0, 0), (x1, y0, z, 1, 0), (x1, y1, z, 1, 1), (x0, y1, z, 0, 1)
    return [[a, b, c], [a, c, d]]


def building():
    triangles = []
    for z in (-27.50, -27.52):                       # each part and its twin
        triangles += quad(60.13, 61.23, 20.79, 21.51, z)     # bay 0: a low pane
        triangles += quad(60.13, 60.38, 21.53, 23.33, z)     # and a strip over it
        triangles += quad(61.28, 62.58, 20.80, 21.50, z)     # bay 1, 5 cm on: a frame's post between
    # a tabletop's glass, flat
    triangles += [[(50, 21, -20, 0, 0), (51, 21, -20, 1, 0), (51, 21, -19, 1, 1)]]
    return mesh(triangles)


class Windows(unittest.TestCase):
    def test_the_glass_is_its_bays_and_its_flat_glass(self):
        bays, flat = pack.bays(building())
        self.assertEqual(len(bays), 2)
        self.assertEqual([len(b.parts) for b in bays], [4, 2], 'a part and its twin, a pane and its strip')
        self.assertEqual(len(flat), 1)
        self.assertTrue(all(b.axis == 2 for b in bays))
        self.assertAlmostEqual(bays[0].low[0], 60.13, places=2)
        self.assertAlmostEqual(bays[1].high[0], 62.58, places=2)

    def test_a_broken_bay_keeps_its_glass_along_its_frame(self):
        data = building()
        bay = pack.bays(data)[0][1]
        blob, points, triangles = pack.remains(data, bay)
        self.assertGreater(triangles, 20)
        reach = pack.EDGE + pack.EDGE_JITTER + pack.STEP
        for p in points:
            x, y = p[0] + bay.centre[0], p[1] + bay.centre[1]
            d = min(x - 61.28, 62.58 - x, y - 20.80, 21.50 - y)
            self.assertLess(d, reach, 'only glass along the frame stays')
        # the twins alike: as many triangles on each side
        sides = [round(p[2] + bay.centre[2], 2) for p in points]
        self.assertEqual(sides.count(-27.5), sides.count(-27.52))


if __name__ == '__main__':
    unittest.main()
