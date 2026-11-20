import importlib.util
from pathlib import Path
import struct
import unittest

from check_boot_classes import Resources, audit, bcw


def resources(specs):
    out = {}
    for name, kind, height in specs:
        out[f'gameplay/items/armour/boots/{name}.options'] = bcw.dump({
            'data': {'model_type': kind, 'base_model': name, 'part_name': 'boot'}})
        vertices = [(-.1, 0., 0.), (-.1, height, 0.), (.1, 0., 0.), (.1, height, 0.)]
        out[f'models/{name}.skinned_model/render/boot/converted_model'] = (
            struct.pack('<III', 3, 4 + len(vertices) * 12, len(vertices)) +
            b''.join(struct.pack('<3f', *p) for p in vertices))
    return out


class BootClassTests(unittest.TestCase):
    def setUp(self):
        self.native = resources([('low', 'data_full', .18),
                                 ('medium', 'data_medium', .27), ('tall', 'data_long', .34)])

    def test_exoskeleton_regression_and_corrected_class(self):
        for kind, expected in [('data_long', 'shaft_too_short'), ('data_full', 'ok')]:
            added = resources([('exo', kind, .22)])
            rows = audit(Resources(self.native), Resources(self.native | added))
            self.assertEqual(next(r for r in rows if '/exo.' in r['path'])['status'], expected)

    def test_full_length_trousers_can_cover_taller_boots(self):
        added = resources([('novice', 'data_full', .26)])
        self.assertTrue(all(r['status'] == 'ok' for r in
                            audit(Resources(self.native), Resources(self.native | added))))

    def test_release_gate_checks_the_composed_config(self):
        path = Path(__file__).resolve().parents[3] / 'release' / 'build_content.py'
        spec = importlib.util.spec_from_file_location('build_content', path)
        build = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(build)
        native = self.native

        class Game:
            def files(self, archives):
                return dict.fromkeys(native, 'resources.db')

            def get(self, archives, paths):
                return {p: native[p] for p in paths}

        world = build.World()
        for path, raw in resources([('exo', 'data_long', .22)]).items():
            world.put(path, raw, 'pack')
        with self.assertRaises(build.Problem):
            build.clothing_check(['stock'], native, Game(), world)
        # An edit over the pack fixes its class; the gate sees that final data.
        path = 'gameplay/items/armour/boots/exo.options'
        world.put(path, resources([('exo', 'data_full', .22)])[path], 'edit')
        build.clothing_check(['stock'], native, Game(), world)


if __name__ == '__main__':
    unittest.main()
