"""Authoring checks; native behavior is check_lamps_026g.py's Unicorn check."""
import json
import math
from pathlib import Path
import struct
import unittest

import make_edits as edits
import make_pack as pack
import make_bottles as bottles_tool


class Lamps(unittest.TestCase):
    def test_restored_lights_bind_once_without_mutating_fixtures(self):
        fixture = dict(min_x=-0.1, min_y=-0.1, min_z=-0.1,
                       max_x=0.1, max_y=0.1, max_z=0.1,
                       lights={'old': {'name': 'native'}})
        lamps = {'lamp000': fixture}
        before = json.dumps(lamps, sort_keys=True)
        lights = [dict(game_object_type='light', position=[0., 0.2, 0.], full_name='donor'),
                  dict(game_object_type='light', position=[20., 0., 0.], full_name='distant'),
                  dict(game_object_type='ambient_light', position=[0., 0., 0.], full_name='ambient')]
        self.assertEqual(edits.rebound_lights('map', lamps, lights), {
            'lamp000': {'light0': dict(x=0., y=0.2, z=0., name='donor')}})
        self.assertEqual(json.dumps(lamps, sort_keys=True), before)
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            edits.rebound_lights('map', dict(lamp000=fixture, lamp001=fixture), lights)

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
                                      {fixture['visual_name']: ([-.296, -.254, -.296], [.296, 0., .296])}, 29, 7)
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

    def test_built_check_finds_a_wrong_index_and_a_moved_light(self):
        fixture = dict(visual_name='props/light/lamp_on', position=[0., 5., 0.],
                       rotation=[0., 0., 0.], scale=[1., 1., 1.])
        cone = dict(visual_name='props/light/cone_light_volume', position=[0., 5.1, 0.],
                    rotation=[0., 0., 0.], scale=[1., 1., 1.])
        other = dict(fixture, position=[10., 5., 0.])
        light = dict(game_object_type='light', full_name='l', position=[0., 4.9, 0.])
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        for v in (fixture, cone, other):
            v['lib_name'] = v['visual_name']
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in (fixture, cone, other)])),
                                                 ('client_objects', (4, [record(light)]))])))
        bounds = {'props/light/lamp_on': ([-.056, -.209, -.056], [.056, 0., .055])}
        _, additions, spec = edits.author('p', original, bounds, 29, 7)
        def built(spec):
            data = bytearray(original)
            for add in additions:
                edits.c.add_entry(data, add['list'], add['after'], add['like'], add['values'])
            edits.c.add_value(data, spec['table'], spec['key'], spec['value'])
            return data
        self.assertEqual(edits.check_built('p', built(spec)), [])
        lamp, neighbour = spec['value']['lamps'].values()
        # A shifted index naming the other lamp's (same) model, and a light
        # the map editor moved.
        lamp['visuals']['intact0']['index'] = neighbour['visuals']['intact0']['index']
        lamp['lights']['light0']['y'] += .1
        problems = edits.check_built('p', built(spec))
        self.assertEqual(len(problems), 3, problems)  # the intact model, its broken one, the light

    def test_a_lit_alarm_owns_the_red_ambient_light_beside_it(self):
        # Vector's alarm lamp (screenshot 2026-11-15-20-23-12: broken, its red
        # glow stayed) beside a room's grey fill, a red glow too far away and
        # an unlit alarm lamp, which glows through a light object if at all.
        alarm = dict(visual_name='props/light/alarm_red_light', position=[10.675, 15.371, -6.341],
                     rotation=[0., 0., 0.], scale=[1., 1., 1.])
        unlit = dict(alarm, visual_name='props/light/alarm_red', position=[30., 15., 0.])
        def ambient(name, position, colour):
            return dict(game_object_type='ambient_light', full_name=name, position=position,
                        color=colour & 0xffffffff)   # the serializer packs an i32's bits
        objects = [ambient('glow', [10.911, 14.843, -6.414], -15857931),   # red 245, green 6, blue 14
                   ambient('fill', [10.5, 15., -6.], -9737365),            # grey
                   ambient('far', [14., 15.371, -6.341], -15857931),
                   ambient('unlit', [30.3, 15., 0.], -15857931)]
        def record(value):
            kinds = {list: 7, int: 1, str: 5}
            return (3, [(k, (kinds[type(v)], v)) for k, v in value.items()])
        for v in (alarm, unlit):
            v['lib_name'] = v['visual_name']
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(alarm), record(unlit)])),
                                                 ('client_objects', (4, [record(o) for o in objects]))])))
        box = ([-.1, -.2, -.1], [.1, 0., .1])
        _, additions, spec = edits.author('p', original, {alarm['visual_name']: box, unlit['visual_name']: box}, 29, 7)
        lit, other = spec['value']['lamps'].values()
        self.assertEqual([light['name'] for light in lit['ambient_lights'].values()], ['glow'])
        self.assertNotIn('ambient_lights', other)
        data = bytearray(original)
        for add in additions:
            edits.c.add_entry(data, add['list'], add['after'], add['like'], add['values'])
        built = bytearray(data)
        edits.c.add_value(built, spec['table'], spec['key'], spec['value'])
        self.assertEqual(edits.check_built('p', built), [])
        lit['ambient_lights']['light0']['y'] += .1   # the map editor moved it
        edits.c.add_value(data, spec['table'], spec['key'], spec['value'])
        self.assertEqual(len(edits.check_built('p', data)), 1)

    def test_a_kerosene_lamp_is_its_lit_glass_and_its_flame(self):
        # London's lamp (screenshot 2026-11-15-20-44-11): the frame stays,
        # the glass and the flame in it go. An unlit frame is scenery, and a
        # flame of no lamp (the Fort has one) is nobody's.
        frame = dict(visual_name='props/light/stand_group_batlamp', position=[46., 9.562, -18.825],
                     rotation=[0., 0., 0.], scale=[1., 1., 1.])
        glass = dict(frame, visual_name='props/light/stand_group_batlamp_glass_emit')
        unlit = dict(frame, position=[60., 9.562, 0.])
        def flame(name, position):
            return dict(game_object_type='particle', lib_name=edits.FLAME, full_name=name, position=position)
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        for v in (frame, glass, unlit):
            v['lib_name'] = v['visual_name']
        objects = [flame('lamp/oil_lamp', [46., 9.69, -18.825]), flame('lone/oil_lamp', [50., 9.69, -18.825])]
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in (frame, glass, unlit)])),
                                                 ('client_objects', (4, [record(o) for o in objects]))])))
        bounds = {frame['visual_name']: ([-.12, 0., -.12], [.12, .62, .12]),
                  glass['visual_name']: ([-.07, .11, -.07], [.07, .39, .07])}
        _, additions, spec = edits.author('p', original, bounds, 29, 7)
        lamp, = spec['value']['lamps'].values()
        self.assertEqual(additions, [])   # nothing replaces the glass
        self.assertEqual(list(lamp['visuals'].values()), [{'index': 1, 'state': 0}])   # the glass, intact only
        self.assertEqual([p['name'] for p in lamp['particles'].values()], ['lamp/oil_lamp'])
        self.assertAlmostEqual(lamp['max_y'], 9.562 + .62, places=5)   # a shot into the frame breaks it
        self.assertAlmostEqual(lamp['glass_to_y'] - lamp['glass_from_y'], .28, places=5)
        built = bytearray(original)
        edits.c.add_value(built, spec['table'], spec['key'], spec['value'])
        self.assertEqual(edits.check_built('p', built), [])

    def test_a_weather_made_from_a_lamp_day_is_authored_over_its_carried_table(self):
        # London's rain: a pack's copy of the day with the day's broken model
        # and table, and a kerosene lamp the day's table does not bind.
        bulb = dict(visual_name='props/light/lamp_on', position=[0., 5., 0.], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        frame = dict(bulb, visual_name='props/light/stand_group_batlamp', position=[20., 0., 0.])
        glass = dict(frame, visual_name='props/light/stand_group_batlamp_glass_emit')
        flame = dict(game_object_type='particle', lib_name=edits.FLAME, full_name='f', position=[20., .2, 0.])
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        for v in (bulb, frame, glass):
            v['lib_name'] = v['visual_name']
        bounds = {bulb['visual_name']: ([-.056, -.209, -.056], [.056, 0., .055]),
                  frame['visual_name']: ([-.12, 0., -.12], [.12, .62, .12]),
                  glass['visual_name']: ([-.07, .11, -.07], [.07, .39, .07])}
        def day(*visuals):
            data = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in visuals])),
                                                 ('client_objects', (4, [record(flame)]))])))
            _, additions, spec = edits.author('day', data, bounds, 29, 7)
            for add in additions:
                edits.c.add_entry(data, add['list'], add['after'], add['like'], add['values'])
            edits.c.add_value(data, spec['table'], spec['key'], spec['value'])
            return data
        self.assertEqual(edits.author('rain', day(bulb, frame, glass), bounds, 29, 7), ([], [], None))
        moves, additions, spec = edits.author('rain', day(bulb, frame), bounds, 29, 7)
        self.assertEqual((moves, additions, spec), ([], [], None))
        carried = day(bulb)
        rain = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in
                                                                    edits.tree(carried)['render_visuals'] + [frame, glass]])),
                                             ('client_objects', (4, [record(flame)]))])))
        edits.c.add_value(rain, '', 'srp_lamps', edits.tree(carried)['srp_lamps'])
        moves, additions, spec = edits.author('rain', rain, bounds, 29, 7)
        self.assertEqual((moves, additions, spec.pop('replaces')), ([], [], True))
        self.assertEqual(sorted(len(row.get('particles', {})) for row in spec['value']['lamps'].values()), [0, 1])
        edits.c.remove_key(rain, 'srp_lamps')
        edits.c.add_value(rain, spec['table'], spec['key'], spec['value'])
        self.assertEqual(edits.check_built('rain', rain), [])

    def assert_glow_alignment(self, fixture, cone, box, expected_tip, moves_expected=1):
        scenery = dict(cone, position=[100., 10., 100.])
        for v in (fixture, cone, scenery):
            v['lib_name'] = v['visual_name']
        objects = [(3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in visual.items()])
                   for visual in (fixture, cone, scenery)]
        original = bytearray(pack.serialize((3, [('render_visuals', (4, objects))])))
        bounds = {fixture['visual_name']: box}
        moves, additions, spec = edits.author('test', original, bounds, 29, 7)
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
        blob, points, triangles = pack.without(data, pack.below(-.1))
        result, result_indices = pack.mesh_tools.mesh(blob)
        self.assertEqual(result, vertices[:3])
        self.assertEqual(result_indices, [0, 1, 2])
        self.assertEqual(triangles, 1)
        self.assertEqual(len(points), 3)

    def test_tube_parts_go_whole_and_the_housing_stays(self):
        # A housing quad and a tube, joined to nothing, its UVs split at a
        # seam (a duplicated corner): every tube triangle goes.
        def vertex(x, y, z, u, v):
            return struct.pack('<3f', x, y, z) + bytes(12) + struct.pack('<2e', u, v)
        vertices = [vertex(0, 0, 0, .1, .6), vertex(1, 0, 0, .2, .6), vertex(0, 0, 1, .1, .7),
                    vertex(0, -1, 0, .9, .1), vertex(1, -1, 0, .9, .2), vertex(0, -1, 1, .95, .1),
                    vertex(1, -1, 0, .5, .9), vertex(1, -1, 1, .5, .95)]
        indices = [0, 1, 2, 3, 4, 5, 5, 6, 7]
        data = (struct.pack('<III', 3, 4 + 28 * len(vertices), len(vertices)) + b''.join(vertices)
                + struct.pack('<III9H', 4, 22, 9, *indices))
        blob, points, triangles = pack.without(data, pack.TUBES_MIDLE)
        result, result_indices = pack.mesh_tools.mesh(blob)
        self.assertEqual((result, result_indices, triangles), (vertices[:3], [0, 1, 2], 1))

    def test_tubes_of_their_own_break_in_a_housing_that_stays(self):
        housing = dict(visual_name='props/light/lamp_midle_04', position=[0., 5., 0.],
                       rotation=[0., 0., 0.], scale=[1., 1., 1.])
        tube = dict(visual_name='fx/lamp2', position=[0., 4.95, .2], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        lone = dict(housing, position=[10., 5., 0.])
        light = dict(game_object_type='light', full_name='l', position=[0., 4.8, 0.])
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        for v in (housing, tube, lone):
            v['lib_name'] = v['visual_name']
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in (housing, tube, lone)])),
                                                 ('client_objects', (4, [record(light)]))])))
        bounds = {'props/light/lamp_midle_04': ([-.351, -.107, -.265], [.35, .007, .265]),
                  'fx/lamp2': ([-.347, -.044, -.024], [.347, 0., .026])}
        _, additions, spec = edits.author('p', original, bounds, 29, 7)
        self.assertEqual(additions, [])
        lamp, = spec['value']['lamps'].values()
        self.assertEqual(lamp['visuals'], {'intact0': {'index': 1, 'state': 0}})
        self.assertEqual(len(lamp['lights']), 1)
        self.assertAlmostEqual(lamp['min_y'], 4.893)   # the housing's bottom
        # Its glass is the tube, not the housing: along the tube's length.
        glass = [[lamp[f'glass_{e}_{a}'] for a in 'xyz'] for e in ('from', 'to')]
        for actual, expected in zip(glass, ([-.347, 4.928, .201], [.347, 4.928, .201])):
            for a, b in zip(actual, expected):
                self.assertAlmostEqual(a, b, places=6)
        data = bytearray(original)
        edits.c.add_value(data, spec['table'], spec['key'], spec['value'])
        self.assertEqual(edits.check_built('p', data), [])

    def test_a_turned_tube_fixture_breaks_along_its_tubes(self):
        # lamp_big_01 turned 45 degrees: its glass follows its 1.5 m length
        # at its tubes' height, under the rods, not a world axis of its box.
        fixture = dict(visual_name='props/light/lamp_big_01', position=[0., 5., 0.],
                       rotation=[0., math.pi / 4, 0.], scale=[1., 1., 1.], lib_name='props/light/lamp_big_01')
        record = (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in fixture.items()])
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record]))])))
        bounds = {'props/light/lamp_big_01': ([-1.289, -.88, -.179], [.211, 0., .156])}
        _, _, spec = edits.author('p', original, bounds, 29, 7)
        lamp, = spec['value']['lamps'].values()
        start, end = ([lamp[f'glass_{e}_{a}'] for a in 'xyz'] for e in ('from', 'to'))
        self.assertAlmostEqual(math.dist(start, end), 1.5, places=5)
        self.assertAlmostEqual(abs(end[0] - start[0]), abs(end[2] - start[2]), places=5)
        self.assertAlmostEqual(start[1], 5. - .822, places=5)
        self.assertAlmostEqual(end[1], 5. - .822, places=5)

    def test_bottles_side_by_side_are_fixtures_of_their_own(self):
        # School's table (2026-11-26): three bottles within 0.3 m, in a project
        # with a lamp table. Each breaks alone, along its own length.
        def record(value):
            return (3, [(k, (7 if isinstance(v, list) else 5, v)) for k, v in value.items()])
        lamp = dict(visual_name='props/light/lamp_on', position=[0., 5., 0.], rotation=[0., 0., 0.], scale=[1., 1., 1.])
        bottles = [dict(visual_name='props/kitchen/bottle_vodka', position=p, rotation=r, scale=[1., 1., 1.])
                   for p, r in (([16.98, 19.48, 33.62], [0., 0., 1.5708]), ([17.04, 19.44, 33.77], [0., 0., 1.5708]),
                                ([16.78, 19.47, 33.72], [0., 0., 0.]))]
        for v in [lamp] + bottles:
            v['lib_name'] = v['visual_name']
        original = bytearray(pack.serialize((3, [('render_visuals', (4, [record(v) for v in [lamp] + bottles])),
                                                 ('client_objects', (4, []))])))
        lamp_bounds = {'props/light/lamp_on': ([-.056, -.209, -.056], [.056, 0., .055])}
        _, additions, spec = edits.author('projects/level_02/day_2/client_project', original, lamp_bounds, 29, 7)
        data = bytearray(original)
        for add in additions:
            edits.c.add_entry(data, add['list'], add['after'], add['like'], add['values'])
        edits.c.add_value(data, spec['table'], spec['key'], spec['value'])
        self.assertEqual(len(spec['value']['lamps']), 1)   # the lamp rule leaves bottles alone
        bottle_bounds = {'props/kitchen/bottle_vodka': ([-.039, .005, -.036], [.037, .305, .04])}
        rows = bottles_tool.rows('projects/level_02/day_2/client_project', bytes(data), bottle_bounds)
        self.assertEqual(sorted(rows), ['bottle0', 'bottle1', 'bottle2'])
        final = edits.tree(bytes(data))['render_visuals']
        for row in rows.values():
            self.assertEqual(row['lights'], {})
            (visual,) = row['visuals'].values()
            self.assertEqual(visual['state'], 0)
            self.assertEqual(final[visual['index']]['visual_name'], 'props/kitchen/bottle_vodka')
            # The burst runs along the bottle: 0.3 m, inside its bounds.
            ends = [[row[f'glass_{e}_{a}'] for a in 'xyz'] for e in ('from', 'to')]
            self.assertAlmostEqual(math.dist(*ends), .3, places=3)
        # A lying bottle's line is level, the standing one's upright.
        self.assertAlmostEqual(rows['bottle0']['glass_from_y'], rows['bottle0']['glass_to_y'], places=3)
        self.assertAlmostEqual(abs(rows['bottle2']['glass_to_y'] - rows['bottle2']['glass_from_y']), .3, places=3)
        for key, row in rows.items():
            edits.c.add_value(data, 'srp_lamps/lamps', key, row)
        self.assertEqual(edits.check_built('projects/level_02/day_2/client_project', data), [])
        # An index a later list shifted onto the lamp is a problem.
        rows['bottle1']['visuals']['intact0']['index'] = 0
        shifted = bytearray(original)
        for add in additions:
            edits.c.add_entry(shifted, add['list'], add['after'], add['like'], add['values'])
        edits.c.add_value(shifted, spec['table'], spec['key'], spec['value'])
        for key, row in rows.items():
            edits.c.add_value(shifted, 'srp_lamps/lamps', key, row)
        self.assertEqual(len(edits.check_built('projects/level_02/day_2/client_project', shifted)), 1)
        with self.assertRaisesRegex(ValueError, 'no lamp table'):
            bottles_tool.rows('projects/level_02/day_2/client_project', bytes(original), bottle_bounds)

    def test_manifest_bindings_are_bounded_and_have_one_owner(self):
        path = Path(__file__).resolve().parents[1] / 'content-edit/breakable-lamps-1.json'
        source = json.loads(path.read_text())
        maps = set()
        for project in source['add_value']:
            maps.add(project['files'][0].split('/')[1])
            self.assertEqual(project['value']['version'], 3)
            self.assertEqual(project['value']['break_material'], 29)
            self.assertEqual(project['value']['effect_material'], 7)
            rows = project['value']['lamps']
            self.assertLessEqual(len(rows), 256)
            visual_ids, light_positions = [], []
            for lamp in rows.values():
                for axis in ('x', 'y', 'z'):
                    self.assertLess(lamp['min_' + axis], lamp['max_' + axis])
                states = {v['state'] for v in lamp['visuals'].values()}
                self.assertIn(0, states)   # tubes of their own have no broken model
                self.assertLessEqual(states, {0, 1})
                visual_ids.extend(v['index'] for v in lamp['visuals'].values())
                light_positions.extend(tuple(l[k] for k in ('x', 'y', 'z')) for l in lamp['lights'].values())
            self.assertEqual(len(visual_ids), len(set(visual_ids)))
            self.assertEqual(len(light_positions), len(set(light_positions)))
            self.assertLessEqual(len(visual_ids) + len(light_positions), 1024)
        self.assertEqual(maps, {f'level_0{i}' for i in range(2, 10)} |
                         {'v011e_level_03', 'v011e_level_04', 'v021d_level_02', 'v021d_level_04',
                          'v021d_level_03', 'v021d_level_05', 'v021d_level_06'})
        # School/Fort/Chemical Plant battery and TDM projects, and the rain and evening of London
        # and of Fort and Chemical Plant v0.21 (their kerosene lamps), whose tables replace the
        # day's they carry.
        self.assertEqual(len(source['add_value']), 116)
        weathers = [f'projects/{level}/{w}_{m}/client_project' for level, modes in
                    (('level_09', (2, 3, 4)), ('v021d_level_04', (2, 4)), ('v021d_level_05', (2, 4)))
                    for w in ('evening', 'rain') for m in modes]
        self.assertEqual(sorted(e['files'] for e in source['remove_key']), sorted(weathers))
        # Every lit kerosene lamp of every map, the 0.21 ports' copies of the models included.
        flames = {project['files'][0].split('/')[1]: sum(len(lamp.get('particles', {}))
                                                         for lamp in project['value']['lamps'].values())
                  for project in source['add_value'] if '/day_4/' in project['files'][0]}
        self.assertEqual({k: v for k, v in flames.items() if v},
                         {'level_04': 1, 'level_05': 9, 'level_09': 10, 'v021d_level_04': 1, 'v021d_level_05': 9})
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
