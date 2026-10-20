"""python3 -m unittest discover -s srp-026g/scripts/weapon-camo (needs numpy and Pillow)."""
import io
import struct
import unittest

import numpy as np
from PIL import Image

import weapon_camo as w


def mesh(quads):
    """A converted_model of 36-byte vertices (UV last) with one quad (two triangles) per
    (u0, v0, u1, v1)."""
    verts, idx = [], []
    for u0, v0, u1, v1 in quads:
        n = len(verts)
        verts += [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
        idx += [n, n + 1, n + 2, n, n + 2, n + 3]
    body = b''.join(bytes(28) + struct.pack('<2f', u, v) for u, v in verts)
    out = struct.pack('<III', 3, 4 + len(body), len(verts)) + body
    return out + struct.pack('<III', 4, 4 + 2 * len(idx), len(idx)) + struct.pack('<%dH' % len(idx), *idx)


class Test(unittest.TestCase):
    def test_dds_is_the_stock_layout_and_decodes(self):
        img = np.zeros((64, 64, 3), np.float32)
        img[:, :32] = (1, 0.5, 0)
        img[:, 32:] = (0, 1, 0.5)
        blob = w.dds_dxt1(img)
        flags, h, wd, linear, _, mips = struct.unpack_from('<6I', blob, 8)
        self.assertEqual((flags, h, wd, mips), (0xA100F, 64, 64, 5))
        self.assertEqual(linear, len(blob) - 128)          # the bytes after the header, one spare
        back = np.asarray(Image.open(io.BytesIO(blob)).convert('RGB'), np.float32) / 255
        self.assertLess(np.abs(back - img).max(), 0.05)

    def test_islands_split_at_corners_and_keep_shared_edges(self):
        quads = [(0.0, 0.0, 0.5, 0.5), (0.5, 0.5, 1.0, 1.0),     # touch at one corner: two islands
                 (0.0, 0.5, 0.25, 1.0), (0.25, 0.5, 0.5, 1.0)]  # share an edge: one island
        uv, tris = w.mesh_uv(mesh(quads))
        labels = w.islands([(uv, tris)], (64, 64))
        a, b, c, d = labels[10, 10], labels[50, 50], labels[50, 5], labels[50, 25]
        self.assertTrue(a and b and c)
        self.assertNotEqual(a, b)
        self.assertEqual(c, d)

    def test_a_part_one_donor_repaints_is_painted(self):
        base = np.full((64, 64, 3), 0.1, np.float32)
        donor = base.copy()
        donor[:32] = (0.2, 0.3, 0.6)                          # the top half repainted blue
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 0.45), (0, 0.55, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        mask, decisions = w.paint_mask(base, [donor], labels)
        self.assertTrue(mask[10].all() and not mask[50].any())
        a = w.aoac(base, mask, labels)
        self.assertTrue(np.allclose(a[50, :, 1], 1) and np.allclose(a[50, :, 2], 0.5) and np.allclose(a[50, :, 0], 1))

    def test_the_suggestion_leaves_small_parts_bare(self):
        base = np.full((64, 64, 3), 0.1, np.float32)
        donor = np.full((64, 64, 3), 0.6, np.float32)          # the premium repainted all of it
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 0.5), (0, 0.6, 0.05, 0.65)]))
        labels = w.islands([(uv, tris)], (64, 64))
        _, decisions = w.paint_mask(base, [donor], labels)
        self.assertEqual(sum(1 for _, yes, _ in decisions.values() if yes), 2)
        self.assertEqual(w.suggest(decisions, labels), {labels[10, 10]})

    def test_a_part_whole_uvs_away_lands_where_it_samples(self):
        uv, tris = w.mesh_uv(mesh([(-5.8, 10.1, -5.6, 10.5)]))   # the TOZ-66 lays parts out so
        labels = w.islands([(uv, tris)], (64, 64))
        self.assertTrue(labels[19, 19] and not labels[40, 40])  # u 0.2..0.4, v 0.1..0.5

    def test_a_part_inside_the_weapon_is_not_seen(self):
        cube = np.array([[x, y, z] for x in (-1, 1) for y in (-1, 1) for z in (-1, 1)], float)
        faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        box = np.array([t for a, b, c, d in faces for t in ((a, b, c), (a, c, d))])
        quad = lambda x0, uv0: (np.array([[x0, -0.5, -0.5], [x0, 0.5, -0.5], [x0, 0.5, 0.5], [x0, -0.5, 0.5]]),
                                np.array([[uv0, 0.1], [uv0 + 0.3, 0.1], [uv0 + 0.3, 0.4], [uv0, 0.4]]),
                                np.array([[0, 1, 2], [0, 2, 3]]))
        inner, outer = quad(0.0, 0.1), quad(3.0, 0.6)                 # inside the box, beside it
        meshes = {'gun': [('box', cube, np.full((8, 2), 0.9), box), ('inner',) + inner, ('outer',) + outer]}
        labels = w.islands([inner[1:], outer[1:]], (64, 64))
        seen = w.visibility(meshes, {'gun:inner', 'gun:outer'}, labels, directions=16, res=96)
        self.assertLess(seen[labels[16, 16]], w.HIDDEN)
        self.assertGreater(seen[labels[16, 48]], w.HIDDEN)

    def test_curves_of_any_length(self):
        curve = lambda k, v: struct.pack('<I', k) + struct.pack('<%df' % (2 * (k - 1)), *[0.0, v] * (k - 1)) + struct.pack('<fI', 36.0, 7)
        values = [0.5 * i for i in range(9)]
        sp = struct.pack('<I', 1) + bytes(8 * 9) + b''.join(curve(8 if i == 4 else 5, v) for i, v in enumerate(values))
        self.assertEqual(w.curve_starts(sp, 't'), values)
        with self.assertRaises(AssertionError):
            w.curve_starts(sp + b'x', 't')

    def test_a_models_surfaces_from_flattened_files(self):
        import tempfile, os
        with tempfile.TemporaryDirectory() as d:
            for f in ('models__weapons__uzi.skinned_model__render__uzisg1__converted_model',
                      'models__weapons__uzi.skinned_model__render__uzisg1__export_properties',
                      'models__weapons__uzi.skinned_model__render__bind_pose',
                      'models__weapons__uzi_camo.skinned_model__render__x__converted_model'):
                open(os.path.join(d, f), 'wb').close()
            self.assertEqual(w.Files(d).surfaces('weapons/uzi'), ['uzisg1'])

    def test_parts_tsv_names_parts_by_a_point_on_each(self):
        uv, tris = w.mesh_uv(mesh([(0, 0, 0.45, 1), (0.55, 0, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        points = w.island_points(labels)
        right = labels[30, 50]
        self.assertEqual(w.parts_islands([points[right]], labels, 't'), {right})
        with self.assertRaises(SystemExit):
            w.parts_islands([(0.5, 0.5)], labels, 't')           # the gap between the parts


if __name__ == '__main__':
    unittest.main()
