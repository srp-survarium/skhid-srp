"""The skeleton cap's authoring: the cut and the Rudnya rows against the built check."""
import math
import struct
import unittest

import make_edits as edits
import make_pack as pack

lamps = edits.lamps


def mesh(parts):
    """A converted_model of `parts`: each a triangle's three (x, y, z, u, v),
    the vertex's other attributes its index's byte."""
    vertices, indices = [], []
    for corners in parts:
        for x, y, z, u, v in corners:
            n = len(vertices)
            vertices.append(struct.pack('<3f', x, y, z) + bytes([n]) * 12 + struct.pack('<2e', u, v))
            indices.append(n)
    return (struct.pack('<III', 3, 4 + 28 * len(vertices), len(vertices)) + b''.join(vertices)
            + struct.pack('<III', 4, 4 + 2 * len(indices), len(indices)) + struct.pack('<%dH' % len(indices), *indices))


def record(value):
    return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])


class SkeletonCap(unittest.TestCase):
    CAP = [(0.1, 0.7, 0.1, 0.8, 0.1), (0.3, 0.7, 0.1, 0.9, 0.1), (0.2, 0.9, 0.3, 0.8, 0.2)]
    SHIRT = [(0.0, 0.2, 0.0, 0.6, 0.3), (0.4, 0.2, 0.0, 0.7, 0.3), (0.2, 0.6, 0.0, 0.6, 0.4)]

    def test_the_cap_is_cut_whole_and_moved_to_its_centre(self):
        data = mesh([self.SHIRT, self.CAP])
        blob, points, triangles = pack.keep(data, pack.cap_part, (0.2, 0.8, 0.2))
        self.assertEqual(triangles, 1)
        for got, (x, y, z, _, _) in zip(points, self.CAP):
            self.assertTrue(all(math.isclose(a, b, abs_tol=1e-6) for a, b in zip(got, (x - 0.2, y - 0.8, z - 0.2))))
        vertices, indices = pack.mesh_tools.mesh(blob)
        self.assertEqual(indices, [0, 1, 2])
        self.assertEqual([v[12:] for v in vertices], [v[12:] for v in pack.mesh_tools.mesh(data)[0][3:]])
        _, rest, triangles = pack.keep(data, pack.body_part, (0.0, 0.0, 0.0))
        self.assertEqual(triangles, 1)
        for got, want in zip(rest, self.SHIRT):
            self.assertTrue(all(math.isclose(a, b, abs_tol=1e-6) for a, b in zip(got, want[:3])))

    def project(self, *visuals):
        return bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in visuals]))])))

    def built(self, data, entries, table):
        data = bytearray(data)
        for e in entries:
            self.assertIsNotNone(lamps.c.add_entry(data, e['list'], e['after'], e['like'], e['values']))
        lamps.c.add_value(data, '', 'srp_lamps', table)
        return data

    SKELETON = dict(position=[11.932, 18.731, 38.563], rotation=[-1.578, -1.374, -1.662], scale=[1., 0.9999999403953552, 1.])
    CENTRE, BOUNDS = [0.2255, 0.765, 0.239], ([-0.119, -0.124, -0.12], [0.119, 0.126, 0.12])

    def test_rudnya_rows_pass_the_built_check_and_a_moved_index_does_not(self):
        for figure in lamps.FIGURES:
            for names in (('visual_name', 'lib_name'), ('lib_name',)):     # the v0.69 ports' maps: lib_name alone
                with self.subTest(figure=figure, names=names):
                    skeleton = dict(self.SKELETON, **{key: figure for key in names})
                    crate = {key: 'props/crate' for key in names} | dict(position=[0., 0., 0.], rotation=[0., 0., 0.],
                                                                         scale=[1., 1., 1.])
                    data = self.project(dict(crate), skeleton, dict(crate, position=[1., 0., 0.]))
                    entries, table = edits.edits_of('p', data, self.CENTRE, self.BOUNDS)
                    self.assertEqual(table['leftover'], 'body')
                    built = self.built(data, entries, table)
                    self.assertEqual(lamps.check_built('p', built), [])
                    visuals = lamps.tree(built)['render_visuals']
                    row = table['lamps']['cap0']
                    bare, cap = (visuals[row['visuals']['broken0']['index']], visuals[row['loose']['cap0']['index']])
                    self.assertEqual(lamps.model_name(bare), lamps.FIGURES[figure][0])
                    self.assertEqual(lamps.model_name(cap), lamps.FIGURES[figure][1])
                    self.assertEqual(set(cap) & {'visual_name', 'lib_name'}, set(names))
                    at = lamps.world_bounds(skeleton, self.CENTRE, self.CENTRE)[0]
                    self.assertTrue(all(math.isclose(a, b, abs_tol=1e-5) for a, b in zip(cap['position'], at)))
                    self.assertGreater(cap['position'][1] - skeleton['position'][1], 0.7, 'the cap is on its head')
                    # the skeleton's origin is out of the cap's bounds: a shot at its legs is no hit
                    self.assertTrue(skeleton['position'][1] < row['min_y'])
                    row['loose']['cap0']['index'] -= 1
                    self.assertEqual(len(lamps.check_built('p', self.built(data, entries, table))), 1)
                    row['loose']['cap0']['index'] += 1
                    self.assertEqual(len(lamps.check_built('p', self.built(data, entries, dict(table, leftover='glass')))), 1)
                    with self.assertRaisesRegex(ValueError, 'a lamp table already'):
                        edits.edits_of('p', built, self.CENTRE, self.BOUNDS)

    def test_each_skeleton_of_a_map_is_its_own_fixture(self):
        figure = next(iter(lamps.FIGURES))
        first = dict(self.SKELETON, lib_name=figure)
        second = dict(first, position=[38.572, 22.192, -32.115], rotation=[0., -0.312, 0.], scale=[1., 1., 1.])
        crate = dict(lib_name='props/crate', position=[0., 0., 0.], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        data = self.project(first, dict(crate), second)
        entries, table = edits.edits_of('p', data, self.CENTRE, self.BOUNDS)
        built = self.built(data, entries, table)
        self.assertEqual(lamps.check_built('p', built), [])
        visuals = lamps.tree(built)['render_visuals']
        self.assertEqual([table['lamps'][k]['visuals']['intact0']['index'] for k in ('cap0', 'cap1')], [0, 4])
        self.assertEqual([lamps.model_name(v).rsplit('/', 1)[1] for v in visuals],
                         ['skeleton_sitting1', 'srp_skeleton_sitting1_bare', 'srp_skeleton_sitting1_cap', 'crate',
                          'skeleton_sitting1', 'srp_skeleton_sitting1_bare', 'srp_skeleton_sitting1_cap'])

if __name__ == '__main__':
    unittest.main()
