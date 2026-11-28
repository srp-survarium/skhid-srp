"""build_content.py's memory budget: the grass world's size, its cells, and the rows of MEMORY_KNOWN."""
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_content as bc  # noqa: E402


def layer(points):
    """A .veg layer of these (x, z) positions: chunk 0x58 (positions) and 0x59 (normals)."""
    positions = b''.join(struct.pack('<3f', x, 0.0, z) for x, z in points)
    normals = b''.join(struct.pack('<3f', 0.0, 1.0, 0.0) for _ in points)
    return (struct.pack('<II', 0x58, len(positions)) + positions
            + struct.pack('<II', 0x59, len(normals)) + normals)


class GrassWorld(unittest.TestCase):
    def test_cells_are_a_layers_16_metre_squares(self):
        # 0 and 15.9 share a square, 16 starts the next, -0.1 is the square before 0.
        self.assertEqual(bc.grass_cells('a.veg', layer([(0, 0), (15.9, 15.9), (16, 0), (-0.1, 0), (0, -0.1)])),
                         (5, 4))

    def test_a_layer_without_positions_is_refused(self):
        with self.assertRaises(bc.Problem):
            bc.grass_cells('a.veg', struct.pack('<II', 0x59, 0))

    def test_fire_stations_measured_block(self):
        # Stalker Maps 2@5's 14 layers, as the engine's profile has them (77,006,472 there: 8-byte rounding).
        self.assertEqual(bc.grass_world_bytes([(120_000, 3_690)] + [(0, 0)] * 2), 3_690 * 0x40C0 + 120_000 * 0x84
                         + 0x24 + 0x20C)

    def test_no_grass_no_block(self):
        self.assertEqual(bc.grass_world_bytes([]), 0)
        self.assertEqual(bc.grass_world_bytes([(0, 0)]), 0)


class Known(unittest.TestCase):
    known = {'big': (100, 'why')}

    def test_within_its_bound(self):
        self.assertEqual(bc.memory_problems({'a': (10, 10)}, {}), [])

    def test_over_its_bound(self):
        self.assertEqual(len(bc.memory_problems({'a': (11, 10)}, {})), 1)

    def test_known_up_to_its_bytes(self):
        self.assertEqual(bc.memory_problems({'big': (100, 10)}, self.known), [])
        self.assertIn('more than', bc.memory_problems({'big': (101, 10)}, self.known)[0])

    def test_a_known_row_within_its_bound_is_stale(self):
        self.assertIn('remove its MEMORY_KNOWN row', bc.memory_problems({'big': (9, 10)}, self.known)[0])

    def test_a_known_row_of_nothing(self):
        self.assertIn('nothing the build has', bc.memory_problems({}, self.known)[0])


if __name__ == '__main__':
    unittest.main()
