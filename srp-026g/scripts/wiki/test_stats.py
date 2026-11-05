import unittest
from stats import sections


class StatsTests(unittest.TestCase):
    def rows(self, weapon):
        return {row["id"]: row for section in sections(weapon, "en") for row in section["rows"]}

    def test_fixed_symmetric_side_values_cancel(self):
        rows = self.rows({"recoil": {"min_left_recoil": 3, "max_left_recoil": 3,
                                    "min_right_recoil": 3, "max_right_recoil": 3}})
        for key in ("horizontal_kick", "horizontal_variability", "horizontal_bias"):
            self.assertTrue(all(value == 0 for value in rows[key]["values"]))

    def test_asymmetric_ranges_preserve_direction_and_variation(self):
        rows = self.rows({"recoil": {"min_left_recoil": 1, "max_left_recoil": 2,
                                    "min_right_recoil": 3, "max_right_recoil": 5}})
        self.assertEqual(rows["horizontal_kick"]["values"], [1, 4])
        self.assertEqual(rows["horizontal_bias"]["values"], [2.5])
        self.assertEqual(rows["horizontal_variability"]["values"], [.645])
        self.assertEqual(rows["horizontal_kick"]["direction"], "neutral")

    def test_missing_values_and_native_default_are_distinct(self):
        rows = self.rows({})
        self.assertEqual(rows["aim_time"]["text"], "—")
        self.assertEqual(rows["hip_multiplier"]["text"], "1 ×")
        self.assertEqual(rows["horizontal_kick"]["values"], [])
        rows = self.rows({"movement_speed_modifier": 0, "breath_vibration": {"base_period": 5}})
        self.assertEqual(rows["movement_speed_modifier"]["text"], "0 %")
        self.assertEqual(rows["breath_vibration.base_period"]["direction"], "neutral")

    def test_ui_units_and_visible_precision(self):
        rows = self.rows({"bullet_damage": .45, "movement_speed_modifier": -.05,
                          "reload_time": 3.1669999})
        self.assertEqual(rows["bullet_damage"]["text"], "45")
        self.assertEqual(rows["movement_speed_modifier"]["text"], "-5 %")
        self.assertEqual(rows["reload_time"]["values"], [3.167])


if __name__ == "__main__":
    unittest.main()
