"""Tests of walked_floors.py on made-up data: python -m unittest test_walked_floors (from this folder)."""
import unittest

import walked_floors as W


def verdict(verdict, samples=20, to=(0, 0, 0), support=0):
    return {'verdict': verdict, 'samples': str(samples), 'to_x': str(to[0]), 'to_y': str(to[1]), 'to_z': str(to[2]),
            'support': str(support)}


def table():
    return {'source': 'made up', 'routes': [
        {'level': 'level_05', 'mode': 2, 'team': 0, 'points': [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]},
        {'level': 'level_01', 'mode': 3, 'team': 1, 'points': [[7.0, 8.0], [9.0, 10.0]], 'heights': [1.0, 2.0]},
        {'level': 'level_01', 'mode': 3, 'team': 1, 'points': [[11.0, 12.0]]},
    ]}


class Apply(unittest.TestCase):
    def test_thin_and_walked_points_leave_the_table_as_it_was(self):
        doc = table()
        lines, per_map = W.apply(doc, {0: {0: verdict('thin', 4), 1: verdict('walked')}, 1: {0: verdict('walked')}})
        self.assertEqual(doc, table())
        self.assertEqual(lines, [])
        self.assertEqual(per_map['Rudnya']['touched'], 0)

    def test_a_moved_point_takes_its_floors_place_and_height(self):
        doc = table()
        lines, per_map = W.apply(doc, {0: {1: verdict('move', 14, (3.5, 0.25, 4.5), 6)}})
        route = doc['routes'][0]
        self.assertEqual(route['points'], [[1.0, 2.0], [3.5, 4.5], [5.0, 6.0]])
        self.assertEqual(route['heights'], [None, 0.25, None])
        self.assertEqual(len(lines), 1)
        self.assertIn('floor within 1 m, 6 samples', lines[0])
        self.assertEqual((per_map['Tarakanovsky Fort']['moved'], per_map['Tarakanovsky Fort']['touched']), (1, 1))

    def test_a_wide_move_says_so_and_keeps_the_other_heights(self):
        doc = table()
        lines, _ = W.apply(doc, {1: {0: verdict('move_wide', 11, (7.5, 1.5, 8.5), 3)}})
        self.assertEqual(doc['routes'][1]['points'], [[7.5, 8.5], [9.0, 10.0]])
        self.assertEqual(doc['routes'][1]['heights'], [1.5, 2.0])
        self.assertIn('floor within 2.5 m', lines[0])

    def test_a_dropped_point_leaves_its_route_with_the_others_heights(self):
        doc = table()
        lines, per_map = W.apply(doc, {1: {0: verdict('drop', 12)}})
        self.assertEqual(doc['routes'][1]['points'], [[9.0, 10.0]])
        self.assertEqual(doc['routes'][1]['heights'], [2.0])
        self.assertIn('DROPPED', lines[0])
        self.assertEqual(per_map['Rudnya']['dropped'], 1)

    def test_a_route_that_ends_with_no_heights_has_no_heights_key(self):
        doc = table()
        W.apply(doc, {1: {0: verdict('drop', 12), 1: verdict('drop', 12)}})
        self.assertEqual(doc['routes'][1]['points'], [])
        self.assertNotIn('heights', doc['routes'][1])

    def test_the_report_counts_per_map(self):
        doc = table()
        lines, per_map = W.apply(doc, {0: {0: verdict('move', 14, (1.5, 0.0, 2.5), 4)}, 2: {0: verdict('drop')}})
        text = W.report(lines, per_map)
        self.assertIn('Tarakanovsky Fort: 1 moved, 0 dropped, 1 routes touched of 1', text)
        self.assertIn('Rudnya: 0 moved, 1 dropped, 1 routes touched of 2', text)
        self.assertIn('total: 1 moved, 1 dropped', text)


if __name__ == '__main__':
    unittest.main()
