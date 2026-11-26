"""The website's maps (build.py's `objects` and `spawns`): Keeper of the Artifacts' hidden pool is
not shown, and a free-for-all shows its own spawns alone.

    python3 -m unittest test_build      (needs pillow, as build.py)
"""
import json
import math
import unittest
from pathlib import Path

import build

KEEPER = Path(__file__).resolve().parents[1] / "content-edit" / "artefact-keeper-1.json"
KEYS = {54: "lifebone", 74: "spring", 76: "onyx"}


def keeper_anomaly():
    """The hidden anomaly exactly as the edit list appends it to every map."""
    (entry,) = json.loads(KEEPER.read_text())["append_entry"]
    return entry["value"]


class KeeperPoolHidden(unittest.TestCase):
    def project(self, *extra):
        return {
            "server_objects": {"respawn_points": []},
            "client_objects": [
                {"game_object_type": "generic_anomaly_core", "full_name": "anomaly/core", "artefacts_enabled": True,
                 "artefacts": [{"dict_id": 74, "count": 3}], "artefact_containers": ["anomaly/container"]},
                {"game_object_type": "artefact_container", "full_name": "anomaly/container",
                 "position": [1.0, 2.0, 3.0], "collision_geometries": []},
                *extra,
            ],
        }

    def test_a_map_with_the_pool_shows_only_its_own_artefacts(self):
        without = build.objects(self.project(), KEYS)
        pool = keeper_anomaly()
        self.assertEqual(pool["artefact_containers"], [])
        with_pool = build.objects(self.project(pool), KEYS)
        self.assertEqual(with_pool, without)
        self.assertEqual([a[3] for a in with_pool["artefacts"]], [["spring"]])

    def test_a_map_with_only_the_pool_shows_no_artefact(self):
        project = {"server_objects": {}, "client_objects": [keeper_anomaly()]}
        self.assertEqual(build.objects(project, KEYS)["artefacts"], [])


class Anomalies(unittest.TestCase):
    """An anomaly's card and area, from its core, zones and containers as the projects hold them."""

    def project(self, **core):
        return {"server_objects": {}, "client_objects": [
            {"game_object_type": "generic_anomaly_core", "full_name": "a/core", "artefacts_enabled": True,
             "artefacts": [{"dict_id": 74, "count": 3}, {"dict_id": 76, "count": 2}],
             "artefact_containers": ["a/c1", "a/c2"],
             "artefacts_first_spawn_time_sec": 70, "artefacts_respawn_time_sec": 12,
             "artefacts_respawn_variation_sec": 30, "artefacts_respawn_chance": 0.5, "max_artefacts_count": 2,
             "states": [{"enabled": True, "groups": [
                 {"enabled": True, "sequential_activation": True, "keep_all_zones_active": False,
                  "min_active_time_sec": 40, "max_active_time_sec": 60, "zones": ["a/hurt", "a/idle"]}]}], **core},
            {"game_object_type": "artefact_container", "full_name": "a/c1", "position": [1.0, 0.0, 2.0],
             "artefacts_search_time_sec": 6, "artefacts_cooldown_time_sec": 80, "collision_geometries": []},
            {"game_object_type": "artefact_container", "full_name": "a/c2", "position": [3.0, 0.0, 4.0],
             "artefacts_search_time_sec": 6, "artefacts_cooldown_time_sec": 80, "collision_geometries": []},
            {"game_object_type": "damage_zone", "full_name": "a/hurt", "max_hit": 0.6, "damage_type": "shock",
             "continuous_hit": False, "interval_in_msec": 1000, "hit_parts_filter": ["right_foot", "left_foot", "brain"],
             "collision_geometries": [{"name": "a/box"}]},
            {"game_object_type": "damage_zone", "full_name": "a/idle", "max_hit": 0, "collision_geometries": [{"name": "a/cyl"}]},
            # A 4 x 2 m box (half sizes 2, 1) turned 90 degrees about y by its mesh, at x 10, z 20.
            {"game_object_type": "collision_geometry", "full_name": "a/box", "position": [10.0, 0.0, 20.0],
             "meshes": [{"type": 1, "position": [0, 0, 0], "rotation": [0, math.pi / 2, 0], "scale": [2.0, 1.0, 1.0]}]},
            # An upright cylinder of radius 3, its object scaled x2: radius 6.
            {"game_object_type": "collision_geometry", "full_name": "a/cyl", "position": [-5.0, 1.0, 0.0], "scale": [2.0, 2.0, 2.0],
             "meshes": [{"type": 2, "position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [3.0, 1.0, 3.0]}]},
        ]}

    def test_its_card_has_the_cores_numbers(self):
        (a,) = build.objects(self.project(), KEYS)["anomalies"]
        self.assertEqual(a["gives"], [["spring", 3], ["onyx", 2]])
        self.assertEqual((a["first"], a["next"]), ([40, 100], [0, 42]))  # never below 0
        self.assertEqual((a["chance"], a["max"], a["containers"]), (0.5, 2, 2))
        self.assertEqual((a["search"], a["cooldown"], a["switch"]), ([6], [80], [(40, 60)]))

    def test_its_damage_is_by_type_and_parts_head_to_feet(self):
        (a,) = build.objects(self.project(), KEYS)["anomalies"]
        self.assertEqual(a["damage"], [{"type": "shock", "parts": ["head", "feet"], "continuous": False,
                                        "interval": 1000, "hit": [0.6, 0.6]}])

    def test_its_area_is_its_zones_seen_from_above(self):
        (a,) = build.objects(self.project(), KEYS)["anomalies"]
        box, cylinder = a["areas"]
        self.assertTrue(box["hurt"])
        self.assertEqual(sorted(map(tuple, box["p"])), [(9.0, 18.0), (9.0, 22.0), (11.0, 18.0), (11.0, 22.0)])
        self.assertEqual(cylinder, {"c": [-5.0, 0.0, 6.0], "hurt": False})

    def test_each_container_names_its_anomaly(self):
        got = build.objects(self.project(), KEYS)
        self.assertEqual([a[3:] for a in got["artefacts"]], [[["onyx", "spring"], [0]]] * 2)

    def test_a_second_anomaly_on_the_same_zones_names_its_twin(self):
        project = self.project()
        core = dict(project["client_objects"][0], full_name="b/core", artefacts=[{"dict_id": 54, "count": 1}],
                    artefact_containers=["b/c"], states=[{"enabled": True, "groups": [{"enabled": True, "zones": ["b/idle"]}]}])
        project["client_objects"] += [
            core,
            {"game_object_type": "artefact_container", "full_name": "b/c", "position": [0.0, 0.0, 0.0], "collision_geometries": []},
            {"game_object_type": "damage_zone", "full_name": "b/idle", "max_hit": 0, "collision_geometries": [{"name": "a/box"}, {"name": "a/cyl"}]},
        ]
        first, second = build.objects(project, KEYS)["anomalies"]
        self.assertNotIn("twin", first)
        self.assertEqual(second["twin"], 0)
        self.assertFalse(any(s["hurt"] for s in second["areas"]))

    def test_an_anomaly_with_its_artefacts_off_shows_its_zones_alone(self):
        got = build.objects(self.project(artefacts_enabled=False), KEYS)
        (a,) = got["anomalies"]
        self.assertEqual((a["gives"], len(a["areas"]), len(a["damage"]), got["artefacts"]), ([], 2, 1, []))


class FreeForAllSpawns(unittest.TestCase):
    # A free-for-all's project as the game has it: Team Deathmatch's points and its own (team 2).
    POINTS = [[1.0, 2.0, 0.0, 0, 90], [3.0, 4.0, 0.0, 1, 270], [5.0, 6.0, 0.0, 2, 0], [7.0, 8.0, 0.0, 2, 180]]

    def test_a_free_for_all_shows_its_team_2_points_alone(self):
        self.assertEqual(build.spawns(build.FREE_FOR_ALL, self.POINTS), self.POINTS[2:])

    def test_a_mode_of_two_teams_shows_every_point(self):
        for mode in (2, 3, 4, 5):
            self.assertEqual(build.spawns(mode, self.POINTS), self.POINTS)


if __name__ == "__main__":
    unittest.main()
