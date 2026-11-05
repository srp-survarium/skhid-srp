"""Authored clips preserve the rig and emit clamped native cubic tracks."""
import copy
import hashlib
import struct
import unittest

import content_edit as ce
from test_content_edit import config


def tracks(clip):
    data = ce.clip_parse(clip)[2]
    result, offset = [], 0
    while offset < len(data):
        count = struct.unpack_from('<I', data, offset)[0]
        result.append([struct.unpack_from('<ff', data, offset + 4 + 8 * i) for i in range(count)])
        offset += 4 + 8 * count
    return result


class PoseClips(unittest.TestCase):
    def setUp(self):
        self.names = config({'bones_names': ['Root', 'LeftHand']})
        self.source = bytearray(ce.CLIP_HEAD.pack(2, 0, 0) + bytes(2 * ce.CLIP_BONE))
        for value in [0, 0, 0, 0, 0, 0, 1, 1, 1, .27, 0, 0, 0, 0, 0, 1, 1, 1]:
            self.source += struct.pack('<I10f', 5, 0, value, 0, value, 0, value, 0, value, 36, -1e9)
        self.spec = {
            'source_sha256': hashlib.sha256(self.source).hexdigest(),
            'names_sha256': hashlib.sha256(self.names).hexdigest(),
            'frames': 18,
            'start': {'LeftHand': [0, .1, .2]},
            'end': {'LeftHand': [1, .3, .4]},
        }

    def build(self, spec=None):
        return ce.make_pose_clip(self.source, self.names, spec or self.spec)

    def test_clamped_curve_has_exact_endpoints_and_zero_endpoint_velocity(self):
        output = self.build()
        self.assertEqual(ce.CLIP_HEAD.unpack_from(output), (2, 0, 0))
        actual = tracks(output)
        self.assertEqual(len(actual), 18)
        for index, channel in enumerate(actual):
            self.assertEqual([pair[0] for pair in channel], [0, 0, 0, 0, 18])
            p = [pair[1] for pair in channel[:4]]
            # Independent cubic Bezier evaluation of the clamped spline.
            at = lambda t: (1-t)**3*p[0] + 3*(1-t)**2*t*p[1] + 3*(1-t)*t*t*p[2] + t**3*p[3]
            self.assertEqual(3 * (p[1] - p[0]), 0)
            self.assertEqual(3 * (p[3] - p[2]), 0)
            if 12 <= index <= 14:
                axis = index - 12
                start, end = self.spec['start']['LeftHand'][axis], self.spec['end']['LeftHand'][axis]
                self.assertAlmostEqual(at(0), start)
                self.assertAlmostEqual(at(1), end)
                self.assertAlmostEqual(at(.5), (start + end) / 2)
            else:
                self.assertEqual(p, [tracks(self.source)[index][0][1]] * 4)

    def test_raise_hold_lower_join_without_a_pose_jump(self):
        raising = tracks(self.build())
        hold = copy.deepcopy(self.spec)
        hold['start'] = hold['end']
        holding = tracks(self.build(hold))
        lower = copy.deepcopy(self.spec)
        lower['start'], lower['end'] = lower['end'], lower['start']
        lowering = tracks(self.build(lower))
        for up, stay, down in zip(raising, holding, lowering):
            self.assertEqual(up[3][1], stay[0][1])
            self.assertEqual(stay[3][1], down[0][1])
            self.assertEqual(down[3][1], up[0][1])

    def test_rejects_stale_sources_invalid_rotations_and_duration(self):
        for change in [
            {'source_sha256': '0' * 64}, {'names_sha256': '0' * 64},
            {'frames': 0}, {'frames': float('inf')},
            {'end': {'UnknownHand': [0, 0, 0]}},
            {'end': {'LeftHand': [float('nan'), 0, 0]}},
            {'end': {'LeftHand': [6.2, 0, 0]}},
        ]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.build(dict(self.spec, **change))
        self.source[3] = 1
        self.spec['source_sha256'] = hashlib.sha256(self.source).hexdigest()
        with self.assertRaisesRegex(ValueError, 'additive'):
            self.build()


if __name__ == '__main__':
    unittest.main()
