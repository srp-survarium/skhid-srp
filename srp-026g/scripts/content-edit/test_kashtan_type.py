"""kashtan-type-1.json: the AEK-919K "Kashtan" typed as the submachine guns are (wpn_smg), not as a
pistol (wpn_pstl), its row in release/content.tsv; and its one edit on a config of that shape."""
import json
import unittest
from pathlib import Path

import content_edit as ce

HERE = Path(__file__).resolve().parent
LIST = HERE / 'kashtan-type-1.json'
CONFIG = 'gameplay/items/weapons/wpn_aek_919k.options'


class KashtanType(unittest.TestCase):
    def test_the_kashtan_is_typed_a_submachine_gun(self):
        edits = json.loads(LIST.read_text(encoding='utf-8'))
        self.assertEqual(edits['set_string'], [
            {'file': CONFIG, 'path': 'parameters/type', 'from': 'wpn_pstl', 'to': 'wpn_smg'}])
        self.assertEqual(set(edits) - {'_about'}, {'set_string'}, 'nothing else of the gun changes')

    def test_its_row_is_in_the_manifest(self):
        rows = (HERE.parents[2] / 'release' / 'content.tsv').read_text(encoding='utf-8').splitlines()
        self.assertIn('kashtan-type-1.json', [row.split('\t')[1] for row in rows if row.startswith('edit\t')])

    def test_the_edit_sets_the_type_alone(self):
        buf = bytearray(24)
        buf[:24] = ce.value_record(buf, 0, '', {'parameters': {'type': 'wpn_pstl', 'bullet_pierce': 0.2}})
        before = {p: v for p, t, v in ce.walk(buf)}
        ce.set_string(buf, 'parameters/type', 'wpn_pstl', 'wpn_smg')
        after = {p: v for p, t, v in ce.walk(buf)}
        self.assertEqual(after['/parameters/type'], b'wpn_smg')
        self.assertEqual({p: v for p, v in after.items() if p != '/parameters/type'},
                         {p: v for p, v in before.items() if p != '/parameters/type'})
        # A stale base is refused.
        with self.assertRaises(ValueError):
            ce.set_string(buf, 'parameters/type', 'wpn_pstl', 'wpn_smg')


if __name__ == '__main__':
    unittest.main()
