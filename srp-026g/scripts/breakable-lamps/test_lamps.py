"""Authoring checks; native behavior is check_lamps_026g.py's Unicorn check."""
import json
from pathlib import Path
import struct
import unittest

import make_edits as edits
import make_pack as pack


class Lamps(unittest.TestCase):
    def test_legacy_glow_follows_bulb_and_break_state(self):
        # Reproduce v0.11 Vostok's offset shaft above a hanging lamp.
        # Another shaft far away belongs to scenery and must stay separate.
        fixture = dict(visual_name='ported/v011e/props/light/green_lamp_with_wire',
                       position=[41.566, 5.845, -4.646], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        cone = dict(visual_name=edits.LEGACY_CONE, position=[41.601, 5.655, -4.663],
                    rotation=[0., 0., 0.], scale=[1.7, 1.162, 1.7])
        self.assert_glow_alignment(fixture, cone, ([-.296, -.640, -.296], [.296, .010, .296]),
                                   [41.566, 5.205, -4.646])

    def test_stock_and_v021_glow_placement_is_preserved(self):
        for prefix in ('', 'ported/v021d/'):
            with self.subTest(prefix=prefix):
                fixture = dict(visual_name=prefix + 'props/light/lamp_on',
                               position=[29.291, 13.152, 90.735], rotation=[0., 0., 0.], scale=[1., 1., 1.])
                cone = dict(visual_name=prefix + 'props/light/cone_light_volume',
                            position=[29.291, 13.318, 90.735], rotation=[0., 0., 0.], scale=[1., 1., 1.])
                self.assert_glow_alignment(fixture, cone, ([-.056, -.209, -.056], [.056, 0., .055]),
                                           [29.291, 12.943, 90.735], moves_expected=0)

    def test_reported_hangar_cone_first_ring_starts_at_bulb(self):
        # Screenshot 2026-10-28-21-37-09; the only broken fixture in
        # match 1cb4275e is lamp007. The previous cone apex was 0.425 m
        # above this bulb after incorrectly borrowing 0.26's offset.
        fixture = dict(visual_name='ported/v011e/props/light/green_lamp',
                       position=[-28.147, 5.187, -73.309], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        cone = dict(visual_name=edits.LEGACY_CONE, position=[-28.163, 5.526, -73.299],
                    rotation=[0., 0., 0.], scale=[1.7, 1.162, 1.7])
        self.assert_glow_alignment(fixture, cone, ([-.296, -.254, -.296], [.296, 0., .296]),
                                   [-28.147, 4.933, -73.309])

    def test_outdoor_cone_uses_its_light_without_binding_a_distant_lamp(self):
        fixture = dict(visual_name='ported/v011e/props/light/green_lamp',
                       position=[0., 5., 0.], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        cone = dict(visual_name=edits.LEGACY_CONE, position=[20., 10.2, 20.],
                    rotation=[0., 0., 0.], scale=[1.7, 1.162, 1.7])
        light = dict(game_object_type='light', position=[20., 10., 20.])
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        for v in (fixture, cone):
            v['lib_name'] = v['visual_name']
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(fixture), record(cone)])),
                                                 ('client_objects', (4, [record(light)]))])))
        moves, _, spec = edits.author('test', original,
                                      {fixture['visual_name']: ([-.296, -.254, -.296], [.296, 0., .296])}, 29)
        self.assertEqual(len(moves), 1)
        move = moves[0]
        edits.c.set_entry(original, move['list'], move['where'], move['from'], move['to'])
        glow = edits.tree(original)['render_visuals'][1]
        start = edits.world_bounds(glow, edits.CONE_LIGHT_START, edits.CONE_LIGHT_START)[0]
        for actual, expected in zip(start, light['position']):
            self.assertAlmostEqual(actual, expected, places=5)
        lamp, = spec['value']['lamps'].values()
        self.assertFalse(lamp['lights'])
        self.assertFalse(any(k.startswith('cone') for k in lamp['visuals']))

    def assert_glow_alignment(self, fixture, cone, box, expected_tip, moves_expected=1):
        scenery = dict(cone, position=[100., 10., 100.])
        for v in (fixture, cone, scenery):
            v['lib_name'] = v['visual_name']
        objects = [(3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in visual.items()])
                   for visual in (fixture, cone, scenery)]
        original = bytearray(pack.serialize((3, [('render_visuals', (4, objects))])))
        bounds = {fixture['visual_name']: box}
        moves, additions, spec = edits.author('test', original, bounds, 29)
        self.assertEqual(len(moves), moves_expected)
        edited = bytearray(original)
        for move in moves:
            edits.c.set_entry(edited, move['list'], move['where'], move['from'], move['to'])
        for add in additions:
            edits.c.add_entry(edited, add['list'], add['after'], add['like'], add['values'])
        visuals = edits.tree(edited)['render_visuals']
        lamp, = spec['value']['lamps'].values()
        binding = lamp['visuals']['cone0']
        self.assertEqual(binding['state'], 0)
        glow = visuals[binding['index']]
        if moves_expected:
            start = edits.world_bounds(glow, edits.CONE_LIGHT_START, edits.CONE_LIGHT_START)[0]
            for actual, expected in zip(start, expected_tip):
                self.assertAlmostEqual(actual, expected, places=5)
            apex = edits.world_bounds(glow, edits.CONE_APEX, edits.CONE_APEX)[0]
            self.assertAlmostEqual(apex[1] - expected_tip[1], 0.14324678, places=5)
        if not moves_expected:
            for key in ('position', 'rotation', 'scale'):
                for actual, expected in zip(glow[key], cone[key]):
                    self.assertAlmostEqual(actual, expected, places=5)
        self.assertEqual(visuals[-1]['position'], scenery['position'])
        self.assertNotIn(len(visuals) - 1, [v['index'] for v in lamp['visuals'].values()])

    def test_rotated_scaled_bounds(self):
        import math
        visual = dict(rotation=[0, math.pi / 2, 0], scale=[2, 1, 3], position=[10, 20, 30])
        lo, hi = edits.world_bounds(visual, [-1, -2, -3], [1, 2, 3])
        for actual, expected in zip(lo + hi, [1, 18, 28, 19, 22, 32]):
            self.assertAlmostEqual(actual, expected)

    def test_socket_crop_preserves_vertex_attributes_and_reindexes(self):
        vertices = [struct.pack('<3f', x, y, z) + bytes([i]) * 16
                    for i, (x, y, z) in enumerate(((0, 0, 0), (1, 0, 0), (0, 1, 0), (0, -1, 0)))]
        indices = [0, 1, 2, 0, 3, 1]
        data = (struct.pack('<III', 3, 4 + 28 * 4, 4) + b''.join(vertices)
                + struct.pack('<III6H', 4, 16, 6, *indices))
        blob, points, triangles = pack.cropped(data, -.1)
        result, result_indices = pack.mesh_tools.mesh(blob)
        self.assertEqual(result, vertices[:3])
        self.assertEqual(result_indices, [0, 1, 2])
        self.assertEqual(triangles, 1)
        self.assertEqual(len(points), 3)

    def test_manifest_bindings_are_bounded_and_have_one_owner(self):
        path = Path(__file__).resolve().parents[1] / 'content-edit/breakable-lamps-1.json'
        source = json.loads(path.read_text())
        maps = set()
        for project in source['add_value']:
            maps.add(project['files'][0].split('/')[1])
            self.assertEqual(project['value']['version'], 2)
            self.assertEqual(project['value']['break_material'], 29)
            rows = project['value']['lamps']
            self.assertLessEqual(len(rows), 256)
            visual_ids, light_positions = [], []
            for lamp in rows.values():
                for axis in ('x', 'y', 'z'):
                    self.assertLess(lamp['min_' + axis], lamp['max_' + axis])
                states = {v['state'] for v in lamp['visuals'].values()}
                self.assertEqual(states, {0, 1})
                visual_ids.extend(v['index'] for v in lamp['visuals'].values())
                light_positions.extend(tuple(l[k] for k in ('x', 'y', 'z')) for l in lamp['lights'].values())
            self.assertEqual(len(visual_ids), len(set(visual_ids)))
            self.assertEqual(len(light_positions), len(set(light_positions)))
            self.assertLessEqual(len(visual_ids) + len(light_positions), 1024)
        self.assertEqual(maps, {f'level_0{i}' for i in range(2, 9)} |
                         {'v011e_level_03', 'v011e_level_04', 'v021d_level_03', 'v021d_level_06'})
        self.assertEqual(len(source['add_value']), 92)
        # Each of Vostok v0.11's ten hanging-lamp shafts must disappear
        # with its fixture. The two outdoor scenery shafts have no owner.
        for project in source['add_value']:
            if '/v011e_level_03/' in project['files'][0]:
                self.assertEqual(sum(k.startswith('cone') for lamp in project['value']['lamps'].values()
                                     for k in lamp['visuals']), 10)
        moved_maps = {e['file'].split('/')[1] for e in source['set_entry']}
        self.assertEqual(moved_maps, {'v011e_level_03'})
        self.assertEqual(len(source['set_entry']), 24)


if __name__ == '__main__':
    unittest.main()
