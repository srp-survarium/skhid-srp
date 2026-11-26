"""reload_commit: each magazine reload clip's seat, from its own sound events and the state's sounds."""
import struct
import unittest

import content_edit as ce


def config(state=314, clips=('tactical', 'empty'), sounds=()):
    value = {'states': {'reload': {
        'state_type': state,
        'animations': {i: 'resources/animations/' + name for i, name in enumerate(clips)},
        'sounds': {'first_view': {i: {0: 258, 1: 'weapons/gun/' + s} for i, s in enumerate(sounds)}}}}}
    buf = bytearray(24)
    buf[:24] = ce.value_record(buf, 0, '', value)
    return buf


def clip(end, knots=(), domains=None):
    track = struct.pack('<Iffff', 2, 0, 0, end, 0)
    start = ce.CLIP_HEAD.pack(1, 0, 0) + bytes(ce.CLIP_BONE)
    if not knots:
        return start + track * 9
    # sound_events as the clips have it: domain 255 (the next sound) is a value add_event_channel refuses.
    head = bytes(8) + struct.pack('<HBB', len(knots), len(knots), 2) + bytes(4)
    events = {'name': 'sound_events', 'type': 2, 'knots': [float(k) for k in knots],
              'domains': list(domains or [255] * len(knots))}
    return bytes(ce.clip_bytes(start, [head], track * 9, [events], []))


AK = ('akm_n_mag_out', 'akm_n_mag_in', 'akm_n_reload_1', 'akm_n_reload_2')
PISTOL = ('pm_mag_out_ready_01', 'pm_mag_out', 'pm_mag_set_ready_01', 'pm_mag_slide_in', 'pm_mag_set_01',
          'pm_mag_power_set_01', 'pm_slider_back')


class Seat(unittest.TestCase):
    def channels(self, b):
        return ce.clip_parse(b)[3]

    def test_the_last_sound_of_the_new_ammunition_going_in(self):
        # The AK-74M's empty reload: out, in, then the bolt (reload_1 / reload_2 are no seat).
        self.assertEqual(ce.reload_seat(self.channels(clip(102, [12.75, 46.5, 68.75, 74])), AK), 46.5)
        # A pistol's tactical reload: the slide-in, then the set; its _ready sounds are no seat.
        self.assertEqual(ce.reload_seat(self.channels(clip(65, [14, 17, 39, 43, 50])), PISTOL), 50)

    def test_domains_index_the_list_and_255_takes_the_next(self):
        # 255 from 0FFh: the first knot plays sound 0 (0x297D11's byte wrap), not 256 % n.
        self.assertEqual(ce.reload_seat(self.channels(clip(170, [63, 70, 121], [2, 0, 1])),
                                        ('akm_n_mag_out', 'akm_n_mag_in', 'mag_set_ready')), 121)
        self.assertEqual(ce.reload_seat(self.channels(clip(110, [0, 21, 47, 90, 110][:4])),
                                        ('wear_line', 'mag_eject', 'mag_fall', 'mag_set_ready', 'mag_set')), None)

    def test_belts_shells_and_strippers(self):
        cases = [(('pkp_ammo_box_set', 'pkp_belt_set', 'pkp_body_close'), 2),
                 (('toz_34_open', 'toz_34_load_1', 'toz_34_load_2', 'toz_34_close'), 3),
                 (('magnum_shells_out', 'magnum_shells_in', 'magnum_close'), 2),
                 (('sks_bullets_set', 'sks_bullets_in', 'sks_shutter_end'), 2),
                 (('rpd_mag_in_01', 'rpd_belt_on', 'rpd_chamber_close_01'), 2)]
        for sounds, frame in cases:
            with self.subTest(sounds=sounds):
                self.assertEqual(ce.reload_seat(self.channels(clip(50, range(1, len(sounds) + 1))), sounds), frame)

    def test_no_seat_sound_or_no_sound_events(self):
        self.assertIsNone(ce.reload_seat(self.channels(clip(50, [1, 2])), ('mag_out', 'shutter_in')))
        self.assertIsNone(ce.reload_seat(self.channels(clip(50)), AK))


class Seats(unittest.TestCase):
    def test_every_item_clip_of_a_magazine_reload(self):
        clips = {'animations/tactical.b-spline': clip(72, [11, 45]),
                 'animations/empty.b-spline': clip(102, [12, 46, 68, 74])}
        for state in (303, 311, 314):
            with self.subTest(state=state):
                self.assertEqual(ce.reload_commit_seats(config(state, sounds=AK), clips.__getitem__),
                                 {'animations/tactical.b-spline': 45, 'animations/empty.b-spline': 46})

    def test_other_reloads_are_left(self):
        self.assertEqual(ce.reload_commit_seats(config(306, sounds=AK), self.fail), {})

    def test_a_clip_without_a_seat_stops_the_build(self):
        clips = {'animations/tactical.b-spline': clip(72, [11]), 'animations/empty.b-spline': clip(102, [12, 46])}
        with self.assertRaisesRegex(ValueError, 'no seat sound'):
            ce.reload_commit_seats(config(sounds=AK), clips.__getitem__)

    def test_the_channel_goes_after_the_live_ones_and_the_tracks_stay(self):
        source = clip(102, [12, 46, 68, 74])
        b = bytearray(source)
        ce.add_event_channel(b, ce.clip_parse(source)[3], {'name': ce.RELOAD_COMMIT, 'type': ce.RELOAD_COMMIT_TYPE,
                                                           'knots': [46], 'domains': [ce.RELOAD_COMMIT_SEATED]})
        _, _, tracks, channels, _ = ce.clip_parse(b)
        self.assertEqual(tracks, ce.clip_parse(source)[2])
        self.assertEqual([c['name'] for c in channels], ['sound_events', 'reload_commit'])
        self.assertEqual((channels[1]['knots'], channels[1]['domains']), ([46.0], [1]))


if __name__ == '__main__':
    unittest.main()
