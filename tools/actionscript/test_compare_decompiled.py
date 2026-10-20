from pathlib import Path
import tempfile
import unittest

import compare_decompiled as compare

SCRIPT = '''package
{
   public class C
   {
      private var a:int;

      public function f(value:int, name:String = null) : int
      {
         var total:int = value;
         trace("value:",total);
         return total;
      }

      public static function g() : void
      {
      }
   }
}
'''


class CompareDecompiledTests(unittest.TestCase):
    def test_methods_and_declarations_are_split(self):
        methods, other = compare.members(SCRIPT)
        self.assertEqual(sorted(methods), [' f', ' g static'])
        self.assertIn('private var a:int;', other)
        self.assertTrue(methods[' f'].rstrip().endswith('}'))

    def test_debug_names_are_normalised_but_strings_are_not(self):
        named = compare.members(SCRIPT)[0][' f']
        anonymous = named.replace('value:int', 'param1:int').replace('name:String', 'param2:String') \
            .replace('var total', 'var _loc3_').replace('= value;', '= param1;').replace('trace("value:",total)', 'trace("value:",_loc3_)') \
            .replace('return total', 'return _loc3_')
        self.assertEqual(compare.normalise(named), compare.normalise(anonymous))
        self.assertNotEqual(compare.normalise(named), compare.normalise(named.replace('"value:"', '"other:"')))

    def test_a_comparison_reports_each_kind_of_difference(self):
        with tempfile.TemporaryDirectory() as temp:
            a, b = Path(temp) / 'a', Path(temp) / 'b'
            for root in (a, b):
                root.mkdir()
                (root / 'Same.as').write_text(SCRIPT)
            (a / 'C.as').write_text(SCRIPT)
            (b / 'C.as').write_text(SCRIPT.replace('return total;', 'return total + 1;')
                                    .replace('private var a:int;', 'private var b:int;'))
            (b / 'New.as').write_text(SCRIPT)
            rows, identical = compare.compare(a, b)
            self.assertEqual(identical, 1)
            self.assertEqual({(status, script, key) for status, script, key, _ in rows}, {
                ('SCRIPT-ONLY-IN-B', 'New.as', ''), ('DECLARATIONS', 'C.as', ''), ('DIFFERENT', 'C.as', ' f')})


if __name__ == '__main__':
    unittest.main()
