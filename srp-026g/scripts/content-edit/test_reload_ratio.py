"""UI reload metadata follows the animation files, including nonzero clip starts."""
import struct
import unittest

import content_edit as ce


def config(state=314, names=None):
    names = names or ['tactical', 'empty'] * 4
    value = {'states': {'reload': {'state_type': state, 'user_animations': {
        i: 'resources/animations/' + name for i, name in enumerate(names)}}}}
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, 0, '', value)
    return buf


def clip(start, end):
    track = struct.pack('<Iffff', 2, start, 0, end, 0)
    return ce.CLIP_HEAD.pack(1, 0, 0) + bytes(ce.CLIP_BONE) + track * 9


class ReloadRatio(unittest.TestCase):
    def test_duration_uses_end_minus_start_and_preserves_config(self):
        clips = {'animations/tactical.b-spline': clip(10, 155),
                 'animations/empty.b-spline': clip(20, 220)}
        buf = config()
        original = bytes(buf)
        self.assertAlmostEqual(ce.reload_clip_ratio(buf, clips.__getitem__), 200 / 145)
        self.assertEqual(bytes(buf), original)

    def test_other_reload_states_have_no_tactical_metadata(self):
        for state in [303, 306, 311]:
            with self.subTest(state=state):
                self.assertIsNone(ce.reload_clip_ratio(config(state), self.fail))

    def test_same_clip_is_not_tactical_but_distinct_equal_length_clips_are(self):
        self.assertIsNone(ce.reload_clip_ratio(config(names=['empty'] * 8), self.fail))
        self.assertEqual(ce.reload_clip_ratio(config(), lambda _: clip(0, 100)), 1)

    def test_tactical_can_be_longer_than_empty(self):
        clips = {'animations/tactical.b-spline': clip(0, 247),
                 'animations/empty.b-spline': clip(0, 159)}
        self.assertAlmostEqual(ce.reload_clip_ratio(config(), clips.__getitem__), 159 / 247)

    def test_missing_or_inconsistent_clips_stop_the_build(self):
        clips = {'animations/tactical.b-spline': clip(0, 100),
                 'animations/empty.b-spline': clip(0, 150),
                 'animations/different.b-spline': clip(0, 160)}
        with self.assertRaises(KeyError):
            ce.reload_clip_ratio(config(), {}.__getitem__)
        with self.assertRaisesRegex(ValueError, 'eight'):
            ce.reload_clip_ratio(config(names=['tactical', 'empty']), clips.__getitem__)
        with self.assertRaisesRegex(ValueError, 'vary'):
            ce.reload_clip_ratio(config(names=['tactical', 'empty'] * 3 + ['tactical', 'different']), clips.__getitem__)
        for end in [0, float('nan'), float('inf')]:
            clips['animations/tactical.b-spline'] = clip(0, end)
            with self.subTest(end=end), self.assertRaisesRegex(ValueError, 'invalid'):
                ce.reload_clip_ratio(config(), clips.__getitem__)


class RetimeReload(unittest.TestCase):
    def test_every_track_and_event_moves_but_pose_and_payload_bytes_stay_exact(self):
        source = bytearray(clip(-5, 145))
        ce.add_event_channel(source, [], {'name': 'reload', 'type': 0,
                                         'knots': [0, 21, 100], 'domains': [1, 2, 3]})
        start, heads, tracks, events, pads = ce.clip_parse(source)
        # Nonzero pose values on all nine curves; these must never be scaled.
        tracks = bytearray(tracks)
        for at in range(8, len(tracks), 20):
            struct.pack_into('<f', tracks, at, 12.5)
        source = ce.clip_bytes(start, heads, tracks, events, pads)
        result = ce.retime_clip(source, 0.75)
        s2, h2, t2, e2, p2 = ce.clip_parse(result)
        self.assertEqual((s2, h2, p2), (start, heads, pads))
        for off in range(0, len(tracks), 20):
            self.assertEqual(t2[off:off + 4], tracks[off:off + 4])
            for at in (off + 4, off + 12):
                self.assertEqual(t2[at + 4:at + 8], tracks[at + 4:at + 8])
                self.assertAlmostEqual(struct.unpack_from('<f', t2, at)[0],
                                       struct.unpack_from('<f', tracks, at)[0] * 0.75)
        self.assertEqual(e2, [dict(events[0], knots=[0, 15.75, 75])])

    def test_pair_drives_all_views_item_phase_and_ui_with_modifiers(self):
        b = config()
        ce.add_value(b, '', 'parameters', {'reload_time': 2.4, 'reload_empty_time': 3.4})
        ce.add_value(b, 'states/reload', 'animations', {0: 'resources/animations/item_tactical',
                                                     1: 'resources/animations/item_empty'})
        ce.add_value(b, '', 'ui_desc', {'props_list': {0: {'prop_id': 162, 'prop_value': '1.5'}}})
        assets = {f'animations/{name}{ext}': data
                  for name, frames in [('tactical', 100), ('empty', 150),
                                       ('item_tactical', 80), ('item_empty', 120)]
                  for ext, data in [('.b-spline', clip(0, frames)), ('.bones_names', b'bone names')]}
        generated, written = ce.retime_reload(b, 'wpn_test', assets.__getitem__)
        self.assertEqual(len(generated), 20)
        self.assertEqual(len(written), 11)
        ratio = ce.reload_clip_ratio(b, generated.__getitem__)
        self.assertAlmostEqual(ratio, 3.4 / 2.4, places=6)
        for key, data in generated.items():
            if key.endswith('.bones_names'):
                self.assertEqual(data, b'bone names')
                continue
            tracks = ce.clip_parse(data)[2]
            expected = (2.4, 3.4)[int(key.split('_')[-1].split('.')[0]) % 2]
            if '/item_' in key:
                expected *= 0.8  # keep the original item's phase relative to the hands
            self.assertAlmostEqual(ce.clip_end(tracks) / 30, expected, places=6)
        # Native speed normalizes tactical to reload_time, then applies the same speed to empty.
        self.assertAlmostEqual(2.4 * 0.75 * ratio, 3.4 * 0.75, places=6)
        prop = ce.value_of(b, ce.find(b, 'ui_desc/props_list[0]/prop_value'))
        self.assertAlmostEqual(float(prop), ratio)
        with self.assertRaisesRegex(ValueError, 'original clips'):
            ce.retime_reload(b, 'wpn_test', generated.__getitem__)

    def test_absent_pair_is_untouched_and_invalid_pair_refused(self):
        b = config()
        self.assertEqual(ce.retime_reload(b, 'wpn_test', self.fail), ({}, []))
        for value in (0, -1, float('inf'), float('nan')):
            b = config()
            ce.add_value(b, '', 'parameters', {'reload_time': 2.4, 'reload_empty_time': value})
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'positive'):
                ce.retime_reload(b, 'wpn_test', self.fail)


if __name__ == '__main__':
    unittest.main()
