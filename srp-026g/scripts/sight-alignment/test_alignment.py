import copy
import math
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

import numpy as np

import align
import model


def de_boor(pairs, t):
    """Independent cubic evaluation with clamped extension of the knot vector."""
    knots, values = np.array(pairs).T
    k = next(
        i
        for i in reversed(range(3, len(knots) - 1))
        if knots[i] <= t and knots[i + 1] > knots[i]
    )

    def knot(i):
        return knots[min(i, len(knots) - 1)]

    d = list(values[k - 3 : k + 1])
    for r in range(1, 4):
        for j in range(3, r - 1, -1):
            lower = knot(j + k - 3)
            upper = knot(j + 1 + k - r)
            alpha = (t - lower) / (upper - lower) if upper != lower else 0
            d[j] = (1 - alpha) * d[j - 1] + alpha * d[j]
    return d[3]


class Curves(unittest.TestCase):
    def test_retained_final_sentinel_is_not_evaluated(self):
        pairs = [[0, 7], [0, 7], [0, 7], [0, 7], [2, -1e9]]
        for t in [-1, 0, 0.25, 1, 2, 3]:
            self.assertAlmostEqual(model.curve(pairs, t), 7)

    def test_nonuniform_cubics_match_independent_de_boor(self):
        pairs = [
            [0, 0.4],
            [0, -0.2],
            [0, 0.1],
            [0, 0.6],
            [0.1, 0.8],
            [0.35, -0.7],
            [0.8, 0.9],
            [1.2, -1e9],
        ]
        for t in np.linspace(0, 1.2, 51):
            self.assertAlmostEqual(model.curve(pairs, t), de_boor(pairs, t), places=10)


class Alignment(unittest.TestCase):
    def test_locator_binding_is_parsed_once_at_boundary(self):
        raw = {"position": [0, 0, 0], "bone_name": "editor_only"}
        for index in (-1, 65535):
            self.assertIsNone(model.Locator.parse(dict(raw, bone_idx=index)).bone)
        self.assertEqual(model.Locator.parse(dict(raw, bone_idx=3)).bone, 3)
        for index in (-2, 65536):
            with self.assertRaises(ValueError):
                model.Locator.parse(dict(raw, bone_idx=index))

    def test_camera_handedness_and_position_centering(self):
        loc = {"position": [0.1, 0.2, 0.3], "rotation": [0.11, -0.07, 0.04]}
        s = {"locator": loc, "offset": 0.03}
        camera = (
            model.matrix((0, 0, s["offset"]))
            @ model.HALF
            @ model.matrix(loc["position"], loc["rotation"])
        )
        target = model.transform(np.array([0.012, -0.009, 1.2]), camera)
        q = model.transform(target, align.view_of(s))
        np.testing.assert_allclose(q, [0.012, -0.009, -1.2], atol=1e-12)
        # Positive camera X must appear right, not mirrored; positive Y goes up.
        self.assertGreater(q[0], 0)
        self.assertLess(q[1], 0)
        moved = copy.deepcopy(loc)
        moved["position"] = (
            np.array(loc["position"]) + np.array([q[0], q[1], 0]) @ camera[:3, :3]
        ).tolist()
        result = model.transform(target, align.view_of(s, moved))
        np.testing.assert_allclose(result, [0, 0, -1.2], atol=1e-12)

    def test_rotation_centering_preserves_roll_and_position(self):
        loc = {"position": [0.1, 0.2, 0.3], "rotation": [0.11, -0.07, 0.04]}
        camera = model.HALF @ model.matrix(loc["position"], loc["rotation"])
        target = model.transform(np.array([0.012, -0.009, 1.2]), camera)
        direction = -(target - loc["position"])
        direction /= np.linalg.norm(direction)
        x, y, z = direction
        roll = loc["rotation"][2]
        sz, cz = math.sin(roll), math.cos(roll)
        result = copy.deepcopy(loc)
        result["rotation"] = [
            math.asin(x * sz + y * cz),
            math.atan2(-x * cz + y * sz, z),
            roll,
        ]
        q = model.transform(target, align.view_of({"locator": result, "offset": 0.03}))
        np.testing.assert_allclose(q[:2], 0, atol=1e-12)
        self.assertLess(q[2], 0)
        self.assertEqual(result["position"], loc["position"])


class Plan(unittest.TestCase):
    def test_angles_invert_locator_rotation(self):
        for r in ([0.11, -0.07, 0.04], [-0.0148, 0.0, -0.0061], [0.0, 0.0, 0.0]):
            np.testing.assert_allclose(align.angles(model.rotation(r)), r, atol=1e-12)

    def test_on_axis_looks_along_the_optic_and_centres_its_dot(self):
        optic = model.HALF @ model.matrix((0.002, 0.03, -0.1), (-0.0148, 0.0, -0.0061))
        dot = model.transform(np.array([0.0005, 0.04, 0.6]), optic)  # 0.6 m along the optic's axis
        s = {"id": "t", "locator": {"position": [0.008, 0.07, 0.146], "rotation": [0.009, 0.0, -0.006]},
             "offset": 0.0, "fov": 52.0, "mount": optic, "marker": dot}
        self.assertGreater(math.hypot(*align.error_pixels(s)), 10)
        loc = align.on_axis(s)
        np.testing.assert_allclose(loc["rotation"], [-0.0148, 0.0, -0.0061], atol=1e-9)
        np.testing.assert_allclose(align.error_pixels(dict(s, locator=loc)), 0, atol=1e-9)
        # a stock attachment on a half-turned scope_point faces the other way: turned back
        s["mount"] = model.HALF @ optic
        np.testing.assert_allclose(align.on_axis(s)["rotation"], loc["rotation"], atol=1e-9)


class Export(unittest.TestCase):
    def fixture(self, root):
        data = bytearray(24)

        def append(blob):
            at = len(data)
            data.extend(blob)
            return at

        def write(at, name, value):
            name_at = append(name.encode() + b"\0")
            if isinstance(value, (dict, list)):
                entries = (
                    sorted(value.items(), key=lambda kv: zlib.crc32(kv[0].encode()))
                    if isinstance(value, dict)
                    else [("", v) for v in value]
                )
                offset = append(bytes(24 * len(entries)))
                kind = 3 if isinstance(value, dict) else 4
                for i, (key, v) in enumerate(entries):
                    write(offset + 24 * i, key, v)
                count = len(entries)
            elif isinstance(value, tuple):
                kind, count = 7, 0
                offset = append(struct.pack("<3f", *value))
            elif isinstance(value, str):
                kind, count = 5, len(value) + 1
                offset = append(value.encode() + b"\0")
            else:
                kind, count, offset = 1, 0, value
            struct.pack_into(
                "<QQIHH",
                data,
                at,
                offset,
                name_at,
                zlib.crc32(name.encode()),
                kind,
                count,
            )

        write(
            0,
            "",
            {
                "locators": [
                    {
                        "name": "aim",
                        "position": (0.01, 0.02, 0.03),
                        "rotation": (0.004, 0.0, 0.0),
                        "bone_idx": 1,
                    },
                    {
                        "name": "barrel_point",
                        "position": (0.01, 0.05, -0.4),
                        "rotation": (0.0, 0.0, 0.0),
                        "bone_idx": 1,
                    },
                ]
            },
        )
        path = Path("models/weapons/test.skinned_model/render/export_properties")
        (root / path).parent.mkdir(parents=True)
        (root / path).write_bytes(data)
        loc = model.locator((root / path).parent.parent, "aim")
        before = {k: list(getattr(loc, k)) for k in ("position", "rotation")}
        after = copy.deepcopy(before)
        after["rotation"][0] = -0.001
        return {
            "file": str(path),
            "sha256": align.digest(root / path),
            "before": before,
            "after": after,
        }

    def test_plan_refuses_traversal_before_reading(self):
        with self.assertRaisesRegex(ValueError, "only edit"):
            align.checked_edits(
                Path("/tmp"),
                {
                    "format": "skhid-sight-alignment-v1",
                    "changes": [{"file": "../secret"}],
                },
            )

    def test_plan_refuses_unknown_format_and_empty_plan(self):
        for plan in ({}, {"format": "skhid-sight-alignment-v1", "changes": []}):
            with self.assertRaises(ValueError):
                align.checked_edits(Path("/tmp"), plan)

    def test_valid_edit_changes_only_aim_rotation_and_source_stays_intact(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            change = self.fixture(root)
            source = (root / change["file"]).read_bytes()
            edits = align.checked_edits(
                root, {"format": "skhid-sight-alignment-v1", "changes": [change]}
            )
            edit = edits["set_entry"][0]
            data = bytearray(source)
            model.ce.set_entry(
                data, edit["list"], edit["where"], edit["from"], edit["to"]
            )
            before = model.ce.walk(source)
            after = model.ce.walk(data)
            self.assertEqual(
                [a[0] for a, b in zip(before, after) if a != b],
                ["/locators[0]/rotation"],
            )
            self.assertEqual(source, (root / change["file"]).read_bytes())

    def test_stale_source_duplicate_model_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            change = self.fixture(root)
            plan = {"format": "skhid-sight-alignment-v1", "changes": [change, change]}
            with self.assertRaisesRegex(ValueError, "conflicting shared-model"):
                align.checked_edits(root, plan)
            plan["changes"] = [change]
            change["after"]["position"][0] = float("nan")
            with self.assertRaisesRegex(ValueError, "finite numbers"):
                align.checked_edits(root, plan)
            change["sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "source hash changed"):
                align.checked_edits(root, plan)


if __name__ == "__main__":
    unittest.main()
