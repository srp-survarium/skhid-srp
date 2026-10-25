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

    def test_a_bare_texture_gets_a_neutral_aoac(self):
        base = np.random.default_rng(1).random((64, 64, 3)).astype(np.float32)
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        self.assertEqual(w.parts_islands([], labels, 't'), set())   # parts.tsv: a line with no parts
        a = w.aoac(base, np.zeros((64, 64), bool), labels)
        self.assertTrue(np.allclose(a[..., 0], 1) and np.allclose(a[..., 1], 1) and np.allclose(a[..., 2], 0.5))
        self.assertLess(np.abs(w.bake(base, np.zeros_like(base), a, 4.0) - base).max(), 1e-5)


if __name__ == '__main__':
    unittest.main()
