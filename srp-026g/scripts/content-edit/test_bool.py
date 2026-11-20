"""Native boolean edits preserve type, reject stale values and never coerce numbers."""
import unittest
import content_edit as ce


def config(value):
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, 0, '', {'flag': value, 'other': True})
    return buf


class BooleanEdits(unittest.TestCase):
    def test_boolean_changes_only_its_value_and_can_be_reversed(self):
        buf = config(False)
        original = bytes(buf)
        offset = ce.find(buf, 'flag')
        ce.set_bool(buf, 'flag', False, True)
        self.assertEqual([i for i, (a, b) in enumerate(zip(original, buf)) if a != b], [offset])
        self.assertEqual(ce.rec(buf, offset)[3], ce.T_BOOL)
        ce.set_bool(buf, 'flag', True, False)
        self.assertEqual(bytes(buf), original)

    def test_stale_or_nonboolean_inputs_fail_without_writing(self):
        for value, old, new, error in [(False, True, False, ValueError),
                                       (False, 0, True, TypeError),
                                       (False, False, 1, TypeError),
                                       (0, False, True, TypeError),
                                       (0.0, False, True, TypeError)]:
            with self.subTest(value=value, old=old, new=new):
                buf = config(value)
                before = bytes(buf)
                with self.assertRaises(error): ce.set_bool(buf, 'flag', old, new)
                self.assertEqual(bytes(buf), before)

    def test_malformed_native_boolean_is_refused(self):
        buf = config(False)
        buf[ce.find(buf, 'flag')] = 2
        before = bytes(buf)
        with self.assertRaises(ValueError): ce.set_bool(buf, 'flag', False, True)
        self.assertEqual(bytes(buf), before)


if __name__ == '__main__': unittest.main()
