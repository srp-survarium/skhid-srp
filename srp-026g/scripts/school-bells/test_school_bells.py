"""school-bell@1: the collection plays one take of them all at random, never one twice in a row."""
from pathlib import Path
import unittest

import importlib.util

_spec = importlib.util.spec_from_file_location('school_bell_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)
_spec = importlib.util.spec_from_file_location('school_bell_edits', Path(__file__).with_name('make_edits.py'))
edits = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(edits)


class Bells(unittest.TestCase):
    def test_the_collection_names_every_take_at_random(self):
        root = pack.parse(bytearray(pack.collection(sorted(pack.TAKES))))
        ((name, (_, fields)),) = root[1]
        fields = dict(fields)
        self.assertEqual(name, 'collection')
        self.assertEqual(fields['type'], (pack.T_STR, 'random'))
        self.assertEqual(fields['dont_repeat_sound_successively'][1], 1)
        files = [dict(item[1])['filename'][1] for item in fields['sound_items'][1]]
        self.assertEqual(files, [pack.TAKES_FOLDER + take for take in sorted(pack.TAKES)])
        self.assertEqual(len(pack.TAKES), pack.RINGS, "one take a collection the lamp patch names")
        self.assertNotIn('05_single_clang', pack.TAKES)



def place(x, **rest):
    return {'position': [x, 19.4, 72.8], 'rotation': [0.0, 0.0, 0.0], 'scale': [1.0, 1.0, 1.0], **rest}


class Placements(unittest.TestCase):
    """School (v0.69): the converter merged three bells' render visuals into clusters; their
    collision objects stand at the bells' places."""
    SOLID = 'resources/models/props/electric/bell_school.model/collision'

    def test_a_visual_and_its_collision_are_one_bell(self):
        root = {'render_visuals': [place(1.0, lib_name='props/electric/bell_school')],
                'collision_objects': [place(1.0, lib_name=self.SOLID)]}
        self.assertEqual(len(edits.bells_of(root)), 1)

    def test_a_collision_with_no_visual_is_a_bell_after_the_visuals(self):
        root = {'render_visuals': [place(1.0, lib_name='props/electric/bell_school'),
                                   place(5.0, lib_name='v069d_school/srp_cluster/alike1')],
                'collision_objects': [place(9.0, lib_name=self.SOLID), place(1.0, lib_name=self.SOLID),
                                      place(5.0, lib_name='resources/models/props/electric/lamp.model/collision')]}
        bells = edits.bells_of(root)
        self.assertEqual([(name, v['position'][0]) for name, v in bells],
                         [('props/electric/bell_school', 1.0), ('props/electric/bell_school', 9.0)])

    def test_school_and_v021_bells_without_collision_as_before(self):
        root = {'render_visuals': [place(x, visual_name='props/electric/bell_school') for x in (1.0, 2.0)]}
        self.assertEqual([v['position'][0] for _, v in edits.bells_of(root)], [1.0, 2.0])


if __name__ == '__main__':
    unittest.main()
