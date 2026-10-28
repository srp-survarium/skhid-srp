"""The render edit preserves geometry and refuses a body/hood or stale rig."""
import struct
import unittest

from headwear import append_headwear_bind, bind_records, rigid_headwear


class HeadwearEdits(unittest.TestCase):
    def test_bind_appends_one_head_copy_without_changing_existing_records(self):
        frame = struct.pack('<9f', 0, 1.6, 0, 0, 0, 0, 1, 1, 1)
        source = struct.pack('<H', 69) + b''.join(
            (b'Head' if i == 44 else str(i).encode()) + b'\0' + frame for i in range(69))
        result = append_headwear_bind(source)
        self.assertEqual(result[2:len(source)], source[2:])
        self.assertEqual(bind_records(result)[69], ('SrpHeadwear', frame))
        with self.assertRaises(ValueError):
            append_headwear_bind(result)

    def test_skin_preserves_positions_uvs_and_indices_for_each_native_layout(self):
        for stride in (36, 40, 44, 48):
            with self.subTest(stride=stride):
                vertex = bytearray(range(stride))
                vertex[12:16] = bytes([44, 21, 44, 44])
                stored = (stride - 36) // 4
                if stored:
                    struct.pack_into('<' + 'f' * stored, vertex, 16, .75, *([0.0] * (stored - 1)))
                source = struct.pack('<III', 3, 4 + stride, 1) + vertex + struct.pack('<III3H', 4, 10, 3, 0, 0, 0)
                result = rigid_headwear(source, 1)
                self.assertEqual(result[:24], source[:24])
                self.assertEqual(result[28 + stored * 4:], source[28 + stored * 4:])
                self.assertEqual(result[24:28], bytes([69] * 4))
                with self.assertRaises(ValueError):
                    rigid_headwear(source, 2)
                bad = bytearray(source)
                bad[24] = 19  # Chest: a hood or torso cannot silently become a hat.
                with self.assertRaises(ValueError):
                    rigid_headwear(bad, 1)
