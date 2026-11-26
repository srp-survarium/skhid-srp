"""The bottle's shards: the cut, the grouping, both sides."""
import math
import struct
import unittest

import make_pack as pack


def mesh(triangles):
    """A converted_model of `triangles`: each three (x, y, z, u, v), its
    attribute vectors (0.6, 0.8, 0), (0, 0, 1), (1, 0, 0)."""
    vectors = bytes([round((c + 1) * 127.5) for c in (0.6, 0.8, 0.0)] + [0]
                    + [round((c + 1) * 127.5) for c in (0.0, 0.0, 1.0)] + [255]
                    + [round((c + 1) * 127.5) for c in (1.0, 0.0, 0.0)] + [255])
    records, indices = [], []
    for corners in triangles:
        for x, y, z, u, v in corners:
            indices.append(len(records))
            records.append(struct.pack('<3f', x, y, z) + vectors + struct.pack('<2e', u, v))
    return (struct.pack('<III', 3, 4 + 28 * len(records), len(records)) + b''.join(records)
            + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices)) + struct.pack('<%dH' % len(indices), *indices))


def area(points):
    a, b, c = points
    ab, ac = [q - p for p, q in zip(a, b)], [q - p for p, q in zip(a, c)]
    cross = (ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0])
    return math.sqrt(sum(c * c for c in cross)) / 2


class Shards(unittest.TestCase):
    def test_a_large_triangle_is_cut_to_steps_its_area_and_uvs_kept(self):
        corners = [(0., 0., 0., 0., 0.), (0.06, 0., 0., 1., 0.), (0., 0.06, 0., 0., 1.)]
        small = pack.subdivided(mesh([corners]))
        self.assertEqual(len(small), 25)    # its longest side 8.5 cm, at most 2 cm a step: 5 x 5
        self.assertAlmostEqual(sum(area([p[0] for p in t]) for t in small), area([c[:3] for c in corners]), places=7)
        for triangle in small:
            for position, uv, vectors, ws in triangle:
                self.assertTrue(max(math.dist(p[0], q[0]) for p, q in zip(triangle, triangle[1:] + triangle[:1]))
                                <= pack.STEP + 1e-6)
                self.assertAlmostEqual(uv[0], position[0] / 0.06, places=2)
                self.assertAlmostEqual(uv[1], position[1] / 0.06, places=2)
                self.assertEqual(ws, [0, 255, 255])

    def bottle(self):
        """A small triangle at each seed and one on the neck: every shard has one."""
        triangles = []
        for x, y, z in pack.SEEDS + [(0.0, 0.25, 0.0)]:
            triangles.append([(x, y, z, 0.1, 0.1), (x + 0.005, y, z, 0.2, 0.1), (x, y + 0.005, z, 0.1, 0.2)])
        return mesh(triangles)

    def test_the_neck_is_one_shard_and_each_seed_its_own_both_sides_about_its_centre(self):
        shards = pack.shards(self.bottle())
        self.assertEqual(len(shards), pack.SHARDS)
        self.assertGreater(shards[0][3][1], pack.NECK, 'shard 0 is the neck')
        for blob, points, triangles, centre in shards:
            self.assertEqual(triangles, 2, 'one triangle, and its back')
            records, indices = pack.mesh_tools.mesh(blob)
            low, high = pack.mesh_tools.bounds(points)
            self.assertTrue(all(abs(lo + hi) < 1e-6 for lo, hi in zip(low, high)), 'its origin its centre')
            front, back = records[:3], records[3:]
            self.assertEqual([r[:12] for r in front], [r[:12] for r in back[::-1]], 'the back: the same corners reversed')
            for f, b in zip(front, back[::-1]):
                for k in range(3):
                    for i in range(3):
                        self.assertLessEqual(abs(f[12 + 4 * k + i] + b[12 + 4 * k + i] - 255), 1, 'its vectors negated')

    def test_a_bottle_with_a_shard_empty_is_refused(self):
        with self.assertRaisesRegex(AssertionError, 'an empty shard'):
            pack.shards(mesh([[(0., 0.25, 0., 0., 0.), (0.005, 0.25, 0., 1., 0.), (0., 0.255, 0., 0., 1.)]]))


if __name__ == '__main__':
    unittest.main()
