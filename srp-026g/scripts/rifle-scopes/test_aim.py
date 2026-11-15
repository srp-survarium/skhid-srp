"""Camera regressions: 0.69 optic frames through 0.26g's native aim order."""

import unittest
import tempfile
from pathlib import Path

import numpy as np

import make_pack as scopes


class OpticalCameraTests(unittest.TestCase):
    def test_optical_eye_follows_the_mount_bone_through_animation(self):
        def write_config(path, value):
            buf = bytearray(24)
            buf[:24] = scopes.ce.value_record(buf, scopes.ce.append(buf, b'\0'), '',
                                              scopes.ce.as_table(value, True))
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(buf)

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            gun, optic = root / 'gun', root / 'optic'
            aim = {'name': 'aim', 'position': [0.0, 0.01, 0.2], 'rotation': [0.0, 0.0, 0.0],
                   'bone_idx': 0, 'bone_name': 'item'}
            mount = {'name': 'scope_point', 'position': [0.02, 0.08, -0.1], 'rotation': [0.1, 0.2, -0.1],
                     'bone_idx': 2, 'bone_name': 'feed_cover'}
            donor = {'name': 'aim', 'position': [0.0, 0.0377224, -0.1614212], 'rotation': [0.0, 0.0, 0.0]}
            write_config(gun / 'render/export_properties', {'locators': [aim, mount]})
            write_config(optic / 'render/export_properties', {'locators': [donor]})
            write_config(root / 'gun.options', {'parameters': {'aim_offset': 0.1}})
            translation = [0.01, -0.02, 0.03]
            needed = scopes.optical_eye(root, 'gun.options', gun, optic, translation)
            scopes.moved_optical_eye(gun, needed)
            actual = scopes.posing.locator(gun, 'aim')
            self.assertEqual(actual.bone, 2)
            raw = scopes.posing.config(gun / 'render/export_properties')['locators']
            self.assertEqual(next(row for row in raw if row['name'] == 'aim')['bone_name'], 'feed_cover')
            for turn in (0.0, 0.5, 1.0):
                pose = scopes.posing.matrix((0.1, 0.02, 0.03), (turn, -0.1 * turn, 0))
                got = (scopes.posing.matrix((0, 0, 0.1)) @ scopes.posing.HALF
                       @ scopes.posing.matrix(actual.position, actual.rotation) @ pose)
                expected = (scopes.posing.matrix(donor['position']) @ scopes.posing.matrix(translation)
                            @ scopes.posing.HALF @ scopes.posing.matrix(mount['position'], mount['rotation']) @ pose)
                np.testing.assert_allclose(got, expected, atol=1e-7)

    def test_aim_offset_is_compensated_along_the_rotated_camera(self):
        # The RPD has a nonzero aim_offset and its optic/eye bones turn relative
        # to one another. Compensating only world Z is wrong on that weapon.
        camera = scopes.posing.matrix((0.013, 0.067, -0.21), (0.13, -0.19, 0.07))
        for offset in (0.0, 0.1, -0.08):
            position, rotation = scopes.locator_from_camera(camera, offset)
            actual = (scopes.posing.matrix((0, 0, offset)) @ scopes.posing.HALF
                      @ scopes.posing.matrix(position, rotation))
            np.testing.assert_allclose(actual, camera, atol=1e-12)
            # A point on the donor axis must land on the game's viewing axis.
            target = scopes.posing.transform(np.array([0.0, 0.0, 0.6]), camera)
            view = scopes.posing.transform(target, np.linalg.inv(actual))
            np.testing.assert_allclose(view, (0, 0, 0.6), atol=1e-12)

    def test_every_mount_preserves_the_donor_eye_and_reticle(self):
        rng = np.random.default_rng(2777)
        # TA01's actual donor eye; the reticle is 59.96 mm ahead of it.
        donor_eye = scopes.posing.matrix((0, 0.0377224, -0.1614212))
        donor_reticle = np.array([0, 0.0377224, -0.10146])
        expected = scopes.posing.transform(donor_reticle, np.linalg.inv(donor_eye))
        for _ in range(120):
            mount = scopes.posing.matrix(rng.uniform(-0.3, 0.3, 3), rng.uniform(-0.7, 0.7, 3))
            offset = float(rng.uniform(-0.1, 0.1))
            camera = donor_eye @ mount
            position, rotation = scopes.locator_from_camera(camera, offset)
            game_camera = (scopes.posing.matrix((0, 0, offset)) @ scopes.posing.HALF
                           @ scopes.posing.matrix(position, rotation))
            mounted_reticle = scopes.posing.transform(donor_reticle, mount)
            np.testing.assert_allclose(
                scopes.posing.transform(mounted_reticle, np.linalg.inv(game_camera)), expected, atol=1e-12)

    def test_non_rigid_camera_is_refused(self):
        with self.assertRaisesRegex(ValueError, 'scale or shear'):
            scopes.locator_from_camera(scopes.posing.matrix(scale=(1, 2, 1)), 0)


if __name__ == '__main__':
    unittest.main()
