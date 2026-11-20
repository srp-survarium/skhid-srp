import unittest
from stats import ROUND_FACTORS, sections

# A round that changes nothing.
PLAIN = {**{factor: 1 for _, factor in ROUND_FACTORS}, "dispersion": 1, "buck_shot": 1}


class StatsTests(unittest.TestCase):
    def rows(self, weapon, ammo=PLAIN):
        return {row["id"]: row for section in sections(weapon, ammo, "en") for row in section["rows"]}

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

    def test_round_scales_what_it_fires(self):
        weapon = {"bullet_damage": .4, "bullet_pierce": .2, "min_damage_amount": .6, "max_damage_distance": 15,
                  "dispersion": {"base_dispersion": .5}, "reload_time": 2.5}
        rows = self.rows(weapon, {**PLAIN, "k_damage": 1.3, "k_arp": 1.3, "k_min_damage_amount": 1.5,
                                  "k_max_damage_distance": 1.2, "dispersion": 2})
        self.assertEqual(rows["bullet_damage"]["text"], "52")
        self.assertEqual(rows["bullet_pierce"]["text"], "26")
        self.assertEqual(rows["min_damage_amount"]["text"], "90 %")
        self.assertEqual(rows["max_damage_distance"]["text"], "18 m")
        self.assertEqual(rows["dispersion.base_dispersion"]["values"], [1])
        self.assertEqual(rows["reload_time"]["text"], "2.5 s")
        self.assertEqual(weapon["bullet_damage"], .4)

    def test_pellets_compare_as_one_shot(self):
        rows = self.rows({"bullet_damage": 1, "bullet_pierce": .3}, {**PLAIN, "k_damage": .2, "buck_shot": 10})
        self.assertEqual(rows["bullet_damage"]["text"], "20 × 10 pellets")
        self.assertEqual(rows["bullet_damage"]["values"], [200])
        self.assertEqual(rows["bullet_pierce"]["text"], "30")


if __name__ == "__main__":
    unittest.main()
