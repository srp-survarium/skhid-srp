"""python3 -m unittest discover -s srp-026g/scripts/weapon-camo (needs numpy and Pillow)."""
import io
import struct
import unittest

import numpy as np
from PIL import Image

import weapon_camo as w


def mesh(quads, corners=None):
    """A converted_model of 36-byte vertices (position first, UV last) with one quad (two triangles)
    per (u0, v0, u1, v1); `corners`: each quad's four positions (all at the origin without)."""
    verts, idx = [], []
    for k, (u0, v0, u1, v1) in enumerate(quads):
        n = len(verts)
        xyz = corners[k] if corners else [(0, 0, 0)] * 4
        verts += list(zip(xyz, [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]))
        idx += [n, n + 1, n + 2, n, n + 2, n + 3]
    body = b''.join(struct.pack('<3f', *xyz) + bytes(16) + struct.pack('<2f', u, v) for xyz, (u, v) in verts)
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

    def test_a_painted_versions_repaint_is_measured_part_by_part(self):
        base = np.full((64, 64, 3), 0.1, np.float32)
        donor = base.copy()
        donor[:32] = (0.2, 0.3, 0.6)                          # the top half repainted blue
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 0.45), (0, 0.55, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        share = w.shares(labels, w.repainted(base, donor))
        self.assertGreater(share[labels[10, 10]], 0.9)
        self.assertLess(share[labels[50, 10]], 0.1)

    def test_an_aoac_changes_nothing_off_the_painted_parts(self):
        base = np.full((64, 64, 3), 0.1, np.float32)
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 0.45), (0, 0.55, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        a = w.aoac(base, labels == labels[10, 10], labels)
        self.assertTrue((a[10, :, 1] < 0.5).all())
        self.assertTrue(np.allclose(a[50, :, 1], 1) and np.allclose(a[50, :, 2], 0.5) and np.allclose(a[50, :, 0], 1))

    def test_a_dark_part_takes_the_paint_as_a_light_one(self):
        base = np.full((64, 64, 3), 0.6, np.float32)
        base[32:] = 0.05                                      # a black receiver under tan furniture
        uv, tris = w.mesh_uv(mesh([(0, 0, 1, 0.45), (0, 0.55, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        a = w.aoac(base, labels > 0, labels)
        self.assertGreater(a[10, 32, 0], 0.95)
        self.assertGreater(a[50, 32, 0], 0.95)

    def test_a_part_is_measured_on_the_weapon_and_mirrored_halves_are_one(self):
        # two quads over the same texels: the left and right sides of a 10 x 30 cm plate pair 4 cm apart
        left = [(-0.02, 0, 0), (-0.02, 0.1, 0), (-0.02, 0.1, 0.3), (-0.02, 0, 0.3)]
        right = [(0.02, y, z) for _, y, z in left]
        m = w.mesh(mesh([(0, 0, 0.5, 0.5), (0, 0, 0.5, 0.5)], [left, right]))
        triangles = w.island_triangles([m])
        labels = w.islands([m], (64, 64), triangles)
        parts = w.parts_of([m], labels, triangles, [(0.0, 1.0)])
        self.assertEqual(len(parts), 1)
        part = parts[labels[10, 10]]
        self.assertAlmostEqual(part.area, 2 * 0.1 * 0.3, places=6)
        self.assertAlmostEqual(part.length, 0.3, places=6)
        self.assertAlmostEqual(part.along, 0.15, places=6)
        self.assertIsNone(part.tube)

    def test_a_tube_has_a_radius_and_a_box_a_plate_or_a_cover_has_none(self):
        def shell(ring, z=(0.0, 0.3), closed=True, end=1.0):    # the side triangles of a prism along z,
            a, b = ring, np.roll(ring, -1, axis=0)              # its far end `end` times as wide
            if not closed:
                a, b = a[:-1], b[:-1]
            middle = ring.mean(axis=0)
            at = lambda p, k: np.column_stack([p if k == 0 else (p - middle) * end + middle, np.full(len(p), z[k])])
            return np.concatenate([np.stack([at(a, 0), at(b, 0), at(b, 1)], 1), np.stack([at(a, 0), at(b, 1), at(a, 1)], 1)])
        around = lambda n, lo=0.0, hi=2 * np.pi, end=False: np.array(
            [(np.cos(t), np.sin(t)) for t in np.linspace(lo, hi, n, endpoint=end)])
        ring = around(12) * 0.01 + (0.3, -0.2)
        self.assertAlmostEqual(w.tube(shell(ring)), 0.01, places=4)          # a barrel, off the origin
        self.assertIsNotNone(w.tube(shell(ring, end=0.6)))                    # one that tapers
        half = around(7, 0, np.pi, True) * 0.01
        self.assertAlmostEqual(w.tube(shell(half, closed=False)), 0.01, places=3)   # half of it, 15 diameters long
        self.assertIsNone(w.tube(shell(around(10, -0.2, np.pi + 0.2, True) * 0.013, (0.0, 0.15), closed=False)))
        #                                                         a slide or a receiver cover: round above, open below
        self.assertIsNone(w.tube(shell(np.array([(0, 0), (0.02, 0), (0.02, 0.03), (0, 0.03)]))))   # a box
        chamfered = np.array([(0.002, 0), (0.023, 0), (0.025, 0.002), (0.025, 0.028), (0.023, 0.03), (0.002, 0.03),
                              (0, 0.028), (0, 0.002)])
        self.assertIsNone(w.tube(shell(chamfered)))                           # a box with its edges broken
        plate = np.array([[(0, y, 0), (0, y + 0.004, 0.1), (1e-5, y + 0.002, 0.2)] for y in np.linspace(0, 0.02, 6)])
        self.assertIsNone(w.tube(plate))                                      # a flat side

    def test_the_rule(self):
        part = lambda area, tube=None, length=0.05, along=0.5: w.Part(area, length, tube, along, 100)
        parts = {1: part(1e-4),                                     # a pin
                 2: part(4e-4), 3: part(4e-4),                      # a polymer button, a metal lever
                 4: part(60e-4, tube=0.009, length=0.4, along=0.8),  # a barrel
                 11: part(12e-4, tube=0.009, length=0.03, along=0.9),  # a piece of it between two sight bases
                 5: part(90e-4, tube=0.018, length=0.2, along=0.9),  # a suppressor
                 6: part(90e-4, length=0.2, along=0.6),              # a pistol's slide: slim, no tube
                 7: part(30e-4, tube=0.006, length=0.2, along=0.1),  # a wire stock's strut
                 8: part(300e-4),                                    # a receiver
                 9: part(5e-4), 10: part(5e-4)}                      # small metal parts, one the premium repaints
        metal = {i: 0.7 for i in parts}
        metal[2] = 0.02
        chosen = w.choose(parts, metal, {9: 0.8, 10: 0.1})
        self.assertEqual(chosen, {2: 'not metal', 6: 'body', 7: 'body', 8: 'body', 9: 'repainted'})
        painted = w.choose(parts, metal, {4: 0.9, 5: 0.9})           # a gun whose painted version paints both:
        self.assertEqual((4 in painted, painted.get(5)), (False, 'repainted'))   # the VSS's suppressor, no barrel
        self.assertEqual(set(w.choose(parts, {}, {})), {6, 7, 8})    # no metalness map: every part as metal
        polymer = w.choose({4: parts[4]}, {4: 0.0}, {})
        self.assertEqual(polymer, {4: 'not metal'})                 # a polymer tube (a round handguard) is painted

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

    def test_parts_tsv_picks_over_the_rule_by_a_point_on_each_part(self):
        import tempfile, os
        uv, tris = w.mesh_uv(mesh([(0, 0, 0.45, 1), (0.55, 0, 1, 1)]))
        labels = w.islands([(uv, tris)], (64, 64))
        points = w.island_points(labels)
        left, right = int(labels[30, 10]), int(labels[30, 50])
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, 'parts.tsv')
            open(path, 'w').write('# picks\na\t+%.4f,%.4f -%.4f,%.4f\nplain\t\n' % (points[right] + points[left]))
            parts = w.read_parts(path)
        self.assertEqual(w.picked({left}, parts['a'], labels, 'a'), {right})
        self.assertEqual(w.picked({left, right}, parts['plain'], labels, 'plain'), set())   # no points: plain
        with self.assertRaises(SystemExit):
            w.picked(set(), [(True, 0.5, 0.5)], labels, 't')           # the gap between the parts

    def test_a_row_names_each_painted_version_with_its_plain_texture(self):
        import tempfile, os
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, 'guns.tsv')
            open(path, 'w').write('# rows\nt\tp1,28:p2,69:p3@69:plain3\tm:s1,m:s2\tt_camo\t28:plain\t69:t_fmap1\nu\t\tm:s3\n')
            rows = w.read_rows(path)
        self.assertEqual(rows[0], w.Row('t', [('p1', '28:plain'), ('28:p2', '28:plain'), ('69:p3', '69:plain3')],
                                        ['m:s1', 'm:s2'], ['t_camo'], '69:t_fmap1'))
        self.assertEqual(rows[1], w.Row('u', [], ['m:s3'], [], None))


    def test_the_tables_and_their_picks_parse(self):
        import os
        here = os.path.dirname(os.path.abspath(__file__))
        seen = {}
        for folder in ('weapon-camo/guns.tsv', 'weapons-031/camo.tsv', 'weapons-046/camo.tsv', 'weapons-069/camo.tsv',
                       'remodel-069/camo.tsv'):
            table = os.path.join(here, '..', *folder.split('/'))
            rows = w.read_rows(table)
            self.assertTrue(rows)
            for row in rows:
                for name in [row.texture] + row.premiums:
                    self.assertNotIn(name, seen, 'a texture gets its mask from one row')
                    seen[name] = folder
                self.assertTrue(all(len(s.split(':')) == 2 for s in row.surfaces), row.texture)
                for names in row.painted + ([(row.metal,)] if row.metal else []):
                    self.assertTrue(all(n[2:3] != ':' or n[:2] in ('28', '69') for n in names), row.texture)
            picks = w.read_parts(os.path.join(os.path.dirname(table), 'parts.tsv'))
            self.assertLessEqual(set(picks), {row.texture for row in rows}, 'parts.tsv names the table\'s textures')

    def test_a_069_mesh_gives_its_uv_points(self):
        import references
        verts = b''.join(struct.pack('<3f', 0, 0, 0) + bytes(16) + struct.pack('<2e', u, v)
                         for u, v in ((0.25, 0.5), (0.75, 0.5), (0.5, 0.125), (0.9, 0.9)))     # 32 bytes, half UVs
        buf = struct.pack('<III', 3, 4 + len(verts), 4) + verts + struct.pack('<II', 5, 3) + b'abc' \
            + struct.pack('<III', 4, 4 + 6, 3) + struct.pack('<3H', 0, 1, 2)
        self.assertEqual(references.uv_069(buf).tolist(), [[0.25, 0.5], [0.75, 0.5], [0.5, 0.125]])


if __name__ == '__main__':
    unittest.main()
