"""Checks of the prediction-fit tools on made-up timelines."""
import unittest

import numpy as np

from predfit import harness, kin, load, models, projection, tables
from predfit.harness import HMAX


def player(keys, yaw=None, alive=None):
    n = len(keys)
    keys = np.asarray(keys, np.uint32)
    yaw = np.zeros(n, np.float32) if yaw is None else np.asarray(yaw, np.float32)
    node = np.ones(n, bool)
    return load.Player(place=0, first=0, keys=keys, yaw=yaw, pitch=np.zeros(n, np.float32),
                       node=node, late=np.full(n, 30), alive=np.ones(n, bool) if alive is None
                       else alive, combat=np.zeros(n, bool), worlds=np.zeros(0, load.WORLD),
                       pos=np.zeros((n, 2)), heading=np.zeros(n))


def samples_of(p, delay, every=7, seed=0):
    s = harness.player_samples(p, every, delay, np.random.default_rng(seed), [60])
    n = len(s['delay'])
    return harness.Samples(match=np.zeros(n, np.int32), names=['m'], **s)


class Kinematics(unittest.TestCase):
    def test_speeds_follow_the_keys(self):
        v = kin.target_of([kin.FORWARD, kin.FORWARD | kin.SPRINT, kin.LEFT, kin.BACK,
                           kin.FORWARD | kin.CROUCH, kin.FORWARD | kin.SPRINT | kin.FIRE,
                           kin.FORWARD | kin.BACK, 0])
        np.testing.assert_allclose(np.hypot(*v.T), [3.5, 5.0, 2.5, 3.0, 2.5, 3.5, 0, 0], atol=1e-6)
        self.assertGreater(v[2, 1], 0)  # left is the body's positive side

    def test_blend_reaches_the_new_speed_in_six_ticks(self):
        b = kin.Blend(kin.target_of([0]))
        speeds = [float(np.hypot(*b.tick(kin.target_of([kin.FORWARD]))[0])) for _ in range(8)]
        np.testing.assert_allclose(speeds[:6], [3.5 * (i + 1) / 6 for i in range(6)], rtol=1e-6)
        self.assertAlmostEqual(speeds[7], 3.5, places=5)

    def test_forward_at_zero_yaw_is_plus_z(self):
        v = kin.to_world(np.array([[1.0, 0.0]]), np.array([0.0]))
        np.testing.assert_allclose(v, [[0.0, 1.0]], atol=1e-9)


class Timelines(unittest.TestCase):
    def test_inputs_become_ticks_with_keys_carried_and_turns_summed(self):
        rows = np.zeros(4, load.INPUT)
        rows['time'] = [100, 100, 125, 250]
        rows['yaw'] = [0.1, 0.2, 0.3, 0.0]
        rows['actions'] = [1, 1, 3, 0]
        first, keys, yaw, _, node = load.timeline(rows)
        self.assertEqual(first, 2)
        self.assertEqual(keys.tolist(), [3, 3, 3, 0])
        np.testing.assert_allclose(yaw, [0.6, 0, 0, 0], atol=1e-6)
        self.assertEqual(node.tolist(), [True, False, False, True])

    def test_ages_count_ticks_since_a_change(self):
        self.assertEqual(harness.ages(np.array([1, 1, 2, 2, 2, 1])).tolist(), [0, 1, 0, 1, 2, 0])

    def test_split_is_by_match_name(self):
        self.assertEqual(harness.is_test('a'), harness.is_test('a'))
        share = np.mean([harness.is_test(f'm{i}') for i in range(2000)])
        self.assertAlmostEqual(share, 0.3, delta=0.04)


class Scores(unittest.TestCase):
    def test_hold_last_is_exact_while_nothing_changes(self):
        s = samples_of(player([kin.FORWARD] * 400), 150)
        pos, look, act = harness.score(s, models.HoldLast().predict(s))
        np.testing.assert_allclose(pos, 0, atol=1e-6)
        np.testing.assert_allclose(look, 0, atol=1e-6)
        np.testing.assert_allclose(act, 0)

    def test_a_turn_not_carried_on_moves_the_body_off_too(self):
        s = samples_of(player([kin.FORWARD] * 400, yaw=[0.01] * 400), 150)
        pos, look, _ = harness.score(s, models.HoldLast().predict(s))
        self.assertGreater(look.min(), 0)
        self.assertGreater(pos.min(), 0)

    def test_a_steady_turn_is_continued_by_the_look_model(self):
        s = samples_of(player([0] * 400, yaw=[0.02] * 400), 150)
        m = models.DampedLook(models.HoldHeld()).fit(s)
        _, look, _ = harness.score(s, m.predict(s))
        self.assertEqual(m.lam_yaw, 1.0)
        np.testing.assert_allclose(look, 0, atol=1e-4)

    def test_a_stop_costs_the_hold_and_the_horizon_bounds_it(self):
        keys = ([kin.FORWARD] * 30 + [0] * 30) * 10
        s = samples_of(player(keys), 250)
        pos, _, _ = harness.score(s, models.HoldLast().predict(s))
        # At most five ticks at walking speed.
        self.assertLess(pos.max(), 3.5 * 0.25 + 1e-6)
        self.assertGreater(pos.max(), 0.05)

    def test_the_realistic_delay_adds_half_the_receivers_round_trip(self):
        s = samples_of(player([0] * 400), None)
        self.assertTrue(np.all(s.delay >= 30 + 30))
        self.assertTrue(np.all(s.delay <= 30 + 30 + harness.FRAME_MS + 1e-3))


class Tables(unittest.TestCase):
    def test_a_rare_cell_falls_back_to_the_coarser_key(self):
        keys = [kin.FORWARD] * 2000
        s = samples_of(player(keys), 100, every=1)
        t = tables.MoveTable(['moves', 'hold_age']).fit(s)
        pred = t.keys(s)
        self.assertTrue(np.all(pred == kin.FORWARD))
        self.assertEqual(t.size_bytes(), 16 * tables.HOLD_N * HMAX)

    def test_look_table_learns_a_steady_turn(self):
        s = samples_of(player([0] * 2000, yaw=[0.02] * 2000), 150, every=1)
        t = tables.LookTable(['turn']).fit(s)
        _, look, _ = harness.score(s, t.predict(s))
        np.testing.assert_allclose(look, 0, atol=1e-3)



class Projection(unittest.TestCase):
    def test_the_projection_keeps_the_mask_and_learns_a_steady_turn(self):
        keys = [kin.FORWARD | kin.FIRE] * 2000
        s = samples_of(player(keys, yaw=[0.03] * 2000), 100, every=1)
        m = projection.ProjectedLook(['turn']).fit(s)
        p = m.predict(s)
        self.assertTrue(np.all(p.keys == (kin.FORWARD | kin.FIRE)))
        # One input a tick turns 0.03: two ticks predicted, so twice the last turn.
        self.assertAlmostEqual(float(m.gains(s)[0, 0]), 2.0)
        _, look, _ = harness.score(s, p)
        self.assertLess(float(np.median(look)), 1e-3)

    def test_no_turn_no_projection(self):
        s = samples_of(player([kin.FORWARD] * 2000), 150, every=1)
        m = projection.ProjectedLook(['turn', 'elapsed']).fit(s)
        pos, look, _ = harness.score(s, m.predict(s))
        np.testing.assert_allclose(pos, 0, atol=1e-6)
        np.testing.assert_allclose(look, 0, atol=1e-6)


if __name__ == '__main__':
    unittest.main()
