"""school-bell@1: the collection plays one take of them all at random, never one twice in a row."""
from pathlib import Path
import unittest

import importlib.util

_spec = importlib.util.spec_from_file_location('school_bell_pack', Path(__file__).with_name('make_pack.py'))
pack = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pack)


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


if __name__ == '__main__':
    unittest.main()
