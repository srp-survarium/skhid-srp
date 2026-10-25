"""Tests of perches.py on made-up data: python -m unittest test_perches (from this folder)."""
import csv
import json
import os
import random
import sys
import tempfile
import unittest

import perches as P


def life(match, spawned, samples):
    """A life from (seconds, x, feet_y, z) samples."""
    return (match, [(int(t * 1000), x, y, z) for t, x, y, z in samples], spawned)


class Visits(unittest.TestCase):
    def test_a_still_stretch_above_the_line_is_a_visit_at_its_mean(self):
        lv = [life('m1', 0, [(60, 0.0, 12.0, 0.0), (65, 0.5, 12.0, 0.2), (70, 0.2, 12.2, 0.1)])]
        (x, y, z, dwell, m), = P.visits_of(lv, 10.0)
        self.assertAlmostEqual(x, 0.233, places=2)
        self.assertAlmostEqual(z, 0.1, places=2)
        self.assertEqual((dwell, m), (10.0, 'm1'))

    def test_below_the_line_moving_a_gap_or_too_short_are_no_visit(self):
        low = life('m', 0, [(60, 0, 5.0, 0), (65, 0, 5.0, 0), (70, 0, 5.0, 0)])
        moving = life('m', 0, [(60, 0, 12.0, 0), (65, 3, 12.0, 0), (70, 6, 12.0, 0)])
        gap = life('m', 0, [(60, 0, 12.0, 0), (75, 0, 12.0, 0)])
        short = life('m', 0, [(60, 0, 12.0, 0), (62, 0, 12.0, 0)])
        self.assertEqual(P.visits_of([low, moving, gap, short], 10.0), [])

    def test_a_wait_at_the_spawn_is_no_visit_but_the_same_stretch_later_is(self):
        wait = life('m', 0, [(5, 0, 12.0, 0), (10, 0, 12.0, 0), (15, 0, 12.0, 0)])
        later = life('m', 0, [(0, 0, 12.0, 0), (50, 9, 12.0, 9), (55, 9, 12.0, 9)])
        away = life('m', 0, [(0, 0, 12.0, 0), (5, 9, 12.0, 9), (10, 9, 12.0, 9)])
        self.assertEqual(P.visits_of([wait], 10.0), [])
        self.assertEqual(len(P.visits_of([later], 10.0)), 1)
        self.assertEqual(len(P.visits_of([away], 10.0)), 1, 'near the spawn only in time, not in place')

    def test_alive_minutes_skips_gaps(self):
        lv = [life('m', 0, [(0, 0, 0, 0), (5, 0, 0, 0), (60, 0, 0, 0), (65, 0, 0, 0)])]
        self.assertAlmostEqual(P.alive_minutes(lv), 10.0 / 60.0)


class Clusters(unittest.TestCase):
    def blobs(self):
        out = []
        for k in range(5):
            out.append((0.3 * k, 10.0, 0.2 * k, 5.0, f'm{k}'))
        for k in range(3):
            out.append((20 + 0.3 * k, 10.0, 0.0, 5.0, f'm{k}'))
        out.append((0.0, 30.0, 0.0, 5.0, 'above'))  # same place a floor higher: its own group
        return out

    def test_peak_cover_groups_the_blobs_and_keeps_floors_apart(self):
        groups = P.cluster(self.blobs())
        self.assertEqual(sorted(len(g) for g in groups), [1, 3, 5])

    def test_the_groups_do_not_depend_on_the_order(self):
        vs = self.blobs()
        base = sorted(sorted(g) for g in P.cluster(vs))
        for seed in range(5):
            random.Random(seed).shuffle(vs)
            self.assertEqual(sorted(sorted(g) for g in P.cluster(vs)), base)

    def test_a_chain_is_cut_by_peak_cover_not_merged(self):
        chain = [(3.0 * k, 10.0, 0.0, 5.0, 'm') for k in range(6)]
        sizes = [len(g) for g in P.cluster(chain)]
        self.assertGreater(len(sizes), 1)
        self.assertEqual(sum(sizes), 6)


class Table(unittest.TestCase):
    def files(self, d):
        """Five matches of 12 places; every one has a place that stands at the perch (50, 21, 7) for 10 s, and one that walks."""
        info = {f'm{k}': dict(level=1, mode=2 if k % 2 else 4, project='level_01_2' if k % 2 else 'level_01_evn_4',
                              alive_places=12, bot_match=False) for k in range(5)}
        info['bots'] = dict(level=1, mode=2, project='level_01_2', alive_places=12, bot_match=True)
        info['small'] = dict(level=1, mode=2, project='level_01_2', alive_places=6, bot_match=False)
        with open(os.path.join(d, 'matches.json'), 'w', encoding='utf-8') as fh:
            json.dump(info, fh)
        with open(os.path.join(d, 'positions.csv'), 'w', newline='', encoding='utf-8') as fh:
            w = csv.writer(fh)
            w.writerow(['match', 'time', 'place', 'alive', 'x', 'y', 'z', 'spawned'])
            for m in list(info):
                for t in range(40, 120, 5):
                    w.writerow([m, t * 1000, 0, 1, -t, 1.0, 0.0, 0])           # walking along the low floor
                for t in (60, 65, 70):
                    w.writerow([m, t * 1000, 1, 1, 50.0, 21.0 + 0.9, 7.0, 0])   # standing at the perch (centre y = feet + 0.9)
        return os.path.join(d, 'matches.json'), os.path.join(d, 'positions.csv')

    def test_the_table_has_the_perch_and_the_map_row_and_keeps_the_routes(self):
        with tempfile.TemporaryDirectory() as d:
            mj, pc = self.files(d)
            routes = os.path.join(d, 'routes.json')
            with open(routes, 'w', encoding='utf-8') as fh:
                json.dump({'format': 2, 'routes': [{'level': 'level_01', 'mode': 2, 'team': 0}], 'note': 'kept'}, fh)
            out = os.path.join(d, 'out.json')
            argv = sys.argv
            sys.argv = ['perches.py', mj, pc, '--routes', routes, '--out', out]
            try:
                P.main()
            finally:
                sys.argv = argv
            with open(out, encoding='utf-8') as fh:
                t = json.load(fh)
        self.assertEqual(t['routes'], [{'level': 'level_01', 'mode': 2, 'team': 0}])
        self.assertEqual(t['note'], 'kept')
        (perch,) = t['perches']
        self.assertEqual((perch['level'], perch['x'], perch['y'], perch['z'], perch['n'], perch['dwell']),
                         ('level_01', 50.0, 21.0, 7.0, 5, 10.0))
        (row,) = t['perch_maps']
        self.assertEqual((row['level'], row['visits']), ('level_01', 5))
        self.assertEqual(row['p75'], 0.1, 'the low floor, where most samples are')
        self.assertGreater(perch['rate'], 0.0)

    def test_the_bot_match_and_the_small_match_are_left_out(self):
        with tempfile.TemporaryDirectory() as d:
            mj, _ = self.files(d)
            self.assertEqual(sorted(P.load_matches(mj, 12, 'all')), ['m0', 'm1', 'm2', 'm3', 'm4'])
            self.assertEqual(sorted(P.load_matches(mj, 12, 'day')), ['m1', 'm3'])

    def test_weather_of(self):
        self.assertEqual([P.weather_of(p) for p in ('level_01_2', 'level_02_evn_4', 'level_03_rain_2', 'level_08_prerain_3')],
                         ['day', 'evn', 'rain', 'prerain'])


if __name__ == '__main__':
    unittest.main()
