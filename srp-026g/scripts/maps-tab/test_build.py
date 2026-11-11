"""The website's maps (build.py's `objects`): Keeper of the Artifacts' hidden pool is not shown.

    python3 -m unittest test_build      (needs pillow, as build.py)
"""
import json
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
                {"game_object_type": "generic_anomaly_core", "full_name": "anomaly/core",
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


if __name__ == "__main__":
    unittest.main()
